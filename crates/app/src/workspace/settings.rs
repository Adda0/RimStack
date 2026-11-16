use super::App;
use super::secret::Secret;
use crate::changes::Changes;
use crate::reports::{
    Database, FolderVerdict, KeptCount, LastLaunch, ReleaseSaid, SettingsReport, ShortContrast,
    Summary,
};
use rimstack_core::base::error::Error;
use rimstack_core::base::locks::Shared;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::base::translate::{LOCALE, detected};
use rimstack_core::kept::foreign;
use rimstack_core::kept::owntheme;
use rimstack_core::kept::settings::{Switch, UPDATE_EVERY};
use rimstack_core::mods::community::{self, OwnRule};
use rimstack_core::mods::publish;
use rimstack_core::rimworld::workshopdb;
use rimstack_core::sources::replacements;
use rimstack_core::{said, say, say_n};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

const GLANCED: usize = 3;

/// An empty box leaves the folder to detection.
pub fn judge_folder(name: &str, typed: &str) -> FolderVerdict {
    use rimstack_core::rimworld::paths::{Folder, Os, judge};
    let kind = match name {
        "game" => Folder::Game,
        "config" => Folder::Config,
        _ => Folder::Workshop,
    };
    let typed = typed.trim();
    let judged = if typed.is_empty() {
        Ok(say!("The folder found automatically is used"))
    } else {
        judge(Os::running(), kind, std::path::Path::new(typed))
    };
    FolderVerdict {
        ok: judged.is_ok(),
        said: judged.unwrap_or_else(|said| said),
    }
}

const SHIPPED: &[(&str, &str)] = &[
    (
        "English",
        include_str!("../../assets/languages/English.json"),
    ),
    (
        "Deutsch",
        include_str!("../../assets/languages/Deutsch.json"),
    ),
    (
        "Español",
        include_str!("../../assets/languages/Español.json"),
    ),
    (
        "Français",
        include_str!("../../assets/languages/Français.json"),
    ),
    (
        "Italiano",
        include_str!("../../assets/languages/Italiano.json"),
    ),
    ("Polski", include_str!("../../assets/languages/Polski.json")),
    (
        "Português",
        include_str!("../../assets/languages/Português.json"),
    ),
    (
        "Русский",
        include_str!("../../assets/languages/Русский.json"),
    ),
    ("한국어", include_str!("../../assets/languages/한국어.json")),
];

const EVERY_TEXT: &str = include_str!("../../assets/words.json");

fn shipped(name: &str) -> Option<&'static str> {
    SHIPPED
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, words)| *words)
}

fn every(minutes: u64) -> Sentence {
    match minutes {
        1 => say!("Every minute"),
        60 => say!("Every hour"),
        n => say_n!(
            usize::try_from(n).unwrap_or(0),
            "Every {} minute",
            "Every {} minutes"
        ),
    }
}

pub fn effects(name: &str) -> Changes {
    Changes::SETTINGS
        | match name {
            "game" | "config" | "workshop" => Changes::FOLDERS,
            "language" => Changes::LANGUAGE,
            "gitForWorkshop" => Changes::UPDATES,
            "showMature" => Changes::SOURCE,
            "logsKept" => Changes::LOGS,
            "communityRules" | "fileHints" | "versionWarnings" | "versionAsideFor" => {
                Changes::PROBLEMS
            }
            "databasesOff" => Changes::COMMUNITY | Changes::PROBLEMS,
            _ => Changes::NONE,
        }
}

impl App {
    pub fn languages_dir(&self) -> PathBuf {
        self.settings_file().with_file_name("languages")
    }

    pub fn theme_file(&self) -> PathBuf {
        self.settings_file().with_file_name("theme.json")
    }

    fn rimsort_rules_file(&self) -> PathBuf {
        self.rimsort_dir().join("dbs").join("userRules.json")
    }

    pub fn keep_setting(&self, name: &str, value: &str) -> Result<(), Error> {
        self.set_setting(name, value)?;
        self.publish(effects(name));
        Ok(())
    }

    /// The game's folders and RimStack's own that are there now.
    pub fn open_folders(&self) -> Vec<(Sentence, PathBuf)> {
        let paths = self.paths();
        let config = paths.config.clone();
        let found = [
            (say!("RimWorld folder"), paths.game.clone()),
            (say!("Local mods folder"), paths.local_mods()),
            (say!("Workshop mods folder"), paths.workshop.clone()),
            (say!("Config and saves folder"), config.clone()),
            (
                say!("Mod settings folder"),
                config.as_ref().map(|c| c.join("Config")),
            ),
            (
                say!("Saves folder"),
                config.as_ref().map(|c| c.join("Saves")),
            ),
            (say!("Game log"), self.game_log()),
            (
                say!("RimStack data folder"),
                Some(self.data_dir().to_path_buf()),
            ),
        ];
        found
            .into_iter()
            .filter_map(|(name, path)| Some((name, path?)))
            .filter(|(_, path)| path.exists())
            .collect()
    }

