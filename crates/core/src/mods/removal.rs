//! Whether a mod can leave a colony mid-save, judged from what it adds: a likelihood, never a
//! promise, since nothing is run.

use crate::base::sentence::Sentence;
use crate::mods::defs::Def;
use crate::mods::infer::Inferred;
use crate::mods::list::Category;
use crate::rimworld::code::Facts;
use crate::say;
use std::collections::{BTreeMap, BTreeSet, HashSet};

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

/// A mod of the list left wanting once others leave, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanting {
    pub at: usize,
    pub why: Vec<Sentence>,
}

/// The mods of a list left wanting once those at `gone` leave it: naming one as a dependency,
/// or, by the rules read from their files, patching its defs, building on its parent defs or
/// using its code. `requires` as `Library::requires_within` gives it, `rules` as `infer::infer`;
/// in list order.
pub fn left_wanting(gone: &[usize], requires: &[Vec<usize>], rules: &[Inferred]) -> Vec<Wanting> {
    let mut found: BTreeMap<usize, Vec<Sentence>> = BTreeMap::new();
    for (at, needs) in requires.iter().enumerate() {
        if !gone.contains(&at) && needs.iter().any(|n| gone.contains(n)) {
            let why = say!("names it as a dependency");
            found.entry(at).or_default().push(why);
        }
    }
    let breaking = |r: &&Inferred| r.needs && gone.contains(&r.first) && !gone.contains(&r.then);
    for rule in rules.iter().filter(breaking) {
        found.entry(rule.then).or_default().push(rule.why.clone());
    }
    (found.into_iter())
        .map(|(at, why)| Wanting { at, why })
        .collect()
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

    #[test]
    fn a_mod_leaving_names_the_active_mods_needing_it_and_why() {
        use crate::mods::{infer, library};
        use crate::rimworld::dotnet::build::Builder;
        use crate::rimworld::paths::{ModRoot, Source};
        let tmp = tempfile::tempdir().unwrap();
        let mods = tmp.path().join("Mods");
        let add = |folder: &str, id: &str, extra: &str, files: &[(&str, &[u8])]| {
            let root = mods.join(folder);
            let about = format!(
                "<ModMetaData><packageId>{id}</packageId><name>{folder}</name>{extra}</ModMetaData>"
            );
            for (file, bytes) in [("About/About.xml", about.as_bytes())].iter().chain(files) {
                let path = root.join(file);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, bytes).unwrap();
            }
        };
        let mut uses = Builder::new();
        uses.reference("LibCode");
        let uses = uses.finish();
        add(
            "Lib",
            "t.lib",
            "",
            &[
                (
                    "Defs/Guns.xml",
                    b"<Defs><ThingDef><defName>Lib_Gun</defName></ThingDef></Defs>",
                ),
                ("Assemblies/LibCode.dll", b"not read"),
            ],
        );
        let deps = "<modDependencies><li><packageId>t.lib</packageId></li></modDependencies>";
        add("App", "t.app", deps, &[]);
        let patch = br#"<Patch><Operation Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Lib_Gun"]/comps</xpath></Operation></Patch>"#;
        add("Patcher", "t.patcher", "", &[("Patches/p.xml", patch)]);
        add("Coder", "t.coder", "", &[("Assemblies/Coder.dll", &uses)]);
        // A patch written for it is skipped without it: nothing is left wanting.
        let guarded = br#"<Patch><Operation Class="PatchOperationFindMod"><mods><li>Lib</li></mods><match Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Lib_Gun"]</xpath></match></Operation></Patch>"#;
        add("Compat", "t.compat", "", &[("Patches/c.xml", guarded)]);
        add(
            "Alone",
            "t.alone",
            "",
            &[(
                "Defs/A.xml",
                b"<Defs><ThingDef><defName>A</defName></ThingDef></Defs>",
            )],
        );
        let roots = [ModRoot {
            path: mods,
            source: Source::Local,
        }];
        let active = [
            "t.lib",
            "t.alone",
            "t.app",
            "t.patcher",
            "t.coder",
            "t.compat",
        ];
        let active = active.map(String::from);
        let library = library::build(library::scan(&roots), &active, "1.6");
        let list = &library.active;
        let paths: Vec<_> = (list.iter())
            .map(|m| library.get(m.installed.unwrap()).unwrap().path.clone())
            .collect();
        let scanned: Vec<infer::Scanned> = (paths.iter())
            .map(|p| infer::scan(std::slice::from_ref(p)))
            .collect();
        let names: Vec<String> = list.iter().map(|m| m.name.clone()).collect();
        let rules = infer::infer(
            &scanned.iter().collect::<Vec<_>>(),
            &names,
            &vec![false; list.len()],
            &paths,
        );
        let requires = library.requires_within(list);
        let place = |name: &str| names.iter().position(|n| n == name).unwrap();
        let report = |gone: &str| -> Vec<(String, Vec<String>)> {
            left_wanting(&[place(gone)], &requires, &rules)
                .into_iter()
                .map(|w| {
                    (
                        names[w.at].clone(),
                        w.why.iter().map(|s| s.to_string()).collect(),
                    )
                })
                .collect()
        };
        assert_eq!(
            report("Lib"),
            [
                (
                    "App".to_string(),
                    vec!["names it as a dependency".to_string()]
                ),
                (
                    "Patcher".to_string(),
                    vec!["Patches/p.xml patches its Lib_Gun".to_string()]
                ),
                (
                    "Coder".to_string(),
                    vec!["Assemblies/Coder.dll uses its code".to_string()]
                ),
            ]
        );
        assert!(report("Alone").is_empty(), "nothing depends on it");
    }
}
