use super::{Installed, Library, ModId, Resolved, resolve_active};
use crate::base::versions::{newer, numbers};
use crate::rimworld::id;
use crate::rimworld::paths::Source;
use std::collections::HashMap;

/// The installed folders sharing one package id: a local copy beside its Workshop subscription,
/// two local folders, a Git checkout, two Workshop items naming one id. The game loads one of
/// them for each entry of the load order, as [`resolve_active`] picks, and skips the others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copies {
    /// In the library's order: the game's own, then local, then Workshop, each by folder.
    pub all: Vec<ModId>,
}

/// Why the game loads one copy of a package id over the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loaded {
    /// The game's own copy comes before any other.
    Official,
    /// The load order names the Workshop copy, by its id with `_steam`.
    Steam,
    /// A copy in the game's Mods folder comes before a Workshop one.
    LocalFirst,
    /// Of several in one place, the first by folder name.
    FirstFolder,
}

impl Loaded {
    /// Why the game reads this copy, in words.
    pub fn words(self) -> crate::base::sentence::Sentence {
        use crate::say;
        match self {
            Loaded::Official => say!("the game's own copy comes first"),
            Loaded::Steam => say!("the load order names it with \"_steam\""),
            Loaded::LocalFirst => {
                say!("a copy in the game's Mods folder comes before a Workshop one")
            }
            Loaded::FirstFolder => say!("its folder name sorts first in its folder"),
        }
    }
}

impl Library {
    /// `id` and every other installed folder of its package id; `None` when it is the only one.
    pub fn copies(&self, id: ModId) -> Option<Copies> {
        let key = self.get(id)?.key()?;
        let all: Vec<ModId> = self
            .ids()
            .filter(|(_, m)| m.key().as_ref() == Some(&key))
            .map(|(at, _)| at)
            .collect();
        (all.len() > 1).then_some(Copies { all })
    }

    /// Every package id installed more than once, by its folded id.
    pub fn duplicates(&self) -> HashMap<String, Copies> {
        let mut by_key: HashMap<String, Copies> = HashMap::new();
        for (at, m) in self.ids() {
            if let Some(key) = m.key() {
                by_key
                    .entry(key)
                    .or_insert(Copies { all: Vec::new() })
                    .all
                    .push(at);
            }
        }
        by_key.retain(|_, copies| copies.all.len() > 1);
        by_key
    }

    /// The load order's entry that makes the game load this copy: its package id, with `_steam`
    /// for a Workshop copy of a mod also installed elsewhere. `None` for a copy the game never
    /// loads, another of the same id and place coming first.
    pub fn written_for(&self, id: ModId) -> Option<String> {
        let m = self.get(id)?;
        let written = self.written_id(id.index());
        let found = resolve_active(std::slice::from_ref(&written), &self.installed);
        matches!(found.first(), Some(Resolved::Found(x)) if x.path == m.path).then_some(written)
    }

    /// Whether the game loads this copy for the order entry `written`.
    pub fn loads_for(&self, id: ModId, written: &str) -> bool {
        let Some(m) = self.get(id) else {
            return false;
        };
        let written = written.to_string();
        let found = resolve_active(std::slice::from_ref(&written), &self.installed);
        matches!(found.first(), Some(Resolved::Found(x)) if x.path == m.path)
    }

