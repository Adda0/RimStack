//! Two active mods shipping a dll of the same name: the game loads the first
//! copy it meets, so a mod bundling shared code like `0Harmony.dll` can put
//! an old one in the place of the mod meant to provide it. Harmony's own
//! guidance is to depend on the Harmony mod rather than bundle the dll, so a
//! bundled `0Harmony.dll` is flagged even with Harmony inactive. Identical
//! copies say nothing: the same file twice behaves as the one copy loaded.

use crate::base::sentence::Sentence;
use crate::mods::describe::{Fix, Trouble, TroubleKind};
use crate::mods::infer::{Dll, Scanned};
use crate::say;
use crate::say_n;

/// The one mod every mod may depend on instead of shipping its own copy.
const HARMONY: &str = "brrainz.harmony";
/// The file name Harmony's dll ships as, folded.
const HARMONY_DLL: &str = "0harmony.dll";

/// Major, minor, build, revision, when the file reads as an assembly.
type Version = Option<[u16; 4]>;

/// The dlls of one active mod that clash with what the game will load.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Bundling the dll the Harmony mod provides: the bundled version, then
    /// the Harmony mod's, when those files read as assemblies. Never `Some`
    /// for the Harmony mod itself; its version is `None` when it is inactive.
    pub harmony: Option<(Version, Version)>,
    /// Files another active mod also ships, in a different version or size.
    pub clashes: Vec<Clash>,
}

impl Found {
    pub fn is_empty(&self) -> bool {
        self.harmony.is_none() && self.clashes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    /// As the file is written here.
    pub file: String,
    /// The version the file here names, when it reads as an assembly.
    pub own: Option<[u16; 4]>,
    /// The other mod's name, for the sentence, and its version.
    pub other: (String, Option<[u16; 4]>),
    /// The other mod loads earlier, so the game uses its copy of the dll.
    pub other_first: bool,
}

/// One dll of the name `folded` that a mod ships, if any.
fn named<'a>(dlls: &'a [Dll], folded: &str) -> Option<&'a Dll> {
    dlls.iter().find(|d| d.folded_file() == *folded)
}

/// `scanned`, `ids` (folded) and `names` per active mod, all in load order.
pub fn check(scanned: &[&Scanned], ids: &[&str], names: &[String]) -> Vec<Found> {
    let harmony_at = ids.iter().position(|id| *id == HARMONY);
    let harmony_version = harmony_at
        .and_then(|at| named(&scanned[at].dlls, HARMONY_DLL))
        .and_then(|d| d.version);
    (scanned.iter().enumerate())
        .map(|(at, own)| {
            let mut found = Found::default();
            if ids[at] != HARMONY
                && let Some(dll) = named(&own.dlls, HARMONY_DLL)
            {
                found.harmony = Some((dll.version, harmony_version));
            }
            for (shipper, theirs) in scanned.iter().enumerate() {
                if shipper == at {
                    continue;
                }
                for dll in &own.dlls {
                    let folded = dll.folded_file();
                    // Bundling Harmony is said by `harmony`, not as a clash.
                    if folded == HARMONY_DLL {
                        continue;
                    }
                    let Some(other) = theirs
                        .dlls
                        .iter()
                        .find(|d| d.folded_file() == folded && !dll.same_file(d))
                    else {
                        continue;
                    };
                    let clash = Clash {
                        file: dll.file.clone(),
                        own: dll.version,
                        other: (names[shipper].clone(), other.version),
                        other_first: shipper < at,
                    };
                    if !found.clashes.contains(&clash) {
                        found.clashes.push(clash);
                    }
                }
            }
            found
        })
        .collect()
}

/// The parts of `version` after the first two when they are not zero:
/// `2.3.1.7`, `2.3` for `2.3.0.0`, `None` for the all-zero build stamp.
fn versioned(version: Option<[u16; 4]>) -> Option<String> {
    let [major, minor, build, revision] = version?;
    let fields = format!("{major}.{minor}.{build}.{revision}");
    Some(fields.trim_end_matches(".0").to_string()).filter(|text| *text != "0.0")
}