    pub fn settings_report(&self) -> SettingsReport {
        let paths = self.paths();
        let shown = |p: &Option<PathBuf>| p.as_ref().map(|p| p.to_string_lossy().into_owned());
        let token_held = self.vault.get(Secret::GithubToken);
        let publish_why_not = self.publish_why_not();
        let settings = self.settings();
        let github_token =
            settings.switch(Switch::GithubTokenKept) && matches!(&token_held, Ok(Some(_)));
        let github_token_note = token_held.err();
        let owned = |name: &str| settings.get(name).map(String::from);
        let config = paths.mods_config();
        let backup = config
            .as_ref()
            .map(|c| c.with_file_name("ModsConfig.xml.rimstack-backup"));
        let logs = self.kept_logs_dir();
        SettingsReport {
            game: shown(&paths.game),
            no_mod_folders: paths.roots().is_empty(),
            config: shown(&paths.config),
            workshop: shown(&paths.workshop),
            game_version: paths.game_version.clone(),
            mods_config: shown(&config),
            backup: shown(&backup.filter(|b| b.exists())),
            set_game: owned("game"),
            set_config: owned("config"),
            set_workshop: owned("workshop"),
            launch_arguments: owned("launchArguments"),
            kept_mods_megabytes: owned("keptModsMegabytes"),
            launch_directly: settings.launch_directly(),
            sandboxed: rimstack_core::base::sandbox::sandboxed(),
            language: owned("language"),
            english: self.language_in_use() == "English",
            language_in_use: self.language_in_use(),
            steamcmd: self.steamcmd().is_some(),
            interface_scale: settings.get("interfaceScale").unwrap_or("1").to_string(),
            font: owned("font"),
            community_rules: settings.switch(Switch::CommunityRules),
            version_warnings: settings.switch(Switch::VersionWarnings),
            version_aside_for: owned("versionAsideFor"),
            file_hints: settings.switch(Switch::FileHints),
            dim_left_out: settings.switch(Switch::DimLeftOut),
            git_for_workshop: settings.switch(Switch::GitForWorkshop),
            auto_updates: settings.switch(Switch::AutoUpdates),
            auto_updates_asked: settings.get(Switch::AutoUpdates.name()).is_some(),
            update_every: UPDATE_EVERY
                .iter()
                .map(|m| every(m.parse().unwrap_or(0)))
                .collect(),
            update_every_at: UPDATE_EVERY
                .iter()
                .position(|m| m.parse() == Ok(settings.update_every_minutes()))
                .unwrap_or(0),
            update_every_said: every(settings.update_every_minutes()),
            update_every_set: settings.get("updateEvery").is_some(),
            git: self.git().clone(),
            self_check: settings.switch(Switch::SelfCheck),
            show_mature: settings.switch(Switch::ShowMature),
            nightly_updates: settings.nightly_updates(),
            logs_kept: settings.logs_kept().unwrap_or(10),
            logs_kept_now: rimstack_core::rimworld::logs::kept(&logs).len(),
            replacements_ignored: settings
                .listed("replacementsIgnored")
                .map(String::from)
                .collect(),
            github_user: owned("githubUser"),
            github_token,
            github_token_note,
            rules_repo: owned("rulesRepo").filter(|repo| !repo.trim().is_empty()),
            rules_repo_default: publish::COMMUNITY.to_string(),
            own_rules: self.user_rules().reading().len(),
            publish_why_not,
            setup_done: settings.get("setupDone").is_some(),
            rimsort: self.rimsort_rules_file().is_file() || self.rimsort_notes_file().is_file(),
            rimpy: self.rimpy_file().is_file(),
            official: self
                .install()
                .official()
                .iter()
                .take(GLANCED)
                .cloned()
                .collect(),
            data_folder: self.data_dir().to_string_lossy().into_owned(),
            setups: self.setups(),
            local_mods: shown(&paths.local_mods()),
            config_folder: shown(&paths.config.as_ref().map(|c| c.join("Config"))),
            game_log: shown(&self.game_log()),
            steamcmd_folder: self
                .data_dir()
                .join("steamcmd")
                .to_string_lossy()
                .into_owned(),
            steamcmd_refused: rimstack_core::sources::redownload::refusal(),
            settings_file: self.settings_file().to_string_lossy().into_owned(),
            diagnostics_file: self.diagnostics_file().to_string_lossy().into_owned(),
            version: build_label(),
            orphan_settings: self.orphan_settings(),
        }
    }

