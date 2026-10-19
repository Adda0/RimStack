//! Setups, RimSort's instances: each has its own game, Config and saves, and
//! Workshop folders and launch arguments, and keeps its own lists, notes,
//! launches and logs. "Default" is RimStack as it always was: the shared
//! `settings.json` and data folder.
//!
//! The shared `settings.json` names the setup in use ("instance"); every other
//! setup is a folder under `setups/` of the data folder, its `settings.json`
//! holding the values in `OWN` and beside it all else the setup keeps for
//! itself. A name without such a file reads as "Default", and a settings file
//! from before setups names none: both work untouched.

use super::App;
use crate::changes::Changes;
use crate::reports::SetupRow;
use rimstack_core::base::error::Error;
use rimstack_core::base::locks::Shared;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::notes::Notes;
use rimstack_core::kept::settings::Settings;
use rimstack_core::{said, say};
use std::path::{Path, PathBuf};

type Result<T, E = Error> = std::result::Result<T, E>;

pub const DEFAULT: &str = "Default";

/// In the shared file only, and only written by switching or renaming: a bare
/// value would swap folders without a rescan.
pub(crate) const IN_USE: &str = "instance";

/// What a setup other than "Default" keeps in its own file; everything else in
/// the settings is shared.
const OWN: [&str; 5] = ["game", "config", "workshop", "launch", "launchArguments"];

const FOLDERS: [&str; 3] = ["game", "config", "workshop"];

const MOST_NAME: usize = 80;

/// Trimmed; never "Default", never a path, never more than a folder name holds.
fn usable(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::refused(said!("The setup needs a name")));
    }
    let bad = ['/', '\\', '<', '>', ':', '"', '|', '?', '*'];
    if name.eq_ignore_ascii_case(DEFAULT)
        || name.chars().count() > MOST_NAME
        || name.contains(bad)
        || name.contains(char::is_control)
        || name.trim_matches('.').is_empty()
    {
        return Err(Error::refused(say!(
            "{} cannot name a setup: “Default” is taken, and slashes and the like are off",
            name
        )));
    }
    Ok(name.to_string())
}

fn dir_of(data_dir: &Path, name: &str) -> PathBuf {
    data_dir.join("setups").join(name)
}

fn file_of(data_dir: &Path, name: &str) -> PathBuf {
    dir_of(data_dir, name).join("settings.json")
}

fn exists(data_dir: &Path, name: &str) -> bool {
    usable(name).is_ok() && file_of(data_dir, name).is_file()
}

/// Launch arguments without any `-savedatafolder=`: a setup copied from
/// another starts with a Config and saves folder of its own.
fn without_save_folder(arguments: &str) -> String {
    let kept = rimstack_core::sources::maintenance::arguments(arguments);
    let kept = kept
        .into_iter()
        .filter(|one| !one.starts_with("-savedatafolder"));
    let quoted = kept.map(|one| match one.contains(char::is_whitespace) {
        true => format!("\"{one}\""),
        false => one,
    });
    quoted.collect::<Vec<_>>().join(" ")
}

pub(crate) struct Setup {
    name: String,
    /// The shared file as on disk.
    shared: Settings,
    /// The setup's own file; empty under "Default".
    own: Settings,
}

impl Setup {
    pub fn read(data_dir: &Path, file: &Path) -> Self {
        let shared = Settings::read(file);
        let named = shared.get(IN_USE).filter(|name| exists(data_dir, name));
        match named.map(String::from) {
            Some(name) => Self {
                own: Settings::read(&file_of(data_dir, &name)),
                name,
                shared,
            },
            None => Self {
                name: DEFAULT.to_string(),
                shared,
                own: Settings::default(),
            },
        }
    }

    pub fn is_default(&self) -> bool {
        self.name == DEFAULT
    }

    /// What the app works with: the shared values, and the setup's own over them.
    pub fn merged(&self) -> Settings {
        let mut all = self.shared.clone();
        if !self.is_default() {
            for name in OWN {
                drop(all.set(name, self.own.get(name).unwrap_or_default()));
            }
        }
        all
    }

    /// Kept in the file it belongs to.
    pub fn set(
        &mut self,
        data_dir: &Path,
        shared_file: &Path,
        name: &str,
        value: &str,
    ) -> Result<()> {
        if name == IN_USE {
            return Err(Error::refused(said!("{} is not a setting", name)));
        }
        if self.is_default() || !OWN.contains(&name) {
            self.shared.set(name, value)?;
            return self.shared.write(shared_file);
        }
        self.own.set(name, value)?;
        self.own.write(&file_of(data_dir, &self.name))
    }
}

