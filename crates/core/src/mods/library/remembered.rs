//! What the last scan read from each mod's files, kept so the next start reads again only
//! files whose size or time changed. Any doubt reads afresh: no file, a damaged one, another
//! format or another game version, a file written too recently to be told apart.

use crate::base::error::Error;
use crate::rimworld::about::About;
use crate::rimworld::code::Code;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

/// Raised whenever what is kept changes shape or meaning.
const FORMAT: u32 = 1;

/// A write this recent may be followed by another within the time's precision, unseen.
const SETTLED: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    size: u64,
    secs: u64,
    nanos: u32,
}

impl Stamp {
    /// `None` for a file not there, or written too recently to trust its time.
    pub fn of(file: &Path) -> Option<Self> {
        let meta = std::fs::metadata(file).ok()?;
        let modified = meta.modified().ok()?;
        let age = SystemTime::now().duration_since(modified);
        if age.is_ok_and(|age| age >= SETTLED) {
            let since = modified.duration_since(SystemTime::UNIX_EPOCH).ok()?;
            return Some(Self {
                size: meta.len(),
                secs: since.as_secs(),
                nanos: since.subsec_nanos(),
            });
        }
        None
    }
}

/// Stamps of every file, in order; `None` when one cannot be stamped.
pub fn stamps(files: &[PathBuf]) -> Option<Vec<(String, Stamp)>> {
    let stamp = |f: &PathBuf| Some((f.to_str()?.to_string(), Stamp::of(f)?));
    files.iter().map(stamp).collect()
}

/// What was read from `file`, stamped just before it was read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Stamped<T> {
    file: String,
    stamp: Stamp,
    read: T,
}

impl<T: Clone> Stamped<T> {
    fn new(file: &Path, stamp: Option<Stamp>, read: &T) -> Option<Self> {
        Some(Self {
            file: file.to_str()?.to_string(),
            stamp: stamp?,
            read: read.clone(),
        })
    }

    fn fits(&self, file: &Path, stamp: Option<Stamp>) -> bool {
        Some(self.file.as_str()) == file.to_str() && Some(self.stamp) == stamp
    }
}

#[derive(Debug, Default)]
pub struct Tally {
    about: AtomicUsize,
    code: AtomicUsize,
    changed: AtomicBool,
}

impl Tally {
    /// About.xml files read and parsed.
    pub fn about(&self) -> usize {
        self.about.load(Ordering::Relaxed)
    }

    /// Mods whose dlls were read.
    pub fn code(&self) -> usize {
        self.code.load(Ordering::Relaxed)
    }

    /// Something kept differs from what was kept before.
    pub fn changed(&self) -> bool {
        self.changed.load(Ordering::Relaxed)
    }

