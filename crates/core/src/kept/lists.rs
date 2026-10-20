use crate::base::error::Error;
use crate::said;

use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Saved {
    pub name: String,
    pub mods: usize,
    /// Seconds since 1970.
    pub modified: u64,
    /// A `<file name>.settings` folder keeps the mods' `Mod_*.xml` with the list.
    pub settings: bool,
}

/// Characters a file name cannot hold become `_`: names differing only so share a file,
/// and `save` refuses the second.
fn safe_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_.()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn file_for(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{}.json", safe_name(name)))
}

/// Beside the list's file, moved and removed with it.
fn settings_for(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{}.settings", safe_name(name)))
}

pub fn save(dir: &Path, name: &str, ids: &[String]) -> Result<(), Error> {
    if name.trim().is_empty() {
        return Err(Error::refused(said!("Give the list a name.")));
    }
    let path = file_for(dir, name);
    if let Some((other, _)) = read(&path).filter(|(kept, _)| kept != name.trim()) {
        return Err(Error::refused(said!(
            "The list {} is kept under the same file name. Give this one another name.",
            other
        )));
    }
    let text = json!({ "name": name.trim(), "ids": ids }).to_string();
    crate::base::atomic_file::write(&path, text)
}

fn read(path: &Path) -> Option<(String, Vec<String>)> {
    let root: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let ids = root.get("ids")?.as_array()?;
    let ids = ids
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    Some((root.get("name")?.as_str()?.to_string(), ids))
}

pub fn all(dir: &Path) -> Vec<Saved> {
    let mut found: Vec<Saved> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let (name, ids) = read(&entry.path())?;
            let modified = entry.metadata().ok()?.modified().ok()?;
            let settings = settings_for(dir, &name).is_dir();
            Some(Saved {
                name,
                mods: ids.len(),
                modified: modified.duration_since(UNIX_EPOCH).ok()?.as_secs(),
                settings,
            })
        })
        .collect();
    found.sort_by_key(|s| s.name.to_lowercase());
    found
}

/// Counts files without opening them: it runs on every change of the load order.
pub fn count(dir: &Path) -> usize {
    let files = std::fs::read_dir(dir).into_iter().flatten().flatten();
    let is_list = |entry: &std::fs::DirEntry| entry.path().extension().is_some_and(|x| x == "json");
    files.filter(is_list).count()
}

pub fn prune(dir: &Path, keep: usize) {
    let mut found = all(dir);
    found.sort_by_key(|s| std::cmp::Reverse(s.modified));
    for old in found.iter().skip(keep) {
        // Best effort: a snapshot that cannot be deleted is only clutter.
        let _ = delete(dir, &old.name);
    }
}

pub fn taken(dir: &Path, name: &str) -> bool {
    file_for(dir, name).exists()
}

pub fn load(dir: &Path, name: &str) -> Result<Vec<String>, Error> {
    let path = file_for(dir, name);
    read(&path)
        .map(|(_, ids)| ids)
        .ok_or_else(|| Error::format(said!("{} is not a saved list", path.display())))
}

/// Removes the list and the settings folder kept beside it, if any.
pub fn delete(dir: &Path, name: &str) -> Result<(), Error> {
    let path = file_for(dir, name);
    std::fs::remove_file(&path).map_err(|e| Error::file(&path, e))?;
    // Best effort: a folder that cannot be removed only clutters.
    let _ = std::fs::remove_dir_all(settings_for(dir, name));
    Ok(())
}

/// Which mod a kept `Mod_*.xml` of the game's `Config` belongs to, for the files' own list.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SettingsFile {
    /// The file's name within `Config`.
    pub file: String,
    /// The installed folder the file is named after, per the game's naming.
    pub folder: String,
    /// The mod's package id, or its folder when it has no readable id.
    #[serde(rename = "mod")]
    pub mod_id: String,
}

const MANIFEST: &str = "settings.json";

pub fn has_settings(dir: &Path, name: &str) -> bool {
    settings_for(dir, name).is_dir()
}

