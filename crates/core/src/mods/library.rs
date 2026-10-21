use crate::base::aside;
use crate::base::error::Error;
use crate::base::sentence::Sentence;
use crate::base::translate::named;
use crate::said;

use crate::base::versions::numbers;
use crate::mods::list::{Category, Mod};
use crate::rimworld::about::{self, About};
use crate::rimworld::id;
use crate::rimworld::paths::{ModRoot, Paths, Source};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use crate::mods::classify::Kinds;
use crate::mods::classify::{Standing, classify};
use crate::mods::details;
use crate::rimworld::code::Code;
use crate::rimworld::game::Game;
pub use mod_id::{MOST_MODS, ModId};

#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub path: PathBuf,
    pub source: Source,
    /// The Workshop id for Workshop mods.
    pub folder: String,
    pub about: Result<About, Error>,
    pub kinds: Kinds,
    /// Empty until `read_code`.
    pub code: Code,
    pub preview: Option<PathBuf>,
    pub git: bool,
    /// It names the Workshop item it is a copy of: a Workshop folder, or `PublishedFileId.txt`.
    pub published: bool,
    /// Seconds since 1970 its folder was made; `None` where the file system does not keep it.
    pub downloaded: Option<u64>,
}

impl Installed {
    pub fn key(&self) -> Option<String> {
        self.about.as_ref().ok().map(|a| id::fold(&a.package_id))
    }

    pub fn assemblies(&self) -> Vec<String> {
        let mut names: Vec<String> = content_folders(&self.path)
            .into_iter()
            .filter(|(name, _)| name == "assemblies")
            .flat_map(|(_, dir)| std::fs::read_dir(dir).into_iter().flatten().flatten())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dll")))
            .filter_map(|p| Some(p.file_stem()?.to_string_lossy().to_lowercase()))
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    pub fn source_label(&self) -> &'static str {
        match self.source {
            Source::Official => "Ludeon",
            Source::Local => "Local",
            Source::Workshop => "Workshop",
        }
    }

    pub fn contains(&self) -> Sentence {
        let k = self.kinds;
        let words = [
            (k.defs, named("defs")),
            (k.patches, named("patches")),
            (k.assemblies, named("code")),
            (k.textures, named("textures")),
        ]
        .iter()
        .filter(|(present, _)| *present)
        .map(|(_, word)| Sentence::named(word))
        .collect::<Vec<_>>();
        Sentence::listed(words)
    }

    pub fn display_name(&self) -> &str {
        match &self.about {
            Ok(About {
                name: Some(name), ..
            }) => name,
            // The game and its expansions carry no name; their folders do.
            Ok(a) if self.source != Source::Official => &a.package_id,
            _ => &self.folder,
        }
    }

    /// The package id, as written, of what a mod holding only translations translates: a mod
    /// it depends on or loads after, else an expansion it names, else the game. RimSort reads
    /// Steam's Translation tag and the dependencies; the tag is not on disk, a lone Languages
    /// folder is.
    pub fn translates(&self, game_version: &str) -> Option<&str> {
        if !self.kinds.only_languages() {
            return None;
        }
        let about = self.about.as_ref().ok()?;
        let named = || {
            (about.dependencies.for_version(game_version).iter())
                .chain(about.load_after.for_version(game_version))
                .chain(&about.force_load_after)
                .map(String::as_str)
        };
        let official = |id: &&str| id::fold(id).starts_with("ludeon.rimworld");
        let named_mod = named().find(|id| !official(id));
        Some(
            named_mod
                .or_else(|| named().next())
                .unwrap_or("Ludeon.RimWorld"),
        )
    }
}

/// At any depth, as a mod may keep its content where its `LoadFolders.xml` points,
/// such as `1.7/Base`; a content folder is not looked inside.
fn content_folders(mod_root: &Path) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    let mut pending = vec![mod_root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if !entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            match name.as_str() {
                "defs" | "patches" | "assemblies" | "textures" | "languages" => {
                    found.push((name, entry.path()))
                }
                "about" | "source" | "sounds" | ".git" => {}
                _ => pending.push(entry.path()),
            }
        }
    }
    found
}

fn kinds(mod_root: &Path) -> Kinds {
    let mut found = Kinds::default();
    for (name, _) in content_folders(mod_root) {
        match name.as_str() {
            "defs" => found.defs = true,
            "patches" => found.patches = true,
            "assemblies" => found.assemblies = true,
            "languages" => found.languages = true,
            _ => found.textures = true,
        }
    }
    found
}

