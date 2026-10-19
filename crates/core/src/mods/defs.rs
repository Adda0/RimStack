//! Two active mods defining the same def: the game keeps the one that loads later.

use crate::base::sentence::Sentence;
use crate::mods::library::ModId;
use crate::say;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Defs named beside a count.
const SHOWN: usize = 3;

/// A def the game loads: the XML element it is written as, e.g. `ThingDef`, and its `defName`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Def {
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    pub earlier: usize,
    pub later: usize,
    /// Sorted.
    pub defs: Vec<Def>,
}

/// By place in `defines`, `earlier` the lower: given in load order, `later`'s defs are kept.
pub fn collisions(defines: &[&HashSet<Def>]) -> Vec<Collision> {
    let mut owners: HashMap<&Def, Vec<usize>> = HashMap::new();
    for (m, defs) in defines.iter().enumerate() {
        for def in defs.iter() {
            owners.entry(def).or_default().push(m);
        }
    }
    let mut shared: HashMap<(usize, usize), Vec<Def>> = HashMap::new();
    for (def, mods) in owners.into_iter().filter(|(_, mods)| mods.len() > 1) {
        for (i, &earlier) in mods.iter().enumerate() {
            for &later in &mods[i + 1..] {
                shared
                    .entry((earlier, later))
                    .or_default()
                    .push(def.clone());
            }
        }
    }
    let mut found: Vec<Collision> = shared
        .into_iter()
        .map(|((earlier, later), mut defs)| {
            defs.sort_unstable();
            Collision {
                earlier,
                later,
                defs,
            }
        })
        .collect();
    found.sort_unstable_by_key(|c| (c.earlier, c.later));
    found
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefClash {
    pub mod_id: ModId,
    pub name: String,
    pub defs: usize,
    /// The first few shared defs, by name.
    pub some: Sentence,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefClashes {
    /// Loading later: the game keeps their defs.
    pub replaced_by: Vec<DefClash>,
    /// Loading earlier: the game keeps this mod's defs.
    pub replaces: Vec<DefClash>,
}

fn some(defs: &[Def]) -> Sentence {
    let named: Vec<String> = (defs.iter().take(SHOWN))
        .map(|d| format!("{} ({})", d.name, d.kind))
        .collect();
    let named = named.join(", ");
    match defs.len().saturating_sub(SHOWN) {
        0 => Sentence::Plain(named),
        more => say!("{} and {} more", named, more),
    }
}

/// `found` by place in `members` (ascending); who wins goes by the place `listed` gives,
/// in the load order. Nothing for a mod no longer in the list.
pub fn of_mod(
    members: &[ModId],
    found: &[Collision],
    mod_id: ModId,
    listed: &dyn Fn(ModId) -> Option<(usize, String)>,
) -> DefClashes {
    let mut said = DefClashes::default();
    let (Ok(place), Some((mine, _))) = (members.binary_search(&mod_id), listed(mod_id)) else {
        return said;
    };
    let mut placed: Vec<(usize, DefClash)> = Vec::new();
    for c in found {
        let other = match (c.earlier == place, c.later == place) {
            (true, _) => c.later,
            (_, true) => c.earlier,
            _ => continue,
        };
        let other_id = members[other];
        let Some((theirs, name)) = listed(other_id) else {
            continue;
        };
        let clash = DefClash {
            mod_id: other_id,
            name,
            defs: c.defs.len(),
            some: some(&c.defs),
        };
        placed.push((theirs, clash));
    }
    placed.sort_unstable_by_key(|(at, _)| *at);
    for (theirs, clash) in placed {
        match theirs > mine {
            true => said.replaced_by.push(clash),
            false => said.replaces.push(clash),
        }
    }
    said
}

/// The places in `defines` sharing a def with another.
pub fn in_collision(found: &[Collision]) -> HashSet<usize> {
    found.iter().flat_map(|c| [c.earlier, c.later]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs(list: &[(&str, &str)]) -> HashSet<Def> {
        (list.iter())
            .map(|(kind, name)| Def {
                kind: kind.to_string(),
                name: name.to_string(),
            })
            .collect()
    }

    #[test]
    fn mods_defining_the_same_def_collide_on_those_only() {
        let mods = [
            defs(&[
                ("ThingDef", "Steel"),
                ("ThingDef", "Gold"),
                ("ThingDef", "Oak"),
            ]),
            defs(&[("ThingDef", "Pine")]),
            defs(&[
                ("ThingDef", "Gold"),
                ("ThingDef", "Steel"),
                ("ThingDef", "Elm"),
            ]),
        ];
        let found = collisions(&mods.iter().collect::<Vec<_>>());
        let names =
            |c: &Collision| -> Vec<String> { c.defs.iter().map(|d| d.name.clone()).collect() };
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].earlier, found[0].later), (0, 2));
        assert_eq!(names(&found[0]), ["Gold", "Steel"]);
    }

    #[test]
    fn the_same_name_for_another_type_of_def_is_no_collision() {
        let mods = [
            defs(&[("ThingDef", "Granite")]),
            defs(&[("TerrainDef", "Granite")]),
        ];
        assert!(collisions(&mods.iter().collect::<Vec<_>>()).is_empty());
    }

    #[test]
    fn the_mod_loading_later_replaces_the_defs_of_the_one_before() {
        let members = [ModId::new(1, 0), ModId::new(1, 1)];
        let mods = [
            defs(&[("ThingDef", "Steel")]),
            defs(&[("ThingDef", "Steel")]),
        ];
        let found = collisions(&mods.iter().collect::<Vec<_>>());
        let in_order = |order: [ModId; 2]| {
            move |id: ModId| {
                let at = order.iter().position(|o| *o == id)?;
                Some((at, format!("mod {}", id.to_qml())))
            }
        };
        let first = of_mod(&members, &found, members[0], &in_order(members));
        assert_eq!(first.replaced_by.len(), 1);
        assert_eq!(first.replaced_by[0].mod_id, members[1]);
        assert!(first.replaces.is_empty());
        let swapped = [members[1], members[0]];
        let first = of_mod(&members, &found, members[0], &in_order(swapped));
        assert!(first.replaced_by.is_empty());
        assert_eq!(first.replaces[0].mod_id, members[1]);
        assert_eq!(first.replaces[0].some.to_string(), "Steel (ThingDef)");
    }

    #[test]
    fn a_few_defs_are_named_and_the_rest_counted() {
        let many: Vec<Def> = ["A", "B", "C", "D", "E"]
            .map(|name| Def {
                kind: "ThingDef".into(),
                name: name.into(),
            })
            .into();
        assert_eq!(
            some(&many).to_string(),
            "A (ThingDef), B (ThingDef), C (ThingDef) and 2 more"
        );
    }
}
