//! What a mod's own files say about it: an `About.xml` that is there and parses, a package
//! id the game can take and no other installed mod has, and, for a Workshop mod, the
//! Workshop item its folder is named for.

use crate::base::error::{Error, Result};
use crate::base::sentence::Sentence;
use crate::mods::describe::{Fix, Trouble, TroubleKind};
use crate::mods::library::Installed;
use crate::rimworld::about::{self, About};
use crate::rimworld::id;
use crate::rimworld::paths::Source;
use crate::say;
use std::collections::HashMap;

/// The files the check reads: `About/About.xml` and `About/PublishedFileId.txt` alone;
/// no defs, no assemblies.
#[derive(Debug, Clone, PartialEq)]
pub struct Files {
    pub folder: String,
    pub workshop: bool,
    pub about: Result<About>,
    /// `About/PublishedFileId.txt` as written.
    pub named: Option<String>,
}

impl Files {
    /// Reads them afresh from the mod's folder; nothing else is touched.
    pub fn read(m: &Installed) -> Self {
        let about = match about::locate(&m.path) {
            None => Err(Error::format(say!("no About.xml"))),
            Some(file) => std::fs::read(&file)
                .map_err(|e| Error::file(&file, e))
                .and_then(|bytes| about::parse(&bytes)),
        };
        Self {
            folder: m.folder.clone(),
            workshop: m.source == Source::Workshop,
            about,
            named: about::published_file_id_text(&m.path),
        }
    }

    /// The files as the last scan read them, without touching the disk again.
    pub fn kept(m: &Installed) -> Self {
        Self {
            folder: m.folder.clone(),
            workshop: m.source == Source::Workshop,
            about: m.about.clone(),
            named: m.named.clone(),
        }
    }
}

/// How many installed mods name each folded package id.
pub fn package_ids(mods: &[Installed]) -> HashMap<String, usize> {
    count(mods.iter().filter_map(|m| m.about.as_ref().ok()))
}

fn count<'a>(abouts: impl Iterator<Item = &'a About>) -> HashMap<String, usize> {
    let mut ids = HashMap::new();
    for about in abouts {
        *ids.entry(id::fold(&about.package_id)).or_insert(0) += 1;
    }
    ids
}

/// The problems in each mod's own files, in the order of `files`; `ids` counts the folded
/// package ids across all installed mods.
pub fn troubles(files: &[Files], ids: &HashMap<String, usize>) -> Vec<Vec<Trouble>> {
    files.iter().map(|f| trouble(f, ids)).collect()
}

/// Reads every mod's own files afresh and counts them: (checked, with a problem).
pub fn check(mods: &[Installed]) -> (usize, usize) {
    let files: Vec<Files> = mods.iter().map(Files::read).collect();
    let ids = count(files.iter().filter_map(|f| f.about.as_ref().ok()));
    let found = troubles(&files, &ids);
    let bad = found.iter().filter(|own| !own.is_empty()).count();
    (files.len(), bad)
}

fn trouble(f: &Files, ids: &HashMap<String, usize>) -> Vec<Trouble> {
    let mut found = Vec::new();
    let about = match &f.about {
        Ok(about) => about,
        Err(why) => {
            found.push(Trouble {
                error: true,
                words: say!(
                    "Its About.xml is missing or broken, so the game cannot read it as a mod: {}",
                    why.sentence()
                ),
                items: Vec::new(),
                short: say!("bad About.xml"),
                fix: Fix::None,
                key: "about-broken".to_string(),
                kind: TroubleKind::AboutBroken,
            });
            return found;
        }
    };
    if !package_id_ok(&about.package_id) {
        found.push(Trouble {
            error: true,
            words: say!("Its package id {} is not one the game can take: it needs letters and a dot, nothing else.", &about.package_id),
            items: Vec::new(),
            short: say!("bad package id"),
            fix: Fix::None,
            key: "bad-package-id".to_string(),
            kind: TroubleKind::BadPackageId,
        });
    } else if ids.get(&id::fold(&about.package_id)).copied().unwrap_or(0) > 1 {
        found.push(Trouble {
            error: false,
            words: say!(
                "Its package id {} is another installed mod's; the game reads only one of them.",
                &about.package_id
            ),
            items: Vec::new(),
            short: say!("an id another mod has"),
            fix: Fix::None,
            key: "duplicate-id".to_string(),
            kind: TroubleKind::Duplicate,
        });
    }
    if !f.workshop {
        return found;
    }
    // A Workshop folder is the Workshop id; its files must name the same item, so a Steam
    // mix-up (another item's files in this folder) shows.
    let offenders: Vec<Sentence> = [
        ("About/PublishedFileId.txt", f.named.as_ref()),
        ("About.xml", about.published_file_id.as_ref()),
    ]
    .into_iter()
    .filter_map(|(file, named)| named.map(|n| (file, n)))
    .filter(|(_, named)| *named != &f.folder)
    .map(|(file, named)| say!("{}: {}", file, named))
    .collect();
    if !offenders.is_empty() {
        found.push(Trouble {
            error: true,
            words: say!("Its files name another Workshop item than the folder it is in."),
            items: offenders,
            short: say!("names another Workshop item"),
            fix: Fix::None,
            key: "wrong-workshop-id".to_string(),
            kind: TroubleKind::WrongWorkshopId,
        });
    }
    found
}

