//! A lockfile of the load order: what a RimWorld Multiplayer host runs, for friends to check against.

use crate::base::error::Error;
use crate::base::sentence::Sentence;
use crate::mods::library::{Installed, Resolved, resolve_active};
use crate::rimworld::id;
use crate::rimworld::paths::Source;
use crate::said;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How the file words where a mod comes from; read back as it was written.
pub fn source_word(source: Source, git: bool) -> &'static str {
    match (source, git) {
        (Source::Workshop, _) => "workshop",
        (_, true) => "git",
        (Source::Local, _) => "local",
        (Source::Official, _) => "local",
    }
}

/// What the file pins down for one mod; all but the package id may be missing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    pub package_id: String,
    pub source: String,
    /// The Workshop id for Workshop mods.
    pub workshop: Option<String>,
    /// Seconds since 1970 Steam published this version, as its own record holds it.
    pub updated: Option<u64>,
    /// The mod's own version, as its About.xml tells it; not kept for Workshop mods.
    pub version: Option<String>,
}

impl Mod {
    pub fn of(m: &Installed, updated: Option<u64>) -> Self {
        let about = m.about.as_ref().ok();
        let version = if m.source != Source::Workshop {
            about.and_then(|a| a.mod_version.clone())
        } else {
            None
        };
        Self {
            package_id: about
                .map(|a| a.package_id.to_lowercase())
                .unwrap_or_default(),
            source: source_word(m.source, m.git).to_string(),
            workshop: (m.source == Source::Workshop).then(|| m.folder.clone()),
            updated,
            version,
        }
    }
}

/// Everything the file holds about the game and the load order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Lock {
    /// The short form, "1.6", as the game's own Version.txt tells it.
    pub game: String,
    /// Folded package ids of the official expansions the host runs, Core left out.
    pub dlcs: Vec<String>,
    pub mods: Vec<Mod>,
}

/// The lockfile of what the load order now runs, in order. `updated_by_workshop` is Steam's
/// own record, keyed by Workshop id.
pub fn locked(
    game_version: &str,
    active: &[String],
    installed: &[Installed],
    updated_by_workshop: &HashMap<String, u64>,
) -> Lock {
    let mods = resolve_active(active, installed)
        .into_iter()
        .filter_map(|held| match held {
            Resolved::Found(m) => Some(m),
            Resolved::Missing(_) => None,
        })
        .map(|m| {
            let updated = updated_by_workshop.get(&m.folder).copied();
            Mod::of(m, updated)
        })
        .collect();
    Lock {
        game: game_version.to_string(),
        dlcs: dlcs_installed(active),
        mods,
    }
}

/// The official expansions the load order runs, from its own ids.
fn dlcs_installed(active: &[String]) -> Vec<String> {
    let mut dlcs: Vec<String> = active
        .iter()
        .map(|written| id::fold(&id::split_steam(written).0))
        .filter(|folded| folded.starts_with("ludeon.rimworld."))
        .collect();
    dlcs.sort();
    dlcs.dedup();
    dlcs
}

/// One line of JSON, so the file travels as text.
pub fn write(path: &std::path::Path, lock: &Lock) -> Result<(), Error> {
    let text = serde_json::to_string(lock).map_err(|problem| {
        Error::format(said!("The lockfile could not be written: {}", problem))
    })?;
    crate::base::atomic_file::write(path, text)
}

pub fn read(path: &std::path::Path) -> Result<Lock, Error> {
    let text = std::fs::read_to_string(path).map_err(|problem| Error::file(path, problem))?;
    serde_json::from_str(&text).map_err(|problem| {
        Error::format(said!(
            "{} is not a lockfile RimStack can read: {}",
            path.to_string_lossy().into_owned(),
            problem
        ))
    })
}

/// What checking a lockfile tells. `same` when nothing differs, `present` when every mod of
/// it is installed here, `ids` its order ready to be applied, `said` one plain sentence
/// per kind of difference.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub same: bool,
    pub present: bool,
    pub ids: Vec<String>,
    pub said: Vec<Sentence>,
}

