//! What the game will complain of in the active mods' XML before it runs: files that do not
//! parse, patches aimed at defs no active mod defines, and mods a patch or def asks for that
//! are not installed. Only the simple xpaths, `Defs/ThingDef[defName="Steel"]…`, are judged.

use crate::base::sentence::Sentence;
use crate::mods::defs::{Def, first_few};
use crate::mods::describe::{Fix, Trouble};
use crate::mods::infer::Scanned;
use crate::{say, say_n};
use roxmltree::{Document, Node, ParsingOptions};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The operations the game reports failed when their xpath selects nothing.
const PATHED: [&str; 9] = [
    "PatchOperationAdd",
    "PatchOperationInsert",
    "PatchOperationRemove",
    "PatchOperationReplace",
    "PatchOperationAttributeAdd",
    "PatchOperationAttributeSet",
    "PatchOperationAttributeRemove",
    "PatchOperationAddModExtension",
    "PatchOperationSetName",
];

/// A patch operation that fails unless one of `defs` is defined; a `None` kind is any type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aim {
    pub defs: Vec<(Option<String>, String)>,
    /// It runs only when, for each, one of the mods named is active (`true`) or none is.
    pub when: Vec<(bool, Vec<String>)>,
    pub file: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Read {
    /// With the line the parser stopped at.
    pub broken: Vec<(PathBuf, u32)>,
    pub aims: Vec<Aim>,
    /// Folded package ids a `MayRequire` names.
    pub may_require: Vec<(String, PathBuf)>,
}

fn parsed(text: &str) -> Result<Document<'_>, u32> {
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    Document::parse_with_options(text.trim_start_matches('\u{feff}'), options)
        .map_err(|e| e.pos().row)
}

fn asked(doc: &Document, file: &Path, into: &mut Read) {
    let named = doc.descendants().filter_map(|n| n.attribute("MayRequire"));
    let ids = named.flat_map(|ids| ids.split(',')).map(str::trim);
    let ids = ids.filter(|id| !id.is_empty());
    into.may_require
        .extend(ids.map(|id| (crate::rimworld::id::fold(id), file.to_path_buf())));
}

pub fn read_defs(text: &str, file: &Path, into: &mut Read) {
    match parsed(text) {
        Ok(doc) => asked(&doc, file, into),
        Err(line) => into.broken.push((file.to_path_buf(), line)),
    }
}

pub fn read_patches(text: &str, file: &Path, into: &mut Read) {
    let doc = match parsed(text) {
        Ok(doc) => doc,
        Err(line) => return into.broken.push((file.to_path_buf(), line)),
    };
    asked(&doc, file, into);
    for op in doc.root_element().children().filter(Node::is_element) {
        walk(op, &[], false, file, into);
    }
}

fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|n| n.has_tag_name(name))
}

fn items<'a, 'i>(node: Option<Node<'a, 'i>>) -> impl Iterator<Item = Node<'a, 'i>> {
    node.into_iter()
        .flat_map(|n| n.children())
        .filter(Node::is_element)
}

fn walk(op: Node, when: &[(bool, Vec<String>)], quiet: bool, file: &Path, into: &mut Read) {
    if op.attribute("MayRequire").is_some() || op.attribute("MayRequireAnyOf").is_some() {
        return;
    }
    let class = op.attribute("Class").unwrap_or_default();
    let class = class.strip_prefix("Verse.").unwrap_or(class);
    let success = child(op, "success").and_then(|s| s.text()).map(str::trim);
    let quiet = quiet || success.is_some_and(|s| s != "Normal");
    let guarded = |wanted: bool, mods: &[String]| {
        let mut more = when.to_vec();
        more.push((wanted, mods.to_vec()));
        more
    };
    match class {
        "PatchOperationSequence" => {
            for each in items(child(op, "operations")) {
                walk(each, when, quiet, file, into);
            }
        }
        "PatchOperationFindMod" => {
            let mods: Vec<String> = items(child(op, "mods"))
                .filter_map(|li| Some(li.text()?.trim().to_string()))
                .collect();
            if let Some(found) = child(op, "match") {
                walk(found, &guarded(true, &mods), quiet, file, into);
            }
            if let Some(not_found) = child(op, "nomatch") {
                walk(not_found, &guarded(false, &mods), quiet, file, into);
            }
        }
        pathed if PATHED.contains(&pathed) && !quiet => {
            let xpath = child(op, "xpath").and_then(|x| x.text());
            if let Some(defs) = xpath.and_then(aimed_at) {
                into.aims.push(Aim {
                    defs,
                    when: when.to_vec(),
                    file: file.to_path_buf(),
                });
            }
        }
        _ => {}
    }
}