    fn change(&self) {
        self.changed.store(true, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct KeptCode {
    /// Every dll `code::read` was given, in order.
    dlls: Vec<(String, Stamp)>,
    code: Code,
}

/// One mod folder's files. A broken About.xml is not kept: it is read again each time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    about: Option<Stamped<About>>,
    named: Option<Stamped<Option<String>>>,
    code: Option<KeptCode>,
}

fn replace<T: PartialEq>(slot: &mut Option<T>, now: Option<T>, tally: &Tally) {
    if *slot != now {
        tally.change();
        *slot = now;
    }
}

impl Entry {
    /// `None` for a mod with no About.xml.
    pub fn about(
        &mut self,
        file: Option<&Path>,
        tally: &Tally,
        read: impl FnOnce(&Path) -> Result<About, Error>,
    ) -> Option<Result<About, Error>> {
        let Some(file) = file else {
            replace(&mut self.about, None, tally);
            return None;
        };
        let stamp = Stamp::of(file);
        if let Some(kept) = self.about.as_ref().filter(|k| k.fits(file, stamp)) {
            return Some(Ok(kept.read.clone()));
        }
        tally.about.fetch_add(1, Ordering::Relaxed);
        let about = read(file);
        let now = about
            .as_ref()
            .ok()
            .and_then(|a| Stamped::new(file, stamp, a));
        replace(&mut self.about, now, tally);
        Some(about)
    }

    /// What `About/PublishedFileId.txt` says; `None` without one.
    pub fn named(
        &mut self,
        file: Option<&Path>,
        tally: &Tally,
        read: impl FnOnce(&Path) -> Option<String>,
    ) -> Option<String> {
        let Some(file) = file else {
            replace(&mut self.named, None, tally);
            return None;
        };
        let stamp = Stamp::of(file);
        if let Some(kept) = self.named.as_ref().filter(|k| k.fits(file, stamp)) {
            return kept.read.clone();
        }
        let named = read(file);
        replace(&mut self.named, Stamped::new(file, stamp, &named), tally);
        named
    }

    /// The code read before from these very dlls.
    pub fn kept_code(&self, dlls: Option<&[(String, Stamp)]>) -> Option<&Code> {
        let kept = self.code.as_ref()?;
        (Some(kept.dlls.as_slice()) == dlls).then_some(&kept.code)
    }

    /// `dlls`: `None` keeps nothing.
    pub fn keep_code(&mut self, dlls: Option<Vec<(String, Stamp)>>, code: &Code, tally: &Tally) {
        tally.code.fetch_add(1, Ordering::Relaxed);
        let now = dlls.map(|dlls| KeptCode {
            dlls,
            code: code.clone(),
        });
        replace(&mut self.code, now, tally);
    }

    pub fn forget_code(&mut self, tally: &Tally) {
        replace(&mut self.code, None, tally);
    }
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Remembered {
    format: u32,
    game_version: String,
    /// The game's `Assembly-CSharp.dll` all kept code was read against.
    game_code: Option<Stamped<()>>,
    /// By mod folder.
    mods: HashMap<String, Entry>,
}

impl Remembered {
    /// Nothing kept.
    pub fn fresh(game_version: &str) -> Self {
        Self {
            format: FORMAT,
            game_version: game_version.to_string(),
            ..Self::default()
        }
    }

    /// Whole or not at all: anything amiss with the file reads every mod afresh.
    pub fn read(file: &Path, game_version: &str) -> Self {
        let bytes = match std::fs::read(file) {
            Ok(bytes) => bytes,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    crate::problem!("{}: {e}; mods read afresh", file.display());
                }
                return Self::fresh(game_version);
            }
        };
        match serde_json::from_slice::<Self>(&bytes) {
            Ok(kept) if kept.format == FORMAT && kept.game_version == game_version => kept,
            Ok(_) => Self::fresh(game_version),
            Err(e) => {
                crate::problem!("{}: {e}; mods read afresh", file.display());
                Self::fresh(game_version)
            }
        }
    }

    pub fn write(&self, file: &Path) -> Result<(), Error> {
        let json = serde_json::to_vec(self).map_err(|e| Error::file(file, e))?;
        crate::base::atomic_file::write(file, json)
    }

    /// Whether kept code was read against `dll` as it is now; when not, all of it is forgotten.
    pub fn same_game(&mut self, dll: Option<&Path>, tally: &Tally) -> bool {
        let now = dll.and_then(|f| Stamped::new(f, Stamp::of(f), &()));
        if now.is_some() && now == self.game_code {
            return true;
        }
        replace(&mut self.game_code, now, tally);
        for entry in self.mods.values_mut() {
            entry.forget_code(tally);
        }
        false
    }

    /// Code is kept only against a known game.
    pub fn game_known(&self) -> bool {
        self.game_code.is_some()
    }

    pub fn take(&mut self, folder: &Path) -> Entry {
        let kept = folder.to_str().and_then(|key| self.mods.remove(key));
        kept.unwrap_or_default()
    }

    /// What to keep for the next start: these folders' entries. Folders not taken are gone.
    pub fn next<'a>(
        mut self,
        entries: impl IntoIterator<Item = (&'a Path, Entry)>,
        tally: &Tally,
    ) -> Self {
        if !self.mods.is_empty() {
            tally.change();
        }
        self.mods = (entries.into_iter())
            .filter_map(|(folder, entry)| Some((folder.to_str()?.to_string(), entry)))
            .collect();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::library::{Library, load};
    use crate::rimworld::dotnet::build::Builder;
    use crate::rimworld::paths::Paths;
    use std::fs;

    /// Written a minute ago: older than `SETTLED`.
    fn put(file: &Path, contents: impl AsRef<[u8]>) {
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, contents).unwrap();
        let earlier = SystemTime::now() - Duration::from_secs(60);
        let opened = fs::File::options().write(true).open(file).unwrap();
        opened.set_modified(earlier).unwrap();
    }

    fn about(id: &str, name: &str) -> String {
        format!("<ModMetaData><packageId>{id}</packageId><name>{name}</name></ModMetaData>")
    }

    fn mod_code(type_name: &str) -> Vec<u8> {
        let mut b = Builder::new();
        b.reference("Assembly-CSharp");
        let thing = b.type_ref("Verse", "Thing");
        b.type_def("Ann", type_name, thing);
        b.finish()
    }

    /// A game, its Core, a mod with code and a Workshop mod without.
    fn installed() -> (tempfile::TempDir, Paths, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let mut game = Builder::new();
        game.type_def("Verse", "Thing", 0);
        put(&root.join("game/Assembly-CSharp.dll"), game.finish());
        put(
            &root.join("game/Data/Core/About/About.xml"),
            about("Ludeon.RimWorld", "Core"),
        );
        put(
            &root.join("game/Mods/guns/About/About.xml"),
            about("Ann.Guns", "Guns"),
        );
        put(
            &root.join("game/Mods/guns/Assemblies/Guns.dll"),
            mod_code("Gun"),
        );
        put(
            &root.join("shop/700/About/About.xml"),
            about("Ann.Hats", "Hats"),
        );
        put(&root.join("shop/700/About/PublishedFileId.txt"), "700\n");
        put(
            &root.join("config/Config/ModsConfig.xml"),
            "<ModsConfigData><activeMods><li>ludeon.rimworld</li></activeMods></ModsConfigData>",
        );
        let paths = Paths {
            game: Some(root.join("game")),
            config: Some(root.join("config")),
            workshop: Some(root.join("shop")),
            game_version: Some("1.6".into()),
        };
        let kept = root.join("data/mods-read.json");
        (tmp, paths, kept)
    }

    /// What the files said, by mod folder in the library's order.
    type Read = Vec<(String, Result<About, Error>, Option<String>, Code)>;

    fn read(library: &Library) -> Read {
        let each = library.installed.iter();
        each.map(|m| {
            (
                m.folder.clone(),
                m.about.clone(),
                m.named.clone(),
                m.code.clone(),
            )
        })
        .collect()
    }

    fn loaded(paths: &Paths, kept: Option<&Path>) -> (Library, Tally) {
        let tally = Tally::default();
        (
            super::super::load_counted(paths, kept, &tally).unwrap(),
            tally,
        )
    }

    #[test]
    fn a_second_scan_reads_nothing_unchanged_and_finds_the_same() {
        let (_tmp, paths, kept) = installed();
        let fresh = load(&paths, None).unwrap();
        assert_ne!(
            fresh.installed[1].code,
            Code::default(),
            "the mod's code is read"
        );
        let (first, tally) = loaded(&paths, Some(&kept));
        assert_eq!((tally.about(), tally.code()), (3, 1));
        assert!(kept.is_file());
        let (again, tally) = loaded(&paths, Some(&kept));
        assert_eq!(
            (tally.about(), tally.code(), tally.changed()),
            (0, 0, false)
        );
        assert_eq!(read(&again), read(&fresh));
        assert_eq!(read(&first), read(&fresh));
    }

    #[test]
    fn a_changed_file_is_read_again_and_only_it() {
        let (tmp, paths, kept) = installed();
        loaded(&paths, Some(&kept));
        put(
            &tmp.path().join("shop/700/About/About.xml"),
            about("Ann.Hats", "Hats Two"),
        );
        put(
            &tmp.path().join("shop/700/About/PublishedFileId.txt"),
            "701\n",
        );
        put(
            &tmp.path().join("game/Mods/guns/Assemblies/Guns.dll"),
            mod_code("Rifle"),
        );
        let (library, tally) = loaded(&paths, Some(&kept));
        assert_eq!((tally.about(), tally.code()), (1, 1));
        let hats = library
            .installed
            .iter()
            .find(|m| m.folder == "700")
            .unwrap();
        assert_eq!(hats.display_name(), "Hats Two");
        assert_eq!(hats.named.as_deref(), Some("701"));
        assert_eq!(read(&library), read(&load(&paths, None).unwrap()));
    }

    #[test]
    fn a_file_just_written_is_never_taken_from_what_was_kept() {
        let (tmp, paths, kept) = installed();
        loaded(&paths, Some(&kept));
        // Same size, and a time the file system may not tell apart from the last.
        let file = tmp.path().join("shop/700/About/About.xml");
        fs::write(&file, about("Ann.Hats", "Hats 2")).unwrap();
        let (library, tally) = loaded(&paths, Some(&kept));
        assert_eq!(tally.about(), 1);
        let hats = library
            .installed
            .iter()
            .find(|m| m.folder == "700")
            .unwrap();
        assert_eq!(hats.display_name(), "Hats 2");
    }

    #[test]
    fn another_game_reads_all_code_again_and_another_version_everything() {
        let (tmp, paths, kept) = installed();
        loaded(&paths, Some(&kept));
        let mut game = Builder::new();
        game.type_def("Verse", "Thing", 0);
        game.type_def("Verse", "Pawn", 0);
        put(&tmp.path().join("game/Assembly-CSharp.dll"), game.finish());
        let (_, tally) = loaded(&paths, Some(&kept));
        assert_eq!((tally.about(), tally.code()), (0, 1));
        let newer = Paths {
            game_version: Some("1.7".into()),
            ..paths.clone()
        };
        let (_, tally) = loaded(&newer, Some(&kept));
        assert_eq!((tally.about(), tally.code()), (3, 1));
    }

    #[test]
    fn without_the_game_s_code_no_code_is_kept() {
        let (tmp, paths, kept) = installed();
        fs::remove_file(tmp.path().join("game/Assembly-CSharp.dll")).unwrap();
        let (library, _) = loaded(&paths, Some(&kept));
        assert!(library.installed.iter().all(|m| m.code == Code::default()));
        let remembered = Remembered::read(&kept, "1.6");
        assert!(remembered.mods.values().all(|e| e.code.is_none()));
        assert!(remembered.mods.values().any(|e| e.about.is_some()));
    }

    #[test]
    fn a_damaged_or_foreign_file_is_set_aside_whole() {
        let (_tmp, paths, kept) = installed();
        let fresh = read(&load(&paths, None).unwrap());
        loaded(&paths, Some(&kept));
        let whole = fs::read_to_string(&kept).unwrap();
        let older = whole.replacen(&format!("\"format\":{FORMAT}"), "\"format\":0", 1);
        assert_ne!(older, whole);
        for damaged in ["", "{", "[1,2]", &whole[..whole.len() / 2], &older] {
            fs::write(&kept, damaged).unwrap();
            let (library, tally) = loaded(&paths, Some(&kept));
            assert_eq!((tally.about(), tally.code()), (3, 1), "{damaged:.20}");
            assert_eq!(read(&library), fresh);
            assert!(
                Remembered::read(&kept, "1.6").game_known(),
                "written whole again"
            );
        }
    }

    #[test]
    fn a_broken_about_is_read_each_time_and_a_gone_folder_is_forgotten() {
        let (tmp, paths, kept) = installed();
        put(
            &tmp.path().join("game/Mods/guns/About/About.xml"),
            "<ModMetaData><name>",
        );
        loaded(&paths, Some(&kept));
        let (library, tally) = loaded(&paths, Some(&kept));
        assert_eq!(tally.about(), 1);
        assert_eq!(library.unreadable, 1);
        fs::remove_dir_all(tmp.path().join("shop/700")).unwrap();
        loaded(&paths, Some(&kept));
        let remembered = Remembered::read(&kept, "1.6");
        assert_eq!(remembered.mods.len(), 2);
    }
}