    /// The repository `rulesRepo` names; the community's when none, why when it is unusable.
    pub fn rules_repo(&self) -> Result<publish::Repo, Error> {
        let kept = self
            .settings()
            .get("rulesRepo")
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(String::from);
        publish::repo_of(kept.as_deref().unwrap_or(publish::COMMUNITY))
    }

    /// Why "Publish my rules" is off now; `None` when it may run.
    fn publish_why_not(&self) -> Option<Sentence> {
        if self.user_rules().reading().is_empty() {
            return Some(say!("There are no own rules to publish yet."));
        }
        match self.vault.get(Secret::GithubToken) {
            Err(why) => return Some(why),
            Ok(Some(token)) if !token.trim().is_empty() => {}
            Ok(_) => {
                return Some(say!(
                    "Put your GitHub token in the settings: the rules are published from your account."
                ));
            }
        }
        self.rules_repo()
            .err()
            .map(|problem| problem.sentence().clone())
    }

    /// The user's own rules as they would be published: the library's name of the earlier mod
    /// for the reviewer to read, its folded id when that is not known.
    fn own_rules_for_publish(&self) -> Vec<OwnRule> {
        let pairs = self.user_rules().reading().pairs().to_vec();
        let name_of = |first: &str| -> String {
            let Ok((library, _)) = &*self.library() else {
                return first.to_string();
            };
            library.name_of(first).unwrap_or_else(|| first.to_string())
        };
        pairs
            .into_iter()
            .map(|(first, then)| OwnRule {
                first_name: name_of(&first),
                first,
                then,
            })
            .collect()
    }

    pub fn keep_github_token(&self, token: &str) -> Result<(), Sentence> {
        let token = token.trim();
        if token.is_empty() {
            return Err(say!("Type the token first"));
        }
        self.vault.set(Secret::GithubToken, token)?;
        self.mark_github_token(true);
        self.publish(Changes::SETTINGS);
        Ok(())
    }

    pub fn forget_github_token(&self) -> Result<(), Sentence> {
        self.vault.delete(Secret::GithubToken)?;
        self.mark_github_token(false);
        self.publish(Changes::SETTINGS);
        Ok(())
    }

    fn mark_github_token(&self, kept: bool) {
        let value = if kept { "on" } else { "off" };
        if let Err(problem) = self.set_setting(Switch::GithubTokenKept.name(), value) {
            rimstack_core::problem!("github token flag: {problem}");
        }
    }