fn is_name(s: &str) -> bool {
    !s.is_empty() && s.len() < 120 && s.chars().all(|c| c.is_alphanumeric() || "_-.".contains(c))
}

/// The one quoted value `part` compares `defName` with: `defName="X"`, `defName/text()='X'`
/// or `defName[text()="X"]`.
fn def_name(part: &str) -> Option<&str> {
    let rest = part.strip_prefix("defName")?;
    let rest = rest.trim_start();
    let rest = (rest.strip_prefix("/text()"))
        .or_else(|| rest.strip_prefix("[text()")?.trim_end().strip_suffix(']'))
        .unwrap_or(rest);
    let rest = rest.trim_start().strip_prefix('=')?.trim();
    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let name = rest[1..].strip_suffix(quote)?;
    is_name(name).then_some(name)
}

/// The defs an xpath can select, when it only selects by `defName`; `None` for any other.
pub fn aimed_at(xpath: &str) -> Option<Vec<(Option<String>, String)>> {
    let mut defs = Vec::new();
    for path in xpath.split('|') {
        let path = path.trim();
        let path = path.strip_prefix('/').unwrap_or(path);
        let typed = path.strip_prefix("Defs/")?;
        let open = typed.find('[')?;
        let kind = typed[..open].trim();
        let kind = match kind {
            "*" => None,
            k if is_name(k) => Some(k.to_string()),
            _ => return None,
        };
        let mut depth = 0;
        let close = typed[open..].char_indices().find_map(|(at, c)| {
            depth += match c {
                '[' => 1,
                ']' => -1,
                _ => 0,
            };
            (depth == 0).then_some(open + at)
        })?;
        for part in typed[open + 1..close].split(" or ") {
            defs.push((kind.clone(), def_name(part.trim())?.to_string()));
        }
    }
    Some(defs)
}

/// What one active mod's XML holds that the game will complain of or leave out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Files, from the mod's folder, and the line the parser stopped at.
    pub broken: Vec<(String, u32)>,
    /// The defs each failing patch names.
    pub misses: Vec<String>,
    /// Mods asked for by name (`FindMod`) or package id (`MayRequire`) that are not installed.
    pub absent: Vec<String>,
}

impl Found {
    pub fn is_empty(&self) -> bool {
        self.broken.is_empty() && self.misses.is_empty() && self.absent.is_empty()
    }
}

