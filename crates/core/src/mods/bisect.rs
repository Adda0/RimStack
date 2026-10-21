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
        let suspected: HashSet<usize> = self.suspects.iter().copied().collect();
        self.seeded
            .iter()
            .filter(|at| suspected.contains(at))
            .map(|&at| self.original[at].as_str())
            .collect()
    }

    /// All `false` when the suspects need each other and cannot be told apart.
    pub fn off(&self) -> Vec<bool> {
        let count = self.original.len();
        let needing = crate::mods::library::needing(&self.requires, count);
        let mut suspect = vec![false; count];
        self.suspects.iter().for_each(|at| suspect[*at] = true);
        // What taking `at` off takes with it beyond `off`: itself and every mod needing it.
        let mut seen = vec![0usize; count];
        let mut round = 0;
        let mut with = |at: usize, off: &[bool]| -> Vec<usize> {
            round += 1;
            let mut taken = Vec::new();
            let mut todo = vec![at];
            while let Some(at) = todo.pop() {
                if off[at] || seen[at] == round {
                    continue;
                }
                seen[at] = round;
                taken.push(at);
                todo.extend(&needing[at]);
            }
            taken
        };
        let half = self.suspects.len() / 2;
        let mut off = vec![false; count];
        let mut suspected = 0;
        let mut smallest: Option<(usize, usize)> = None;
        for &at in self.suspects.iter().rev() {
            let taken = with(at, &off);
            let more = suspected + taken.iter().filter(|t| suspect[**t]).count();
            if more <= half {
                taken.into_iter().for_each(|t| off[t] = true);
                suspected = more;
            } else if more < self.suspects.len() && smallest.is_none_or(|(least, _)| more < least) {
                smallest = Some((more, at));
            }
            if suspected == half {
                break;
            }
        }
        // Nothing went off, so the smallest is taken from all on.
        if suspected == 0
            && let Some((_, at)) = smallest
        {
            with(at, &off).into_iter().for_each(|t| off[t] = true);
        }
        off
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

    /// A file that cannot be read, or whose places point past its mods, is no search under way.
    pub fn read(file: &Path) -> Option<Self> {
        let search: Self = serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()?;
        let count = search.original.len();
        let inside = |at: &usize| *at < count;
        let once: HashSet<&usize> = search.suspects.iter().collect();
        let sound = once.len() == search.suspects.len()
            && search.suspects.iter().all(inside)
            && search.seeded.iter().all(inside)
            && search.requires.len() == count
            && search.requires.iter().flatten().all(inside);
        sound.then_some(search)
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

    #[test]
    fn a_search_whose_places_point_past_its_mods_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("bisect.json");
        let read = |text: &str| {
            std::fs::write(&file, text).unwrap();
            Bisect::read(&file)
        };
        let good = r#"{"original":["a","b"],"suspects":[0,1],"seeded":[1],"requires":[[1],[]]}"#;
        assert!(read(good).is_some());
        for bad in [
            r#"{"original":["a","b"],"suspects":[0,2],"seeded":[],"requires":[[],[]]}"#,
            r#"{"original":["a","b"],"suspects":[0,0],"seeded":[],"requires":[[],[]]}"#,
            r#"{"original":["a","b"],"suspects":[0],"seeded":[4],"requires":[[],[]]}"#,
            r#"{"original":["a","b"],"suspects":[0],"seeded":[],"requires":[[]]}"#,
            r#"{"original":["a","b"],"suspects":[0],"seeded":[],"requires":[[7],[]]}"#,
        ] {
            assert_eq!(read(bad), None, "{bad}");
        }
    }
}