/// Copies the given `Config` files beside the list's file, so they travel with it.
/// The folder is written empty-named then renamed over any earlier one, which a reader
/// sees whole or not at all. No matched file drops an earlier folder.
pub fn keep_settings(
    dir: &Path,
    name: &str,
    config: &Path,
    files: &[SettingsFile],
) -> Result<usize, Error> {
    let to = settings_for(dir, name);
    if files.is_empty() {
        let _ = std::fs::remove_dir_all(&to);
        return Ok(0);
    }
    let staged = crate::base::aside::beside(&to, crate::base::aside::NEW);
    let _ = std::fs::remove_dir_all(&staged);
    std::fs::create_dir_all(&staged).map_err(|e| Error::file(&staged, e))?;
    let kept = files.iter().try_for_each(|file| {
        let from = config.join("Config").join(&file.file);
        let to_file = staged.join(&file.file);
        std::fs::copy(&from, &to_file)
            .map(drop)
            .map_err(|e| Error::file(&from, e))
    });
    let written = kept.and_then(|_| {
        let manifest = serde_json::json!({ "settings": files }).to_string();
        crate::base::atomic_file::write(&staged.join(MANIFEST), manifest)?;
        Ok(files.len())
    });
    if let Err(problem) = written {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(problem);
    }
    // Away, over, gone: a crash between the two renames only leaves the old folder gone.
    let old = crate::base::aside::beside(&to, crate::base::aside::OLD);
    let _ = std::fs::remove_dir_all(&old);
    if to.exists() {
        std::fs::rename(&to, &old).map_err(|e| Error::file(&old, e))?;
    }
    std::fs::rename(&staged, &to).map_err(|e| Error::file(&to, e))?;
    let _ = std::fs::remove_dir_all(&old);
    Ok(files.len())
}

/// What the list keeps, from the files' own manifest.
pub fn kept_settings(dir: &Path, name: &str) -> Result<Vec<SettingsFile>, Error> {
    let manifest = settings_for(dir, name).join(MANIFEST);
    let read = || -> Option<Vec<SettingsFile>> {
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&manifest).ok()?).ok()?;
        serde_json::from_value(root.get("settings")?.clone()).ok()
    };
    let files = read()
        .ok_or_else(|| Error::format(said!("{} keeps no mod settings", manifest.display())))?;
    for file in &files {
        if file.folder.is_empty() || !names_settings(&file.file) {
            return Err(Error::format(said!(
                "{} names no settings file of the game",
                manifest.display()
            )));
        }
    }
    Ok(files)
}

fn names_settings(file: &str) -> bool {
    Path::new(file).file_name().and_then(|name| name.to_str()) == Some(file)
        && file.starts_with("Mod_")
        && file.ends_with(".xml")
}

/// What `use_settings` did: the files set aside, then the kept ones that took their place.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    pub set_aside: usize,
    pub used: usize,
}

/// The kept files go into `Config`. The ones they replace — every `Mod_*.xml` of the
/// same mods now there — are set aside into `aside` first, as a reset would.
pub fn use_settings(
    dir: &Path,
    name: &str,
    config: &Path,
    aside: &Path,
    installed: impl Fn(&str) -> bool,
) -> Result<Applied, Error> {
    let kept = kept_settings(dir, name)?;
    let from = settings_for(dir, name);
    let mut folders: HashSet<&str> = HashSet::new();
    let mut replaced: Vec<PathBuf> = Vec::new();
    for file in &kept {
        if folders.insert(file.folder.as_str()) {
            replaced.extend(crate::rimworld::mod_settings::of(
                config,
                &file.folder,
                &installed,
            ));
        }
    }
    let mut done = Applied {
        set_aside: crate::base::aside::set_aside(config, aside, &replaced)?,
        used: 0,
    };
    let into = config.join("Config");
    std::fs::create_dir_all(&into).map_err(|e| Error::file(&into, e))?;
    for file in &kept {
        let to = into.join(&file.file);
        std::fs::copy(from.join(&file.file), &to).map_err(|e| Error::file(&to, e))?;
        done.used += 1;
    }
    Ok(done)
}

