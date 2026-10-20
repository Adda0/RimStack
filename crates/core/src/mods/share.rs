use crate::mods::library::Installed;
use crate::rimworld::paths::Source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub package_id: String,
    /// The Workshop item's id; `None` off the Workshop.
    pub workshop_id: Option<String>,
    pub url: Option<String>,
}

impl Entry {
    pub fn of(m: &Installed) -> Self {
        let about = m.about.as_ref().ok();
        let page = "https://steamcommunity.com/sharedfiles/filedetails/?id=".to_string();
        let workshop_id = (m.source == Source::Workshop).then(|| m.folder.clone());
        Self {
            name: m.display_name().to_string(),
            package_id: about
                .map(|a| a.package_id.to_lowercase())
                .unwrap_or_default(),
            url: about
                .and_then(|a| a.url.clone())
                .or_else(|| workshop_id.as_ref().map(|id| format!("{page}{id}"))),
            workshop_id,
        }
    }
}

/// Three header lines, then a line per mod in load order.
pub fn listing(game_version: &str, mods: &[Entry]) -> String {
    let mut text = format!(
        "Created with RimStack\nRimWorld game version this list was created for: {game_version}\nTotal # of mods: {}\n\n",
        mods.len()
    );
    for m in mods {
        let url = m.url.as_deref().unwrap_or("No url specified");
        text += &format!("{} [{}][{url}]\n", m.name, m.package_id);
    }
    text
}

/// One Workshop page a line, for the Workshop mods, in listed order.
pub fn workshop_links(entries: &[Entry]) -> String {
    let mut text = String::new();
    for id in entries.iter().filter_map(|m| m.workshop_id.as_deref()) {
        text += &format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}\n");
    }
    text
}

/// The code's start: RimStack's name, then the layout's version.
const PREFIX: &str = "rimstack-list:1:";

/// base64url without padding; only the alphabet below is taken as valid.
fn url64(bytes: &[u8]) -> String {
    const LETTERS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut text = String::new();
    for piece in bytes.chunks(3) {
        let whole = |n: usize| piece.get(n).copied().unwrap_or(0);
        let (a, b, c) = (whole(0), whole(1), whole(2));
        text.push(LETTERS[(a >> 2) as usize] as char);
        text.push(LETTERS[((a << 4 | b >> 4) & 0x3f) as usize] as char);
        if piece.len() > 1 {
            text.push(LETTERS[((b << 2 | c >> 6) & 0x3f) as usize] as char);
        }
        if piece.len() > 2 {
            text.push(LETTERS[(c & 0x3f) as usize] as char);
        }
    }
    text
}