/// The load order now, checked against the file's.
pub fn check(
    lock: &Lock,
    game_version: &str,
    active: &[String],
    installed: &[Installed],
    updated_by_workshop: &HashMap<String, u64>,
) -> Report {
    let now = locked(game_version, active, installed, updated_by_workshop);
    let (now_of, lock_of) = (by_fold(&now), by_fold(lock));

    let mut said: Vec<Sentence> = Vec::new();
    if lock.game != game_version {
        said.push(said!(
            "The lockfile was made for RimWorld {}.",
            lock.game.clone()
        ));
    }
    let missing_dlcs: Vec<String> = lock
        .dlcs
        .iter()
        .filter(|dlc| !now.dlcs.contains(dlc))
        .cloned()
        .collect();
    if !missing_dlcs.is_empty() {
        said.push(said!(
            "The lockfile runs with {} the game here does not have.",
            listed(&missing_dlcs)
        ));
    }
    let extra_dlcs: Vec<String> = now
        .dlcs
        .iter()
        .filter(|dlc| !lock.dlcs.contains(dlc))
        .cloned()
        .collect();
    if !extra_dlcs.is_empty() {
        said.push(said!(
            "The game here runs with {} the lockfile does not have.",
            listed(&extra_dlcs)
        ));
    }

    // The official expansions are told above, with the difference the player can act on.
    let dlc = |package_id: &str| id::fold(package_id).starts_with("ludeon.rimworld.");
    let missing: Vec<String> = lock
        .mods
        .iter()
        .filter(|m| !now_of.contains_key(&id::fold(&m.package_id)))
        .map(|m| m.package_id.clone())
        .filter(|id| !dlc(id))
        .collect();
    if !missing.is_empty() {
        said.push(said!(
            "{} of the lockfile's mods is not installed here.",
            listed(&missing)
        ));
    }
    let extra: Vec<String> = now
        .mods
        .iter()
        .filter(|m| !lock_of.contains_key(&id::fold(&m.package_id)))
        .map(|m| m.package_id.clone())
        .filter(|id| !dlc(id))
        .collect();
    if !extra.is_empty() {
        said.push(said!(
            "{} runs here but is not in the lockfile.",
            listed(&extra)
        ));
    }

    // The shared mods in the order each side runs them; a mod that moved among them is out
    // of order. The missing and extra ones are told above and would only add noise here.
    let shared = |side: &Lock, other: &HashMap<String, &Mod>| -> Vec<String> {
        side.mods
            .iter()
            .filter(|m| other.contains_key(&id::fold(&m.package_id)))
            .map(|m| id::fold(&m.package_id))
            .collect()
    };
    let (lock_order, now_order) = (shared(lock, &now_of), shared(&now, &lock_of));
    let place_in = |order: &[String], folded: &str| order.iter().position(|o| o == folded);
    let out_of_order: Vec<String> = lock_order
        .iter()
        .filter(|folded| place_in(&lock_order, folded) != place_in(&now_order, folded))
        .filter_map(|folded| lock_of[folded].package_id.clone().into())
        .collect();
    if !out_of_order.is_empty() {
        said.push(said!("Out of order: {}.", listed(&out_of_order)));
    }

    for mine in &lock.mods {
        let Some(here) = now_of.get(&id::fold(&mine.package_id)) else {
            continue;
        };
        if mine.source != here.source {
            said.push(said!(
                "{} is a {} mod here, a {} mod in the lockfile.",
                here.package_id.clone(),
                here.source.clone(),
                mine.source.clone()
            ));
        }
        if let (Some(file), Some(have)) = (mine.updated, here.updated)
            && file != have
        {
            if have > file {
                said.push(said!(
                    "{} was updated since the lockfile was made.",
                    here.package_id.clone()
                ));
            } else {
                said.push(said!(
                    "{} is older than the lockfile's.",
                    here.package_id.clone()
                ));
            }
        }
        match (&mine.version, &here.version) {
            (Some(file), Some(have)) if file != have => {
                said.push(said!(
                    "{} is version {} here, {} in the lockfile.",
                    here.package_id.clone(),
                    have.clone(),
                    file.clone()
                ));
            }
            (Some(file), None) => {
                said.push(said!(
                    "{} tells no version here, {} in the lockfile.",
                    here.package_id.clone(),
                    file.clone()
                ));
            }
            _ => {}
        }
    }

    Report {
        same: said.is_empty(),
        present: missing.is_empty(),
        ids: lock.mods.iter().map(|m| m.package_id.clone()).collect(),
        said,
    }
}

fn by_fold(side: &Lock) -> HashMap<String, &Mod> {
    side.mods
        .iter()
        .map(|m| (id::fold(&m.package_id), m))
        .collect()
}

