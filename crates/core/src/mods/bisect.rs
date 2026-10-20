//! Assumes one mod is at fault; two that only break together are not found.

use crate::base::error::Error;
use crate::mods::library::Library;
use crate::mods::list::Mod;
use crate::rimworld::paths::Source;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bisect {
    pub original: Vec<String>,
    pub suspects: Vec<usize>,
    /// `original` places of the mods added since the launch without errors, tried first.
    #[serde(default)]
    seeded: Vec<usize>,
    requires: Vec<Vec<usize>>,
}

impl Bisect {
    /// `first` marks the mods to be tried first: they end the suspects, and `off` takes the
    /// last half of them, so they are switched off in the first round.
    pub fn new(
        original: Vec<String>,
        fixed: &[bool],
        first: &[bool],
        requires: Vec<Vec<usize>>,
    ) -> Self {
        let suspect = |at: &usize| !fixed.get(*at).copied().unwrap_or(false);
        let seeded = |at: &usize| first.get(*at).copied().unwrap_or(false);
        let (rest, seed): (Vec<usize>, Vec<usize>) = (0..original.len())
            .filter(suspect)
            .partition(|at| !seeded(at));
        Self {
            suspects: [rest, seed].concat(),
            seeded: (0..original.len()).filter(suspect).filter(seeded).collect(),
            original,
            requires,
        }
    }

    /// The mods added since the launch without errors still suspected.
    pub fn seeded(&self) -> Vec<&str> {
        self.seeded
            .iter()
            .filter(|at| self.suspects.contains(at))
            .map(|&at| self.original[at].as_str())
            .collect()
    }

    fn with_dependents(&self, off: Vec<bool>) -> Vec<bool> {
        crate::mods::library::with_needing(&self.requires, off)
    }

    /// All `false` when the suspects need each other and cannot be told apart.
    pub fn off(&self) -> Vec<bool> {
        let none = vec![false; self.original.len()];
        let suspected = |off: &[bool]| self.suspects.iter().filter(|at| off[**at]).count();
        let half = self.suspects.len() / 2;
        let mut off = none.clone();
        let mut smallest: Option<Vec<bool>> = None;
        for at in self.suspects.iter().rev() {
            let mut with = off.clone();
            with[*at] = true;
            let with = self.with_dependents(with);
            let count = suspected(&with);
            if count <= half {
                off = with;
            } else if count < self.suspects.len()
                && smallest.as_ref().is_none_or(|s| count < suspected(s))
            {
                smallest = Some(with);
            }
            if suspected(&off) == half {
                break;
            }
        }
        match suspected(&off) {
            0 => smallest.unwrap_or(none),
            _ => off,
        }
    }

    pub fn is_stuck(&self) -> bool {
        self.suspects.len() > 1 && self.off().iter().all(|off| !off)
    }

    pub fn list(&self) -> Vec<String> {
        let off = self.off();
        let kept = self.original.iter().zip(off).filter(|(_, off)| !off);
        kept.map(|(id, _)| id.clone()).collect()
    }

    /// The mod at fault was on if it broke, and off if it did not.
    pub fn answer(&mut self, broken: bool) {
        let off = self.off();
        self.suspects.retain(|at| off[*at] != broken);
    }

    pub fn found(&self) -> Option<&str> {
        match self.suspects.as_slice() {
            [one] => Some(self.original[*one].as_str()),
            _ => None,
        }
    }

    pub fn rounds_left(&self) -> u32 {
        self.suspects
            .len()
            .max(1)
            .next_power_of_two()
            .trailing_zeros()
    }

    /// A file that cannot be read is no search under way.
    pub fn read(file: &Path) -> Option<Self> {
        serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
    }

    pub fn write(&self, file: &Path) -> Result<(), Error> {
        let text = serde_json::to_string(self).map_err(|e| Error::format(e.to_string()))?;
        crate::base::atomic_file::write(file, text)
    }
}

