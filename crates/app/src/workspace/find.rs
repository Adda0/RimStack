//! Looking for text inside the files of installed mods, off the window's thread.

use super::App;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::{say, say_n};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Shown hits stop here; the search then says it is capped.
const MOST_HITS: usize = 1000;
/// Bigger files are skipped: an archive gladly names itself like a text file.
const MOST_BYTES: u64 = 8 * 1024 * 1024;
/// Text endings besides XML, when looking wider.
const TEXT_ENDINGS: [&str; 4] = ["xml", "txt", "json", "md"];
const PROGRESS_EVERY: usize = 200;
const BATCH_OF: usize = 50;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub mod_id: ModId,
    pub mod_name: String,
    /// The mod's folder, absolute; the window opens it.
    pub folder: String,
    /// Inside the mod's folder, with slashes.
    pub file: String,
    /// First line is 1.
    pub line: usize,
    /// The line the match is on, trimmed.
    pub text: String,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Found {
    pub files: usize,
    pub hits: usize,
    /// Stopped at [`MOST_HITS`].
    pub capped: bool,
    pub stopped: bool,
}

/// The text to look for, folded once if the case does not matter or built as a pattern.
pub enum Needle {
    Plain { folded: String, match_case: bool },
    Pattern(regex::Regex),
}

impl Needle {
    pub fn make(text: &str, pattern: bool, match_case: bool) -> Result<Needle, Sentence> {
        let text = text.trim();
        if !pattern {
            return Ok(Needle::Plain {
                folded: match match_case {
                    true => text.to_string(),
                    false => text.to_lowercase(),
                },
                match_case,
            });
        }
        match regex::RegexBuilder::new(text)
            .case_insensitive(!match_case)
            .build()
        {
            Ok(pattern) => Ok(Needle::Pattern(pattern)),
            Err(why) => {
                let why = why.to_string();
                let why = why.lines().next().unwrap_or(&why).to_string();
                Err(say!("The text is not a regular expression: {}", why))
            }
        }
    }

    fn hits(&self, line: &str) -> bool {
        match self {
            Needle::Pattern(pattern) => pattern.is_match(line),
            Needle::Plain { folded, match_case } => match match_case {
                true => line.contains(folded),
                false => line.to_lowercase().contains(folded.as_str()),
            },
        }
    }
}

/// Walks the mods' folders (id, name, folder) for files the needle hits; batches of
/// hits go to `hits`, the count of files looked into goes to `progress` every 200.
pub fn files(
    roots: &[(ModId, String, PathBuf)],
    needle: &Needle,
    xml_only: bool,
    stop: &AtomicBool,
    hits: &(dyn Fn(&[Hit]) + Sync),
    progress: &(dyn Fn(usize) + Sync),
) -> Found {
    let mut found = Found::default();
    let mut batch: Vec<Hit> = Vec::new();
    let flush = |batch: &mut Vec<Hit>| {
        if !batch.is_empty() {
            hits(batch);
            batch.clear();
        }
    };
    'mods: for (id, name, folder) in roots {
        let mut pending = vec![folder.clone()];
        while let Some(dir) = pending.pop() {
            if stop.load(Ordering::Relaxed) {
                found.stopped = true;
                break 'mods;
            }
            let Ok(read) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut entries: Vec<_> = read.flatten().collect();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    if !entry.file_name().to_string_lossy().starts_with('.') {
                        pending.push(entry.path());
                    }
                    continue;
                }
                if !kind.is_file() {
                    continue;
                }
                let ending = entry.file_name();
                let ending = ending.to_string_lossy();
                let ending = ending
                    .rsplit_once('.')
                    .map(|(_, ending)| ending)
                    .unwrap_or("");
                let wanted = match xml_only {
                    true => ending.eq_ignore_ascii_case("xml"),
                    false => TEXT_ENDINGS.iter().any(|e| ending.eq_ignore_ascii_case(e)),
                };
                if !wanted {
                    continue;
                }
                let path = entry.path();
                if !size_fits(&path) {
                    continue;
                }
                found.files += 1;
                if found.files.is_multiple_of(PROGRESS_EVERY) {
                    progress(found.files);
                }
                let Ok(content) = std::fs::read(&path) else {
                    continue;
                };
                let content = String::from_utf8_lossy(&content);
                let content = content.trim_start_matches('\u{feff}');
                for (at, line) in content.lines().enumerate() {
                    if stop.load(Ordering::Relaxed) {
                        found.stopped = true;
                        break 'mods;
                    }
                    if !needle.hits(line) {
                        continue;
                    }
                    batch.push(Hit {
                        mod_id: *id,
                        mod_name: name.clone(),
                        folder: folder.to_string_lossy().into_owned(),
                        file: relative(folder, &path),
                        line: at + 1,
                        text: line.trim().to_string(),
                    });
                    found.hits += 1;
                    if found.hits >= MOST_HITS {
                        found.capped = true;
                        flush(&mut batch);
                        return found;
                    }
                    if batch.len() >= BATCH_OF {
                        flush(&mut batch);
                    }
                }
            }
        }
    }
    flush(&mut batch);
    found
}

