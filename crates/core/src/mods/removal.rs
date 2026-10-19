//! Whether a mod can leave a colony mid-save, judged from what it adds: a likelihood, never a
//! promise, since nothing is run.

use crate::base::sentence::Sentence;
use crate::mods::defs::Def;
use crate::mods::list::Category;
use crate::rimworld::code::Facts;
use crate::say;
use std::collections::{BTreeSet, HashSet};

/// Def types a save refers to by name: things on maps, pawns and what they carry, the world.
/// Their mod gone, the save points at defs that no longer exist.
const KEPT_IN_SAVES: [&str; 24] = [
    "AbilityDef",
    "BackstoryDef",
    "BeardDef",
    "FactionDef",
    "GameConditionDef",
    "GeneDef",
    "HairDef",
    "HediffDef",
    "JobDef",
    "MemeDef",
    "PawnKindDef",
    "PreceptDef",
    "QuestScriptDef",
    "RecipeDef",
    "ResearchProjectDef",
    "RoyalTitleDef",
    "SitePartDef",
    "TattooDef",
    "TerrainDef",
    "ThingDef",
    "ThoughtDef",
    "TraitDef",
    "WorldObjectDef",
    "XenotypeDef",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removal {
    /// Likely safe; the category says why.
    Safe(Option<Category>),
    /// It defines defs of these types, which a save keeps; sorted.
    Defs(Vec<String>),
    /// Its code adds parts of the game, such as comps or hediffs, that a save can keep.
    Code,
}

pub fn removal(category: Option<Category>, defines: &HashSet<Def>, facts: &Facts) -> Removal {
    let kept: BTreeSet<&str> = (defines.iter())
        .map(|d| d.kind.as_str())
        .filter(|kind| KEPT_IN_SAVES.contains(kind))
        .collect();
    if !kept.is_empty() {
        return Removal::Defs(kept.into_iter().map(String::from).collect());
    }
    // Classified by what most of its code does: a few added types there are its own windows or
    // faster methods, not parts of the world.
    let looks_or_runs = matches!(
        category,
        Some(Category::Textures | Category::Interface | Category::Tweaks)
    );
    if facts.adding > 0 && !looks_or_runs {
        return Removal::Code;
    }
    Removal::Safe(category)
}

impl Removal {
    pub fn harms(&self) -> bool {
        !matches!(self, Removal::Safe(_))
    }

    pub fn said(&self) -> Sentence {
        match self {
            Removal::Safe(Some(Category::Textures)) => {
                say!(
                    "Likely safe to remove mid-save: it changes how things look, not what they are."
                )
            }
            Removal::Safe(Some(Category::Interface)) => {
                say!("Likely safe to remove mid-save: it changes the interface, not the world.")
            }
            Removal::Safe(Some(Category::Tweaks)) => {
                say!(
                    "Likely safe to remove mid-save: it changes how the game runs, not what is in it."
                )
            }
            Removal::Safe(Some(Category::Patches)) => {
                say!("Likely safe to remove mid-save: it only patches defs other mods define.")
            }
            Removal::Safe(_) => {
                say!("Likely safe to remove mid-save: it adds nothing a save is known to keep.")
            }
            Removal::Defs(kinds) => say!(
                "Removing it mid-save likely damages the save, which keeps what it defines: {}",
                kinds.join(", ")
            ),
            Removal::Code => say!(
                "Removing it mid-save may damage the save: its code adds parts of the game, such as comps or hediffs, that a save can keep."
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs(all: &[(&str, &str)]) -> HashSet<Def> {
        (all.iter())
            .map(|(kind, name)| Def {
                kind: kind.to_string(),
                name: name.to_string(),
            })
            .collect()
    }

    #[test]
    fn a_mod_adding_things_or_hediffs_likely_damages_a_save_and_one_changing_looks_does_not() {
        let none = Facts::default();
        let adds = defs(&[
            ("ThingDef", "Gun"),
            ("SoundDef", "Bang"),
            ("HediffDef", "Shot"),
            ("ThingDef", "Ammo"),
        ]);
        let found = removal(Some(Category::Content), &adds, &none);
        assert_eq!(
            found,
            Removal::Defs(vec!["HediffDef".into(), "ThingDef".into()])
        );
        assert!(found.harms());
        assert_eq!(
            found.said().to_string(),
            "Removing it mid-save likely damages the save, which keeps what it defines: HediffDef, ThingDef"
        );
        // Defs a save keeps win over any category.
        let textures = removal(Some(Category::Textures), &adds, &none);
        assert!(textures.harms());

        let sounds = defs(&[("SoundDef", "Bang"), ("KeyBindingDef", "Fire")]);
        assert_eq!(
            removal(Some(Category::Textures), &HashSet::new(), &none),
            Removal::Safe(Some(Category::Textures))
        );
        assert!(!removal(Some(Category::Patches), &sounds, &none).harms());
        assert!(
            (removal(Some(Category::Interface), &HashSet::new(), &none).said())
                .to_string()
                .starts_with("Likely safe")
        );
    }

    #[test]
    fn code_adding_parts_of_the_game_may_damage_a_save_unless_it_only_draws_or_tweaks() {
        let comps = Facts {
            types: 4,
            adding: 2,
            ..Facts::default()
        };
        let nothing = HashSet::new();
        assert_eq!(
            removal(Some(Category::Content), &nothing, &comps),
            Removal::Code
        );
        assert_eq!(removal(None, &nothing, &comps), Removal::Code);
        assert_eq!(
            removal(Some(Category::Tweaks), &nothing, &comps),
            Removal::Safe(Some(Category::Tweaks))
        );
    }
}