/// How a saved list differs from the current load order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Differences {
    /// Indices in the saved list of mods only there.
    pub only_saved: Vec<usize>,
    /// Indices in the current list of mods only there.
    pub only_current: Vec<usize>,
    /// Indices in the saved list of the shared mods not in the longest run
    /// both lists hold in one order: the fewest moves, not every shifted one.
    pub moved: Vec<usize>,
}

/// Ids are matched as the game matches them, case-insensitive.
pub fn compare(saved: &[String], current: &[String]) -> Differences {
    let saved_owned: Vec<String> = saved
        .iter()
        .map(|id| crate::rimworld::id::fold(id))
        .collect();
    let current_owned: Vec<String> = current
        .iter()
        .map(|id| crate::rimworld::id::fold(id))
        .collect();
    let saved_folded: Vec<&str> = saved_owned.iter().map(String::as_str).collect();
    let current_folded: Vec<&str> = current_owned.iter().map(String::as_str).collect();
    let in_saved: HashMap<&str, usize> = saved_folded
        .iter()
        .enumerate()
        .map(|(at, id)| (*id, at))
        .collect();
    let in_current: HashMap<&str, usize> = current_folded
        .iter()
        .enumerate()
        .map(|(at, id)| (*id, at))
        .collect();

    let only_saved = saved_folded
        .iter()
        .enumerate()
        .filter(|(_, id)| !in_current.contains_key(*id))
        .map(|(at, _)| at)
        .collect();
    let only_current = current_folded
        .iter()
        .enumerate()
        .filter(|(_, id)| !in_saved.contains_key(*id))
        .map(|(at, _)| at)
        .collect();

    // The shared mods in the current order, as their places in the saved one.
    let shared: Vec<usize> = current_folded
        .iter()
        .filter_map(|id| in_saved.get(id).copied())
        .collect();
    // The shared mods standing inside the longest rising run count as still
    // in place: the run is the most two orders can hold without a move.
    let mut tails: Vec<usize> = Vec::with_capacity(shared.len());
    let mut before: Vec<Option<usize>> = Vec::with_capacity(shared.len());
    for (at, &place) in shared.iter().enumerate() {
        let run = tails.partition_point(|&tail| shared[tail] < place);
        before.push(run.checked_sub(1).map(|earlier| tails[earlier]));
        if run == tails.len() {
            tails.push(at);
        } else {
            tails[run] = at;
        }
    }
    let mut kept = vec![false; shared.len()];
    if let Some(&end) = tails.last() {
        let mut at = end;
        loop {
            kept[at] = true;
            let Some(earlier) = before[at] else {
                break;
            };
            at = earlier;
        }
    }
    let rank_of: HashMap<usize, usize> = shared
        .iter()
        .enumerate()
        .map(|(at, &place)| (place, at))
        .collect();
    let moved = saved_folded
        .iter()
        .enumerate()
        .filter(|(saved_at, id)| in_current.contains_key(*id) && !kept[rank_of[saved_at]])
        .map(|(saved_at, _)| saved_at)
        .collect();

    Differences {
        only_saved,
        only_current,
        moved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_are_counted_by_their_files() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(count(tmp.path()), 0);
        save(tmp.path(), "one", &["a".to_string()]).unwrap();
        save(tmp.path(), "two", &[]).unwrap();
        std::fs::write(tmp.path().join("notes.txt"), "").unwrap();
        assert_eq!((count(tmp.path()), all(tmp.path()).len()), (2, 2));
        assert_eq!(count(&tmp.path().join("absent")), 0);
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_list_is_kept_listed_loaded_and_deleted_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("lists");
        assert!(all(&dir).is_empty());
        save(&dir, "Medieval run", &ids(&["a", "b"])).unwrap();
        save(&dir, "base", &ids(&["a"])).unwrap();
        let found = all(&dir);
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["base", "Medieval run"]);
        assert_eq!(found[1].mods, 2);
        assert_eq!(load(&dir, "Medieval run").unwrap(), ids(&["a", "b"]));
        delete(&dir, "base").unwrap();
        assert_eq!(all(&dir).len(), 1);
        assert!(load(&dir, "base").is_err());
    }

    #[test]
    fn saving_under_the_same_name_replaces_the_list() {
        let tmp = tempfile::tempdir().unwrap();
        save(tmp.path(), "run", &ids(&["a"])).unwrap();
        save(tmp.path(), " run ", &ids(&["b", "c"])).unwrap();
        assert_eq!(all(tmp.path()).len(), 1);
        assert_eq!(load(tmp.path(), "run").unwrap(), ids(&["b", "c"]));
    }

    #[test]
    fn a_name_cannot_leave_the_folder_or_be_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("lists");
        save(&dir, "../../escape/it", &ids(&["a"])).unwrap();
        assert!(!tmp.path().join("escape").exists());
        assert_eq!(all(&dir)[0].name, "../../escape/it");
        assert!(save(&dir, "  ", &ids(&["a"])).is_err());
    }

    #[test]
    fn pruning_keeps_the_given_number_of_lists() {
        let tmp = tempfile::tempdir().unwrap();
        for n in 0..5 {
            save(tmp.path(), &format!("snapshot {n}"), &ids(&["a"])).unwrap();
        }
        prune(tmp.path(), 3);
        assert_eq!(all(tmp.path()).len(), 3);
        prune(tmp.path(), 10);
        assert_eq!(all(tmp.path()).len(), 3);
    }

    #[test]
    fn other_files_in_the_folder_are_not_lists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("notes.json"), "[1,2]").unwrap();
        std::fs::write(tmp.path().join("x.txt"), "hello").unwrap();
        assert!(all(tmp.path()).is_empty());
    }

    #[test]
    fn saving_a_list_never_replaces_another_whose_name_is_written_alike() {
        let tmp = tempfile::tempdir().unwrap();
        save(tmp.path(), "war_crimes", &ids(&["first.list"])).unwrap();
        let _ = save(tmp.path(), "war/crimes", &ids(&["second.list"]));
        assert_eq!(
            load(tmp.path(), "war_crimes").unwrap(),
            ids(&["first.list"])
        );
    }

    #[test]
    fn comparing_identical_lists_shows_no_differences() {
        let list = ids(&["a", "b", "c"]);
        assert_eq!(compare(&list, &list), Differences::default());
    }

    #[test]
    fn comparing_matches_ids_as_the_game_does() {
        let saved = ids(&["A.B", " C.d"]);
        let current = ids(&["a.b", "c.D"]);
        assert_eq!(compare(&saved, &current), Differences::default());
    }

    #[test]
    fn comparing_finds_the_mods_only_one_side_holds() {
        let saved = ids(&["a", "b", "c"]);
        let current = ids(&["b", "c", "d"]);
        let diff = compare(&saved, &current);
        assert_eq!(diff.only_saved, vec![0]);
        assert_eq!(diff.only_current, vec![2]);
        assert_eq!(diff.moved, Vec::<usize>::new());
    }

    #[test]
    fn one_mod_out_of_place_moves_only_it() {
        let saved = ids(&["a", "b", "c", "d"]);
        let current = ids(&["a", "c", "d", "b"]);
        assert_eq!(compare(&saved, &current).moved, vec![1]);
        let saved = ids(&["a", "b", "c", "d"]);
        let current = ids(&["d", "a", "b", "c"]);
        assert_eq!(compare(&saved, &current).moved, vec![3]);
    }

    #[test]
    fn a_reversed_list_keeps_its_longest_run_still() {
        let saved = ids(&["a", "b", "c"]);
        let current = ids(&["c", "b", "a"]);
        assert_eq!(compare(&saved, &current).moved, vec![1, 2]);
    }

    #[test]
    fn a_moved_mod_is_named_among_ones_only_elsewhere() {
        let saved = ids(&["a", "b", "x", "c"]);
        let current = ids(&["a", "x", "b", "y", "c"]);
        let diff = compare(&saved, &current);
        assert_eq!(diff.only_current, vec![3]);
        assert_eq!(diff.only_saved, Vec::<usize>::new());
        assert_eq!(diff.moved, vec![2]);
    }

    fn a_mod_file(config: &Path, file: &str, contents: &str) {
        let config = config.join("Config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join(file), contents).unwrap();
    }

    fn foo_settings(file: &str) -> SettingsFile {
        SettingsFile {
            file: file.to_string(),
            folder: "Foo".to_string(),
            mod_id: "ann.foo".to_string(),
        }
    }

    #[test]
    fn saved_settings_go_back_into_config_over_the_ones_set_aside() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("lists");
        let config = tmp.path().join("config");
        a_mod_file(&config, "Mod_Foo_Settings.xml", "first");
        a_mod_file(&config, "Mod_Foo_Other.xml", "other");
        a_mod_file(&config, "Mod_Bar_Settings.xml", "bar");
        save(&dir, "run", &ids(&["ann.foo"])).unwrap();
        assert_eq!(
            keep_settings(
                &dir,
                "run",
                &config,
                &[foo_settings("Mod_Foo_Settings.xml")]
            )
            .unwrap(),
            1
        );
        assert!(has_settings(&dir, "run"));
        assert!(all(&dir).iter().any(|l| l.settings));
        // The kept file is untouched, still with the mod's other file in Config.
        std::fs::write(config.join("Config/Mod_Foo_Settings.xml"), "changed").unwrap();
        let aside = tmp.path().join("aside");
        let installed = |folder: &str| matches!(folder, "Foo" | "Bar");
        let done = use_settings(&dir, "run", &config, &aside, installed).unwrap();
        // Both of Foo's files are set aside, only the kept one comes back.
        assert_eq!(
            done,
            Applied {
                set_aside: 2,
                used: 1
            }
        );
        assert_eq!(
            std::fs::read_to_string(config.join("Config/Mod_Foo_Settings.xml")).unwrap(),
            "first"
        );
        assert!(!config.join("Config/Mod_Foo_Other.xml").exists());
        assert_eq!(
            std::fs::read_to_string(aside.join("Mod_Foo_Other.xml")).unwrap(),
            "other"
        );
        // Bar's are no part of the list's settings: untouched in Config.
        assert_eq!(
            std::fs::read_to_string(config.join("Config/Mod_Bar_Settings.xml")).unwrap(),
            "bar"
        );
        // Keeping again over what was kept, then deleting, leaves nothing.
        a_mod_file(&config, "Mod_Foo_Settings.xml", "second");
        keep_settings(
            &dir,
            "run",
            &config,
            &[foo_settings("Mod_Foo_Settings.xml")],
        )
        .unwrap();
        delete(&dir, "run").unwrap();
        assert!(!has_settings(&dir, "run"));
        assert!(!settings_for(&dir, "run").exists());
    }

    #[test]
    fn keeping_no_files_and_applying_none_are_no_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("lists");
        save(&dir, "run", &ids(&["ann.foo"])).unwrap();
        assert_eq!(keep_settings(&dir, "run", tmp.path(), &[]).unwrap(), 0);
        assert!(!has_settings(&dir, "run"));
        assert!(kept_settings(&dir, "run").is_err());
    }

    #[test]
    fn a_kept_mod_without_files_now_sets_only_what_is_there_aside() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("lists");
        let config = tmp.path().join("config");
        a_mod_file(&config, "Mod_Foo_Settings.xml", "first");
        save(&dir, "run", &ids(&["ann.foo"])).unwrap();
        keep_settings(
            &dir,
            "run",
            &config,
            &[foo_settings("Mod_Foo_Settings.xml")],
        )
        .unwrap();
        std::fs::remove_file(config.join("Config/Mod_Foo_Settings.xml")).unwrap();
        let aside = tmp.path().join("aside");
        let done = use_settings(&dir, "run", &config, &aside, |_| true).unwrap();
        assert_eq!(
            done,
            Applied {
                set_aside: 0,
                used: 1
            }
        );
        assert!(!aside.exists());
        assert_eq!(
            std::fs::read_to_string(config.join("Config/Mod_Foo_Settings.xml")).unwrap(),
            "first"
        );
    }
}