/// `read` in any order, with each mod's folder in `roots`; `active` the active mods' names.
pub fn check(
    read: &[&Scanned],
    roots: &[PathBuf],
    active: &HashSet<&str>,
    installed_name: &dyn Fn(&str) -> bool,
    installed_id: &dyn Fn(&str) -> bool,
) -> Vec<Found> {
    let defined: HashSet<&Def> = read.iter().flat_map(|s| &s.defines).collect();
    let named: HashSet<&str> = defined.iter().map(|d| d.name.as_str()).collect();
    let is_defined = |(kind, name): &(Option<String>, String)| match kind {
        Some(kind) => defined.contains(&Def {
            kind: kind.clone(),
            name: name.clone(),
        }),
        None => named.contains(name.as_str()),
    };
    let runs = |(wanted, mods): &(bool, Vec<String>)| {
        mods.iter().any(|m| active.contains(m.as_str())) == *wanted
    };
    let shown = |file: &Path, root: &Path| {
        let from_root = file.strip_prefix(root).unwrap_or(file);
        from_root.to_string_lossy().replace('\\', "/")
    };
    let each = read.iter().zip(roots);
    each.map(|(s, root)| {
        let broken = (s.xml.broken.iter())
            .map(|(file, line)| (shown(file, root), *line))
            .collect();
        let missing = |aim: &&Aim| aim.when.iter().all(runs) && !aim.defs.iter().any(is_defined);
        let misses = (s.xml.aims.iter().filter(missing))
            .map(|aim| {
                let names: Vec<&str> = aim.defs.iter().map(|(_, n)| n.as_str()).collect();
                names.join(" / ")
            })
            .collect();
        let mut absent: Vec<String> = (s.asks_for.iter())
            .map(|(name, _)| name.clone())
            .filter(|name| !installed_name(name))
            .chain(
                (s.xml.may_require.iter())
                    .map(|(id, _)| id.clone())
                    .filter(|id| !installed_id(id)),
            )
            .collect();
        absent.sort_unstable();
        absent.dedup();
        Found {
            broken,
            misses,
            absent,
        }
    })
    .collect()
}

/// Ignorable by `Trouble::key`, like the rest.
pub fn troubles(found: &Found) -> Vec<Trouble> {
    let mut said = Vec::new();
    if !found.broken.is_empty() {
        let files: Vec<String> = (found.broken.iter())
            .map(|(file, line)| format!("{file}:{line}"))
            .collect();
        let n = files.len();
        said.push(Trouble {
            error: true,
            words: say_n!(
                n,
                "{} XML file will not load, so the game skips what it holds: {}",
                "{} XML files will not load, so the game skips what they hold: {}",
                first_few(&files)
            ),
            short: say!("XML that will not load"),
            fix: Fix::None,
            key: "xml-broken".to_string(),
        });
    }
    if !found.misses.is_empty() {
        let n = found.misses.len();
        said.push(Trouble {
            error: false,
            words: say_n!(
                n,
                "{} patch aims at a def no active mod defines, so the game reports it failed: {}",
                "{} patches aim at defs no active mod defines, so the game reports them failed: {}",
                first_few(&found.misses)
            ),
            short: say!("Patches miss their defs"),
            fix: Fix::None,
            key: "xml-misses".to_string(),
        });
    }
    said
}