    /// The entry of `active` the game loads this copy for, if any.
    pub fn loaded_as<'a>(&self, id: ModId, active: &'a [String]) -> Option<&'a str> {
        let m = self.get(id)?;
        let found = resolve_active(active, &self.installed);
        let mut each = found.iter().zip(active);
        each.find_map(|(r, written)| match r {
            Resolved::Found(x) if x.path == m.path => Some(written.as_str()),
            _ => None,
        })
    }

    /// The copies the game reads for the order `active`, each with the entry that names it;
    /// of the entries naming one id, only the first is read, the others are ignored.
    pub fn in_use(&self, active: &[String]) -> HashMap<ModId, String> {
        let found = resolve_active(active, &self.installed);
        let mut read: HashMap<String, (ModId, String)> = HashMap::new();
        for (r, written) in found.iter().zip(active) {
            let Resolved::Found(m) = r else {
                continue;
            };
            let Some(key) = m.key() else {
                continue;
            };
            let Some(at) = self.ids().find(|(_, x)| x.path == m.path).map(|(at, _)| at) else {
                continue;
            };
            read.entry(key).or_insert_with(|| (at, written.clone()));
        }
        read.into_values().collect()
    }

    /// Whether the game still loads a copy for `written` once `id`'s folder is gone.
    pub fn another_loads(&self, id: ModId, written: &str) -> bool {
        let steam = id::split_steam(written).1;
        let others = self.copies(id).map(|c| c.all).unwrap_or_default();
        others
            .into_iter()
            .filter(|&other| other != id)
            .filter_map(|other| self.get(other))
            .any(|other| !steam || other.source == Source::Workshop)
    }

    /// Why the game loads `id` of its copies for the entry `written`.
    pub fn loaded_why(&self, id: ModId, written: &str) -> Option<Loaded> {
        let m = self.get(id)?;
        let copies = self.copies(id)?;
        let mut others = copies
            .all
            .iter()
            .filter(|&&o| o != id)
            .filter_map(|&o| self.get(o));
        let same_place = others.any(|o| o.source == m.source);
        let elsewhere = copies
            .all
            .iter()
            .filter(|&&o| o != id)
            .filter_map(|&o| self.get(o))
            .any(|o| o.source != m.source);
        let steam = id::split_steam(written).1;
        Some(match m.source {
            Source::Official => Loaded::Official,
            Source::Local if !steam && elsewhere => Loaded::LocalFirst,
            Source::Workshop if steam && elsewhere => Loaded::Steam,
            _ if same_place => Loaded::FirstFolder,
            Source::Local => Loaded::LocalFirst,
            _ => Loaded::Steam,
        })
    }

    /// Whether `copy` is a newer version of the mod than `than`.
    pub fn newer_copy(&self, than: ModId, copy: ModId) -> bool {
        match (self.get(than), self.get(copy)) {
            (Some(than), Some(copy)) => is_newer(than, copy),
            _ => false,
        }
    }
}

