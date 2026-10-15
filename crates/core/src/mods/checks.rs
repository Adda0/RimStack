use crate::mods::rules;
use crate::rimworld::about::About;
use crate::rimworld::id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    Requires { id: String, name: Option<String> },
    Incompatible { with: usize },
    GameVersion { supports: Vec<String> },
}

impl Problem {
    /// Many mods run fine on a version they do not list, so that is only a warning.
    pub fn is_error(&self) -> bool {
        !matches!(self, Problem::GameVersion { .. })
    }
}

pub fn check(
    mods: &[Option<&About>],
    game_version: &str,
    installed_name: impl Fn(&str) -> Option<String>,
) -> Vec<Vec<Problem>> {
    let position = rules::positions(mods);
    let mut problems: Vec<Vec<Problem>> = vec![Vec::new(); mods.len()];
    for (me, about) in mods.iter().enumerate() {
        let Some(about) = about else { continue };
        for dep in about.dependencies.for_version(game_version) {
            let key = id::split_steam(dep).0;
            let met = about
                .accepted_as(dep)
                .any(|accepted| position.contains_key(&id::split_steam(accepted).0));
            if !met {
                problems[me].push(Problem::Requires {
                    id: dep.clone(),
                    name: installed_name(&key),
                });
            }
        }
        for other in about.incompatible_with.for_version(game_version) {
            let Some(&with) = position.get(&id::split_steam(other).0) else {
                continue;
            };
            for (a, b) in [(me, with), (with, me)] {
                let problem = Problem::Incompatible { with: b };
                if a != b && !problems[a].contains(&problem) {
                    problems[a].push(problem);
                }
            }
        }
        let supports = &about.supported_versions;
        let listed = supports.iter().any(|v| v.trim() == game_version);
        if !game_version.is_empty() && !supports.is_empty() && !listed {
            problems[me].push(Problem::GameVersion {
                supports: supports.clone(),
            });
        }
    }
    problems
}

pub fn version_warnings_shown(off: bool, aside_for: Option<&str>, game_version: &str) -> bool {
    !off && aside_for.is_none_or(|version| version.trim() != game_version)
}

/// 0 for an item that is not marked.
pub fn place_among(marked: &[bool], at: usize) -> (usize, usize) {
    let total = marked.iter().filter(|m| **m).count();
    let place = match marked.get(at) {
        Some(true) => marked[..=at].iter().filter(|m| **m).count(),
        _ => 0,
    };
    (place, total)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_marked_item_knows_its_place_among_the_marked() {
        let marked = [false, true, true, false, true];
        assert_eq!(super::place_among(&marked, 2), (2, 3));
        assert_eq!(super::place_among(&marked, 4), (3, 3));
        assert_eq!(super::place_among(&marked, 0), (0, 3));
        assert_eq!(super::place_among(&marked, 9), (0, 3));
    }

    use super::*;
    use crate::rimworld::about::Versioned;

    fn about(id: &str) -> About {
        About {
            package_id: id.to_string(),
            ..About::default()
        }
    }

    fn list(items: &[&str]) -> Versioned {
        Versioned {
            base: items.iter().map(|s| s.to_string()).collect(),
            ..Versioned::default()
        }
    }

    fn check_all(abouts: &[About]) -> Vec<Vec<Problem>> {
        let refs: Vec<Option<&About>> = abouts.iter().map(Some).collect();
        check(&refs, "1.6", |key| {
            (key == "on.disk").then(|| "On Disk".to_string())
        })
    }

    #[test]
    fn a_list_with_everything_it_needs_has_no_problems() {
        let mut a = about("a.a");
        a.dependencies = list(&["B.B_steam"]);
        a.supported_versions = vec!["1.5".into(), "1.6".into()];
        let found = check_all(&[a, about("b.b")]);
        assert!(found.iter().all(Vec::is_empty));
    }

    #[test]
    fn a_requirement_that_is_not_active_is_reported_with_its_name_if_installed() {
        let mut a = about("a.a");
        a.dependencies = list(&["On.Disk", "nowhere.mod"]);
        let found = check_all(&[a]);
        assert_eq!(
            found[0],
            vec![
                Problem::Requires {
                    id: "On.Disk".into(),
                    name: Some("On Disk".into())
                },
                Problem::Requires {
                    id: "nowhere.mod".into(),
                    name: None
                },
            ]
        );
        assert!(found[0][0].is_error());
    }

    #[test]
    fn an_active_alternative_meets_a_requirement() {
        let mut a = about("a.a");
        a.dependencies = list(&["main.mod"]);
        a.alternatives
            .insert("main.mod".into(), vec!["Main.Mod.Dev".into()]);
        let found = check_all(&[a, about("main.mod.dev")]);
        assert!(found[0].is_empty());
    }

    #[test]
    fn an_incompatible_pair_shows_on_both_mods_once() {
        let mut a = about("a.a");
        a.incompatible_with = list(&["b.b", "absent.mod"]);
        let mut b = about("b.b");
        b.incompatible_with = list(&["a.a"]);
        let found = check_all(&[a, b]);
        assert_eq!(found[0], vec![Problem::Incompatible { with: 1 }]);
        assert_eq!(found[1], vec![Problem::Incompatible { with: 0 }]);
    }

    #[test]
    fn a_mod_that_does_not_list_the_game_version_gets_a_warning() {
        let mut a = about("a.a");
        a.supported_versions = vec!["1.4".into(), "1.5".into()];
        let found = check_all(&[a, about("no.versions")]);
        assert_eq!(
            found[0],
            vec![Problem::GameVersion {
                supports: vec!["1.4".into(), "1.5".into()]
            }]
        );
        assert!(!found[0][0].is_error());
        assert!(found[1].is_empty());
    }

    #[test]
    fn a_mod_that_cannot_be_read_has_no_problems_of_these_kinds() {
        let found = check(&[None], "1.6", |_| None);
        assert_eq!(found, vec![Vec::new()]);
    }

    #[test]
    fn version_warnings_set_aside_for_one_version_come_back_on_the_next() {
        assert!(version_warnings_shown(false, None, "1.7"));
        assert!(!version_warnings_shown(true, None, "1.7"));
        assert!(!version_warnings_shown(false, Some("1.7"), "1.7"));
        assert!(version_warnings_shown(false, Some("1.7"), "1.8"));
        assert!(!version_warnings_shown(true, Some("1.6"), "1.7"));
    }
}