fn listed(names: &[String]) -> Sentence {
    Sentence::listed(names.iter().map(|n| Sentence::from(n.as_str())).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_mod(
        package_id: &str,
        source: &str,
        workshop: Option<&str>,
        updated: Option<u64>,
        version: Option<&str>,
    ) -> Mod {
        Mod {
            package_id: package_id.into(),
            source: source.into(),
            workshop: workshop.map(|w| w.into()),
            updated,
            version: version.map(|v| v.into()),
        }
    }

    /// Reads `now` as if it were the installed mods and the load order: an `Installed`
    /// list is built from the same facts the file keeps.
    fn installed_of(now: &[Mod]) -> Vec<Installed> {
        now.iter()
            .map(|m| {
                let about = crate::rimworld::about::About {
                    package_id: m.package_id.clone(),
                    mod_version: m.version.clone(),
                    ..Default::default()
                };
                let (source, git) = match m.source.as_str() {
                    "workshop" => (Source::Workshop, false),
                    "git" => (Source::Local, true),
                    _ => (Source::Local, false),
                };
                Installed {
                    path: m.package_id.as_str().into(),
                    source,
                    folder: m.workshop.clone().unwrap_or_default(),
                    about: Ok(about),
                    kinds: Default::default(),
                    code: Default::default(),
                    preview: None,
                    git,
                    published: matches!(source, Source::Workshop),
                    downloaded: None,
                }
            })
            .collect()
    }

    fn run(lock: &[Mod], now: &[Mod], game: &str, updated: &[(String, u64)]) -> Report {
        let lock_active: Vec<String> = lock.iter().map(|m| m.package_id.clone()).collect();
        let lock = Lock {
            game: "1.6".into(),
            dlcs: dlcs_installed(&lock_active),
            mods: lock.to_vec(),
        };
        let active: Vec<String> = now.iter().map(|m| m.package_id.clone()).collect();
        let updated_by_workshop = updated.iter().cloned().collect::<HashMap<String, u64>>();
        check(
            &lock,
            game,
            &active,
            &installed_of(now),
            &updated_by_workshop,
        )
    }

    #[test]
    fn a_lockfile_round_trips_through_its_file() {
        let lock = Lock {
            game: "1.6".into(),
            dlcs: vec!["ludeon.rimworld.royalty".into()],
            mods: vec![
                a_mod("ann.roads", "git", None, None, Some("1.2")),
                a_mod(
                    "harmony",
                    "workshop",
                    Some("2009463077"),
                    Some(1711286985),
                    None,
                ),
            ],
        };
        let path = std::env::temp_dir().join("rimstack-lock-round-trip.json");
        write(&path, &lock).unwrap();
        assert_eq!(read(&path).unwrap(), lock);
    }

    #[test]
    fn a_file_that_is_no_lockfile_is_refused() {
        let path = std::env::temp_dir().join("rimstack-lock-broken.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(read(&path).is_err());
    }

    #[test]
    fn a_matching_load_order_says_nothing_and_can_be_applied() {
        let now = vec![
            a_mod("ludeon.rimworld", "local", None, None, None),
            a_mod(
                "harmony",
                "workshop",
                Some("2009463077"),
                Some(1711286985),
                None,
            ),
            a_mod("ann.roads", "git", None, None, Some("1.2")),
        ];
        let report = run(&now, &now, "1.6", &[("2009463077".into(), 1711286985)]);
        assert!(report.same, "{:?}", report.said);
        assert!(report.present);
        assert_eq!(report.ids, ["ludeon.rimworld", "harmony", "ann.roads"]);
    }

    #[test]
    fn every_mismatch_kind_is_told_in_its_own_sentence() {
        let lock = vec![
            a_mod("ludeon.rimworld", "local", None, None, None),
            a_mod("ludeon.rimworld.royalty", "local", None, None, None),
            a_mod(
                "harmony",
                "workshop",
                Some("2009463077"),
                Some(1711286985),
                None,
            ),
            a_mod("ann.roads", "git", None, None, Some("1.2")),
            a_mod("gone.mod", "local", None, None, Some("3")),
        ];
        let now = vec![
            a_mod("ludeon.rimworld", "local", None, None, None),
            a_mod("ann.roads", "git", None, None, Some("1.3")),
            a_mod(
                "harmony",
                "workshop",
                Some("2009463077"),
                Some(1711287000),
                None,
            ),
            a_mod("extra.mod", "workshop", Some("42"), None, None),
        ];
        let report = run(&lock, &now, "1.6", &[("2009463077".into(), 1711287000)]);
        let said: Vec<String> = report.said.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            said,
            [
                "The lockfile runs with ludeon.rimworld.royalty the game here does not have.",
                "gone.mod of the lockfile's mods is not installed here.",
                "extra.mod runs here but is not in the lockfile.",
                "Out of order: harmony, ann.roads.",
                "harmony was updated since the lockfile was made.",
                "ann.roads is version 1.3 here, 1.2 in the lockfile.",
            ]
        );
        assert!(!report.same);
        assert!(!report.present);
    }

    #[test]
    fn another_game_version_a_workshop_mod_gone_local_and_an_older_one_are_told() {
        let lock = vec![
            a_mod(
                "harmony",
                "workshop",
                Some("2009463077"),
                Some(1711286985),
                None,
            ),
            a_mod("ann.lib", "workshop", Some("5"), Some(2000), None),
        ];
        let now = vec![
            a_mod("harmony", "local", None, None, None),
            a_mod("ann.lib", "workshop", Some("5"), Some(1000), None),
        ];
        let report = run(
            &lock,
            &now,
            "1.5",
            &[("5".to_string(), 1000), ("2009463077".into(), 1711286985)],
        );
        let said: Vec<String> = report.said.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            said,
            [
                "The lockfile was made for RimWorld 1.6.",
                "harmony is a local mod here, a workshop mod in the lockfile.",
                "ann.lib is older than the lockfile's.",
            ]
        );
    }

    #[test]
    fn a_mod_whose_version_is_gone_is_told() {
        let lock = vec![a_mod("ann.roads", "git", None, None, Some("1.2"))];
        let now = vec![a_mod("ann.roads", "git", None, None, None)];
        let report = run(&lock, &now, "1.6", &[]);
        assert_eq!(
            report.said[0].to_string(),
            "ann.roads tells no version here, 1.2 in the lockfile."
        );
    }
}
