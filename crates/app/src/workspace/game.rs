use super::App;
use rimstack_core::base::error::Error;
use rimstack_core::kept::launches;
use rimstack_core::rimworld::{modsconfig, paths::Source};
use rimstack_core::said;
use std::path::PathBuf;

const APP_ID: u32 = 294_100;

const SAVE_BACKUPS: usize = 5;

/// The text as JavaScript's `encodeURIComponent` gives it: Steam reads
/// the launch arguments of a `steam://run` address that way.
fn uri_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => out.push(char::from(byte)),
            b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => {
                out.push(char::from(byte))
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn steam_address(arguments: &str) -> String {
    match arguments.trim() {
        "" => format!("steam://rungameid/{APP_ID}"),
        given => format!("steam://run/{APP_ID}//{}/", uri_component(given)),
    }
}

impl App {
    pub fn game_running(&self) -> bool {
        let paths = self.paths();
        let game = paths.game.as_deref();
        game.is_some_and(rimstack_core::sources::maintenance::game_running)
    }

    /// Slow: copies the newest save. Never run it on the window's thread.
    pub fn back_up_for_launch(&self) {
        if let Err(problem) = self.back_up_newest_save("save-backups", SAVE_BACKUPS) {
            rimstack_core::problem!("save not backed up: {problem}");
        }
        self.keep_mod_copies_for_launch();
    }

    /// Before the game reads them: a copy of each active Workshop mod the Workshop updated
    /// since its newest kept copy, and no others, so a launch with no updates copies nothing.
    /// Kept in RimStack's data folder hold what a broken update can be rolled back to.
    fn keep_mod_copies_for_launch(&self) {
        use rimstack_core::sources::fallback;
        let store = fallback::store(self.data_dir());
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return;
        };
        let active = self.active_now(library);
        let record = self.steam_record();
        let now: Vec<(String, PathBuf, u64)> = active
            .iter()
            .filter_map(|mod_id| {
                let m = library.get(*mod_id)?;
                if m.source != Source::Workshop {
                    return None;
                }
                Some((m.folder.clone(), m.path.clone(), *record.0.get(&m.folder)?))
            })
            .collect();
        let times: Vec<(String, u64)> = now
            .iter()
            .map(|(folder, _, at)| (folder.clone(), *at))
            .collect();
        let wanted: std::collections::HashSet<String> =
            fallback::to_keep(&times, |id| fallback::kept(&store, id).last().copied())
                .into_iter()
                .collect();
        // Gathered one mod at a time and straight onto disk: only what changed, sized by
        // whatever Steam left there since the last kept copy.
        for (folder, path, at) in &now {
            if !wanted.contains(folder) {
                continue;
            }
            if let Err(problem) = fallback::keep(path, &store, folder, *at) {
                rimstack_core::problem!("{problem}");
            }
        }
        let cap = self
            .settings()
            .get("keptModsMegabytes")
            .and_then(|size| size.parse::<u64>().ok())
            .unwrap_or(fallback::DEFAULT_CAP_MEGABYTES);
        if let Err(problem) = fallback::prune(&store, cap) {
            rimstack_core::problem!("{problem}");
        }
    }

    pub fn record_launch(&self) {
        let list = self.session().current().to_vec();
        if let Err(problem) = launches::record(&self.launches_file(), &list, super::now_secs()) {
            rimstack_core::problem!("{problem}");
        }
        self.mark_all_seen();
        self.record_versions();
    }

    /// Slow. A launch is recorded only once it is started; a failed start records nothing.
    pub fn launch(&self, record: bool) -> Result<Option<String>, Error> {
        if record {
            self.back_up_for_launch();
        }
        let started = self.start_game()?;
        if record {
            self.record_launch();
        }
        Ok(started)
    }

    /// Only the game is left to run. Gives its Steam address, or nothing when started here.
    pub fn start_game(&self) -> Result<Option<String>, Error> {
        let (directly, arguments) = {
            let settings = self.settings();
            let arguments = settings.get("launchArguments").unwrap_or_default();
            (settings.launch_directly(), arguments.to_string())
        };
        let arguments = self.with_save_folder(arguments);
        if !directly {
            return Ok(Some(steam_address(&arguments)));
        }
        let paths = self.paths();
        let game = paths
            .game
            .as_ref()
            .ok_or_else(|| Error::refused(said!("The game's folder is not known")))?;
        rimstack_core::sources::maintenance::start_game(game, &arguments)?;
        Ok(None)
    }

    /// A setup with a Config and saves folder of its own hands it to the game;
    /// "Default" and typed arguments saying otherwise stay as they are.
    fn with_save_folder(&self, arguments: String) -> String {
        if self.setup_is_default() || arguments.contains("-savedatafolder") {
            return arguments;
        }
        match self.paths().config.clone() {
            None => arguments,
            Some(config) => {
                let folder = config.to_string_lossy();
                let folder = match folder.contains(char::is_whitespace) {
                    true => std::borrow::Cow::from(format!("\"{folder}\"")),
                    false => folder,
                };
                format!("-savedatafolder={folder} {arguments}")
            }
        }
    }

    pub fn mods_config_list(&self) -> Option<Vec<String>> {
        let file = self.paths().mods_config()?;
        modsconfig::read(&file).ok().map(|config| config.active)
    }

    /// `None` unless the game has since emptied `ModsConfig.xml`.
    pub fn list_reset_by_game(&self) -> Option<Vec<String>> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().ok()?;
        let last = launches::read(&self.launches_file()).pop()?;
        let saved = self.session().saved().to_vec();
        let official = |m: &rimstack_core::mods::list::Mod| {
            let installed = m.installed.and_then(|i| library.get(i));
            installed.is_some_and(|i| i.source == Source::Official)
        };
        let emptied = library.arrange(&saved).active.iter().all(official);
        (emptied && last.ids.len() > saved.len()).then_some(last.ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    #[test]
    fn steam_is_given_the_arguments_as_a_browser_would_encode_them() {
        assert_eq!(steam_address("  "), "steam://rungameid/294100");
        assert_eq!(
            steam_address("-logfile /tmp/a b.txt é"),
            "steam://run/294100//-logfile%20%2Ftmp%2Fa%20b.txt%20%C3%A9/"
        );
    }

    #[test]
    fn a_launch_is_recorded_with_its_list_and_steam_starts_the_game_by_default() {
        let (_tmp, app) = installed();
        app.record_launch();
        let record = launches::read(&app.launches_file());
        assert_eq!(record.len(), 1);
        assert_eq!(record[0].ids, app.session().current().to_vec());
        assert_eq!(
            app.start_game().unwrap().as_deref(),
            Some("steam://rungameid/294100")
        );
    }

    #[test]
    fn a_list_emptied_to_the_official_mods_offers_the_last_launch_s() {
        let both = vec!["ludeon.rimworld".to_string(), "ann.roads".to_string()];
        let (_tmp, app) = installed();
        assert_eq!(app.list_reset_by_game(), None);
        launches::record(&app.launches_file(), &both, 1).unwrap();
        assert_eq!(app.list_reset_by_game(), Some(both.clone()));
        assert_eq!(
            app.mods_config_list(),
            Some(vec!["ludeon.rimworld".to_string()])
        );

        let (tmp, app) = installed();
        std::fs::write(
            tmp.path().join("config/Config/ModsConfig.xml"),
            "<ModsConfigData><activeMods><li>ludeon.rimworld</li><li>ann.roads</li></activeMods></ModsConfigData>",
        )
        .unwrap();
        launches::record(&app.launches_file(), &both, 1).unwrap();
        assert_eq!(app.list_reset_by_game(), None);
    }

    #[test]
    fn a_direct_start_that_fails_records_no_launch_and_leaves_new_mods_unseen() {
        let (tmp, app) = installed();
        app.mark_all_seen();
        let about = tmp.path().join("game/Mods/hats/About");
        std::fs::create_dir_all(&about).unwrap();
        std::fs::write(
            about.join("About.xml"),
            "<ModMetaData><packageId>Ann.Hats</packageId><name>Hats</name></ModMetaData>",
        )
        .unwrap();
        app.rescan();
        let hats = tmp.path().join("game/Mods/hats");
        let hats = hats.to_string_lossy();
        assert!(app.seen().read().unwrap().is_new(&hats));

        app.set_setting("launch", "direct").unwrap();
        assert!(app.launch(true).is_err());
        assert!(launches::read(&app.launches_file()).is_empty());
        assert!(app.seen().read().unwrap().is_new(&hats));
    }

    #[test]
    fn a_launch_that_started_is_recorded_once() {
        let (_tmp, app) = installed();
        assert_eq!(
            app.launch(true).unwrap().as_deref(),
            Some("steam://rungameid/294100")
        );
        assert_eq!(launches::read(&app.launches_file()).len(), 1);
        app.launch(false).unwrap();
        assert_eq!(launches::read(&app.launches_file()).len(), 1);
    }
}