impl App {
    /// Where the setup in use keeps its lists, notes, launches and logs: the
    /// data folder itself under "Default".
    pub fn own_dir(&self) -> PathBuf {
        let setup = self.setup.reading();
        match setup.is_default() {
            true => self.data_dir().to_path_buf(),
            false => dir_of(self.data_dir(), &setup.name),
        }
    }

    pub fn setup_in_use(&self) -> String {
        self.setup.reading().name.clone()
    }

    pub(crate) fn setup_is_default(&self) -> bool {
        self.setup.reading().is_default()
    }

    /// "Default" first, the rest by name.
    pub fn setups(&self) -> Vec<SetupRow> {
        let in_use = self.setup_in_use();
        let found = std::fs::read_dir(self.data_dir().join("setups"));
        let mut names: Vec<String> = found
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| exists(self.data_dir(), name))
            .collect();
        names.sort_by_key(|name| name.to_lowercase());
        let default = SetupRow {
            shown: say!("Default"),
            name: DEFAULT.to_string(),
            current: in_use == DEFAULT,
            removable: false,
        };
        let others = names.into_iter().map(|name| SetupRow {
            shown: Sentence::Plain(name.clone()),
            current: name == in_use,
            removable: name != in_use,
            name,
        });
        std::iter::once(default).chain(others).collect()
    }

    /// Usable and naming no setup but `past`, whatever the case.
    fn free_name(&self, name: &str, past: Option<&str>) -> Result<String> {
        let name = usable(name)?;
        let taken = self
            .setups()
            .iter()
            .any(|row| Some(row.name.as_str()) != past && row.name.eq_ignore_ascii_case(&name));
        match taken {
            true => Err(Error::refused(say!("{} names a setup already", name))),
            false => Ok(name),
        }
    }

    /// The game folders and launch of the one in use, a Config and saves
    /// folder of its own that starts from the load order in use, and nothing
    /// else yet. Switching to it stays the user's.
    pub fn add_setup(&self, name: &str) -> Result<()> {
        let name = self.free_name(name, None)?;
        let saves = dir_of(self.data_dir(), &name).join("savedata");
        let config = saves.join("Config");
        std::fs::create_dir_all(&config).map_err(|e| Error::file(&config, e))?;
        if let Some(list) = self.paths().mods_config().filter(|list| list.is_file()) {
            let copy = config.join("ModsConfig.xml");
            std::fs::copy(&list, &copy).map_err(|e| Error::file(&copy, e))?;
        }
        let mut own = Settings::default();
        {
            let settings = self.settings();
            for key in ["game", "workshop", "launch"] {
                own.set(key, settings.get(key).unwrap_or_default())?;
            }
            let arguments = settings.get("launchArguments").unwrap_or_default();
            own.set("launchArguments", &without_save_folder(arguments))?;
        }
        own.set("config", &saves.to_string_lossy())?;
        own.write(&file_of(self.data_dir(), &name))?;
        self.publish(Changes::SETTINGS);
        Ok(())
    }

    /// Its folder moves with the name, and so do the folders set inside it.
    pub fn rename_setup(&self, from: &str, to: &str) -> Result<()> {
        if from.eq_ignore_ascii_case(DEFAULT) {
            return Err(Error::refused(say!("{} cannot be renamed", from)));
        }
        if !exists(self.data_dir(), from) {
            return Err(Error::refused(say!("There is no setup named {}", from)));
        }
        let to = self.free_name(to, Some(from))?;
        let in_use = self.setup_in_use() == from;
        if in_use && self.game_running() {
            return Err(Error::refused(said!(
                "Not while the game runs: it uses the folders of this setup"
            )));
        }
        let mut setup = self.setup.writing();
        let (here, there) = (dir_of(self.data_dir(), from), dir_of(self.data_dir(), &to));
        std::fs::rename(&here, &there).map_err(|e| Error::file(&here, e))?;
        let mut own = Settings::read(&file_of(self.data_dir(), &to));
        for key in FOLDERS {
            let moved = own.get(key).and_then(|set| {
                let inside = Path::new(set).strip_prefix(&here).ok()?;
                Some(there.join(inside).to_string_lossy().into_owned())
            });
            if let Some(moved) = moved {
                own.set(key, &moved)?;
            }
        }
        own.write(&file_of(self.data_dir(), &to))?;
        if !in_use {
            drop(setup);
            self.publish(Changes::SETTINGS);
            return Ok(());
        }
        setup.shared.set(IN_USE, &to)?;
        setup.shared.write(&self.settings_file())?;
        setup.name = to;
        setup.own = own;
        *self.settings.writing() = setup.merged();
        drop(setup);
        self.publish(Changes::SETTINGS | Changes::FOLDERS);
        Ok(())
    }

    /// Moved aside, not deleted: its saves, lists and notes can still be
    /// picked up by hand. "Default" and the setup in use refuse.
    pub fn delete_setup(&self, name: &str) -> Result<()> {
        if name.eq_ignore_ascii_case(DEFAULT) {
            return Err(Error::refused(say!("{} cannot be removed", name)));
        }
        if self.setup_in_use() == name {
            return Err(Error::refused(said!(
                "The setup in use cannot be removed: switch to another first"
            )));
        }
        if !exists(self.data_dir(), name) {
            return Err(Error::refused(say!("There is no setup named {}", name)));
        }
        let aside = self.removed_setups_dir();
        std::fs::create_dir_all(&aside).map_err(|e| Error::file(&aside, e))?;
        let aside = aside.join(format!("{name} {}", super::now_secs()));
        let dir = dir_of(self.data_dir(), name);
        std::fs::rename(&dir, &aside).map_err(|e| Error::file(&dir, e))?;
        self.publish(Changes::SETTINGS);
        Ok(())
    }

    pub fn removed_setups_dir(&self) -> PathBuf {
        self.data_dir().join("setups-removed")
    }

    /// Refused while the game runs, with changes not saved, or while a
    /// mod-finding search is on: each belongs to the setup in use.
    fn may_switch_setup(&self) -> Result<()> {
        if self.game_running() {
            return Err(Error::refused(said!(
                "Not while the game runs: it uses the folders of the setup in use"
            )));
        }
        if self.session().is_dirty() {
            return Err(Error::refused(said!(
                "There are unsaved changes: save or undo them before switching setups"
            )));
        }
        if self.hunt().active {
            return Err(Error::refused(said!(
                "A search for a broken mod is on: end it before switching setups"
            )));
        }
        Ok(())
    }

    /// Kept on disk before the app turns to it; the mod folders are read again
    /// as after a folder change.
    pub fn use_setup(&self, name: &str) -> Result<()> {
        if self.setup_in_use() == name {
            return Ok(());
        }
        self.may_switch_setup()?;
        let to_default = name == DEFAULT;
        if !to_default && !exists(self.data_dir(), name) {
            return Err(Error::refused(say!("There is no setup named {}", name)));
        }
        let mut setup = self.setup.writing();
        setup
            .shared
            .set(IN_USE, if to_default { "" } else { name })?;
        setup.shared.write(&self.settings_file())?;
        setup.name = name.to_string();
        setup.own = match to_default {
            true => Settings::default(),
            false => Settings::read(&file_of(self.data_dir(), name)),
        };
        *self.settings.writing() = setup.merged();
        drop(setup);
        if let Some(notes) = self.notes.get() {
            *notes.writing() = Notes::read(&self.notes_file());
        }
        if let Some(seen) = self.seen.get() {
            *seen.writing() = rimstack_core::kept::new_mods::Seen::read(&self.seen_file());
        }
        self.publish(
            Changes::SETTINGS
                | Changes::FOLDERS
                | Changes::NOTES
                | Changes::LISTS
                | Changes::LAUNCHES
                | Changes::LOGS
                | Changes::SAVES
                | Changes::SEARCH
                | Changes::UPDATES,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    #[test]
    fn a_settings_file_from_before_setups_reads_as_default_and_is_left_as_it_was() {
        let (tmp, app) = installed();
        let written = r#"{"font": "Serif", "launchArguments": "-quicktest", "game": "/games/rw"}"#;
        std::fs::create_dir_all(tmp.path().join("settings/RimStack")).unwrap();
        std::fs::write(app.settings_file(), written).unwrap();
        let app = App::new(
            tmp.path().join("home"),
            &tmp.path().join("settings"),
            &tmp.path().join("data"),
            rimstack_core::rimworld::paths::Overrides::default(),
        );
        assert_eq!(app.setup_in_use(), DEFAULT);
        assert_eq!(app.own_dir(), app.data_dir());
        assert_eq!(app.notes_file(), app.data_dir().join("notes.json"));
        assert_eq!(app.lists_dir(), app.data_dir().join("lists"));
        assert_eq!(app.launches_file(), app.data_dir().join("launches.json"));
        assert_eq!(*app.settings(), Settings::read(&app.settings_file()));
        assert_eq!(app.settings().get("launchArguments"), Some("-quicktest"));
        app.keep_setting("font", "Sans").unwrap();
        app.add_setup("Other").unwrap();
        let kept = Settings::read(&app.settings_file());
        assert_eq!(kept.get("game"), Some("/games/rw"));
        assert_eq!(kept.get(IN_USE), None);
        assert_eq!(app.setups().len(), 2);
    }

    #[test]
    fn two_setups_keep_their_own_lists_notes_folders_and_launch() {
        let (tmp, app) = installed();
        app.keep_setting("launchArguments", "-quicktest -savedatafolder=/x")
            .unwrap();
        app.keep_list("Default's", None).unwrap();
        app.notes()
            .writing()
            .set("ann.roads", note("kept by Default"));
        app.notes().reading().write(&app.notes_file()).unwrap();

        app.add_setup("Second").unwrap();
        app.use_setup("Second").unwrap();
        app.reread_folders();
        assert_eq!(app.setup_in_use(), "Second");
        assert!(app.own_dir().starts_with(app.data_dir().join("setups")));
        assert!(app.saved_lists().is_empty());
        assert!(app.notes().reading().get("ann.roads").is_none());
        assert_eq!(app.settings().get("launchArguments"), Some("-quicktest"));
        let saves = app.own_dir().join("savedata");
        assert_eq!(app.paths().config.as_deref(), Some(saves.as_path()));
        assert!(
            saves.join("Config/ModsConfig.xml").is_file(),
            "the list in use came along"
        );
        let address = app.start_game().unwrap().unwrap();
        assert!(address.contains("savedatafolder"), "{address}");

        app.keep_setting("launchArguments", "-popupwindow").unwrap();
        app.keep_setting("font", "Mono").unwrap();
        app.keep_list("Second's", None).unwrap();
        app.notes()
            .writing()
            .set("ann.roads", note("kept by Second"));
        app.notes().reading().write(&app.notes_file()).unwrap();

        app.use_setup(DEFAULT).unwrap();
        app.reread_folders();
        let names = |app: &App| -> Vec<String> {
            app.saved_lists()
                .into_iter()
                .map(|list| list.name)
                .collect()
        };
        assert_eq!(names(&app), ["Default's"]);
        let text = |app: &App| app.notes().reading().get("ann.roads").unwrap().text.clone();
        assert_eq!(text(&app), "kept by Default");
        assert_eq!(app.paths().config, Some(tmp.path().join("config")));
        let shared = app.settings();
        assert_eq!(
            shared.get("launchArguments"),
            Some("-quicktest -savedatafolder=/x")
        );
        assert_eq!(shared.get("font"), Some("Mono"), "shared by every setup");
        drop(shared);

        app.use_setup("Second").unwrap();
        assert_eq!(names(&app), ["Second's"]);
        assert_eq!(text(&app), "kept by Second");
        assert_eq!(app.settings().get("launchArguments"), Some("-popupwindow"));
        let restarted = App::new(
            tmp.path().join("home"),
            &tmp.path().join("settings"),
            &tmp.path().join("data"),
            rimstack_core::rimworld::paths::Overrides::default(),
        );
        assert_eq!(restarted.setup_in_use(), "Second");
    }

    fn note(text: &str) -> rimstack_core::kept::notes::Note {
        rimstack_core::kept::notes::Note {
            text: text.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn names_are_checked_and_default_and_the_one_in_use_stay() {
        let (_tmp, app) = installed();
        app.add_setup("One").unwrap();
        app.add_setup("two").unwrap();
        let names: Vec<_> = app.setups().into_iter().map(|row| row.name).collect();
        assert_eq!(names, [DEFAULT, "One", "two"]);
        for refused in ["TWO", "default", "a/b", "..", " ", "../up"] {
            assert!(app.add_setup(refused).is_err(), "{refused}");
        }
        assert!(app.rename_setup(DEFAULT, "Root").is_err());
        assert!(app.delete_setup(DEFAULT).is_err());
        app.use_setup("One").unwrap();
        assert!(app.delete_setup("One").is_err());
        app.rename_setup("One", "Uno").unwrap();
        assert_eq!(app.setup_in_use(), "Uno");
        let saves = app.own_dir().join("savedata");
        assert_eq!(
            app.settings().get("config"),
            Some(&*saves.to_string_lossy())
        );
        app.delete_setup("two").unwrap();
        assert_eq!(app.setups().len(), 2);
        let aside = std::fs::read_dir(app.removed_setups_dir()).unwrap().count();
        assert_eq!(aside, 1, "moved aside, not destroyed");
    }

    #[test]
    fn unsaved_changes_hold_a_switch_off() {
        let (_tmp, app) = installed();
        app.add_setup("B").unwrap();
        {
            let mut session = app.session();
            let mut changed = session.current().to_vec();
            changed.push("ann.roads".to_string());
            session.record(changed);
        }
        assert!(app.use_setup("B").is_err());
        assert_eq!(app.setup_in_use(), DEFAULT);
    }
}
