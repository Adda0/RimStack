use crate::mods::library::Installed;
use crate::rimworld::paths::Source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub package_id: String,
    pub url: Option<String>,
}

impl Entry {
    pub fn of(m: &Installed) -> Self {
        let about = m.about.as_ref().ok();
        let workshop = || {
            let page = "https://steamcommunity.com/sharedfiles/filedetails/?id=";
            (m.source == Source::Workshop).then(|| format!("{page}{}", m.folder))
        };
        Self {
            name: m.display_name().to_string(),
            package_id: about
                .map(|a| a.package_id.to_lowercase())
                .unwrap_or_default(),
            url: about.and_then(|a| a.url.clone()).or_else(workshop),
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

pub fn ids_in(text: &str) -> Vec<String> {
    let reads_as_id = |s: &str| {
        let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
        // A game version in brackets (`[1.6]`) has a dot too, and no letter.
        let lettered = s.chars().any(|c| c.is_ascii_alphabetic());
        s.contains('.') && lettered && s.chars().all(plain)
    };
    let mut ids: Vec<String> = Vec::new();
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
