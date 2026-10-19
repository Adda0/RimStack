//! What RimSort and RimPy keep about mods besides load order rules, to take it over.
//! Their files are only read.

use crate::base::error::Error;
use std::path::{Path, PathBuf};

/// One mod's tags, note and colour as another manager kept them, by its folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Kept {
    pub folder: PathBuf,
    pub tags: Vec<String>,
    pub note: String,
    /// `#rrggbb`; empty for none.
    pub colour: String,
}

/// RimSort's `aux_metadata.db`: a row per mod folder, its tags joined through `tags_table`.
/// Only mods with a tag, a note or a colour are given.
pub fn rimsort(db: &Path) -> Result<Vec<Kept>, Error> {
    use rusqlite::{Connection, OpenFlags};
    let failed = |e: rusqlite::Error| Error::file(db, e);
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(db, flags).map_err(failed)?;
    // Tags are joined with a unit separator, which no tag holds.
    let mut asked = connection
        .prepare(
            "SELECT a.path, a.user_notes, a.color_hex, group_concat(t.tag, char(31))
             FROM auxiliary_metadata a
             LEFT JOIN tags_table x ON x.left_id = a.path
             LEFT JOIN mod_tags t ON t.id = x.right_id
             GROUP BY a.path",
        )
        .map_err(failed)?;
    let rows = asked
        .query_map([], |row| {
            let text = |at| {
                row.get::<_, Option<String>>(at)
                    .map(Option::unwrap_or_default)
            };
            Ok(Kept {
                folder: PathBuf::from(text(0)?),
                note: text(1)?.trim().to_string(),
                colour: text(2)?.trim().to_lowercase(),
                tags: text(3)?
                    .split('\u{1f}')
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(String::from)
                    .collect(),
            })
        })
        .map_err(failed)?;
    let mut found = Vec::new();
    for row in rows {
        let kept = row.map_err(failed)?;
        if !kept.tags.is_empty() || !kept.note.is_empty() || is_colour(&kept.colour) {
            found.push(kept);
        }
    }
    Ok(found)
}

/// RimPy's `config.ini`: under `[Colors]`, a mod's folder name (its Workshop id, or its folder
/// in the game's Mods) and its colour, `#rrggbb`.
pub fn rimpy(ini: &str) -> Vec<Kept> {
    let mut section = "";
    let mut found = Vec::new();
    for line in ini.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim();
            continue;
        }
        if !section.eq_ignore_ascii_case("colors") {
            continue;
        }
        let Some((folder, colour)) = line.split_once(['=', ':']) else {
            continue;
        };
        let colour = colour.trim().to_lowercase();
        if !folder.trim().is_empty() && is_colour(&colour) {
            found.push(Kept {
                folder: PathBuf::from(folder.trim()),
                colour,
                ..Kept::default()
            });
        }
    }
    found
}

/// `#rrggbb`, or `#aarrggbb` as Qt writes a colour with its alpha.
pub fn is_colour(text: &str) -> bool {
    text.strip_prefix('#')
        .is_some_and(|hex| matches!(hex.len(), 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit()))
}

/// The red, green and blue of `#rrggbb` or `#aarrggbb`.
pub fn rgb(colour: &str) -> Option<[u8; 3]> {
    if !is_colour(colour) {
        return None;
    }
    let hex = &colour[colour.len() - 6..];
    let part = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([part(0)?, part(2)?, part(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rimpy_colours_are_read_from_their_section_only() {
        let ini = "[General]\nfoo = #ffffff\n\n[Colors]\n1847679158 = #FF0000\nLocal Mod: #00ff00\nbroken = red\n[Other]\n2 = #0000ff\n";
        let found: Vec<(String, String)> = rimpy(ini)
            .into_iter()
            .map(|k| (k.folder.to_string_lossy().into_owned(), k.colour))
            .collect();
        assert_eq!(
            found,
            [
                ("1847679158".to_string(), "#ff0000".to_string()),
                ("Local Mod".to_string(), "#00ff00".to_string())
            ]
        );
    }

    #[test]
    fn colours_are_told_from_other_text_with_or_without_alpha() {
        assert_eq!(rgb("#ff8000"), Some([255, 128, 0]));
        assert_eq!(rgb("#80ff8000"), Some([255, 128, 0]));
        assert_eq!(rgb("#ff80"), None);
        assert_eq!(rgb("red"), None);
    }

    #[test]
    fn rimsort_tags_notes_and_colours_are_read_and_bare_rows_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("aux_metadata.db");
        let made = rusqlite::Connection::open(&db).unwrap();
        made.execute_batch(
            "CREATE TABLE auxiliary_metadata (path VARCHAR PRIMARY KEY, user_notes VARCHAR, color_hex VARCHAR);
             CREATE TABLE mod_tags (id INTEGER PRIMARY KEY, tag VARCHAR UNIQUE);
             CREATE TABLE tags_table (left_id INTEGER, right_id INTEGER);
             INSERT INTO auxiliary_metadata VALUES ('/w/294100/1', 'breaks saves', '#ff0000'),
                 ('/w/294100/2', '', NULL), ('/w/294100/3', '', NULL);
             INSERT INTO mod_tags VALUES (1, 'must'), (2, 'qol');
             INSERT INTO tags_table VALUES ('/w/294100/3', 1), ('/w/294100/3', 2);",
        )
        .unwrap();
        drop(made);
        let mut found = rimsort(&db).unwrap();
        found.sort_by(|a, b| a.folder.cmp(&b.folder));
        assert_eq!(
            found,
            [
                Kept {
                    folder: "/w/294100/1".into(),
                    tags: vec![],
                    note: "breaks saves".into(),
                    colour: "#ff0000".into()
                },
                Kept {
                    folder: "/w/294100/3".into(),
                    tags: vec!["must".into(), "qol".into()],
                    note: String::new(),
                    colour: String::new()
                },
            ]
        );
        assert!(rimsort(&dir.path().join("missing.db")).is_err());
    }
}