/// The shape the game takes: letters and a dot, beside digits, `-` and `_`.
fn package_id_ok(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && id.contains('.')
        && id.chars().any(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::library;
    use crate::rimworld::paths::ModRoot;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn xml(package: &str, published: Option<&str>) -> String {
        let named = published
            .map(|p| format!("<publishedFileId>{p}</publishedFileId>"))
            .unwrap_or_default();
        format!("<ModMetaData><packageId>{package}</packageId>{named}</ModMetaData>")
    }

    fn add(
        root: &Path,
        folder: &str,
        package: &Option<&str>,
        txt: Option<&str>,
        published: Option<&str>,
    ) {
        if let Some(package) = package {
            fs::create_dir_all(root.join(folder).join("About")).unwrap();
            fs::write(
                root.join(folder).join("About/About.xml"),
                xml(package, published),
            )
            .unwrap();
            if let Some(txt) = txt {
                fs::write(
                    root.join(folder).join("About/PublishedFileId.txt"),
                    format!("{txt}\n"),
                )
                .unwrap();
            }
        } else {
            fs::create_dir_all(root.join(folder)).unwrap();
        }
    }

    /// folder, About.xml's package id (None: no About.xml), PublishedFileId.txt, its <publishedFileId>.
    type Made<'a> = (
        &'a str,
        &'a Option<&'a str>,
        Option<&'a str>,
        Option<&'a str>,
    );

    fn scan_them(mods: Vec<Made<'_>>) -> (TempDir, Vec<Installed>) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("workshop");
        for (folder, package, txt, published) in mods {
            add(&root, folder, package, txt, published);
        }
        (
            tmp,
            library::scan(&[ModRoot {
                path: root,
                source: Source::Workshop,
            }]),
        )
    }

    fn keys(found: &[Trouble]) -> Vec<&str> {
        found.iter().map(|t| t.key.as_str()).collect()
    }

    #[test]
    fn a_folder_whose_published_file_id_names_another_item_is_flagged() {
        let good = Some("ann.mod");
        let (_tmp, mods) = scan_them(vec![("123", &good, Some("456"), None)]);
        let ids = package_ids(&mods);
        let found = troubles(&mods.iter().map(Files::kept).collect::<Vec<_>>(), &ids);
        assert_eq!(keys(&found[0]), ["wrong-workshop-id"]);
        assert_eq!(found[0][0].kind, TroubleKind::WrongWorkshopId);
        assert!(found[0][0].error);
    }

    #[test]
    fn a_matching_published_file_id_is_not_flagged() {
        let good = Some("ann.mod");
        let other = Some("ann.other");
        let (_tmp, mods) = scan_them(vec![
            ("123", &good, Some("123"), Some("123")),
            ("456", &other, Some("456"), None),
        ]);
        let ids = package_ids(&mods);
        let found = troubles(&mods.iter().map(Files::kept).collect::<Vec<_>>(), &ids);
        assert!(found.iter().all(|own| own.is_empty()), "{found:?}");
    }

    #[test]
    fn an_about_xml_naming_another_item_is_flagged_too() {
        let good = Some("ann.mod");
        let (_tmp, mods) = scan_them(vec![("123", &good, None, Some("789"))]);
        let ids = package_ids(&mods);
        let found = troubles(&mods.iter().map(Files::kept).collect::<Vec<_>>(), &ids);
        assert_eq!(keys(&found[0]), ["wrong-workshop-id"]);
    }

    #[test]
    fn a_missing_about_xml_is_flagged() {
        let (_tmp, mods) = scan_them(vec![("123", &None, None, None)]);
        let ids = package_ids(&mods);
        let found = troubles(&mods.iter().map(Files::kept).collect::<Vec<_>>(), &ids);
        assert_eq!(keys(&found[0]), ["about-broken"]);
        assert_eq!(found[0][0].kind, TroubleKind::AboutBroken);
    }

    #[test]
    fn a_package_id_the_game_cannot_take_is_flagged() {
        let bad = Some("bad id");
        let (_tmp, mods) = scan_them(vec![("123", &bad, None, None)]);
        let ids = package_ids(&mods);
        let found = troubles(&mods.iter().map(Files::kept).collect::<Vec<_>>(), &ids);
        assert_eq!(keys(&found[0]), ["bad-package-id"]);
        assert_eq!(found[0][0].kind, TroubleKind::BadPackageId);
    }

    #[test]
    fn a_package_id_two_installed_mods_share_is_flagged_on_both() {
        let same = Some("ann.mod");
        let (_tmp, mods) = scan_them(vec![
            ("123", &same, Some("123"), None),
            ("456", &same, Some("456"), None),
        ]);
        let ids = package_ids(&mods);
        let found = troubles(&mods.iter().map(Files::kept).collect::<Vec<_>>(), &ids);
        assert_eq!(
            found.iter().map(|own| keys(own)).collect::<Vec<_>>(),
            vec![vec!["duplicate-id"], vec!["duplicate-id"]]
        );
        assert_eq!(found[0][0].kind, TroubleKind::Duplicate);
    }

    #[test]
    fn reading_afresh_counts_the_mods_whose_files_have_a_problem() {
        let good = Some("ann.mod");
        let (_tmp, mods) = scan_them(vec![
            ("123", &good, Some("123"), None),
            ("456", &Some("ann.other"), Some("999"), None),
        ]);
        assert_eq!(check(&mods), (2, 1));
    }
}