/// Ignorable by `Trouble::key`, like the rest.
pub fn troubles(found: &Found) -> Vec<Trouble> {
    let mut said = Vec::new();
    if let Some((own, mods)) = found.harmony {
        let words = match (versioned(own), versioned(mods)) {
            (Some(own), Some(mods)) => say!(
                "Ships its own 0Harmony.dll, version {}, while Harmony [brrainz.harmony] provides version {}: a bundled copy can take the place of Harmony's and break the mods that use it, so this mod should depend on Harmony instead",
                own,
                mods
            ),
            (Some(own), None) => say!(
                "Ships its own 0Harmony.dll, version {}: a bundled copy can take the place of the one Harmony [brrainz.harmony] provides and break the mods that use it, so this mod should depend on Harmony instead",
                own
            ),
            _ => say!(
                "Ships its own 0Harmony.dll: a bundled copy can take the place of the one Harmony [brrainz.harmony] provides and break the mods that use it, so this mod should depend on Harmony instead"
            ),
        };
        said.push(Trouble {
            error: false,
            words,
            items: Vec::new(),
            short: say!("Ships its own 0Harmony.dll"),
            fix: Fix::None,
            key: "assembly-harmony".to_string(),
            kind: TroubleKind::OwnHarmony,
        });
    }
    if !found.clashes.is_empty() {
        let files: Vec<Sentence> = (found.clashes.iter())
            .map(|clash| {
                let file = match (versioned(clash.own), versioned(clash.other.1)) {
                    (Some(own), Some(theirs)) => format!("{} ({own} vs {theirs})", clash.file),
                    _ => clash.file.clone(),
                };
                match clash.other_first {
                    true => say!(
                        "{}, also in {}, which loads first: the game uses that copy",
                        file,
                        clash.other.0
                    ),
                    false => say!(
                        "{}, also in {}: this mod loads first, so the game uses this copy",
                        file,
                        clash.other.0
                    ),
                }
            })
            .collect();
        let n = files.len();
        said.push(Trouble {
            error: false,
            words: say_n!(
                n,
                "{} dll another active mod also ships, differently; the game loads only the first copy:",
                "{} dlls other active mods also ship, differently; the game loads only the first copy:"
            ),
            items: files,
            short: say!("A dll two active mods ship"),
            fix: Fix::None,
            key: "assembly-clash".to_string(),
            kind: TroubleKind::DllClash,
        });
    }
    said
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rimworld::dotnet::build::Builder;
    use std::fs;

    fn a_dll(name: &str, version: [u16; 4]) -> Dll {
        let mut bytes = Builder::new();
        bytes.assembly(name, version);
        let bytes = bytes.finish();
        Dll {
            file: format!("{name}.dll"),
            size: bytes.len() as u64,
            version: Some(version),
        }
    }

    fn a_mod(dlls: Vec<Dll>) -> Scanned {
        Scanned {
            dlls,
            ..Scanned::default()
        }
    }

    fn checked(scanned: &[&Scanned], ids: &[&str], names: &[&str]) -> Vec<Found> {
        let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        check(scanned, ids, &names)
    }

    #[test]
    fn a_bundled_harmony_is_flagged_with_both_versions() {
        let harmony = a_mod(vec![a_dll("0Harmony", [2, 3, 1, 7])]);
        let bundler = a_mod(vec![a_dll("0Harmony", [2, 1, 0, 0])]);
        let found = checked(
            &[&harmony, &bundler],
            &["brrainz.harmony", "some.mod"],
            &["Harmony", "Some Mod"],
        );
        assert!(found[0].is_empty());
        assert_eq!(
            found[1].harmony,
            Some((Some([2, 1, 0, 0]), Some([2, 3, 1, 7])))
        );
        assert!(found[1].clashes.is_empty());
        let said = troubles(&found[1]);
        assert_eq!(said.len(), 1);
        assert!(said[0].words.to_string().contains("[brrainz.harmony]"));
        assert!(said[0].words.to_string().contains("2.1"));
        assert!(said[0].words.to_string().contains("2.3.1.7"));
    }

    #[test]
    fn bundling_harmony_is_flagged_even_with_harmony_off() {
        let bundler = a_mod(vec![Dll {
            file: "0Harmony.dll".to_string(),
            size: 1234,
            version: None,
        }]);
        let found = checked(&[&bundler], &["some.mod"], &["Some Mod"]);
        assert_eq!(found[0].harmony, Some((None, None)));
    }

    #[test]
    fn an_harmony_mod_itself_ships_its_dll_unsaid() {
        let harmony = a_mod(vec![a_dll("0Harmony", [2, 3, 1, 7])]);
        let found = checked(&[&harmony], &["brrainz.harmony"], &["Harmony"]);
        assert!(found[0].is_empty());
    }

    #[test]
    fn identical_copies_say_nothing() {
        // One mod's two load folders may hold the same file; two mods may too.
        let a = a_mod(vec![a_dll("Lib", [1, 0, 0, 0])]);
        let b = a_mod(vec![a_dll("Lib", [1, 0, 0, 0])]);
        let found = checked(&[&a, &b], &["a.a", "b.b"], &["A", "B"]);
        assert!(found[0].is_empty());
        assert!(found[1].is_empty());
    }

    #[test]
    fn a_different_copy_is_flagged_on_both_who_loads_first_named() {
        let a = a_mod(vec![a_dll("SomeLib", [1, 2, 0, 0])]);
        let b = a_mod(vec![a_dll("SomeLib", [1, 3, 0, 0])]);
        let found = checked(&[&a, &b], &["a.a", "b.b"], &["A Mod", "B Mod"]);
        assert_eq!(
            found[0].clashes,
            vec![Clash {
                file: "SomeLib.dll".to_string(),
                own: Some([1, 2, 0, 0]),
                other: ("B Mod".to_string(), Some([1, 3, 0, 0])),
                other_first: false,
            }]
        );
        assert_eq!(
            found[1].clashes,
            vec![Clash {
                file: "SomeLib.dll".to_string(),
                own: Some([1, 3, 0, 0]),
                other: ("A Mod".to_string(), Some([1, 2, 0, 0])),
                other_first: true,
            }]
        );
        let items = |found: &Found| -> Vec<String> {
            troubles(found)[0]
                .items
                .iter()
                .map(Sentence::to_string)
                .collect()
        };
        assert_eq!(
            items(&found[1]),
            ["SomeLib.dll (1.3 vs 1.2), also in A Mod, which loads first: the game uses that copy"]
        );
        assert_eq!(
            items(&found[0]),
            [
                "SomeLib.dll (1.2 vs 1.3), also in B Mod: this mod loads first, so the game uses this copy"
            ]
        );
    }

    #[test]
    fn each_dll_two_mods_ship_differently_is_an_item_of_its_own() {
        let a = a_mod(vec![a_dll("One", [1, 2, 0, 0]), a_dll("Two", [2, 1, 0, 0])]);
        let b = a_mod(vec![a_dll("One", [1, 3, 0, 0]), a_dll("Two", [2, 2, 0, 0])]);
        let found = checked(&[&a, &b], &["a.a", "b.b"], &["A Mod", "B Mod"]);
        let said = troubles(&found[1]);
        assert_eq!(said.len(), 1);
        assert_eq!(
            said[0].words.to_string(),
            "2 dlls other active mods also ship, differently; the game loads only the first copy:"
        );
        let items: Vec<String> = said[0].items.iter().map(Sentence::to_string).collect();
        assert_eq!(
            items,
            [
                "One.dll (1.3 vs 1.2), also in A Mod, which loads first: the game uses that copy",
                "Two.dll (2.2 vs 2.1), also in A Mod, which loads first: the game uses that copy"
            ]
        );
        assert_eq!(
            said[0].in_full().to_string(),
            format!("{}\n- {}\n- {}", said[0].words, items[0], items[1])
        );
    }

    #[test]
    fn a_different_size_of_an_unreadable_copy_is_flagged_too() {
        let a = a_mod(vec![a_dll("SomeLib", [1, 0, 0, 0])]);
        let mut broken = a_dll("SomeLib", [9, 9, 9, 9]);
        broken.version = None;
        broken.size += 10;
        let b = a_mod(vec![broken]);
        let found = checked(&[&a, &b], &["a.a", "b.b"], &["A", "B"]);
        assert_eq!(found[0].clashes.len(), 1);
    }

    #[test]
    fn the_scan_records_file_size_and_version() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Assemblies");
        fs::create_dir_all(&dir).unwrap();
        let mut b = Builder::new();
        b.reference("Assembly-CSharp");
        b.assembly("0Harmony", [2, 3, 6, 0]);
        fs::write(dir.join("0Harmony.dll"), b.finish()).unwrap();
        let scanned = crate::mods::infer::scan(&[tmp.path().to_path_buf()]);
        assert_eq!(scanned.dlls.len(), 1);
        assert_eq!(scanned.dlls[0].version, Some([2, 3, 6, 0]));
        assert!(scanned.assemblies.contains("0harmony"));
    }
}
