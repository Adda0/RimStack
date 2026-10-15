//! Nothing is run to classify a mod.

use crate::mods::list::Category;
use crate::rimworld::code::Facts;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Kinds {
    pub defs: bool,
    pub patches: bool,
    pub assemblies: bool,
    pub textures: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Standing {
    pub official: bool,
    pub before_game: bool,
    pub dependents: usize,
}

/// A mod with defs counts as interface only from this many drawing types.
const OWN_WINDOWS: usize = 5;

pub fn classify(kinds: Kinds, facts: &Facts, standing: Standing) -> Category {
    if standing.official {
        return Category::Game;
    }
    if standing.before_game {
        return Category::Loaders;
    }
    if standing.dependents >= 2 {
        return Category::Libraries;
    }
    let few_windows = kinds.defs && facts.drawing < OWN_WINDOWS;
    if facts.is_interface() && !few_windows {
        return Category::Interface;
    }
    if !kinds.defs && facts.is_tweak() {
        return Category::Tweaks;
    }
    if kinds.defs || kinds.assemblies {
        Category::Content
    } else if kinds.textures {
        Category::Textures
    } else if kinds.patches {
        Category::Patches
    } else {
        Category::Content
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const CODE: Kinds = Kinds {
        defs: false,
        patches: false,
        assemblies: true,
        textures: false,
    };

    fn draws() -> Facts {
        Facts {
            types: 10,
            drawing: 5,
            ..Facts::default()
        }
    }

    #[test]
    fn where_a_mod_stands_comes_before_what_it_holds() {
        let none = Facts::default();
        let official = Standing {
            official: true,
            before_game: true,
            dependents: 9,
        };
        assert_eq!(classify(CODE, &draws(), official), Category::Game);
        let loader = Standing {
            before_game: true,
            dependents: 900,
            ..Standing::default()
        };
        assert_eq!(classify(CODE, &none, loader), Category::Loaders);
        let needed = Standing {
            dependents: 2,
            ..Standing::default()
        };
        assert_eq!(classify(CODE, &draws(), needed), Category::Libraries);
        let once = Standing {
            dependents: 1,
            ..Standing::default()
        };
        assert_eq!(classify(CODE, &draws(), once), Category::Interface);
    }

    #[test]
    fn code_that_is_interface_decides_also_beside_defs() {
        let alone = Standing::default();
        let with_defs = Kinds { defs: true, ..CODE };
        assert_eq!(classify(with_defs, &draws(), alone), Category::Interface);
        let two_windows = Facts {
            types: 3,
            drawing: 2,
            ..Facts::default()
        };
        assert_eq!(classify(with_defs, &two_windows, alone), Category::Content);
        assert_eq!(classify(CODE, &two_windows, alone), Category::Interface);
    }

    #[test]
    fn code_that_changes_how_the_game_plays_and_adds_nothing_is_a_tweak() {
        let alone = Standing::default();
        let ticking = Facts {
            types: 12,
            patches: 6,
            on_playing: 6,
            ..Facts::default()
        };
        assert_eq!(classify(CODE, &ticking, alone), Category::Tweaks);
        let with_defs = Kinds { defs: true, ..CODE };
        assert_eq!(classify(with_defs, &ticking, alone), Category::Content);
        let adds = Facts {
            adding: 1,
            ..ticking
        };
        assert_eq!(classify(CODE, &adds, alone), Category::Content);
        let two_patches = Facts {
            patches: 2,
            on_playing: 2,
            ..ticking
        };
        assert_eq!(classify(CODE, &two_patches, alone), Category::Content);
        let elsewhere = Facts {
            on_playing: 2,
            on_loading: 4,
            ..ticking
        };
        assert_eq!(classify(CODE, &elsewhere, alone), Category::Content);
    }

    #[test]
    fn without_interface_code_the_folders_decide() {
        let (alone, none) = (Standing::default(), Facts::default());
        let only = |defs, patches, assemblies, textures| Kinds {
            defs,
            patches,
            assemblies,
            textures,
        };
        assert_eq!(classify(CODE, &none, alone), Category::Content);
        assert_eq!(
            classify(only(true, true, false, true), &none, alone),
            Category::Content
        );
        assert_eq!(
            classify(only(false, true, false, true), &none, alone),
            Category::Textures
        );
        assert_eq!(
            classify(only(false, true, false, false), &none, alone),
            Category::Patches
        );
        assert_eq!(classify(Kinds::default(), &none, alone), Category::Content);
    }
}