pub fn start(library: &Library, mods: &[Mod], recent: &HashSet<String>) -> Bisect {
    let installed = |m: &Mod| library.get(m.installed?);
    let fixed: Vec<bool> = mods
        .iter()
        .map(|m| installed(m).is_some_and(|i| i.source == Source::Official))
        .collect();
    let first: Vec<bool> = mods
        .iter()
        .map(|m| recent.contains(&crate::rimworld::id::split_steam(&m.id).0))
        .collect();
    Bisect::new(
        mods.iter().map(|m| m.id.clone()).collect(),
        &fixed,
        &first,
        library.requires_within(mods),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_knows_what_each_listed_mod_requires() {
        use crate::mods::library::{build, scan};
        use crate::rimworld::paths::ModRoot;
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Mods");
        let abouts = [
            (
                "lib",
                "<ModMetaData><packageId>a.lib</packageId><name>Lib</name></ModMetaData>",
            ),
            (
                "user",
                "<ModMetaData><packageId>a.user</packageId><name>User</name><modDependencies><li><packageId>A.Lib</packageId></li><li><packageId>not.listed</packageId></li></modDependencies></ModMetaData>",
            ),
        ];
        for (folder, about) in abouts {
            std::fs::create_dir_all(root.join(folder).join("About")).unwrap();
            std::fs::write(root.join(folder).join("About/About.xml"), about).unwrap();
        }
        let roots = [ModRoot {
            path: root,
            source: Source::Local,
        }];
        let active = ["a.user".to_string(), "a.lib".to_string()];
        let library = build(scan(&roots), &active, "1.6");
        let search = start(&library, &library.active, &HashSet::new());
        assert_eq!(search.original, active);
        assert_eq!(search.suspects, [0, 1]);
        assert_eq!(search.requires, [vec![1], vec![]]);
        assert_eq!(search.list(), ["a.lib"]);
    }

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("mod.{i}")).collect()
    }

    fn hunt(mut search: Bisect, culprit: &str) -> (String, u32) {
        let mut rounds = 0;
        while search.found().is_none() {
            let broken = search.list().iter().any(|id| id == culprit);
            search.answer(broken);
            rounds += 1;
            assert!(rounds < 64, "the search does not end");
        }
        (search.found().unwrap().to_string(), rounds)
    }

    #[test]
    fn every_mod_is_found_in_as_many_rounds_as_promised() {
        for n in [1, 2, 3, 8, 13, 100] {
            for culprit in 0..n {
                let search = Bisect::new(ids(n), &[], &[], vec![Vec::new(); n]);
                let promised = search.rounds_left();
                let (found, rounds) = hunt(search, &format!("mod.{culprit}"));
                assert_eq!(found, format!("mod.{culprit}"));
                assert!(
                    rounds <= promised,
                    "{n} mods: {rounds} rounds, {promised} promised"
                );
            }
        }
    }

    #[test]
    fn the_game_and_its_expansions_are_never_suspected_or_switched_off() {
        let fixed = [true, true, false, false, false];
        let search = Bisect::new(ids(5), &fixed, &[], vec![Vec::new(); 5]);
        assert_eq!(search.suspects, [2, 3, 4]);
        assert_eq!(search.list(), ["mod.0", "mod.1", "mod.2", "mod.3"]);
        let (found, _) = hunt(search, "mod.4");
        assert_eq!(found, "mod.4");
    }

    #[test]
    fn a_mod_goes_off_with_the_mod_it_needs() {
        let requires = vec![vec![1], vec![3], vec![], vec![]];
        let search = Bisect::new(ids(4), &[], &[], requires.clone());
        assert_eq!(search.off(), [true, false, true, false]);
        assert_eq!(search.list(), ["mod.1", "mod.3"]);
        for culprit in 0..4 {
            let (found, _) = hunt(
                Bisect::new(ids(4), &[], &[], requires.clone()),
                &format!("mod.{culprit}"),
            );
            assert_eq!(found, format!("mod.{culprit}"));
        }
    }

    #[test]
    fn a_mod_everything_needs_is_still_told_from_the_rest() {
        let requires = vec![vec![], vec![0], vec![0], vec![0]];
        for culprit in 0..4 {
            let (found, _) = hunt(
                Bisect::new(ids(4), &[], &[], requires.clone()),
                &format!("mod.{culprit}"),
            );
            assert_eq!(found, format!("mod.{culprit}"));
        }
    }

    #[test]
    fn the_mods_added_since_the_last_launch_without_errors_go_off_first() {
        let first = [true, false, false, true];
        let search = Bisect::new(ids(4), &[], &first, vec![vec![]; 4]);
        // The first round takes off the two seeded mods, the seasoned ones stay.
        assert_eq!(search.off(), [true, false, false, true]);
        assert_eq!(search.list(), ["mod.1", "mod.2"], "{:?}", search.suspects);
        assert_eq!(search.seeded(), ["mod.0", "mod.3"]);
        let six = [true, false, false, false, false, true];
        let (found, rounds) = hunt(Bisect::new(ids(6), &[], &six, vec![vec![]; 6]), "mod.1");
        assert_eq!(found, "mod.1");
        assert!(rounds <= 3, "{rounds} rounds");
    }

    #[test]
    fn mods_that_need_each_other_cannot_be_told_apart() {
        let search = Bisect::new(ids(2), &[], &[], vec![vec![1], vec![0]]);
        assert!(search.is_stuck());
        assert_eq!(search.found(), None);
        assert_eq!(search.list(), ids(2));
        assert!(!Bisect::new(ids(2), &[], &[], vec![vec![], vec![]]).is_stuck());
    }

    #[test]
    fn a_search_survives_being_written_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kept/bisect.json");
        assert_eq!(Bisect::read(&file), None);
        let mut search = Bisect::new(ids(6), &[true], &[true], vec![vec![]; 6]);
        search.answer(false);
        search.write(&file).unwrap();
        assert_eq!(Bisect::read(&file), Some(search));
    }
}