    /// The user's own rules as a pull request to the community rules repository, from the
    /// user's GitHub account. Blocks on the network; the token goes only to GitHub.
    pub fn publish_rules(&self) -> Result<publish::Published, Error> {
        let own = self.own_rules_for_publish();
        if let Some(why_not) = self.publish_why_not() {
            return Err(Error::refused(why_not));
        }
        let token = match self.vault.get(Secret::GithubToken) {
            Ok(Some(token)) => token,
            Ok(None) => {
                return Err(Error::refused(say!(
                    "Put your GitHub token in the settings: the rules are published from your account."
                )));
            }
            Err(why) => return Err(Error::refused(why)),
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        publish::publish(&token, &self.rules_repo()?, &own, now)
    }

    pub fn summary(&self) -> Summary {
        let config = self.paths().config.clone();
        let saves = config
            .as_ref()
            .and_then(|config| std::fs::read_dir(config.join("Saves")).ok())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("rws"))
            })
            .count();
        let kept = config
            .as_deref()
            .and_then(rimstack_core::rimworld::rimsaves::settings);
        let last = rimstack_core::kept::launches::read(&self.launches_file()).pop();
        let ignored = self.replacements_ignored();
        let replacements = match &*self.library() {
            Ok((library, _)) => {
                let known = self.replacements();
                let mods = library.workshop_mods();
                let offered = mods.keys().filter(|id| known.contains_key(**id));
                offered.filter(|id| !ignored.contains(**id)).count()
            }
            Err(_) => 0,
        };
        let folders = kept.map_or(1, |s| s.folders.len());
        let snapshots = rimstack_core::kept::lists::count(&self.snapshots_dir());
        let rules = self.user_rules().reading().len();
        let community = (*self.community()).as_ref().map_or(0, |c| c.len());
        let saved = match folders {
            0 | 1 => Sentence::Number(saves as i64),
            _ => say!("{} in {} RimSaves folders", saves, folders),
        };
        let counts = [
            (say!("Saves"), saved),
            (say!("Snapshots"), Sentence::Number(snapshots as i64)),
            (say!("Own rules"), Sentence::Number(rules as i64)),
            (
                say!("Community rules"),
                match community {
                    0 => say!("Not downloaded"),
                    n => say_n!(n, "{} mod", "{} mods"),
                },
            ),
        ];
        let noted = [
            Some(say_n!(saves, "{} save", "{} saves")),
            Some(say_n!(snapshots, "{} snapshot", "{} snapshots")),
            (community > 0).then(|| say_n!(community, "rules for {} mod", "rules for {} mods")),
        ];
        Summary {
            saves,
            folders,
            snapshots,
            replacements,
            rules,
            kept_note: Sentence::listed(noted.into_iter().flatten().collect()),
            kept: counts
                .into_iter()
                .filter(|(_, value)| !value.is_empty())
                .map(|(label, value)| KeptCount { label, value })
                .collect(),
            last_launch: last.map(|l| LastLaunch {
                time: l.time,
                errors: l.errors,
                mods: l.ids.len(),
            }),
        }
    }

    /// Shipped first, then the user's own, by name.
    pub fn languages(&self) -> Vec<String> {
        let files = std::fs::read_dir(self.languages_dir())
            .into_iter()
            .flatten();
        let mut own: Vec<String> = files
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|x| x == "json"))
            .filter_map(|path| Some(path.file_stem()?.to_string_lossy().into_owned()))
            .filter(|name| shipped(name).is_none())
            .collect();
        own.sort();
        SHIPPED
            .iter()
            .map(|(name, _)| name.to_string())
            .chain(own)
            .collect()
    }

    /// Settings-folder texts take precedence over shipped ones.
    pub fn language_table(&self, name: &str) -> Map<String, Value> {
        let file = self.languages_dir().join(format!("{name}.json"));
        let own = std::fs::read_to_string(file).unwrap_or_default();
        rimstack_core::base::translate::table(shipped(name).unwrap_or_default(), &own)
    }

    /// Else the first system language with a file, else English.
    pub fn language_in_use(&self) -> String {
        let chosen = self
            .settings()
            .get("language")
            .map(str::trim)
            .map(String::from);
        if let Some(chosen) = chosen.filter(|chosen| !chosen.is_empty()) {
            return chosen;
        }
        let dir = self.languages_dir();
        let detect = || {
            let names = self.languages();
            let tables: Vec<(&str, Map<String, Value>)> = names
                .iter()
                .map(|name| (name.as_str(), self.language_table(name)))
                .collect();
            let offered: Vec<(&str, &str)> = tables
                .iter()
                .map(|(name, table)| {
                    (
                        *name,
                        table
                            .get(LOCALE)
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    )
                })
                .collect();
            detected(&self.system_languages, &offered)
                .unwrap_or("English")
                .to_string()
        };
        let found = self
            .detected_language
            .get(rimstack_core::base::watch::changed_at(&dir), detect);
        found.as_ref().clone()
    }

    pub fn speak(&self) -> Map<String, Value> {
        self.language_table(&self.language_in_use())
    }

    pub fn start_language(&self, name: &str) -> Result<PathBuf, Error> {
        let name = name.trim();
        if name.is_empty() || name.contains(['/', '\\']) {
            return Err(Error::refused(said!(
                "A language needs a name without slashes"
            )));
        }
        let dir = self.languages_dir();
        let file = dir.join(format!("{name}.json"));
        if !file.exists() {
            let start = shipped(name).unwrap_or(EVERY_TEXT);
            std::fs::create_dir_all(&dir)
                .and_then(|()| std::fs::write(&file, start))
                .map_err(|problem| Error::file(&file, problem))?;
        }
        Ok(file)
    }

    pub fn own_colours(&self, theme: &str) -> Map<String, Value> {
        let text = std::fs::read_to_string(self.theme_file()).unwrap_or_default();
        let file = serde_json::from_str::<Value>(&text).unwrap_or(Value::Null);
        let known = self.settings().get("ownColoursFor").map(String::from);
        let flat_for = match known {
            Some(known) => known,
            None if owntheme::is_flat(&file) => {
                if let Err(problem) = self.set_setting("ownColoursFor", theme) {
                    rimstack_core::problem!("{problem}");
                }
                theme.to_string()
            }
            None => String::new(),
        };
        owntheme::colours_for(&file, theme, &flat_for)
    }

    pub fn start_own_theme(&self, theme: &str, colours: &Value) -> Result<PathBuf, Error> {
        let file = self.theme_file();
        let now = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        let wanted = match now {
            None => Some(json!({ theme: colours })),
            Some(now) => owntheme::with_theme(&now, theme, colours),
        };
        if let Some(pretty) = wanted.and_then(|w| serde_json::to_string_pretty(&w).ok()) {
            rimstack_core::base::atomic_file::write(&file, pretty)?;
            self.publish(Changes::THEME);
        }
        Ok(file)
    }

    pub fn databases(&self) -> Vec<Database> {
        let data = self.data_dir();
        let version = self.paths().game_version.clone().unwrap_or_default();
        let all = [
            (
                "rules",
                "Community Rules Database",
                Sentence::named("Load-order rules, for Sort"),
                community::file_in(data),
            ),
            (
                "worksOn",
                "No Version Warning",
                Sentence::named("Mods that work on this game version without saying so"),
                community::works_on_file(data, &version),
            ),
            (
                "replacements",
                "Use This Instead",
                Sentence::named("Maintained replacements for abandoned mods"),
                replacements::file_in(data),
            ),
            (
                "names",
                "Steam Workshop Database",
                Sentence::named("Names of mods that are not installed"),
                workshopdb::file_in(data),
            ),
        ];
        all.into_iter()
            .map(|(id, name, used_for, file)| {
                let updated = rimstack_core::base::watch::changed_at(&file) / 1000;
                let on = self.database_on(id);
                let waiting = on && updated == 0;
                Database {
                    id,
                    name,
                    used_for: if waiting {
                        say!("Used once downloaded: {}", used_for.inside())
                    } else {
                        used_for
                    },
                    updated,
                    on,
                    waiting,
                }
            })
            .collect()
    }

    pub fn use_database(&self, id: &str, on: bool) -> Result<(), Error> {
        let off: Vec<&str> = self
            .databases()
            .iter()
            .filter(|database| match database.id == id {
                true => !on,
                false => !database.on,
            })
            .map(|database| database.id)
            .collect();
        self.keep_setting("databasesOff", &off.join(","))
    }

    /// Browse searches the index the same download writes, so it is fetched even where the names
    /// are switched off until the index exists.
    fn names_wanted(&self) -> bool {
        self.database_on("names")
            || !rimstack_core::rimworld::workshopindex::file_in(self.data_dir()).exists()
    }

    pub fn download_databases(&self) -> Result<(), Error> {
        let data = self.data_dir();
        let mut failed: Vec<Error> = Vec::new();
        if self.database_on("rules") {
            let file = community::file_in(data);
            match community::download(&file) {
                Ok(fresh) => self.community.put(super::written(file), Some(fresh)),
                Err(why) => failed.push(why),
            }
        }
        let paths = self.paths();
        let version = paths.game_version.as_deref();
        if let Some(version) = version.filter(|_| self.database_on("worksOn")) {
            let file = community::works_on_file(data, version);
            match community::download_works_on(version, &file) {
                Ok(fresh) => self.works_on.put(super::written(file), fresh),
                Err(why) => failed.push(why),
            }
        }
        if self.database_on("replacements") {
            failed.extend(replacements::download(&replacements::file_in(data)).err());
            let _ = self.replacements();
        }
        if self.names_wanted() {
            let file = workshopdb::file_in(data);
            let own_file = workshopdb::own_file_in(data);
            match workshopdb::download(&file) {
                Ok(fresh) => self.workshop_names.put(
                    (super::written(file), super::written(own_file.clone())),
                    workshopdb::merged(fresh, workshopdb::read(&own_file)),
                ),
                Err(why) => failed.push(why),
            }
        }
        *self
            .community_problem
            .write()
            .unwrap_or_else(|p| p.into_inner()) = failed.first().map(|why| why.sentence().clone());
        self.publish(Changes::COMMUNITY | Changes::PROBLEMS);
        for why in &failed {
            rimstack_core::problem!("{why}");
        }
        failed.into_iter().next().map_or(Ok(()), Err)
    }

    /// Names the installed Workshop mods, and refreshes from Steam the names of mods the load
    /// order or a saved list refers to that are not installed. Kept beside the download.
    pub fn refresh_workshop_names(&self) -> Result<Sentence, Error> {
        use rimstack_core::rimworld::{id, paths::Source, workshop::fetch_details};
        let data = self.data_dir();
        let (file, own_file) = (workshopdb::file_in(data), workshopdb::own_file_in(data));
        let mut own = workshopdb::read(&own_file);
        let library = self.library();
        let library = library.as_ref().as_ref().map(|(library, _)| library).ok();
        let mut changed = 0;
        let workshop = library
            .into_iter()
            .flat_map(|library| library.installed.iter())
            .filter(|m| m.source == Source::Workshop);
        for m in workshop {
            let (Some(key), Ok(about)) = (m.key(), &m.about) else {
                continue;
            };
            let name = about
                .name
                .clone()
                .unwrap_or_else(|| about.package_id.clone());
            let item = workshopdb::Item {
                workshop_id: m.folder.clone(),
                name,
            };
            if own.get(&key) != Some(&item) {
                own.insert(key, item);
                changed += 1;
            }
        }
        let merged = workshopdb::merged(workshopdb::read(&file), own.clone());
        let installed_ids: HashSet<String> = library
            .into_iter()
            .flat_map(|library| &library.installed)
            .filter_map(|m| m.key())
            .collect();
        let missing = |ids: &[String]| {
            ids.iter()
                .map(|folder_id| id::fold(folder_id))
                .filter(|folded| !installed_ids.contains(folded))
                .filter_map(|folded| {
                    merged
                        .get(&folded)
                        .map(|item| (folded, item.workshop_id.clone()))
                })
                .collect::<Vec<(String, String)>>()
        };
        let mut ids: Vec<(String, String)> = missing(self.session().current());
        for dir in [self.lists_dir(), self.snapshots_dir()] {
            for saved in rimstack_core::kept::lists::all(&dir) {
                if let Ok(here) = rimstack_core::kept::lists::load(&dir, &saved.name) {
                    ids.extend(missing(&here));
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        let workshop_ids: Vec<String> = ids.iter().map(|(_, id)| id.clone()).collect();
        let by_id: HashMap<&str, &str> = ids
            .iter()
            .map(|(folded, id)| (id.as_str(), folded.as_str()))
            .collect();
        let mut renamed = 0;
        for found in fetch_details(&workshop_ids)? {
            let Some(folded) = by_id.get(found.id.as_str()) else {
                continue;
            };
            let item = workshopdb::Item {
                workshop_id: found.id.clone(),
                name: found.title,
            };
            // Only a real change comes into `own`: the download's entry unchanged stays its own.
            if merged.get(*folded) != Some(&item) && own.get(*folded) != Some(&item) {
                own.insert(folded.to_string(), item);
                renamed += 1;
            }
        }
        workshopdb::store(&own, &own_file)?;
        self.workshop_names.put(
            (super::written(file.clone()), super::written(own_file)),
            workshopdb::merged(workshopdb::read(&file), own),
        );
        self.publish(Changes::COMMUNITY | Changes::PROBLEMS);
        let mut said: Vec<Sentence> = Vec::new();
        if changed > 0 {
            said.push(say_n!(
                changed,
                "Names of {} installed mod refreshed.",
                "Names of {} installed mods refreshed.",
                changed
            ));
        }
        if renamed > 0 {
            said.push(say_n!(
                renamed,
                "Names of {} mod re-read from Steam.",
                "Names of {} mods re-read from Steam.",
                renamed
            ));
        }
        if said.is_empty() {
            said.push(say!("Nothing changed."));
        }
        Ok(Sentence::joined(" ", said))
    }

    /// Why the community data last failed to download, until one succeeds.
    pub fn community_problem(&self) -> Option<Sentence> {
        self.community_problem
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// 0: the game version, 1: the installed version, 2: never.
    pub fn version_warnings(&self, choice: i32) -> Result<(), Error> {
        let installed = self.paths().game_version.clone().unwrap_or_default();
        let aside_for = if choice == 1 { installed.as_str() } else { "" };
        self.keep_setting("versionWarnings", if choice == 2 { "off" } else { "on" })?;
        self.keep_setting("versionAsideFor", aside_for)
    }

    /// The RimSort database that keeps tags, notes and colours, read only.
    fn rimsort_notes_file(&self) -> PathBuf {
        self.rimsort_dir()
            .join("instances")
            .join("Default")
            .join("aux_metadata.db")
    }

    /// The RimPy settings file that keeps row colours, read only.
    fn rimpy_file(&self) -> PathBuf {
        self.home
            .join("AppData/LocalLow")
            .join("RimPy Mod Manager")
            .join("config.ini")
    }

    /// Imports RimSort's own load order rules and its tags, notes and colours, read only.
    pub fn import_rimsort(&self) -> Sentence {
        let mut said = Vec::new();
        if self.rimsort_rules_file().is_file() || !self.rimsort_notes_file().is_file() {
            said.push(self.import_rimsort_rules());
        }
        if self.rimsort_notes_file().is_file() {
            let path = self.rimsort_notes_file();
            said.push(match foreign::rimsort(&path) {
                Ok(kept) => self.adopted(&kept),
                Err(problem) => problem.sentence().clone(),
            });
        }
        Sentence::joined(" ", said)
    }

    /// Imports RimPy's colours for mods, read only.
    pub fn import_rimpy(&self) -> Sentence {
        let path = self.rimpy_file();
        match std::fs::read_to_string(&path) {
            Ok(text) => self.adopted(&foreign::rimpy(&text)),
            Err(e) => Error::file(&path, e).sentence().clone(),
        }
    }

    fn adopted(&self, kept: &[foreign::Kept]) -> Sentence {
        let added = self.adopt(kept);
        say_n!(
            added,
            "Tags, notes or colours for {} mod, now here too.",
            "Tags, notes or colours for {} mods, now here too.",
            added
        )
    }

    /// The older rules-only import, used by the settings page's RimSort button.
    fn import_rimsort_rules(&self) -> Sentence {
        let path = self.rimsort_rules_file();
        let read = std::fs::read_to_string(&path).map_err(|e| Error::file(&path, e));
        let pairs = match read.and_then(|text| Ok(community::parse(&text)?.pairs())) {
            Ok(pairs) => pairs,
            Err(problem) => return problem.sentence().clone(),
        };
        let mut yours = self.user_rules().writing();
        let added = yours.import(&pairs);
        let kept = yours.clone();
        drop(yours);
        self.write_user_rules(kept);
        if pairs.is_empty() {
            return say!("RimSort has no rules of your own.");
        }
        self.publish(Changes::RULES | Changes::PROBLEMS);
        say_n!(
            pairs.len(),
            "RimSort's {} rule: {} new, now yours here.",
            "RimSort's {} rules: {} new, now yours here.",
            added
        )
    }

    /// A dated, unused folder named mod-settings in the setup's own data folder.
    fn settings_aside_dir(&self) -> PathBuf {
        rimstack_core::sources::maintenance::free_folder(
            &self
                .own_dir()
                .join(format!("mod-settings-{}", super::now_secs())),
        )
    }

    /// The game rewrites a mod's settings whenever they change; moving them under a
    /// running game would undo itself.
    fn running_check(&self) -> Result<(), Error> {
        if self.game_running() {
            return Err(Error::refused(said!(
                "RimWorld is running. Close the game before moving mod settings aside."
            )));
        }
        Ok(())
    }

    fn config_dir(&self) -> Result<PathBuf, Error> {
        self.paths()
            .config
            .clone()
            .ok_or_else(|| Error::refused(said!("The game's settings folder is not known")))
    }

    /// A moved file changes the orphan list and an open mod's details.
    fn finish_aside(
        &self,
        moved: Result<usize, Error>,
        aside: &std::path::Path,
    ) -> Result<Sentence, Error> {
        match moved? {
            0 => Ok(say!("No mod has settings")),
            count => {
                self.publish(Changes::MOD_SETTINGS);
                Ok(say_n!(
                    count,
                    "{} file moved to {}",
                    "{} files moved to {}",
                    aside.display()
                ))
            }
        }
    }

    pub fn set_mod_settings_aside(&self) -> Sentence {
        let moved = self.running_check().and_then(|()| {
            let config = self.config_dir()?;
            let aside = self.settings_aside_dir();
            self.finish_aside(
                rimstack_core::sources::maintenance::set_mod_settings_aside(&config, &aside),
                &aside,
            )
        });
        moved.unwrap_or_else(|problem| problem.sentence().clone())
    }

    /// The mod's `Mod_*` files are set aside, so it starts from the ones it ships, or its
    /// plain defaults; not while the game runs.
    pub fn reset_mod_settings(
        &self,
        mod_id: rimstack_core::mods::library::ModId,
    ) -> Result<Sentence, Error> {
        self.running_check()?;
        let config = self.config_dir()?;
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let m = library.get(mod_id).ok_or_else(|| {
            Error::refused(said!(
                "That mod is no longer where it was read; read the folders again"
            ))
        })?;
        let files = rimstack_core::rimworld::mod_settings::of(
            &config,
            &m.folder,
            installed_folders(library),
        );
        if files.is_empty() {
            return Ok(say!("This mod has no settings files"));
        }
        let aside = self.settings_aside_dir();
        self.finish_aside(
            rimstack_core::sources::maintenance::set_aside(&config, &aside, &files),
            &aside,
        )
    }

    /// A mod's `Mod_*` files, by name.
    pub fn mod_settings_files(&self, folder: &str) -> Vec<String> {
        let Ok(config) = self.config_dir() else {
            return Vec::new();
        };
        let Ok((library, _)) = &*self.library() else {
            return Vec::new();
        };
        rimstack_core::rimworld::mod_settings::of(&config, folder, installed_folders(library))
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .collect()
    }

    /// The `Mod_*` files whose mod is gone. Folder names cannot be judged before the folders
    /// are read, so nothing is called an orphan without them.
    pub fn orphan_settings(&self) -> Vec<String> {
        let paths = self.paths();
        let Some(config) = paths.config.as_ref() else {
            return Vec::new();
        };
        let Ok((library, _)) = &*self.library() else {
            return Vec::new();
        };
        if library.installed.is_empty() || paths.workshop.is_none() || paths.game.is_none() {
            return Vec::new();
        }
        rimstack_core::rimworld::mod_settings::orphans(config, installed_folders(library))
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .collect()
    }

    pub fn set_orphan_settings_aside(&self) -> Result<Sentence, Error> {
        self.running_check()?;
        let config = self.config_dir()?;
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let files =
            rimstack_core::rimworld::mod_settings::orphans(&config, installed_folders(library));
        if files.is_empty() {
            return Ok(say!("All settings files belong to installed mods"));
        }
        let aside = self.settings_aside_dir();
        self.finish_aside(
            rimstack_core::sources::maintenance::set_aside(&config, &aside, &files),
            &aside,
        )
    }
}

/// A mod's settings are named after its folder, so those say whether one is still there.
fn installed_folders(
    library: &rimstack_core::mods::library::Library,
) -> impl Fn(&str) -> bool + '_ {
    let folders: HashSet<&str> = library
        .installed
        .iter()
        .map(|m| m.folder.as_str())
        .collect();
    move |folder| folders.contains(folder)
}

pub fn short_contrast(colours: &Map<String, Value>) -> Vec<ShortContrast> {
    owntheme::short_pairs(colours)
        .into_iter()
        .map(|(what, found, needed)| ShortContrast {
            what,
            found,
            needed,
        })
        .collect()
}

const COMMIT: &str = match option_env!("RIMSTACK_COMMIT") {
    Some(commit) => commit,
    None => "",
};

pub fn build_label() -> String {
    match COMMIT {
        "" => env!("CARGO_PKG_VERSION").to_string(),
        commit => format!("{} ({commit})", env!("CARGO_PKG_VERSION")),
    }
}

pub fn newest_release(nightly: bool) -> ReleaseSaid {
    use rimstack_core::sources::release::{Channel, Running, check, newer};
    let running = Running {
        commit: COMMIT,
        made: option_env!("RIMSTACK_COMMIT_TIME")
            .and_then(|made| made.parse().ok())
            .unwrap_or(0),
    };
    let channel = if nightly {
        Channel::Nightly
    } else {
        Channel::Stable
    };
    match check(channel, std::env::consts::OS, running) {
        Ok(Some(release)) if nightly || newer(env!("CARGO_PKG_VERSION"), &release.version) => {
            ReleaseSaid::Newer {
                version: release.version,
                page: release.page,
            }
        }
        Ok(_) => ReleaseSaid::Latest {},
        Err(problem) => ReleaseSaid::Failed {
            problem: problem.sentence().clone(),
        },
    }
}

/// The families to choose from: "" for the system's first, then each once, by name ignoring case.
pub fn font_choices(mut names: Vec<String>) -> Vec<String> {
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names.insert(0, String::new());
    names
}

/// Where in `fonts` a name holds `wanted`, ignoring case; "" is the system's, named `system`.
pub fn fonts_matching(fonts: &[impl AsRef<str>], wanted: &str, system: &str) -> Vec<usize> {
    let wanted = wanted.to_lowercase();
    let named = |name: &str| match name.is_empty() {
        true => system.to_lowercase(),
        false => name.to_lowercase(),
    };
    (0..fonts.len())
        .filter(|&at| named(fonts[at].as_ref()).contains(&wanted))
        .collect()
}

#[cfg(test)]
mod tests;
