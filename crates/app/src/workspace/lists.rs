use super::App;
use crate::changes::Changes;
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::{journal, lists};
use rimstack_core::mods::library::ModId;
use rimstack_core::rimworld::modsconfig;
use rimstack_core::say;
use serde::Serialize;
use std::path::Path;

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryLine {
    pub time: u64,
    pub said: Sentence,
    pub restore: String,
    pub bring_back: String,
}

/// A mod in a comparison group.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ComparedMod {
    pub id: String,
    pub name: String,
}

/// Comparison of a saved list against the current load order.
#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub only_saved: Vec<ComparedMod>,
    pub only_current: Vec<ComparedMod>,
    pub moved: Vec<ComparedMod>,
    /// The saved-only mods that are installed, given to `takeAll` to join the
    /// load order in the saved list's own order.
    pub addable: Vec<ModId>,
}

impl App {
    pub fn saved_lists(&self) -> Vec<lists::Saved> {
        lists::all(&self.lists_dir())
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

    pub fn original_list(&self) -> Option<Vec<String>> {
        let config = self.paths().mods_config()?;
        let original = config.with_file_name("ModsConfig.xml.rimstack-original");
        modsconfig::read(&original).ok().map(|config| config.active)
    }
    /// A saved list against the load order as it is: all groups empty for no
    /// such list, or before the library is read.
    pub fn compare_saved(&self, name: &str) -> Comparison {
        let Some(saved) = self.list_named(name) else {
            return Comparison::default();
        };
        let current = self.session().current().to_vec();
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return Comparison::default();
        };
        let diff = lists::compare(&saved, &current);
        let arranged_saved = library.arrange(&saved);
        let arranged_current = library.arrange(&current);
        let shown = |at: usize, arranged: &rimstack_core::mods::library::Arranged| {
            let m = &arranged.active[at];
            ComparedMod {
                id: m.id.clone(),
                name: m.name.clone(),
            }
        };
        Comparison {
            only_saved: diff
                .only_saved
                .iter()
                .map(|&at| shown(at, &arranged_saved))
                .collect(),
            only_current: diff
                .only_current
                .iter()
                .map(|&at| shown(at, &arranged_current))
                .collect(),
            moved: diff
                .moved
                .iter()
                .map(|&at| shown(at, &arranged_saved))
                .collect(),
            addable: diff
                .only_saved
                .iter()
                .filter_map(|&at| arranged_saved.active[at].installed)
                .collect(),
        }
    }
    /// Writes the load order as a lockfile: everything a friend needs to check they run the same.
    pub fn write_lockfile(&self, file: &Path) -> Result<(), Error> {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return Err(Error::refused(say!("The mods are not read yet.")));
        };
        let lock = rimstack_core::mods::lock::locked(
            &library.game_version,
            self.session().current(),
            &library.installed,
            &self.steam_record().0,
        );
        rimstack_core::mods::lock::write(file, &lock)
    }
    /// Checks the load order against a lockfile; `Err(a file that is not one)`.
    pub fn check_lockfile(&self, file: &Path) -> Result<rimstack_core::mods::lock::Report, Error> {
        let lock = rimstack_core::mods::lock::read(file)?;
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return Err(Error::refused(say!("The mods are not read yet.")));
        };
        Ok(rimstack_core::mods::lock::check(
            &lock,
            &library.game_version,
            self.session().current(),
            &library.installed,
            &self.steam_record().0,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    #[test]
    fn a_lockfile_round_trips_and_a_differing_one_is_told() {
        let (_tmp, app) = installed();
        let file = std::env::temp_dir().join("rimstack-app-lock-test.json");
        app.write_lockfile(&file).unwrap();
        let same = app.check_lockfile(&file).unwrap();
        assert!(
            same.same,
            "{:?}",
            same.said.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        );
        assert!(same.present);
        assert_eq!(same.ids, ["ludeon.rimworld"]);

        let other = std::env::temp_dir().join("rimstack-app-lock-other.json");
        std::fs::write(
            &other,
            r#"{"game":"1.5","dlcs":[],"mods":[{"packageId":"gone.mod","source":"local"}]}"#,
        )
        .unwrap();
        let differing = app.check_lockfile(&other).unwrap();
        let said: Vec<String> = differing.said.iter().map(|s| s.to_string()).collect();
        assert!(
            said.iter().any(|s| s.contains("made for RimWorld 1.5"))
                && said.iter().any(|s| s.contains("not installed"))
                && said.iter().any(|s| s.contains("not in the lockfile")),
            "{said:?}"
        );
        assert!(!differing.present);

        std::fs::write(&other, "no lockfile").unwrap();
        assert!(app.check_lockfile(&other).is_err());
    }

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
    fn a_list_is_kept_found_and_deleted_and_the_history_says_what_takes_it_back() {
        let (_tmp, app) = installed();
        app.keep_list("Mine", Some(vec!["a.b".into()])).unwrap();
        assert!(app.keep_list(" ", None).is_err());
        assert_eq!(app.saved_lists()[0].mods, 1);
        assert_eq!(app.list_named("Mine").unwrap(), ["a.b"]);
        app.delete_list("Mine");
        assert!(app.list_named("Mine").is_none());

        let snapshot = app.snapshot("@saving", &["a.b".to_string()]).unwrap();
        app.note_change(
            rimstack_core::say!("Saved the load order: {}", "x"),
            Some(&snapshot),
        );
        app.note(Sentence::from_kept("[\"Set aside\"]"), String::new());
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
    fn comparing_a_saved_list_with_the_load_order_groups_each_mod() {
        let (_tmp, app) = installed();
        let saved = vec![
            "ludeon.rimworld".to_string(),
            "ann.roads".to_string(),
            "gone.mod".to_string(),
        ];
        app.keep_list("run", Some(saved)).unwrap();

        // Only rimworld active: the mods the run switched off are saved-only.
        app.session().record(vec!["ludeon.rimworld".to_string()]);
        let comp = app.compare_saved("run");
        fn ids(mods: &[ComparedMod]) -> Vec<&str> {
            mods.iter().map(|m| m.id.as_str()).collect()
        }
        assert_eq!(ids(&comp.only_saved), ["ann.roads", "gone.mod"]);
        assert_eq!(ids(&comp.only_current), Vec::<&str>::new());
        assert_eq!(ids(&comp.moved), Vec::<&str>::new());
        // A saved-only mod that is not installed cannot join the load order.
        assert_eq!(comp.addable.len(), 1);

        // The same mods, but the load order places roads first.
        app.session()
            .record(vec!["ann.roads".to_string(), "ludeon.rimworld".to_string()]);
        let comp = app.compare_saved("run");
        assert_eq!(ids(&comp.only_saved), ["gone.mod"]);
        assert_eq!(ids(&comp.only_current), Vec::<&str>::new());
        assert_eq!(ids(&comp.moved), ["ann.roads"]);
        assert_eq!(comp.addable.len(), 0);

        let none = app.compare_saved("absent");
        assert!(
            none.only_saved.is_empty() && none.only_current.is_empty() && none.moved.is_empty()
        );
    }
}
