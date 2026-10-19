//! Mod list files of other managers: RimSort writes CSV exports of its
//! panels and reads RimSort/RimPy XML and text lists; RimStack does the same.

use crate::base::error::{Error, Result};
use crate::rimworld::id;
use crate::said;

use roxmltree::{Document, Node};

/// One line of a CSV list, from whatever RimStack knows of the mod; empty
/// stays empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub author: String,
    pub package_id: String,
    /// The Workshop item's id; empty off the Workshop.
    pub file_id: String,
    /// The game versions its About.xml names.
    pub supported: Vec<String>,
    /// Seconds since 1970 it was subscribed to and downloaded; `None` when not known.
    pub downloaded: Option<u64>,
    /// Seconds since 1970 the Workshop last published it; `None` when not known.
    pub updated: Option<u64>,
    /// As it sits on disk: Ludeon, Local or Workshop.
    pub source: String,
    pub path: String,
    /// Its own page, else its Workshop page; empty without either.
    pub url: String,
}

/// The columns of RimSort's CSV export, in its order.
const COLUMNS: [&str; 10] = [
    "Name",
    "Author",
    "Package ID",
    "Published File Id",
    "Supported Versions",
    "Mod Downloaded",
    "Updated on Workshop",
    "Source",
    "Path",
    "Workshop Page",
];

fn at(secs: u64) -> String {
    let said = i64::try_from(secs)
        .ok()
        .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0));
    said.map_or_else(
        || secs.to_string(),
        |at| at.format("%Y-%m-%d %H:%M:%S").to_string(),
    )
}

fn cell(text: &str) -> String {
    if !text.contains([',', '"', '\n', '\r']) {
        return text.to_string();
    }
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// The header line and a line per mod, Excel-readable with the unicode mark.
pub fn csv(rows: &[Row]) -> String {
    let mut text = COLUMNS.join(",").to_string();
    for row in rows {
        text += "\r\n";
        let cells = [
            cell(&row.name),
            cell(&row.author),
            cell(&row.package_id),
            cell(&row.file_id),
            cell(&row.supported.join("; ")),
            row.downloaded.map(at).unwrap_or_default(),
            row.updated.map(at).unwrap_or_default(),
            cell(&row.source),
            cell(&row.path),
            cell(&row.url),
        ];
        text += &cells.join(",");
    }
    text += "\r\n";
    format!("\u{feff}{text}")
}

fn ids_of(walk: impl Iterator<Item = String>) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for text in walk {
        let folded = id::fold(text.trim());
        if !folded.is_empty() && !ids.contains(&folded) {
            ids.push(folded);
        }
    }
    ids
}

fn items(list: Option<Node>) -> Vec<String> {
    let Some(node) = list else {
        return Vec::new();
    };
    node.children()
        .filter(|n| n.tag_name().name().eq_ignore_ascii_case("li"))
        .filter_map(|n| n.text().map(str::to_string))
        .collect()
}

fn below<'a>(node: &Node<'a, 'a>, names: &[&str]) -> Option<Node<'a, 'a>> {
    names.iter().try_fold(*node, |node, name| {
        node.children()
            .find(|n| n.tag_name().name().eq_ignore_ascii_case(name))
    })
}

/// The package ids of the mod lists RimSort's import reads: a `ModsConfigData`
/// made by the game, RimSort or RimPy, a `<savedModList>` (.rml) or a save
/// (.rws). Ids come out folded, in listed order, each once.
pub fn ids_in_list(text: &str) -> Result<Vec<String>> {
    let doc = Document::parse(text.strip_prefix('\u{feff}').unwrap_or(text))
        .map_err(|e| Error::format(said!("malformed XML: {}", e)))?;
    let root = doc.root_element();
    let list = below(&root, &["activeMods"])
        .or_else(|| below(&root, &["meta", "modIds"]))
        .ok_or_else(|| Error::format(said!("no mod list found")))?;
    Ok(ids_of(items(Some(list)).into_iter()))
}

/// Detects the kind by root, whatever the comment around it.
pub fn ids_from_text(text: &str) -> Result<Vec<String>> {
    let trimmed = text.trim_start_matches(['\u{feff}', ' ', '\t', '\r', '\n']);
    if trimmed.starts_with('<') {
        return ids_in_list(text);
    }
    if trimmed.starts_with('{') {
        let found: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| Error::format(said!("malformed JSON: {}", e)))?;
        let ids = found
            .pointer("/activeMods/li")
            .or_else(|| found.pointer("/activeMods"))
            .and_then(|ids| serde_json::from_value::<Vec<String>>(ids.clone()).ok())
            .ok_or_else(|| Error::format(said!("no mod list found")))?;
        return Ok(ids_of(ids.into_iter()));
    }
    Ok(crate::mods::share::ids_in(text))
}

