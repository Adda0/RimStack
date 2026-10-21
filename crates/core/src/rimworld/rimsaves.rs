//! RimSaves (aRandomKiwi) names a save `Folder#§#Name.rws`; a save in
//! `Default` has no prefix. Its previews and facts sit beside `Saves`, in
//! `RimSaves/Previews/<stem>.dat` and `RimSaves/Meta/<stem>.dat`. Read only:
//! RimStack never writes them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SEPARATOR: &str = "#§#";
pub const DEFAULT_FOLDER: &str = "Default";

pub fn split(stem: &str) -> (&str, &str) {
    match stem.split_once(SEPARATOR) {
        Some((folder, name)) if !folder.is_empty() => (folder, name),
        _ => (DEFAULT_FOLDER, stem),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    /// In RimSaves' order; `Default` is always present.
    pub folders: Vec<String>,
    pub current: String,
}

pub fn parse_settings(xml: &str) -> Option<Settings> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let doc = crate::base::xml::parse(xml).ok()?;
    let block = doc.descendants().find(|n| n.has_tag_name("ModSettings"))?;
    let child = |name: &str| block.children().find(|n| n.has_tag_name(name));
    let mut folders: Vec<String> = child("folders")
        .into_iter()
        .flat_map(|n| n.children())
        .filter(|n| n.has_tag_name("li"))
        .filter_map(|n| n.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if !folders.iter().any(|f| f == DEFAULT_FOLDER) {
        folders.insert(0, DEFAULT_FOLDER.to_string());
    }
    // The game leaves a value out of the file while it is the default.
    let current = child("curFolder").and_then(|n| n.text()).map(str::trim);
    Some(Settings {
        folders,
        current: current.unwrap_or(DEFAULT_FOLDER).to_string(),
    })
}

/// `None` when RimSaves was never used.
pub fn settings(config: &Path) -> Option<Settings> {
    let entries = std::fs::read_dir(config.join("Config")).ok()?;
    let file = entries.flatten().map(|e| e.path()).find(|p| {
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        name.starts_with("Mod_") && name.ends_with("_RimSaves.xml")
    })?;
    parse_settings(&std::fs::read_to_string(file).ok()?)
}

fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (other, _) => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parses the text of a `Meta/<name>.dat`: `key:value` pairs joined by
/// commas, each part escaped.
pub fn parse_meta(text: &str) -> BTreeMap<String, String> {
    text.split(',')
        .filter_map(|pair| pair.split_once(':'))
        .map(|(key, value)| (unescape(key), unescape(value)))
        .filter(|(key, _)| !key.is_empty())
        .collect()
}

pub fn meta(config: &Path, stem: &str) -> BTreeMap<String, String> {
    let file = config
        .join("RimSaves")
        .join("Meta")
        .join(format!("{stem}.dat"));
    std::fs::read_to_string(file)
        .map(|t| parse_meta(&t))
        .unwrap_or_default()
}

/// The `.dat` file holds an ordinary PNG or JPEG.
pub fn preview(config: &Path, stem: &str) -> Option<PathBuf> {
    let dir = config.join("RimSaves").join("Previews");
    ["dat", "jpg"]
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|p| p.is_file())
}

/// The labels are English keys that the interface translates.
pub fn facts(noted: &BTreeMap<String, String>) -> Vec<(&'static str, String)> {
    use crate::base::translate::named;
    let known = [
        (named("Colony"), "faction"),
        (named("Colonists"), "nbColonist"),
        (named("Settlements"), "nbColony"),
        (named("Storyteller"), "storyteller"),
        (named("Difficulty"), "storytellerDifficulty"),
        (named("Scenario"), "scenario"),
        (named("Seed"), "seed"),
    ];
    let filled = |key: &str| noted.get(key).filter(|value| !value.is_empty()).cloned();
    known
        .into_iter()
        .filter_map(|(what, key)| Some((what, filled(key)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_facts_a_save_has_are_listed_in_their_order() {
        let noted = parse_meta("seed:abc,faction:New Hope,nbColonist:,unknown:1");
        assert_eq!(
            facts(&noted),
            [
                ("Colony", "New Hope".to_string()),
                ("Seed", "abc".to_string())
            ]
        );
    }
    use std::fs;

    #[test]
    fn a_file_name_splits_into_folder_and_name() {
        assert_eq!(split("Medieval#§#Stonehold"), ("Medieval", "Stonehold"));
        assert_eq!(split("Stonehold"), ("Default", "Stonehold"));
        assert_eq!(split("#§#odd"), ("Default", "#§#odd"));
        assert_eq!(split("A#§#B#§#C"), ("A", "B#§#C"));
    }

    #[test]
    fn settings_list_the_folders_and_the_current_one() {
        let xml = "\u{feff}<SettingsBlock><ModSettings Class=\"aRandomKiwi.ARS.Settings\">
            <curFolder>Medieval</curFolder>
            <folders><li>Default</li><li>Medieval</li><li> Space </li></folders>
        </ModSettings></SettingsBlock>";
        let found = parse_settings(xml).unwrap();
        assert_eq!(found.folders, ["Default", "Medieval", "Space"]);
        assert_eq!(found.current, "Medieval");
    }

    #[test]
    fn settings_left_at_their_defaults_still_have_the_default_folder() {
        let found = parse_settings("<SettingsBlock><ModSettings/></SettingsBlock>").unwrap();
        assert_eq!(found.folders, ["Default"]);
        assert_eq!(found.current, "Default");
        assert_eq!(parse_settings("not xml"), None);
    }

    #[test]
    fn facts_are_unescaped() {
        let found = parse_meta(
            "faction:New%20Arrivals,storyteller:Cassandra%20Classic,nbColonist:7,odd,seed:a%3Ab%2Cc,name:caf%C3%A9",
        );
        assert_eq!(found["faction"], "New Arrivals");
        assert_eq!(found["storyteller"], "Cassandra Classic");
        assert_eq!(found["nbColonist"], "7");
        assert_eq!(found["seed"], "a:b,c");
        assert_eq!(found["name"], "café");
        assert_eq!(found.len(), 5);
        assert!(parse_meta("").is_empty());
    }

    #[test]
    fn everything_is_found_in_the_game_s_settings_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path();
        assert_eq!(settings(config), None);
        assert!(meta(config, "x").is_empty());
        assert_eq!(preview(config, "x"), None);

        fs::create_dir_all(config.join("Config")).unwrap();
        fs::create_dir_all(config.join("RimSaves/Meta")).unwrap();
        fs::create_dir_all(config.join("RimSaves/Previews")).unwrap();
        fs::write(config.join("Config/Mod_1713367505_RimSaves.xml"), "<SettingsBlock><ModSettings><folders><li>Default</li><li>Run</li></folders></ModSettings></SettingsBlock>").unwrap();
        fs::write(config.join("Config/Mod_1_Other.xml"), "<x/>").unwrap();
        fs::write(config.join("RimSaves/Meta/Run#§#Base.dat"), "nbColony:2").unwrap();
        fs::write(config.join("RimSaves/Previews/Run#§#Base.dat"), "picture").unwrap();

        assert_eq!(settings(config).unwrap().folders, ["Default", "Run"]);
        assert_eq!(meta(config, "Run#§#Base")["nbColony"], "2");
        assert_eq!(
            preview(config, "Run#§#Base"),
            Some(config.join("RimSaves/Previews/Run#§#Base.dat"))
        );
    }
}