/// The mods a mod has patches or defs for that are not installed: a quiet note, not a problem.
pub fn leaves_out(found: &Found) -> Option<Sentence> {
    let n = found.absent.len();
    (n > 0).then(|| {
        say_n!(
            n,
            "Holds patches or defs for {} mod that is not installed, which the game leaves out: {}",
            "Holds patches or defs for {} mods that are not installed, which the game leaves out: {}",
            first_few(&found.absent)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::infer::scan_checking;
    use std::fs;

    fn a_mod(files: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        for (file, text) in files {
            let path = tmp.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        let root = tmp.path().to_path_buf();
        (tmp, root)
    }

    const GAME: &str = r#"<Defs>
        <ThingDef><defName>Steel</defName></ThingDef>
        <ThingDef><defName>Wood</defName></ThingDef>
        <RecipeDef><defName>Make_Wall</defName></RecipeDef>
    </Defs>"#;

    const PATCHES: &str = r#"<?xml version="1.0" encoding="utf-8"?>
    <Patch>
        <Operation Class="PatchOperationReplace">
            <xpath>/Defs/ThingDef[defName="Steel"]/statBases/MarketValue</xpath>
        </Operation>
        <Operation Class="PatchOperationAdd">
            <xpath>Defs/ThingDef[defName = 'Gold' or defName=&quot;Wood&quot;]/comps</xpath>
        </Operation>
        <Operation Class="PatchOperationRemove">
            <xpath>Defs/ThingDef[defName="Make_Wall"]</xpath>
        </Operation>
        <Operation Class="PatchOperationSequence">
            <operations>
                <li Class="PatchOperationAdd"><xpath>Defs/*[defName[text()="Uranium_Typo"]]</xpath></li>
            </operations>
        </Operation>
        <Operation Class="PatchOperationAdd">
            <success>Always</success>
            <xpath>Defs/ThingDef[defName="Silenced"]</xpath>
        </Operation>
        <Operation Class="PatchOperationAdd" MayRequire="Ludeon.RimWorld.Biotech">
            <xpath>Defs/GeneDef[defName="Guarded"]</xpath>
        </Operation>
        <Operation Class="PatchOperationAdd">
            <xpath>Defs/ThingDef[contains(defName, "Steel")]</xpath>
        </Operation>
        <Operation Class="PatchOperationFindMod">
            <mods><li>Vanilla Expanded</li></mods>
            <match Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="VE_Only"]</xpath></match>
            <nomatch Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Without_VE"]</xpath></nomatch>
        </Operation>
        <Operation Class="PatchOperationFindMod">
            <mods><li>Core</li></mods>
            <match Class="PatchOperationAdd"><xpath>Defs/ThingDef[defName="Core_Typo"]</xpath></match>
        </Operation>
    </Patch>"#;

    #[test]
    fn patches_missing_their_defs_broken_files_and_absent_mods_are_found() {
        let (_g, game) = a_mod(&[("Defs/things.xml", GAME)]);
        let (_m, mine) = a_mod(&[
            ("Patches/p.xml", PATCHES),
            (
                "Defs/broken.xml",
                "<Defs>\n<ThingDef>\n<defName>X</defName>\n</Defs>",
            ),
            (
                "Defs/fine.xml",
                r#"<Defs><ThingDef MayRequire="Some.Missing, ludeon.rimworld.royalty"><defName>Mine</defName></ThingDef></Defs>"#,
            ),
        ]);
        let read = [
            scan_checking(std::slice::from_ref(&game)),
            scan_checking(std::slice::from_ref(&mine)),
        ];
        let borrowed: Vec<&Scanned> = read.iter().collect();
        let active: HashSet<&str> = ["Core", "Mine"].into();
        let found = check(
            &borrowed,
            &[game, mine],
            &active,
            &|name| name == "Core" || name == "Mine",
            &|id| id == "ludeon.rimworld.royalty",
        );
        assert!(found[0].is_empty(), "{:?}", found[0]);
        let mut misses = found[1].misses.clone();
        misses.sort_unstable();
        assert_eq!(
            misses,
            ["Core_Typo", "Make_Wall", "Uranium_Typo", "Without_VE"]
        );
        assert_eq!(found[1].broken, [("Defs/broken.xml".to_string(), 4)]);
        assert_eq!(
            found[1].absent,
            [
                "Vanilla Expanded",
                "ludeon.rimworld.biotech",
                "some.missing"
            ]
        );
        let said = troubles(&found[1]);
        let keys: Vec<&str> = said.iter().map(|t| t.key.as_str()).collect();
        assert_eq!(keys, ["xml-broken", "xml-misses"]);
        // Each names what it counts, not the count again.
        assert!(said[0].words.to_string().ends_with(": Defs/broken.xml:4"));
        assert!(said[1].words.to_string().contains(&found[1].misses[0]));
        let left_out = leaves_out(&found[1]).unwrap().to_string();
        assert!(left_out.contains(&found[1].absent[0]), "{left_out}");
    }

    #[test]
    fn only_xpaths_selecting_by_def_name_are_judged() {
        let aimed = |x| aimed_at(x).map(|d| d.into_iter().map(|(_, n)| n).collect::<Vec<_>>());
        assert_eq!(
            aimed(r#"Defs/ThingDef[defName="A"]/x | /Defs/ThingDef[defName="B"]/x"#),
            Some(vec!["A".to_string(), "B".to_string()])
        );
        assert_eq!(aimed(r#"Defs/ThingDef[@Name="Base"]"#), None);
        assert_eq!(aimed(r#"Defs/ThingDef[not(defName="A")]"#), None);
        assert_eq!(aimed("Defs/ThingDef/comps"), None);
        assert_eq!(aimed(r#"//ThingDef[defName="A"]"#), None);
    }
}