fn read_mod(path: PathBuf, source: Source) -> Installed {
    let about = match about::locate(&path) {
        None => Err(Error::format(said!("no About.xml"))),
        Some(file) => std::fs::read(&file)
            .map_err(|e| Error::file(&file, e))
            .and_then(|bytes| about::parse(&bytes)),
    };
    let published = match source {
        Source::Official | Source::Workshop => true,
        Source::Local => about::published_file_id(&path).is_some(),
    };
    Installed {
        folder: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        kinds: kinds(&path),
        code: Code::default(),
        preview: about::preview(&path),
        git: path.join(".git").exists(),
        published,
        downloaded: made_at(&path),
        about,
        source,
        path,
    }
}

fn made_at(path: &Path) -> Option<u64> {
    let made = std::fs::metadata(path).and_then(|m| m.created()).ok()?;
    made.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// Sorted by source, then folder name.
pub fn scan(roots: &[ModRoot]) -> Vec<Installed> {
    let folders: Vec<(PathBuf, Source)> = roots
        .iter()
        .flat_map(|root| {
            std::fs::read_dir(&root.path)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().is_dir() && aside::left_beside(&e.file_name()).is_none())
                .map(|e| (e.path(), root.source))
        })
        .collect();
    let mut mods: Vec<Installed> = folders
        .into_par_iter()
        .map(|(path, source)| read_mod(path, source))
        .collect();
    mods.sort_by(|a, b| (a.source, &a.folder).cmp(&(b.source, &b.folder)));
    mods
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resolved<'a> {
    Found(&'a Installed),
    Missing(&'a str),
}

/// When several installed mods share an id, the official one wins, then local, then Workshop.
/// An id ending in `_steam` picks the Workshop copy.
pub fn resolve_active<'a>(active: &'a [String], mods: &'a [Installed]) -> Vec<Resolved<'a>> {
    let mut by_key: HashMap<String, Vec<&Installed>> = HashMap::new();
    for m in mods {
        if let Some(key) = m.key() {
            by_key.entry(key).or_default().push(m);
        }
    }
    active
        .iter()
        .map(|written| {
            let (key, steam) = id::split_steam(written);
            let copies = by_key.get(&key).map(Vec::as_slice).unwrap_or_default();
            let chosen = if steam {
                copies.iter().find(|m| m.source == Source::Workshop)
            } else {
                copies.iter().min_by_key(|m| m.source)
            };
            match chosen {
                Some(m) => Resolved::Found(m),
                // "x_steam" may also simply be a mod's real id.
                None => match by_key.get(&id::fold(written)).and_then(|c| c.first()) {
                    Some(m) => Resolved::Found(m),
                    None => Resolved::Missing(written),
                },
            }
        })
        .collect()
}

/// Nothing is read without the game's own code to hold it against.
pub fn read_code(mods: &mut [Installed], game: Option<&Game>, game_version: &str) {
    let Some(game) = game else {
        return;
    };
    mods.par_iter_mut()
        .filter(|m| m.kinds.assemblies && m.source != Source::Official)
        .for_each(|m| m.code = crate::rimworld::code::read(&m.path, game_version, game));
}

fn before_game(mods: &[Installed], game_version: &str) -> std::collections::HashSet<String> {
    let official = |m: &&Installed| m.source == Source::Official;
    let mut ahead: std::collections::HashSet<String> = mods
        .iter()
        .filter(official)
        .filter_map(Installed::key)
        .collect();
    let game: std::collections::HashSet<String> = ahead.clone();
    loop {
        let more: Vec<String> = mods
            .iter()
            .filter_map(|m| Some((m.key()?, m.about.as_ref().ok()?)))
            .filter(|(key, _)| !ahead.contains(key))
            .filter(|(_, about)| {
                let asks = about.load_before.for_version(game_version);
                let asks = asks.iter().chain(&about.force_load_before);
                asks.into_iter().any(|id| ahead.contains(&id::fold(id)))
            })
            .map(|(key, _)| key)
            .collect();
        if more.is_empty() {
            break;
        }
        ahead.extend(more);
    }
    ahead.retain(|key| !game.contains(key));
    ahead
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Library {
    pub installed: Vec<Installed>,
    reading: u32,
    pub game_version: String,
    pub active: Vec<Mod>,
    pub inactive: Vec<Mod>,
    pub missing: usize,
    pub unreadable: usize,
    names: HashMap<String, String>,
    pub categories: Vec<Category>,
}

impl Library {
    pub fn get(&self, id: ModId) -> Option<&Installed> {
        let ours = id.reading() == self.reading;
        ours.then(|| self.installed.get(id.index())).flatten()
    }

    pub fn category_of(&self, id: ModId) -> Option<Category> {
        let ours = id.reading() == self.reading;
        ours.then(|| self.categories.get(id.index()).copied())
            .flatten()
    }

    fn id_at(&self, index: usize) -> ModId {
        ModId::new(self.reading, index)
    }

    pub fn ids(&self) -> impl Iterator<Item = (ModId, &Installed)> {
        let ids = (0..self.installed.len()).map(|index| self.id_at(index));
        ids.zip(&self.installed)
    }

    pub fn name_of(&self, key: &str) -> Option<String> {
        self.names.get(key).cloned()
    }

    pub fn abouts(&self, mods: &[Mod]) -> Vec<Option<&About>> {
        mods.iter()
            .map(|m| self.get(m.installed?)?.about.as_ref().ok())
            .collect()
    }

    /// Name, or the id when not installed.
    pub fn requirements(&self, m: &Installed) -> Vec<(String, bool)> {
        let Ok(about) = &m.about else {
            return Vec::new();
        };
        about
            .dependencies
            .for_version(&self.game_version)
            .iter()
            .map(|dep| self.named(dep))
            .collect()
    }

    /// The installed mods requiring `m` on this game version: name and library id.
    pub fn needed_by(&self, m: &Installed) -> Vec<(String, ModId)> {
        let Some(key) = m.key() else {
            return Vec::new();
        };
        let needs = |about: &About| {
            let deps = about.dependencies.for_version(&self.game_version);
            deps.iter()
                .any(|dep| id::fold(&id::split_steam(dep).0) == key)
        };
        self.ids()
            .filter(|(_, other)| other.about.as_ref().is_ok_and(needs))
            .map(|(at, other)| (other.display_name().to_string(), at))
            .collect()
    }

    /// The active mods requiring `m` on this game version: name and library id.
    pub fn needed_by_active(&self, m: &Installed) -> Vec<(String, ModId)> {
        let mine = self
            .ids()
            .find(|(_, other)| std::ptr::eq(*other, m))
            .map(|(at, _)| at);
        let active = |at: ModId| self.active.iter().any(|m| m.installed == Some(at));
        self.needed_by(m)
            .into_iter()
            .filter(|(_, at)| Some(*at) != mine && active(*at))
            .collect()
    }

    /// For each of `mods`, the places in `mods` of the ones it requires on this game version.
    pub fn requires_within(&self, mods: &[Mod]) -> Vec<Vec<usize>> {
        let abouts = self.abouts(mods);
        let position = crate::mods::rules::positions(&abouts);
        let place = |dep: &String| position.get(&id::split_steam(dep).0).copied();
        abouts
            .iter()
            .map(|about| match about {
                Some(a) => (a.dependencies.for_version(&self.game_version).iter())
                    .filter_map(place)
                    .collect(),
                None => Vec::new(),
            })
            .collect()
    }

    /// What `m` translates, named as `requirements` names.
    pub fn translated(&self, m: &Installed) -> Option<(String, bool)> {
        m.translates(&self.game_version).map(|id| self.named(id))
    }

    fn named(&self, package_id: &str) -> (String, bool) {
        let key = id::fold(package_id);
        match self
            .installed
            .iter()
            .find(|x| x.key().as_deref() == Some(&key))
        {
            Some(found) => (found.display_name().to_string(), true),
            None => (package_id.to_string(), false),
        }
    }
}

/// `off` grown by every mod needing one in it, directly or through another;
/// `requires` as [`Library::requires_within`] gives it.
pub fn with_needing(requires: &[Vec<usize>], mut off: Vec<bool>) -> Vec<bool> {
    loop {
        let needs_one = |at: usize| requires[at].iter().any(|r| off[*r]);
        let more: Vec<usize> = (0..off.len())
            .filter(|at| !off[*at] && needs_one(*at))
            .collect();
        if more.is_empty() {
            return off;
        }
        more.into_iter().for_each(|at| off[at] = true);
    }
}

fn patched(aim: &crate::rimworld::code::Aim) -> String {
    format!("{}.{}", aim.target, aim.method).to_lowercase()
}

/// Folder size in bytes by walking its entries; `None` if walk fails.
fn folder_bytes(path: &Path) -> Option<u64> {
    crate::base::walk::whole_tree(path)
        .ok()?
        .into_iter()
        .filter_map(|entry| std::fs::metadata(&entry.path).ok().map(|m| m.len()))
        .reduce(|a, b| a.saturating_add(b))
}

/// Folded package ids of the official DLCs it depends on, for any game version, sorted.
fn official_dlcs(about: Option<&About>) -> Vec<String> {
    let Some(about) = about else {
        return Vec::new();
    };
    let deps = &about.dependencies;
    let mut dlcs: Vec<String> = (deps.base.iter())
        .chain(deps.by_version.values().flatten())
        .map(|dep| id::fold(&id::split_steam(dep).0))
        .filter(|folded| folded.starts_with("ludeon.rimworld."))
        .collect();
    dlcs.sort();
    dlcs.dedup();
    dlcs
}

/// `supports` stops at `game_version`: a mod made for a newer game sorts as made for this one.
fn row(m: &Installed, id: String, category: Category, installed: ModId, game_version: &str) -> Mod {
    let about = m.about.as_ref().ok();
    let newest = about.and_then(|a| a.supported_versions.iter().map(|v| numbers(v)).max());
    let game = (!game_version.trim().is_empty()).then(|| numbers(game_version));
    Mod {
        id,
        tags: Vec::new(),
        local: m.source == Source::Local,
        workshop: m.source == Source::Workshop,
        git: m.git,
        code: m.kinds.assemblies,
        patched: m.code.aims.iter().map(patched).collect(),
        defs: Default::default(),
        sub: String::new(),
        colour: String::new(),
        pinned: false,
        load_micros: None,
        updated: None,
        downloaded: m.downloaded,
        supports: match (newest, game) {
            (Some(newest), Some(game)) => newest.min(game),
            (newest, _) => newest.unwrap_or_default(),
        },
        name: m.display_name().to_string(),
        author: about.map(|a| a.authors.join(", ")).unwrap_or_default(),
        version: about
            .and_then(|a| a.mod_version.clone())
            .unwrap_or_default(),
        game: about
            .and_then(|a| details::newest(&a.supported_versions).map(String::from))
            .unwrap_or_default(),
        category,
        installed: Some(installed),
        // Steam keeps a Workshop folder's size; walking thousands of them would take long.
        bytes: (m.source != Source::Workshop)
            .then(|| folder_bytes(&m.path))
            .flatten(),
        needs_dlcs: official_dlcs(about),
        translation: m.translates(game_version).is_some(),
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Arranged {
    pub active: Vec<Mod>,
    pub inactive: Vec<Mod>,
    /// Ids with no installed mod stay in `active` as placeholders.
    pub missing: usize,
}

impl Library {
    pub fn find(&self, text: &str, most: usize) -> Vec<ModId> {
        let text = text.trim().to_lowercase();
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<(bool, usize, String, ModId)> = Vec::new();
        for (i, m) in self.ids() {
            let name = m.display_name().to_lowercase();
            let by_id = m.key().is_some_and(|key| key.contains(&text));
            if words.iter().all(|w| name.contains(w)) || by_id {
                found.push((!name.starts_with(&text), name.chars().count(), name, i));
            }
        }
        found.sort();
        found.into_iter().take(most).map(|f| f.3).collect()
    }

    pub fn workshop_mods(&self) -> HashMap<&str, ModId> {
        self.ids()
            .filter(|(_, m)| m.source == Source::Workshop)
            .map(|(i, m)| (m.folder.as_str(), i))
            .collect()
    }

    pub fn active_ids(&self, ids: &[String]) -> std::collections::HashSet<ModId> {
        let active = self.arrange(ids).active;
        active.iter().filter_map(|m| m.installed).collect()
    }

    pub fn arrange(&self, ids: &[String]) -> Arranged {
        let index_of: HashMap<&Path, usize> = self
            .installed
            .iter()
            .enumerate()
            .map(|(i, m)| (m.path.as_path(), i))
            .collect();
        let mut arranged = Arranged::default();
        let mut is_active = vec![false; self.installed.len()];
        for (written, entry) in ids.iter().zip(resolve_active(ids, &self.installed)) {
            match entry {
                Resolved::Found(m) => {
                    let i = index_of[m.path.as_path()];
                    is_active[i] = true;
                    arranged.active.push(row(
                        m,
                        written.clone(),
                        self.categories[i],
                        self.id_at(i),
                        &self.game_version,
                    ));
                }
                Resolved::Missing(written) => {
                    arranged.missing += 1;
                    arranged.active.push(Mod {
                        id: written.to_string(),
                        game: String::new(),
                        tags: Vec::new(),
                        local: false,
                        workshop: false,
                        git: false,
                        code: false,
                        patched: Default::default(),
                        defs: Default::default(),
                        sub: String::new(),
                        colour: String::new(),
                        pinned: false,
                        load_micros: None,
                        updated: None,
                        downloaded: None,
                        supports: Vec::new(),
                        needs_dlcs: Vec::new(),
                        translation: false,
                        name: written.to_string(),
                        author: String::new(),
                        version: String::new(),
                        category: Category::Content,
                        installed: None,
                        bytes: None,
                    });
                }
            }
        }
        for (i, m) in self.installed.iter().enumerate() {
            // The game lets only one of the two be active.
            if !is_active[i] {
                arranged.inactive.push(row(
                    m,
                    self.written_id(i),
                    self.categories[i],
                    self.id_at(i),
                    &self.game_version,
                ));
            }
        }
        arranged
            .inactive
            .sort_by_cached_key(|m| (m.category, m.name.to_lowercase()));
        arranged
    }

    /// Its package id, with `_steam` for a Workshop copy of a mod also installed elsewhere.
    /// Empty for a folder that cannot be read as a mod.
    fn written_id(&self, i: usize) -> String {
        let m = &self.installed[i];
        let Some(key) = m.key() else {
            return String::new();
        };
        let also_elsewhere = || {
            let same =
                |x: &&Installed| x.source != Source::Workshop && x.key().as_ref() == Some(&key);
            self.installed.iter().any(|x| same(&x))
        };
        if m.source == Source::Workshop && also_elsewhere() {
            format!("{key}_steam")
        } else {
            key
        }
    }
}

pub fn build(mods: Vec<Installed>, active: &[String], game_version: &str) -> Library {
    let mut dependents: HashMap<String, usize> = HashMap::new();
    for about in mods.iter().filter_map(|m| m.about.as_ref().ok()) {
        for dep in about.dependencies.for_version(game_version) {
            *dependents.entry(id::fold(dep)).or_default() += 1;
        }
    }
    let ahead = before_game(&mods, game_version);
    let category = |m: &Installed| {
        let key = m.key();
        let standing = Standing {
            official: m.source == Source::Official,
            before_game: key.as_ref().is_some_and(|k| ahead.contains(k)),
            dependents: key.and_then(|k| dependents.get(&k).copied()).unwrap_or(0),
        };
        classify(m.kinds, &m.code.facts, standing)
    };
    if mods.len() > MOST_MODS {
        crate::problem!(
            "{} mods: past {MOST_MODS} the window cannot name them",
            mods.len()
        );
    }
    let mut library = Library {
        reading: mod_id::next_reading(),
        game_version: game_version.to_string(),
        categories: mods.iter().map(category).collect(),
        unreadable: mods.iter().filter(|m| m.about.is_err()).count(),
        ..Library::default()
    };
    for m in &mods {
        if let Some(key) = m.key() {
            let name = || m.display_name().to_string();
            library.names.entry(key).or_insert_with(name);
        }
    }
    library.installed = mods;
    let arranged = library.arrange(active);
    library.active = arranged.active;
    library.inactive = arranged.inactive;
    library.missing = arranged.missing;
    library
}

pub fn load(paths: &Paths) -> Result<Library, Error> {
    let roots = paths.roots();
    if roots.is_empty() {
        return Err(Error::refused(said!(
            "RimWorld was not found. Set RIMSTACK_GAME to its folder."
        )));
    }
    let config = paths.mods_config().ok_or_else(|| {
        Error::refused(said!(
            "RimWorld's settings folder was not found. Set RIMSTACK_CONFIG to it."
        ))
    })?;
    let active = crate::rimworld::modsconfig::read(&config)?.active;
    let version = paths.game_version.as_deref().unwrap_or_default();
    let (mut mods, game) = rayon::join(
        || scan(&roots),
        || paths.game.as_deref().and_then(Game::find),
    );
    read_code(&mut mods, game.as_ref(), version);
    Ok(build(mods, &active, version))
}

mod copies;
mod mod_id;
pub use copies::{Copies, Loaded};
#[cfg(test)]
mod real;
#[cfg(test)]
mod tests;