pub fn file(path: &std::path::Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::file(path, e))?;
    ids_from_text(&text).map_err(|e| e.of(path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, id: &str) -> Row {
        Row {
            name: name.to_string(),
            package_id: id.to_string(),
            ..Row::default()
        }
    }

    #[test]
    fn the_csv_has_rimsort_s_columns_and_a_line_per_mod() {
        let rows = [
            Row {
                name: "Harmony".into(),
                author: "brrainz".into(),
                package_id: "brrainz.harmony".into(),
                file_id: "2009463077".into(),
                supported: vec!["1.5".into(), "1.6".into()],
                downloaded: Some(700000000),
                updated: None,
                source: "Workshop".into(),
                path: "/home/a/RimWorld/workshop/content/294100/2009463077".into(),
                url: "https://steamcommunity.com/sharedfiles/filedetails/?id=2009463077".into(),
            },
            row("An Oddity, \"quoted\"", "odd.thing"),
        ];
        let text = csv(&rows);
        let lines: Vec<&str> = text.trim_end().split("\r\n").collect();
        assert_eq!(
            &lines[0][3..],
            "Name,Author,Package ID,Published File Id,Supported Versions,Mod Downloaded,Updated on Workshop,Source,Path,Workshop Page"
        );
        assert_eq!(
            lines[1],
            "Harmony,brrainz,brrainz.harmony,2009463077,1.5; 1.6,1992-03-07 20:26:40,,Workshop,/home/a/RimWorld/workshop/content/294100/2009463077,https://steamcommunity.com/sharedfiles/filedetails/?id=2009463077"
        );
        assert_eq!(lines[2], "\"An Oddity, \"\"quoted\"\"\",,odd.thing,,,,,,,");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn a_csv_line_with_empty_mods_is_just_the_header() {
        assert_eq!(csv(&[]), format!("\u{feff}{}\r\n", COLUMNS.join(",")));
    }

    #[test]
    fn a_game_written_modsconfig_gives_its_list() {
        let list = ids_from_text(
            "<ModsConfigData><activeMods><li>Ludeon.RimWorld</li> <li>Ann.A</li></activeMods></ModsConfigData>",
        )
        .unwrap();
        assert_eq!(list, ["ludeon.rimworld", "ann.a"]);
    }

    #[test]
    fn a_saved_list_and_a_save_give_their_mod_ids() {
        let rml = ids_from_text(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?><savedModList><meta><modIds><li>ludeon.rimworld</li><li>brrainz.harmony</li></modIds></meta></savedModList>",
        )
        .unwrap();
        assert_eq!(rml, ["ludeon.rimworld", "brrainz.harmony"]);
        let rws = ids_from_text(
            "<savegame><meta><modIds><li>ann.a</li></modIds></meta><game/></savegame>",
        )
        .unwrap();
        assert_eq!(rws, ["ann.a"]);
    }

    #[test]
    fn rimsort_s_json_export_gives_its_list() {
        let text = ids_from_text(
            "{ \"version\": \"1.6\", \"activeMods\": { \"li\": [\"Ann.A\", \"Brrainz.Harmony\"] } }",
        )
        .unwrap();
        assert_eq!(text, ["ann.a", "brrainz.harmony"]);
        let plain = ids_from_text("{ \"version\": \"1.6\", \"activeMods\": [\"Ann.A\"] }").unwrap();
        assert_eq!(plain, ["ann.a"]);
    }

    #[test]
    fn every_listed_id_comes_out_once_and_folded() {
        let list = ids_from_text(
            "<ModsConfigData><activeMods><li>Ann.A</li><li>ann.a</li><li> Ann.A </li></activeMods></ModsConfigData>",
        )
        .unwrap();
        assert_eq!(list, ["ann.a"]);
    }

    #[test]
    fn a_file_without_a_list_is_refused_and_bad_xml_is_named() {
        assert!(ids_from_text("<ModsConfigData/>").is_err());
        assert!(ids_from_text("<ModsConfigData").is_err());
        assert!(ids_from_text("{ \"other\": 1 }").is_err());
        assert!(ids_from_text("{ no json").is_err());
    }

    #[test]
    fn what_is_written_reads_back() {
        let entries = [
            crate::mods::share::Entry {
                name: "Harmony".into(),
                package_id: "brrainz.harmony".into(),
                url: Some(
                    "https://steamcommunity.com/sharedfiles/filedetails/?id=2009463077".into(),
                ),
            },
            crate::mods::share::Entry {
                name: "Ann.A".into(),
                package_id: "ann.a".into(),
                url: None,
            },
        ];
        let text = crate::mods::share::listing("1.6", &entries);
        assert_eq!(ids_from_text(&text).unwrap(), ["brrainz.harmony", "ann.a"]);
        let base = "<ModsConfigData><activeMods/></ModsConfigData>";
        let ids = vec!["lodeon.rimworld".to_string(), "ann.a".to_string()];
        let written = crate::rimworld::modsconfig::with_active(base, &ids).unwrap();
        assert_eq!(ids_from_text(&written).unwrap(), ids);
    }

    #[test]
    fn a_plain_text_list_reads_as_on_the_clipboard() {
        let text = ids_from_text("Created with RimStack\nTotal # of mods: 1\n\nHarmony [brrainz.harmony][https://steamcommunity.com/sharedfiles/filedetails/?id=2009463077]\n").unwrap();
        assert_eq!(text, ["brrainz.harmony"]);
    }
}