/// By `modVersion` when both give one and they differ, which few do; else by the latest game
/// version each supports, since an author may add a game's folder without raising it.
fn is_newer(than: &Installed, copy: &Installed) -> bool {
    let (Ok(was), Ok(is)) = (&than.about, &copy.about) else {
        return false;
    };
    if let (Some(was), Some(is)) = (&was.mod_version, &is.mod_version)
        && (newer(was, is) || newer(is, was))
    {
        return newer(was, is);
    }
    let latest = |versions: &[String]| versions.iter().max_by_key(|v| numbers(v)).cloned();
    match (
        latest(&was.supported_versions),
        latest(&is.supported_versions),
    ) {
        (Some(was), Some(is)) => newer(&was, &is),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::library::{build, scan};
    use crate::rimworld::paths::ModRoot;
    use std::path::Path;

    fn add(root: &Path, folder: &str, id: &str, extra: &str) {
        let about = root.join(folder).join("About");
        std::fs::create_dir_all(&about).unwrap();
        let xml = format!("<ModMetaData><packageId>{id}</packageId>{extra}</ModMetaData>");
        std::fs::write(about.join("About.xml"), xml).unwrap();
    }

    /// `(source, folder, id)` each, read as the game's own, local and Workshop folders.
    fn made(folders: &[(Source, &str, &str)], active: &[&str]) -> (tempfile::TempDir, Library) {
        let tmp = tempfile::tempdir().unwrap();
        let roots = [
            (tmp.path().join("Data"), Source::Official),
            (tmp.path().join("Mods"), Source::Local),
            (tmp.path().join("workshop"), Source::Workshop),
        ]
        .map(|(path, source)| ModRoot { path, source });
        for (source, folder, id) in folders {
            let root = roots.iter().find(|r| r.source == *source).unwrap();
            add(&root.path, folder, id, "");
        }
        let active: Vec<String> = active.iter().map(|a| a.to_string()).collect();
        (tmp, build(scan(&roots), &active, "1.7"))
    }

    fn library(local: &str, workshop: &str) -> (tempfile::TempDir, Library) {
        let tmp = tempfile::tempdir().unwrap();
        let roots = [
            (tmp.path().join("Mods"), Source::Local),
            (tmp.path().join("workshop"), Source::Workshop),
        ]
        .map(|(path, source)| ModRoot { path, source });
        add(&roots[0].path, "3535", "Ann.Progress", local);
        add(&roots[1].path, "3535", "ann.progress", workshop);
        add(&roots[1].path, "77", "Ann.Other", "");
        let library = build(scan(&roots), &["ann.progress".into()], "1.7");
        (tmp, library)
    }

    fn by(library: &Library, source: Source, folder: &str) -> ModId {
        let mut ids = library.ids();
        ids.find(|(_, m)| m.source == source && m.folder == folder)
            .unwrap()
            .0
    }

    fn versions(mod_version: &str, game: &[&str]) -> String {
        let game: String = game.iter().map(|v| format!("<li>{v}</li>")).collect();
        format!(
            "<modVersion>{mod_version}</modVersion><supportedVersions>{game}</supportedVersions>"
        )
    }

    #[test]
    fn every_folder_of_one_package_id_is_a_copy_whatever_its_place() {
        use Source::*;
        let (_tmp, library) = made(
            &[
                (Official, "Core", "Ludeon.RimWorld"),
                (Local, "core-copy", "ludeon.rimworld"),
                (Local, "b-hats", "Ann.Hats"),
                (Local, "a-hats", "ann.hats"),
                (Workshop, "1", "Ann.Hats"),
                (Workshop, "2", "Ann.Roads"),
                (Workshop, "3", "ann.roads"),
                (Workshop, "4", "Ann.Alone"),
            ],
            &[],
        );
        let at = |source, folder| by(&library, source, folder);
        let hats = [at(Local, "a-hats"), at(Local, "b-hats"), at(Workshop, "1")];
        for copy in hats {
            assert_eq!(library.copies(copy).unwrap().all, hats);
        }
        let roads = [at(Workshop, "2"), at(Workshop, "3")];
        assert_eq!(library.copies(roads[1]).unwrap().all, roads);
        let core = [at(Official, "Core"), at(Local, "core-copy")];
        assert_eq!(library.copies(core[1]).unwrap().all, core);
        assert_eq!(library.copies(at(Workshop, "4")), None);
        let all = library.duplicates();
        let mut keys: Vec<&str> = all.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["ann.hats", "ann.roads", "ludeon.rimworld"]);
        assert_eq!(all["ann.hats"].all, hats);
    }

    #[test]
    fn a_copy_is_named_in_the_order_the_way_the_game_then_loads_it() {
        use Source::*;
        let (_tmp, library) = made(
            &[
                (Local, "b-hats", "Ann.Hats"),
                (Local, "a-hats", "ann.hats"),
                (Workshop, "1", "Ann.Hats"),
                (Workshop, "2", "Ann.Hats"),
                (Workshop, "3", "Ann.Roads"),
                (Workshop, "4", "ann.roads"),
            ],
            &[],
        );
        let at = |source, folder| by(&library, source, folder);
        let written = |source, folder| library.written_for(at(source, folder));
        assert_eq!(written(Local, "a-hats").as_deref(), Some("ann.hats"));
        assert_eq!(written(Local, "b-hats"), None);
        assert_eq!(written(Workshop, "1").as_deref(), Some("ann.hats_steam"));
        assert_eq!(written(Workshop, "2"), None);
        assert_eq!(written(Workshop, "3").as_deref(), Some("ann.roads"));
        assert_eq!(written(Workshop, "4"), None);
    }

    #[test]
    fn the_copy_loaded_for_an_entry_is_told_with_why() {
        use Source::*;
        let (_tmp, library) = made(
            &[
                (Official, "Core", "Ludeon.RimWorld"),
                (Local, "core-copy", "ludeon.rimworld"),
                (Local, "b-hats", "Ann.Hats"),
                (Local, "a-hats", "ann.hats"),
                (Local, "progress", "Ann.Progress"),
                (Workshop, "1", "Ann.Progress"),
                (Workshop, "3", "Ann.Roads"),
                (Workshop, "4", "ann.roads"),
            ],
            &[],
        );
        let at = |source, folder| by(&library, source, folder);
        let active: Vec<String> = [
            "ludeon.rimworld",
            "ann.hats",
            "ann.progress_steam",
            "ann.roads",
        ]
        .map(String::from)
        .to_vec();
        let why = |source, folder| {
            let id = at(source, folder);
            library.loaded_why(id, library.loaded_as(id, &active)?)
        };
        assert_eq!(why(Official, "Core"), Some(Loaded::Official));
        assert_eq!(why(Local, "core-copy"), None);
        assert_eq!(why(Local, "a-hats"), Some(Loaded::FirstFolder));
        assert_eq!(why(Local, "b-hats"), None);
        assert_eq!(why(Workshop, "1"), Some(Loaded::Steam));
        assert_eq!(why(Local, "progress"), None);
        assert_eq!(why(Workshop, "3"), Some(Loaded::FirstFolder));
        assert_eq!(why(Workshop, "4"), None);
        let local = at(Local, "progress");
        assert_eq!(
            library.loaded_why(local, "ann.progress"),
            Some(Loaded::LocalFirst)
        );
    }

    #[test]
    fn a_copy_in_use_stands_in_for_another_only_where_the_entry_finds_one() {
        use Source::*;
        let (_tmp, library) = made(
            &[
                (Local, "progress", "Ann.Progress"),
                (Workshop, "1", "Ann.Progress"),
                (Workshop, "2", "Ann.Alone"),
            ],
            &[],
        );
        let (local, workshop) = (
            at_of(&library, Local, "progress"),
            at_of(&library, Workshop, "1"),
        );
        assert!(library.another_loads(local, "ann.progress"));
        assert!(!library.another_loads(workshop, "ann.progress_steam"));
        assert!(library.another_loads(workshop, "ann.progress"));
        assert!(!library.another_loads(at_of(&library, Workshop, "2"), "ann.alone"));
    }

    fn at_of(library: &Library, source: Source, folder: &str) -> ModId {
        by(library, source, folder)
    }

    #[test]
    fn a_copy_is_newer_by_its_own_version_else_by_the_game_s() {
        let newer = |local: &str, workshop: &str| {
            let (_tmp, library) = library(local, workshop);
            let local = by(&library, Source::Local, "3535");
            library.newer_copy(local, by(&library, Source::Workshop, "3535"))
        };
        assert!(newer(
            &versions("0.9", &["1.7"]),
            &versions("0.10", &["1.6"])
        ));
        assert!(!newer(
            &versions("0.20.1", &["1.6"]),
            &versions("0.20.0", &["1.7"])
        ));
        assert!(newer(
            &versions("0.20.0", &["1.6"]),
            &versions("0.20", &["1.6", "1.7"])
        ));
        let game = |versions: &[&str]| {
            self::versions("", versions).replace("<modVersion></modVersion>", "")
        };
        assert!(newer(&game(&["1.5", "1.6"]), &game(&["1.6", "1.7"])));
        assert!(!newer(&game(&["1.7"]), &game(&["1.7"])));
        assert!(!newer("", ""));
    }
}
