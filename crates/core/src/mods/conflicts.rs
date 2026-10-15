//! The game keeps a texture from the mod that loads last.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// How the game names a texture: its path from here, lower case, `/` between parts.
fn named_under(dir: &Path) -> impl Iterator<Item = (String, PathBuf)> + '_ {
    crate::base::walk::files(dir)
        .into_iter()
        .filter_map(move |path| {
            let parts = path.strip_prefix(dir).ok()?.components();
            let parts: Vec<String> = parts
                .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
                .collect();
            Some((parts.join("/"), path))
        })
}

fn textures_under(dir: &Path, into: &mut Vec<(String, PathBuf)>) {
    for (name, path) in named_under(dir) {
        if let Some(texture) = name.strip_suffix(".png") {
            into.push((texture.to_string(), path));
        }
    }
}

/// When two load folders have the same texture, the later folder's file wins.
pub fn files(load_folders: &[PathBuf]) -> std::collections::BTreeMap<String, PathBuf> {
    let mut found = Vec::new();
    for folder in load_folders {
        let textures = std::fs::read_dir(folder).into_iter().flatten().flatten();
        let is_textures = |e: &std::fs::DirEntry| e.file_name().eq_ignore_ascii_case("textures");
        for dir in textures.filter(is_textures) {
            textures_under(&dir.path(), &mut found);
        }
    }
    found.into_iter().collect()
}

/// Hidden textures appear under their own path, not the renamed one.
pub fn files_with_hidden(
    load_folders: &[PathBuf],
) -> std::collections::BTreeMap<String, (PathBuf, bool)> {
    fn under(dir: &Path, into: &mut Vec<(String, (PathBuf, bool))>) {
        const HIDDEN: &str = ".png.rimstack-hidden";
        for (name, path) in named_under(dir) {
            if let Some(texture) = name.strip_suffix(".png") {
                into.push((texture.to_string(), (path, false)));
            } else if let Some(texture) = name.strip_suffix(HIDDEN) {
                let own = crate::mods::hidden::original_of(&path);
                if let Some(own) = own.filter(|own| !own.exists()) {
                    into.push((texture.to_string(), (own, true)));
                }
            }
        }
    }
    let mut found = Vec::new();
    for folder in load_folders {
        let textures = std::fs::read_dir(folder).into_iter().flatten().flatten();
        let is_textures = |e: &std::fs::DirEntry| e.file_name().eq_ignore_ascii_case("textures");
        for dir in textures.filter(is_textures) {
            under(&dir.path(), &mut found);
        }
    }
    found.into_iter().collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shared {
    pub name: String,
    pub first: PathBuf,
    pub second: PathBuf,
}

pub fn shared(first_folders: &[PathBuf], second_folders: &[PathBuf]) -> Vec<Shared> {
    let second = files(second_folders);
    files(first_folders)
        .into_iter()
        .filter_map(|(name, path)| {
            let other = second.get(&name)?.clone();
            Some(Shared {
                name,
                first: path,
                second: other,
            })
        })
        .collect()
}

/// Lower case, without `.png`.
pub fn provided(load_folders: &[PathBuf]) -> Vec<String> {
    files(load_folders).into_keys().collect()
}

/// A pair is stored once, lower place first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overlaps {
    shared: HashMap<(usize, usize), usize>,
}

impl Overlaps {
    pub fn with(&self, this: usize) -> Vec<(usize, usize)> {
        let mut found: Vec<(usize, usize)> = self
            .shared
            .iter()
            .filter_map(|(&(a, b), &count)| match (a == this, b == this) {
                (true, _) => Some((b, count)),
                (_, true) => Some((a, count)),
                _ => None,
            })
            .collect();
        found.sort_unstable();
        found
    }

    pub fn is_empty(&self) -> bool {
        self.shared.is_empty()
    }
}

pub fn overlaps(provided: &[Vec<String>]) -> Overlaps {
    let mut providers: HashMap<&str, Vec<usize>> = HashMap::new();
    for (m, names) in provided.iter().enumerate() {
        for name in names {
            providers.entry(name).or_default().push(m);
        }
    }
    let mut shared: HashMap<(usize, usize), usize> = HashMap::new();
    for mods in providers.values().filter(|mods| mods.len() > 1) {
        for (i, &a) in mods.iter().enumerate() {
            for &b in &mods[i + 1..] {
                *shared.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
    }
    Overlaps { shared }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn folder(files: &[&str]) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        for file in files {
            let path = tmp.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        let root = tmp.path().to_path_buf();
        (tmp, root)
    }

    #[test]
    fn textures_are_named_by_their_path_under_textures_without_case_or_extension() {
        let (_t, root) = folder(&[
            "Textures/Things/Wall.PNG",
            "Textures/UI/icon.png",
            "Textures/UI/icon.dds",
            "textures/Things/Door.png",
            "About/Preview.png",
            "1.6/Textures/Things/Wall.png",
        ]);
        let found = provided(&[root.clone(), root.join("1.6"), root.join("nowhere")]);
        assert_eq!(found, ["things/door", "things/wall", "ui/icon"]);
    }

    #[test]
    fn the_textures_two_mods_share_come_with_both_files() {
        let (_a, first) = folder(&["Textures/Things/Wall.png", "Textures/only_a.png"]);
        let (_b, second) = folder(&["1.6/Textures/things/wall.PNG", "Textures/only_b.png"]);
        let found = shared(
            std::slice::from_ref(&first),
            &[second.clone(), second.join("1.6")],
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "things/wall");
        assert_eq!(found[0].first, first.join("Textures/Things/Wall.png"));
        assert_eq!(found[0].second, second.join("1.6/Textures/things/wall.PNG"));
    }

    #[test]
    fn a_hidden_texture_is_still_listed_under_its_own_name() {
        let (_t, root) = folder(&[
            "Textures/a.png",
            "Textures/sub/b.png.rimstack-hidden",
            "Textures/c.png",
            "Textures/c.png.rimstack-hidden",
        ]);
        let found = files_with_hidden(std::slice::from_ref(&root));
        assert_eq!(found["a"], (root.join("Textures/a.png"), false));
        assert_eq!(found["sub/b"], (root.join("Textures/sub/b.png"), true));
        assert_eq!(found["c"], (root.join("Textures/c.png"), false));
        assert_eq!(provided(&[root]), ["a", "c"]);
    }

    #[test]
    fn shared_textures_are_counted_per_pair_of_mods() {
        let names = |list: &[&str]| -> Vec<String> { list.iter().map(|s| s.to_string()).collect() };
        let found = overlaps(&[
            names(&["a", "b", "c"]),
            names(&["b", "c", "d"]),
            names(&["x"]),
            names(&["c"]),
        ]);
        assert_eq!(found.with(0), [(1, 2), (3, 1)]);
        assert_eq!(found.with(1), [(0, 2), (3, 1)]);
        assert_eq!(found.with(3), [(0, 1), (1, 1)]);
        assert!(found.with(2).is_empty());
        assert!(overlaps(&[names(&["a"]), names(&["b"])]).is_empty());
    }
}