fn size_fits(path: &Path) -> bool {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(u64::MAX) <= MOST_BYTES
}

fn relative(folder: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(folder).unwrap_or(path);
    let parts: Vec<String> = rel
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    parts.join("/")
}

/// Every hit as one sentence: the count, or why the run ended short.
pub fn said_of(found: &Found) -> Sentence {
    let n = found.hits;
    match (found.stopped, found.capped, n) {
        (true, _, _) => say_n!(n, "Stopped; {} hit found", "Stopped; {} hits found"),
        (false, true, _) => say!(
            "Only the first {} hits are shown; narrow the search",
            MOST_HITS
        ),
        (false, false, 0) => say!("Nothing found"),
        (false, false, _) => say_n!(n, "{} hit", "{} hits"),
    }
}

pub fn searching(files: usize) -> Sentence {
    say_n!(files, "{} file searched", "{} files searched")
}

impl App {
    /// The folders looked into: the active mods normally, every installed one when `every`.
    pub fn file_search_roots(&self, every: bool) -> Vec<(ModId, String, PathBuf)> {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return Vec::new();
        };
        let active = self.active_now(library);
        library
            .ids()
            .filter(|&(id, _)| every || active.contains(&id))
            .map(|(id, m)| (id, m.display_name().to_string(), m.path.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn mods(root: &Path) -> Vec<(ModId, String, PathBuf)> {
        let write = |rel: &str, text: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(
            "hats/Defs/Hats.xml",
            "<Defs>\n<ThingDef><defName>CowboyHat</defName></ThingDef>\n</Defs>\n",
        );
        write(
            "hats/Patches/tweaks.xml",
            "<Patch>COWBOYHAT everywhere</Patch>\n<Patch>quiet</Patch>\n",
        );
        write(
            "hats/About/About.xml",
            "<ModMetaData><packageId>Ann.Hats</packageId></ModMetaData>",
        );
        write("hats/Notes/readme.md", "A hat for a cowboyhat lover.\n");
        let big = format!(
            "cowboyhat{}",
            " ".repeat(usize::try_from(MOST_BYTES).unwrap() + 1)
        );
        write("hats/big/file.xml", &big);
        write(
            "roads/Defs/Roads.xml",
            "<Defs><ThingDef><defName>DirtRoad</defName></ThingDef></Defs>\n",
        );
        write("hats/.git/config", "cowboyhat but hidden\n");
        write("hats/bin/picture.png", "cowboyhat in a picture\n");
        vec![
            (
                ModId::from_qml(1).unwrap(),
                "Hats".to_string(),
                root.join("hats"),
            ),
            (
                ModId::from_qml(2).unwrap(),
                "Roads".to_string(),
                root.join("roads"),
            ),
        ]
    }

    fn looked(
        roots: &[(ModId, String, PathBuf)],
        needle: &Needle,
        xml_only: bool,
    ) -> (Vec<Hit>, Found) {
        let (send, got) = std::sync::mpsc::channel::<Vec<Hit>>();
        let found = files(
            roots,
            needle,
            xml_only,
            &AtomicBool::new(false),
            &|batch| {
                let _ = send.send(batch.to_vec());
            },
            &|_| {},
        );
        (got.try_iter().flatten().collect(), found)
    }

    #[test]
    fn plain_text_finds_the_line_in_xml_files() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = mods(tmp.path());
        let (hits, found) = looked(
            &roots,
            &Needle::make("DirtRoad", false, false).unwrap(),
            true,
        );
        assert_eq!(
            hits,
            vec![Hit {
                mod_id: ModId::from_qml(2).unwrap(),
                mod_name: "Roads".to_string(),
                folder: tmp.path().join("roads").to_string_lossy().into_owned(),
                file: "Defs/Roads.xml".to_string(),
                line: 1,
                text: "<Defs><ThingDef><defName>DirtRoad</defName></ThingDef></Defs>".to_string(),
            }]
        );
        assert_eq!(
            found,
            Found {
                files: 4,
                hits: 1,
                capped: false,
                stopped: false
            }
        );
    }

    #[test]
    fn without_the_case_set_both_spellings_match() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = mods(tmp.path());
        let (hits, found) = looked(
            &roots,
            &Needle::make("cowboyhat", false, false).unwrap(),
            true,
        );
        assert_eq!((hits.len(), found.hits), (2, 2));
        assert!(hits.iter().all(|hit| hit.mod_name == "Hats"));
    }

