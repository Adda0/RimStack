use super::App;
use crate::changes::Changes;
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::{journal, lists};
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::share;
use rimstack_core::rimworld::modsconfig;
use rimstack_core::say;
use rimstack_core::sources::rentry;
use serde::Serialize;
use std::path::{Path, PathBuf};

const CORE: &str = "ludeon.rimworld";

const SNAPSHOTS_SHOWN: usize = 5;

const HISTORY_SHOWN: usize = 40;

/// The English an earlier version named a snapshot by, before its kind.
const OLDER_NAMES: [(&str, &str); 4] = [
    ("Before saving, ", "@saving"),
    ("Before sorting, ", "@sorting"),
    ("Before using another list, ", "@list"),
    ("Before finding the broken mod, ", "@hunt"),
];

/// The month names an earlier version wrote in the system's language.
const MONTHS: [&[&str]; 12] = [
    &["jan", "janv"],
    &["feb", "fév", "févr"],
    &["mar", "mars", "mär"],
    &["apr", "avr"],
    &["may", "mai"],
    &["jun", "juin"],
    &["jul", "juil"],
    &["aug", "août"],
    &["sep", "sept"],
    &["oct", "okt"],
    &["nov"],
    &["dec", "déc", "dez"],
];

fn before(kind: &str, when: &str) -> Option<Sentence> {
    Some(match kind {
        "@saving" => say!("Before saving, {}", when),
        "@sorting" => say!("Before sorting, {}", when),
        "@list" => say!("Before using another list, {}", when),
        "@hunt" => say!("Before finding the broken mod, {}", when),
        _ => return None,
    })
}

/// "2026-10-13 02.56.55", or an earlier version's own form "12 oct. 2026 20.17.39".
fn stamp(text: &str) -> String {
    let parts: Vec<&str> = text.split(' ').collect();
    let clock = |time: &str| -> Option<String> {
        let numbers: Vec<&str> = time.split('.').collect();
        let digits = |s: &&str| s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit());
        (numbers.len() == 3 && numbers.iter().all(digits))
            .then(|| format!("{}:{}", numbers[0], numbers[1]))
    };
    let dated = |day: &str| {
        let b = day.as_bytes();
        day.len() == 10
            && b[4] == b'-'
            && b[7] == b'-'
            && day
                .bytes()
                .enumerate()
                .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    };
    match parts.as_slice() {
        [day, time] if dated(day) => clock(time).map(|t| format!("{day} {t}")),
        [day, month, year, time] => {
            let month = month.trim_end_matches('.').to_lowercase();
            let at = MONTHS
                .iter()
                .position(|names| names.contains(&month.as_str()));
            let day = day.parse::<u8>().ok().filter(|_| day.len() <= 2);
            let year = Some(*year).filter(|y| y.len() == 4 && y.parse::<u16>().is_ok());
            match (at, day, year, clock(time)) {
                (Some(at), Some(day), Some(year), Some(t)) => {
                    Some(format!("{year}-{:02}-{day:02} {t}", at + 1))
                }
                _ => None,
            }
        }
        _ => None,
    }
    .unwrap_or_else(|| text.to_string())
}