/// `None` on a letter outside the alphabet or a length no base64 has.
fn from_url64(piece: &str) -> Option<Vec<u8>> {
    let value = |letter: u8| -> Option<u32> {
        match letter {
            b'A'..=b'Z' => Some(u32::from(letter - b'A')),
            b'a'..=b'z' => Some(u32::from(letter - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(letter - b'0') + 52),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    };
    let piece = piece.as_bytes();
    if piece.len() % 4 == 1 {
        return None;
    }
    let mut bytes = Vec::with_capacity(piece.len() / 4 * 3);
    for part in piece.chunks(4) {
        let mut gone = 0u32;
        for (at, letter) in part.iter().enumerate() {
            gone |= value(*letter)? << (18 - at * 6);
        }
        bytes.push((gone >> 16) as u8);
        if part.len() > 2 {
            bytes.push((gone >> 8) as u8);
        }
        if part.len() > 3 {
            bytes.push(gone as u8);
        }
    }
    Some(bytes)
}

/// Compressed: long lists of similar ids would make the code a page long.
fn deflate(text: &str) -> Vec<u8> {
    use std::io::Write;
    let mut to = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = to.write_all(text.as_bytes());
    to.finish().unwrap_or_default()
}

fn inflate(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut from = flate2::read::ZlibDecoder::new(bytes);
    let mut text = Vec::new();
    from.read_to_end(&mut text).ok()?;
    Some(text)
}

/// A package id a line, its Workshop id beside it when known, compressed and
/// as base64url after the prefix. What the import reads back.
pub fn code(entries: &[Entry]) -> String {
    let mut text = String::new();
    for entry in entries {
        if entry.package_id.is_empty() {
            continue;
        }
        text += &entry.package_id;
        if let Some(id) = &entry.workshop_id {
            text += &format!(" {id}");
        }
        text += "\n";
    }
    format!("{PREFIX}{}", url64(&deflate(&text)))
}

/// `(package id, Workshop id)` pairs from a code, whatever text sits around
/// it. `None` without a code or when a code present does not decode: the
/// caller then falls back to reading the text as a plain list.
pub fn decode(text: &str) -> Option<Vec<(String, Option<String>)>> {
    for piece in text.split(char::is_whitespace) {
        let Some(rest) = piece.strip_prefix("rimstack-list:") else {
            continue;
        };
        let bytes = from_url64(rest.strip_prefix("1:")?)?;
        let text = String::from_utf8(inflate(&bytes)?).ok()?;
        let mut pairs: Vec<(String, Option<String>)> = Vec::new();
        for line in text.lines() {
            let mut parts = line.split_whitespace();
            let id = crate::rimworld::id::fold(parts.next()?);
            if id.is_empty() || pairs.iter().any(|(own, _)| own == &id) {
                continue;
            }
            let workshop = parts
                .next()
                .filter(|w| crate::rimworld::about::is_workshop_id(w))
                .map(String::from);
            pairs.push((id, workshop));
        }
        return (!pairs.is_empty()).then_some(pairs);
    }
    None
}

pub fn ids_in(text: &str) -> Vec<String> {
    let shared = decode(text).map_or_else(Vec::new, |pairs| {
        pairs.into_iter().map(|(id, _)| id).collect()
    });
    let reads_as_id = |s: &str| {
        let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
        // A game version in brackets (`[1.6]`) has a dot too, and no letter.
        let lettered = s.chars().any(|c| c.is_ascii_alphabetic());
        s.contains('.') && lettered && s.chars().all(plain)
    };
    let mut ids: Vec<String> = shared;
    for line in text.lines().map(str::trim) {
        let bracketed = line
            .split('[')
            .skip(1)
            .filter_map(|part| part.split(']').next());
        let found = bracketed.map(str::trim).find(|part| reads_as_id(part));
        let id = found.or(Some(line).filter(|l| reads_as_id(l)));
        if let Some(id) = id.map(crate::rimworld::id::fold)
            && !ids.contains(&id)
        {
            ids.push(id);
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::library::scan;
    use crate::rimworld::paths::ModRoot;

    fn a_mod(root: &std::path::Path, folder: &str, about: &str) {
        let dir = root.join(folder).join("About");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("About.xml"), about).unwrap();
    }

    #[test]
    fn a_mod_is_found_at_its_author_s_page_or_on_the_workshop() {
        let tmp = tempfile::tempdir().unwrap();
        let about = |id: &str, url: &str| {
            format!(
                "<ModMetaData><name>{id}</name><packageId>Me.{id}</packageId>{url}</ModMetaData>"
            )
        };
        a_mod(tmp.path(), "111", &about("Plain", ""));
        a_mod(
            tmp.path(),
            "222",
            &about("Paged", "<url>https://example.org</url>"),
        );
        let workshop = scan(&[ModRoot {
            path: tmp.path().to_path_buf(),
            source: Source::Workshop,
        }]);
        let local = scan(&[ModRoot {
            path: tmp.path().to_path_buf(),
            source: Source::Local,
        }]);
        let page = "https://steamcommunity.com/sharedfiles/filedetails/?id=111";
        assert_eq!(Entry::of(&workshop[0]).url.as_deref(), Some(page));
        assert_eq!(
            Entry::of(&workshop[1]).url.as_deref(),
            Some("https://example.org")
        );
        assert_eq!(Entry::of(&local[0]).url, None);
        assert_eq!(Entry::of(&local[0]).package_id, "me.plain");
    }

    #[test]
    fn the_list_reads_as_rimsort_writes_it() {
        let entry = |name: &str, id: &str, url: Option<&str>| Entry {
            name: name.into(),
            package_id: id.into(),
            workshop_id: None,
            url: url.map(String::from),
        };
        let mods = [
            entry(
                "Harmony",
                "brrainz.harmony",
                Some("https://github.com/pardeike/HarmonyRimWorld"),
            ),
            entry("RimWorld", "ludeon.rimworld", None),
        ];
        assert_eq!(
            listing("1.6", &mods),
            "Created with RimStack\n\
             RimWorld game version this list was created for: 1.6\n\
             Total # of mods: 2\n\n\
             Harmony [brrainz.harmony][https://github.com/pardeike/HarmonyRimWorld]\n\
             RimWorld [ludeon.rimworld][No url specified]\n"
        );
    }

    #[test]
    fn a_share_code_reads_back_its_mods() {
        let entries = [
            Entry {
                name: "RimWorld".into(),
                package_id: "ludeon.rimworld".into(),
                workshop_id: None,
                url: None,
            },
            Entry {
                name: "Harmony".into(),
                package_id: "brrainz.harmony".into(),
                workshop_id: Some("2009463077".into()),
                url: None,
            },
        ];
        let said = code(&entries);
        assert!(said.starts_with(PREFIX));
        assert_eq!(
            decode(&said),
            Some(vec![
                ("ludeon.rimworld".to_string(), None),
                (
                    "brrainz.harmony".to_string(),
                    Some("2009463077".to_string())
                ),
            ])
        );
        assert_eq!(
            ids_in(&format!("a friend's list: {said}\nann.extra")),
            ["ludeon.rimworld", "brrainz.harmony", "ann.extra"]
        );
    }

    #[test]
    fn garbage_is_no_share_code() {
        assert_eq!(decode("just some words"), None);
        assert_eq!(decode("rimstack-list:2:AAAA"), None);
        assert_eq!(decode("rimstack-list:1:not a code!"), None);
        assert_eq!(decode("rimstack-list:1:A"), None);
        assert_eq!(decode("rimstack-list:1:aaaaaaaa"), None);
    }

    #[test]
    fn workshop_links_are_a_line_a_mod() {
        let entries = [
            Entry {
                name: "Harmony".into(),
                package_id: "brrainz.harmony".into(),
                workshop_id: Some("2009463077".into()),
                url: None,
            },
            Entry {
                name: "Local".into(),
                package_id: "ann.a".into(),
                workshop_id: None,
                url: None,
            },
        ];
        assert_eq!(
            workshop_links(&entries),
            "https://steamcommunity.com/sharedfiles/filedetails/?id=2009463077\n"
        );
    }

    #[test]
    fn a_pasted_list_gives_its_package_ids_in_order() {
        let pasted = "Created with RimSort v1.15.0\nTotal # of mods: 3\n\n\
            [CF] Tools [1.6] [Author.Tools][https://steamcommunity.com/sharedfiles/filedetails/?id=1]\n\
            RimWorld [ludeon.rimworld][No url specified]\n\
            brrainz.harmony\n\
            RimWorld [ludeon.rimworld][No url specified]\n\
            just some words\n";
        assert_eq!(
            ids_in(pasted),
            ["author.tools", "ludeon.rimworld", "brrainz.harmony"]
        );
        assert!(ids_in("nothing here\n1.6\n").is_empty());
    }
}
