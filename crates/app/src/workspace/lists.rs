use super::App;
use crate::changes::Changes;
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::{journal, lists, orders};
use rimstack_core::mods::library::ModId;
use rimstack_core::rimworld::modsconfig;
use rimstack_core::{say, say_n};
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

/// A load order ModsConfig.xml once held, from the kept history.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PastOrder {
    pub time: u64,
    pub said: Sentence,
    pub mods: usize,
    /// Only in that order; only in the current one; in both, elsewhere.
    pub added: usize,
    pub removed: usize,
    pub moved: usize,
}

/// The keys `compare_orders` takes besides `list:<name>` and `order:<at>`.
const CURRENT: &str = "current";
const GAME: &str = "game";

/// A mod in a comparison group.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ComparedMod {
    pub id: String,
    pub name: String,
    pub installed: bool,
    /// For a moved mod, the places it sits later (+) or earlier (−) on the right,
    /// among the mods both hold; 0 elsewhere.
    pub by: isize,
    /// Installed or not; for a moved mod, by how many places.
    pub said: Sentence,
}

/// A comparison group, its heading counting its mods.
#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub heading: Sentence,
    pub mods: Vec<ComparedMod>,
}

/// One order against another.
#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// The orders' titles.
    pub left: Sentence,
    pub right: Sentence,
    pub only_left: Group,
    pub only_right: Group,
    pub moved: Group,
    /// Both read, and the same mods in the same order.
    pub same: bool,
    /// With the load order on the right, the installed left-only mods, given to
    /// `takeAll` to join it in the left order's own order; empty otherwise.
    pub addable: Vec<ModId>,
    /// The groups as text: each heading, then a `- ` line per mod with its package id.
    pub report: String,
}

/// An order to compare, or with only a `title` the caption over a kind of them: a menu entry.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderChoice {
    pub key: String,
    pub text: Sentence,
    pub title: Sentence,
    pub pick: bool,
}

fn current_title() -> Sentence {
    say!("the load order")
}

fn game_title() -> Sentence {
    say!("the game's ModsConfig.xml")
}

fn order_title(seconds: u64) -> Sentence {
    use chrono::TimeZone;
    let at = i64::try_from(seconds)
        .ok()
        .and_then(|s| chrono::Local.timestamp_opt(s, 0).single());
    let moment = at.map_or(String::new(), |at| at.format("%Y-%m-%d %H:%M").to_string());
    say!("the order of {}", moment)
}

fn how_said(how: orders::How) -> Sentence {
    match how {
        orders::How::Saved => say!("Saved from RimStack"),
        orders::How::Game => say!("Rewritten by RimWorld as it started"),
        orders::How::Outside => say!("Changed outside RimStack"),
    }
}

fn only_in(n: usize, title: &Sentence) -> Sentence {
    say_n!(n, "{} mod only in {}", "{} mods only in {}", title.inside())
}

fn places(by: isize) -> Sentence {
    let n = by.unsigned_abs();
    match by.cmp(&0) {
        std::cmp::Ordering::Greater => say_n!(n, "{} place later", "{} places later"),
        std::cmp::Ordering::Less => say_n!(n, "{} place earlier", "{} places earlier"),
        std::cmp::Ordering::Equal => say!("in the same place, among mods that moved"),
    }
}