    #[test]
    fn with_the_case_set_only_the_same_spelling_matches() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = mods(tmp.path());
        let (hits, found) = looked(
            &roots,
            &Needle::make("cowboyhat", false, true).unwrap(),
            true,
        );
        assert_eq!(found.hits, 0);
        assert!(hits.is_empty());
        let (hits, _) = looked(
            &roots,
            &Needle::make("CowboyHat", false, true).unwrap(),
            true,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 2);
    }

    #[test]
    fn a_pattern_matches_against_each_line() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = mods(tmp.path());
        let (hits, found) = looked(
            &roots,
            &Needle::make(r"<defName>\w+Hat</defName>", true, true).unwrap(),
            true,
        );
        assert_eq!(found.hits, 1);
        assert_eq!(hits[0].file, "Defs/Hats.xml");
    }

    #[test]
    fn a_pattern_that_is_none_says_so() {
        let refused = Needle::make("<[", true, true)
            .err()
            .expect("a bad pattern is refused");
        assert!(matches!(refused, Sentence::Said { .. }));
    }

    #[test]
    fn looking_wider_takes_every_text_file_but_never_pictures() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = mods(tmp.path());
        let (hits, found) = looked(
            &roots,
            &Needle::make("cowboyhat", false, false).unwrap(),
            false,
        );
        assert!(hits.iter().any(|hit| hit.file == "Notes/readme.md"));
        assert!(!hits.iter().any(|hit| hit.file == "bin/picture.png"));
        assert!(!hits.iter().any(|hit| hit.file.contains(".git")));
        assert!(!hits.iter().any(|hit| hit.file == "big/file.xml"));
        assert_eq!(found.hits, 3);
    }

    #[test]
    fn the_stop_flag_ends_the_walk() {
        let tmp = tempfile::tempdir().unwrap();
        for n in 0..60 {
            std::fs::create_dir_all(tmp.path().join(format!("many/Def{n}"))).unwrap();
            std::fs::write(
                tmp.path().join(format!("many/Def{n}/Things{n}.xml")),
                "needle <needle/>\n",
            )
            .unwrap();
        }
        let roots = vec![(
            ModId::from_qml(1).unwrap(),
            "Many".to_string(),
            tmp.path().join("many"),
        )];
        let stop = AtomicBool::new(false);
        let hits = std::sync::atomic::AtomicUsize::new(0);
        let found = files(
            &roots,
            &Needle::make("needle", false, false).unwrap(),
            true,
            &stop,
            &|batch| {
                hits.fetch_add(batch.len(), Ordering::Relaxed);
                stop.store(true, Ordering::Relaxed);
            },
            &|_| {},
        );
        assert!(found.stopped);
        assert!((BATCH_OF..60).contains(&hits.load(Ordering::Relaxed)));
    }

    #[test]
    fn the_run_reports_what_it_saw() {
        assert_eq!(
            said_of(&Found {
                files: 3,
                hits: 0,
                capped: false,
                stopped: false
            }),
            say!("Nothing found")
        );
        assert!(matches!(
            said_of(&Found {
                files: 3,
                hits: 2,
                capped: false,
                stopped: false
            }),
            Sentence::Counted { .. }
        ));
        assert!(matches!(
            said_of(&Found {
                files: 3,
                hits: 2,
                capped: true,
                stopped: false
            }),
            Sentence::Said { .. }
        ));
        assert!(matches!(
            said_of(&Found {
                files: 3,
                hits: 2,
                capped: false,
                stopped: true
            }),
            Sentence::Counted { .. }
        ));
    }

    #[test]
    fn the_active_mods_are_the_default_root_of_the_search() {
        let (_tmp, app) = crate::workspace::tests::installed();
        let roots = app.file_search_roots(false);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].1, "Ludeon.RimWorld");
        assert!(app.file_search_roots(true).len() > 1);
    }
}