pub fn snapshot_title(name: &str) -> Sentence {
    let (kind, rest) = name.split_once(' ').unwrap_or((name, ""));
    if let Some(title) = before(kind, &stamp(rest)) {
        return title;
    }
    OLDER_NAMES
        .iter()
        .find_map(|(english, kind)| {
            let rest = name.strip_prefix(english)?;
            before(kind, &stamp(rest))
        })
        .unwrap_or_else(|| Sentence::Plain(name.to_string()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub name: String,
    pub title: Sentence,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HistoryLine {
    pub time: u64,
    pub said: Sentence,
    pub restore: String,
    pub bring_back: String,
}

impl App {
    /// `active` as text, one a line, as `Copy the load order` gives them.
    pub fn listing(&self, active: &[ModId]) -> String {
        use rimstack_core::mods::share::{Entry, listing};
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return String::new();
        };
        let entries: Vec<Entry> = active
            .iter()
            .filter_map(|&id| library.get(id))
            .map(Entry::of)
            .collect();
        let paths = self.paths();
        let version = paths
            .full_game_version
            .as_deref()
            .unwrap_or(&library.game_version);
        listing(version, &entries)
    }

    pub fn saved_lists(&self) -> Vec<lists::Saved> {
        lists::all(&self.lists_dir())
    }

    /// `active` as RimSort's CSV export, at `path` with `.csv` added when it names no
    /// extension; the file written and how many mods are in it.
    pub fn export_csv(&self, active: &[ModId], path: &Path) -> Result<(PathBuf, usize), Error> {
        use rimstack_core::mods::share_file::{self, Row};
        use rimstack_core::rimworld::paths::Source;
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return Err(Error::refused(say!("The mods are not read yet.")));
        };
        let steam = self.steam_record();
        let rows: Vec<Row> = active
            .iter()
            .filter_map(|&id| {
                let installed = library.get(id)?;
                let about = installed.about.as_ref().ok();
                Some(Row {
                    name: installed.display_name().to_string(),
                    author: about.map(|a| a.authors.join(", ")).unwrap_or_default(),
                    package_id: about.map(|a| a.package_id.clone()).unwrap_or_default(),
                    file_id: match installed.source {
                        Source::Workshop => installed.folder.clone(),
                        _ => String::new(),
                    },
                    supported: about
                        .map(|a| a.supported_versions.clone())
                        .unwrap_or_default(),
                    downloaded: installed
                        .created
                        .filter(|_| installed.source != Source::Official),
                    updated: steam.times.get(&installed.folder).copied(),
                    source: installed.source_label().to_string(),
                    path: installed.path.to_string_lossy().into_owned(),
                    url: about.and_then(|a| a.url.clone()).unwrap_or_default(),
                })
            })
            .collect();
        let mut path = path.to_path_buf();
        if path.extension().is_none() {
            path.set_extension("csv");
        }
        let written = rows.len();
        rimstack_core::base::atomic_file::write(&path, share_file::csv(&rows))?;
        Ok((path, written))
    }

    /// Newest first.
    pub fn recent_snapshots(&self) -> Vec<Snapshot> {
        let mut found = lists::all(&self.snapshots_dir());
        found.sort_by_key(|l| std::cmp::Reverse(l.modified));
        found
            .into_iter()
            .take(SNAPSHOTS_SHOWN)
            .map(|l| Snapshot {
                title: snapshot_title(&l.name),
                name: l.name,
            })
            .collect()
    }

    /// Newest first.
    pub fn history(&self) -> Vec<HistoryLine> {
        let snapshots: Vec<String> = lists::all(&self.snapshots_dir())
            .into_iter()
            .map(|l| l.name)
            .collect();
        journal::read(&self.journal_file())
            .into_iter()
            .rev()
            .take(HISTORY_SHOWN)
            .map(|entry| {
                let (how, about) = entry.revert.split_once(' ').unwrap_or_default();
                let kept = |name: &str| snapshots.iter().any(|s| s == name);
                HistoryLine {
                    time: entry.time,
                    said: Sentence::from_kept(&entry.what),
                    restore: match how == "list" && kept(about) {
                        true => about.to_string(),
                        false => String::new(),
                    },
                    bring_back: match how {
                        "unhide" => about.to_string(),
                        _ => String::new(),
                    },
                }
            })
            .collect()
    }

    /// `None` for the load order being edited.
    pub fn keep_list(&self, name: &str, ids: Option<Vec<String>>) -> Result<(), Error> {
        let ids = ids.unwrap_or_else(|| self.session().current().to_vec());
        lists::save(&self.lists_dir(), name, &ids)?;
        self.publish(Changes::LISTS);
        Ok(())
    }

    pub fn delete_list(&self, name: &str) {
        match lists::delete(&self.lists_dir(), name) {
            Ok(()) => self.publish(Changes::LISTS),
            Err(problem) => rimstack_core::problem!("{problem}"),
        }
    }

    pub fn list_named(&self, name: &str) -> Option<Vec<String>> {
        lists::load(&self.lists_dir(), name).ok()
    }

    pub fn snapshot_named(&self, name: &str) -> Option<Vec<String>> {
        lists::load(&self.snapshots_dir(), name).ok()
    }

    pub fn fresh_list(&self, with_expansions: bool) -> Vec<String> {
        let mut ids = vec![CORE.to_string()];
        if with_expansions {
            let found = self.install().official();
            ids.extend(
                found
                    .iter()
                    .map(|o| o.package_id.clone())
                    .filter(|id| id != CORE),
            );
        }
        ids
    }

    /// Blocks on the network for a link.
    pub fn ids_in_copied(&self, text: &str) -> Result<Vec<String>, Error> {
        let text = match rentry::link_id(text) {
            Some(page) => rentry::fetch(&page)?,
            None => text.to_string(),
        };
        Ok(share::ids_in(&text))
    }

    /// Blocks on the network.
    pub fn publish_list(&self, listing: &str) -> Result<rentry::Published, Error> {
        rentry::publish(listing)
    }

    pub fn original_list(&self) -> Option<Vec<String>> {
        let config = self.paths().mods_config()?;
        let original = config.with_file_name("ModsConfig.xml.rimstack-original");
        modsconfig::read(&original).ok().map(|config| config.active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    #[test]
    fn a_snapshot_is_titled_by_its_kind_and_moment_whatever_version_named_it() {
        let title = |name: &str| snapshot_title(name).to_string();
        assert_eq!(
            title("@sorting 2026-10-13 02.56.55"),
            "Before sorting, 2026-10-13 02:56"
        );
        assert_eq!(
            title("Before saving, 12 oct. 2026 20.17.39"),
            "Before saving, 2026-10-12 20:17"
        );
        assert_eq!(
            title("@hunt 3 Mär 2025 08.05.00"),
            "Before finding the broken mod, 2025-03-03 08:05"
        );
        assert_eq!(title("@list someday"), "Before using another list, someday");
        assert_eq!(title("My own list"), "My own list");
    }

    #[test]
    fn a_fresh_list_is_the_game_alone_or_with_its_expansions() {
        let (tmp, app) = installed();
        let about = tmp.path().join("game/Data/Royalty/About");
        std::fs::create_dir_all(&about).unwrap();
        std::fs::write(
            about.join("About.xml"),
            "<ModMetaData><packageId>Ludeon.RimWorld.Royalty</packageId></ModMetaData>",
        )
        .unwrap();
        assert_eq!(app.fresh_list(false), ["ludeon.rimworld"]);
        assert_eq!(
            app.fresh_list(true),
            ["ludeon.rimworld", "ludeon.rimworld.royalty"]
        );
    }

    #[test]
    fn a_list_is_kept_found_and_deleted_and_the_history_says_what_takes_it_back() {
        let (_tmp, app) = installed();
        app.keep_list("Mine", Some(vec!["a.b".into()])).unwrap();
        assert!(app.keep_list(" ", None).is_err());
        assert_eq!(app.saved_lists()[0].mods, 1);
        assert_eq!(app.list_named("Mine").unwrap(), ["a.b"]);
        app.delete_list("Mine");
        assert!(app.list_named("Mine").is_none());

        let snapshot = app.snapshot("@saving", &["a.b".to_string()]).unwrap();
        app.settle_writes();
        app.note_change(
            rimstack_core::say!("Saved the load order: {}", "x"),
            Some(&snapshot),
        );
        app.note(Sentence::from_kept("[\"Set aside\"]"), String::new());
        app.settle_writes();
        let history = app.history();
        assert_eq!(history[0].said.to_string(), "Set aside");
        assert_eq!(history[1].said.to_string(), "Saved the load order: x");
        assert_eq!(history[1].restore, snapshot);
        assert_eq!(app.recent_snapshots()[0].name, snapshot);
        assert!(
            app.recent_snapshots()[0]
                .title
                .to_string()
                .starts_with("Before saving, 2")
        );
        assert_eq!(app.snapshot_named(&snapshot).unwrap(), ["a.b"]);
        assert_eq!(app.original_list(), None);
    }

    #[test]
    fn the_active_mods_are_written_as_rimsorts_csv() {
        let (tmp, app) = installed();
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let active: Vec<ModId> = library
            .arrange(&["ludeon.rimworld".to_string()])
            .active
            .iter()
            .filter_map(|m| m.installed)
            .collect();
        let asked = tmp.path().join("home/modlist");
        let (written, count) = app.export_csv(&active, &asked).unwrap();
        assert_eq!(written, asked.with_extension("csv"));
        assert_eq!(count, 1);
        let text = std::fs::read_to_string(&written).unwrap();
        let mut lines = text.split("\r\n");
        assert_eq!(
            lines.next().unwrap().strip_prefix('\u{feff}').unwrap(),
            "Name,Author,Package ID,Published File Id,Supported Versions,Mod Downloaded,Updated on Workshop,Source,Path,Workshop Page"
        );
        let path = tmp.path().join("game/Data/Core");
        assert_eq!(
            lines.next().unwrap(),
            format!(
                "Ludeon.RimWorld,,Ludeon.RimWorld,,,,,Ludeon,{},",
                path.display()
            )
        );
        assert_eq!(lines.next(), Some(""));
    }
}