fn report(comparison: &Comparison) -> String {
    let put = crate::words::put;
    let (left, right) = (comparison.left.inside(), comparison.right.inside());
    let mut lines = vec![put(&say!("{} compared with {}", left, right))];
    if comparison.same {
        lines.push(String::new());
        lines.push(put(&say!("Both hold the same mods in the same order")));
    }
    for group in [
        &comparison.only_left,
        &comparison.only_right,
        &comparison.moved,
    ] {
        if group.mods.is_empty() {
            continue;
        }
        lines.push(String::new());
        lines.push(put(&group.heading));
        for m in &group.mods {
            lines.push(format!("- {} [{}], {}", m.name, m.id, put(&m.said)));
        }
    }
    lines.join("\n")
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

    /// `None` for the load order being edited. `with_settings` also keeps every active
    /// mod's `Mod_*.xml` beside the list; without it, a list's earlier folder stands.
    pub fn keep_list(
        &self,
        name: &str,
        ids: Option<Vec<String>>,
        with_settings: bool,
    ) -> Result<(), Error> {
        let ids = ids.unwrap_or_else(|| self.session().current().to_vec());
        if with_settings {
            self.keep_list_settings(name, &ids)?;
        }
        lists::save(&self.lists_dir(), name, &ids)?;
        self.publish(Changes::LISTS);
        Ok(())
    }

    /// The active mods' own `Mod_*.xml` files, by the folder the game names them after.
    fn keep_list_settings(&self, name: &str, ids: &[String]) -> Result<(), Error> {
        let config = self.config_dir()?;
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let arranged = library.arrange(ids);
        let active: std::collections::HashSet<&str> = arranged
            .active
            .iter()
            .filter_map(|m| m.installed)
            .filter_map(|id| library.get(id))
            .map(|m| m.folder.as_str())
            .collect();
        let installed = super::settings::installed_folders(library);
        let files: Vec<lists::SettingsFile> = rimstack_core::rimworld::mod_settings::files(&config)
            .into_iter()
            .filter_map(|path| {
                let owner = rimstack_core::rimworld::mod_settings::owner(&path, &installed)?;
                if !active.contains(owner) {
                    return None;
                }
                let mod_id = library
                    .installed
                    .iter()
                    .find(|m| m.folder == owner)
                    .and_then(|m| m.key())
                    .unwrap_or_else(|| owner.to_string());
                Some(lists::SettingsFile {
                    file: path.file_name()?.to_string_lossy().into_owned(),
                    folder: owner.to_string(),
                    mod_id,
                })
            })
            .collect();
        lists::keep_settings(&self.lists_dir(), name, &config, &files)?;
        Ok(())
    }

    /// The settings kept with a list take the place of the current ones, which the
    /// usual dated folder holds instead; never under a running game.
    pub fn use_list_settings(&self, name: &str) -> Result<(), Error> {
        self.running_check()?;
        let config = self.config_dir()?;
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let aside = self.settings_aside_dir();
        let done = lists::use_settings(
            &self.lists_dir(),
            name,
            &config,
            &aside,
            super::settings::installed_folders(library),
        )?;
        if done != lists::Applied::default() {
            self.publish(Changes::MOD_SETTINGS);
        }
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

    /// Newest first, each order against the load order as it is now.
    pub fn past_orders(&self) -> Vec<PastOrder> {
        let current = self.session().current().to_vec();
        orders::read(&self.orders_file())
            .into_iter()
            .rev()
            .map(|entry| {
                let diff = lists::compare(&entry.ids, &current);
                PastOrder {
                    time: entry.time,
                    said: how_said(entry.how),
                    mods: entry.ids.len(),
                    added: diff.only_left.len(),
                    removed: diff.only_right.len(),
                    moved: diff.moved.len(),
                }
            })
            .collect()
    }

    /// `at` is the place in `past_orders` (newest first, 0 up).
    pub fn past_order(&self, at: usize) -> Option<Vec<String>> {
        self.past_entry(at).map(|entry| entry.ids)
    }

    fn past_entry(&self, at: usize) -> Option<orders::Entry> {
        orders::read(&self.orders_file()).into_iter().rev().nth(at)
    }

    pub fn original_list(&self) -> Option<Vec<String>> {
        let config = self.paths().mods_config()?;
        let original = config.with_file_name("ModsConfig.xml.rimstack-original");
        modsconfig::read(&original).ok().map(|config| config.active)
    }

    /// An order to compare by its key, with its title: `current` for the load order being
    /// edited, `game` for ModsConfig.xml as it is on disk, `list:<name>` for a saved list,
    /// `order:<at>` for the place in `past_orders`. No ids for one that is not there.
    fn order_of(&self, key: &str) -> (Sentence, Option<Vec<String>>) {
        match key.split_once(':') {
            Some(("list", name)) => (Sentence::from(name), self.list_named(name)),
            Some(("order", at)) => match at.parse().ok().and_then(|at| self.past_entry(at)) {
                Some(entry) => (order_title(entry.time), Some(entry.ids)),
                None => (Sentence::from(key), None),
            },
            _ if key == CURRENT => (current_title(), Some(self.session().current().to_vec())),
            _ if key == GAME => {
                let config = self.paths().mods_config();
                let read = config.and_then(|config| modsconfig::read(&config).ok());
                (game_title(), read.map(|config| config.active))
            }
            _ => (Sentence::from(key), None),
        }
    }

    /// Every order `compare_orders` takes, under a caption per kind, for a menu.
    pub fn order_choices(&self) -> Vec<OrderChoice> {
        let pick = |key: String, text: Sentence| OrderChoice {
            key,
            text,
            title: Sentence::default(),
            pick: true,
        };
        let caption = |title: Sentence| OrderChoice {
            key: String::new(),
            text: Sentence::default(),
            title,
            pick: false,
        };
        let mut choices = vec![
            pick(CURRENT.to_string(), current_title()),
            pick(GAME.to_string(), game_title()),
        ];
        let saved = self.saved_lists();
        if !saved.is_empty() {
            choices.push(caption(say!("Saved lists")));
        }
        for list in saved {
            choices.push(pick(
                format!("list:{}", list.name),
                Sentence::from(list.name),
            ));
        }
        let past = orders::read(&self.orders_file());
        if !past.is_empty() {
            choices.push(caption(say!("Load order history")));
        }
        for (at, entry) in past.into_iter().rev().enumerate() {
            let text = Sentence::listed(vec![order_title(entry.time), how_said(entry.how)]);
            choices.push(pick(format!("order:{at}"), text));
        }
        choices
    }

    /// Two orders, each named by its key as `order_of` takes it: no groups when either is
    /// not there, or before the library is read.
    pub fn compare_orders(&self, left: &str, right: &str) -> Comparison {
        let (left_title, left_ids) = self.order_of(left);
        let (right_title, right_ids) = self.order_of(right);
        let nothing = |left, right| Comparison {
            left,
            right,
            ..Comparison::default()
        };
        let (Some(left_ids), Some(right_ids)) = (left_ids, right_ids) else {
            return nothing(left_title, right_title);
        };
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return nothing(left_title, right_title);
        };
        let diff = lists::compare(&left_ids, &right_ids);
        let arranged_left = library.arrange(&left_ids);
        let arranged_right = library.arrange(&right_ids);
        let shown = |m: &rimstack_core::mods::list::Mod, by: isize, said: Sentence| ComparedMod {
            id: m.id.clone(),
            name: m.name.clone(),
            installed: m.installed.is_some(),
            by,
            said,
        };
        let alone = |m: &rimstack_core::mods::list::Mod| {
            let said = match m.installed {
                Some(_) => say!("installed"),
                None => say!("not installed"),
            };
            shown(m, 0, said)
        };
        let only_left: Vec<ComparedMod> = diff
            .only_left
            .iter()
            .map(|&at| alone(&arranged_left.active[at]))
            .collect();
        let only_right: Vec<ComparedMod> = diff
            .only_right
            .iter()
            .map(|&at| alone(&arranged_right.active[at]))
            .collect();
        let moved: Vec<ComparedMod> = diff
            .moved
            .iter()
            .map(|m| shown(&arranged_left.active[m.at], m.by, places(m.by)))
            .collect();
        let addable = match right {
            CURRENT => diff
                .only_left
                .iter()
                .filter_map(|&at| arranged_left.active[at].installed)
                .collect(),
            _ => Vec::new(),
        };
        let mut comparison = Comparison {
            same: only_left.is_empty() && only_right.is_empty() && moved.is_empty(),
            only_left: Group {
                heading: only_in(only_left.len(), &left_title),
                mods: only_left,
            },
            only_right: Group {
                heading: only_in(only_right.len(), &right_title),
                mods: only_right,
            },
            moved: Group {
                heading: say_n!(
                    moved.len(),
                    "{} mod placed elsewhere in {}",
                    "{} mods placed elsewhere in {}",
                    right_title.inside()
                ),
                mods: moved,
            },
            left: left_title,
            right: right_title,
            addable,
            report: String::new(),
        };
        comparison.report = report(&comparison);
        comparison
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
        app.keep_list("Mine", Some(vec!["a.b".into()]), false)
            .unwrap();
        assert!(app.keep_list(" ", None, false).is_err());
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
    fn settings_kept_with_a_list_take_the_current_ones_place() {
        let (tmp, app) = installed();
        let config = tmp.path().join("config");
        let write = |file: &str, contents: &str| {
            std::fs::write(config.join(format!("Config/{file}")), contents).unwrap();
        };
        write("Mod_Core_Settings.xml", "first");
        write("Mod_roads_Settings.xml", "roads");
        let ids = vec!["ludeon.rimworld".to_string(), "ann.roads".to_string()];
        app.keep_list("run", Some(ids), true).unwrap();
        assert!(
            app.saved_lists()
                .iter()
                .any(|l| l.name == "run" && l.settings)
        );

        // The game touched Core's settings: using the list's ones sets them aside first.
        write("Mod_Core_Settings.xml", "changed");
        app.use_list_settings("run").unwrap();
        assert_eq!(
            std::fs::read_to_string(config.join("Config/Mod_Core_Settings.xml")).unwrap(),
            "first"
        );
        let asides: Vec<_> = std::fs::read_dir(app.own_dir())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("mod-settings-"))
            .collect();
        assert_eq!(asides.len(), 1);
        assert_eq!(
            std::fs::read_to_string(asides[0].path().join("Mod_Core_Settings.xml")).unwrap(),
            "changed"
        );

        // Without the settings asked for, no folder; using one there is not refuses.
        app.keep_list("empty", Some(vec![]), true).unwrap();
        assert!(app.use_list_settings("empty").is_err());
        app.delete_list("run");
        assert!(!app.saved_lists().iter().any(|l| l.settings));
    }

    #[test]
    fn comparing_two_orders_groups_each_mod_from_any_side() {
        let (_tmp, app) = installed();
        let saved = vec![
            "ludeon.rimworld".to_string(),
            "ann.roads".to_string(),
            "gone.mod".to_string(),
        ];
        app.keep_list("run", Some(saved), false).unwrap();
        fn ids(group: &Group) -> Vec<(&str, bool, isize, String)> {
            group
                .mods
                .iter()
                .map(|m| (m.id.as_str(), m.installed, m.by, m.said.to_string()))
                .collect()
        }
        let installed = |id| (id, true, 0, "installed".to_string());
        let missing = |id| (id, false, 0, "not installed".to_string());

        // Only rimworld active: the mods the run switched off are left-only.
        app.session().record(vec!["ludeon.rimworld".to_string()]);
        let comp = app.compare_orders("list:run", "current");
        assert_eq!(
            ids(&comp.only_left),
            [installed("ann.roads"), missing("gone.mod")]
        );
        assert_eq!(comp.only_left.heading.to_string(), "2 mods only in run");
        assert!(comp.only_right.mods.is_empty() && comp.moved.mods.is_empty() && !comp.same);
        // A left-only mod that is not installed cannot join the load order.
        assert_eq!(comp.addable.len(), 1);

        // The same mods, but the load order places roads first.
        app.session()
            .record(vec!["ann.roads".to_string(), "ludeon.rimworld".to_string()]);
        let comp = app.compare_orders("list:run", "current");
        assert_eq!(ids(&comp.only_left), [missing("gone.mod")]);
        assert!(comp.only_right.mods.is_empty());
        assert_eq!(
            ids(&comp.moved),
            [("ann.roads", true, -1, "1 place earlier".to_string())]
        );
        assert_eq!(comp.addable.len(), 0);
        assert_eq!(
            comp.report,
            "run compared with the load order\n\n\
             1 mod only in run\n- gone.mod [gone.mod], not installed\n\n\
             1 mod placed elsewhere in the load order\n- Ann.Roads [ann.roads], 1 place earlier"
        );

        // ModsConfig.xml on disk holds rimworld alone; the load order is not on the right,
        // so nothing is offered to join it.
        let comp = app.compare_orders("current", "game");
        assert_eq!(ids(&comp.only_left), [installed("ann.roads")]);
        assert_eq!(
            comp.only_left.heading.to_string(),
            "1 mod only in the load order"
        );
        assert!(comp.only_right.mods.is_empty() && comp.moved.mods.is_empty());
        assert!(comp.addable.is_empty());
        let comp = app.compare_orders("game", "list:run");
        assert_eq!(
            ids(&comp.only_right),
            [installed("ann.roads"), missing("gone.mod")]
        );
        assert_eq!(comp.left.to_string(), "the game's ModsConfig.xml");
        assert!(comp.addable.is_empty());

        let same = app.compare_orders("list:run", "list:run");
        assert!(same.same && same.only_left.mods.is_empty() && same.moved.mods.is_empty());
        assert_eq!(
            same.report,
            "run compared with run\n\nBoth hold the same mods in the same order"
        );
        for (left, right) in [
            ("list:absent", "current"),
            ("current", "order:9"),
            ("x", "game"),
        ] {
            let none = app.compare_orders(left, right);
            let groups = [&none.only_left, &none.only_right, &none.moved];
            assert!(!none.same && groups.iter().all(|g| g.mods.is_empty()));
        }

        // Every kind of order is offered, the past ones under their own caption.
        app.record_order(orders::How::Saved, &["ludeon.rimworld".to_string()]);
        let choices = app.order_choices();
        let keys: Vec<&str> = choices.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["current", "game", "", "list:run", "", "order:0"]);
        assert_eq!(choices[2].title.to_string(), "Saved lists");
        let past = app.compare_orders("order:0", "list:run");
        assert!(past.left.to_string().starts_with("the order of 2"));
        assert_eq!(ids(&past.only_right).len(), 2);
    }
}
