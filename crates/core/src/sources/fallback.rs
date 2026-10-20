//! A copy of each active Workshop mod, kept at every launch whose update it is not kept from
//! yet (by the Workshop's update time), so that an update that broke a save can be rolled back
//! into the game's Mods folder, where a copy wins over Steam's. Kept under RimStack's data
//! folder as `<store>/<Workshop id>/<time updated>`, pruned to a size the user sets.

use crate::base::error::{Error, Kind, Result};
use crate::base::walk;
use crate::said;
use std::path::{Path, PathBuf};

pub const DEFAULT_CAP_MEGABYTES: u64 = 5120;

/// The launch whose flow keeps copies. Steam rewrites a mod's files under update, so hard
/// linked copies would change under the kept one: real copies it is, within `prune`'s cap.
pub fn store(data_dir: &Path) -> PathBuf {
    data_dir.join("kept-mods")
}

/// The Workshop times of the kept copies of a mod, oldest first. Only whole folders with a
/// time for a name count: an interrupted `keep` leaves a partial another keeps over.
pub fn kept(store: &Path, workshop_id: &str) -> Vec<u64> {
    let mut times: Vec<u64> = std::fs::read_dir(store.join(workshop_id))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u64>().ok())
        .collect();
    times.sort_unstable();
    times
}

/// The ids whose Workshop time is newer than the newest kept copy: the only ones a launch
/// copies, so a launch with no updates copies nothing.
pub fn to_keep(now: &[(String, u64)], newest_kept: impl Fn(&str) -> Option<u64>) -> Vec<String> {
    now.iter()
        .filter(|(id, at)| newest_kept(id).is_none_or(|kept| kept < *at))
        .map(|(id, _)| id.clone())
        .collect()
}

/// The kept copy's size: its files', as ever.
fn size_of(dir: &Path) -> u64 {
    walk::files(dir)
        .iter()
        .filter_map(|file| std::fs::metadata(file).ok().map(|m| m.len()))
        .sum()
}

/// Copies `mod_root` into `store/<id>/<time updated>`; `Ok(false)` when that copy is kept
/// already or Steam said no update time for the mod.
pub fn keep(mod_root: &Path, store: &Path, workshop_id: &str, updated: u64) -> Result<bool> {
    if updated == 0 {
        return Ok(false);
    }
    let to = store.join(workshop_id).join(updated.to_string());
    if to.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(&to).map_err(|e| Error::file(&to, e))?;
    walk::copy(mod_root, &to).map_err(|e| {
        let _ = remove_if_dir(&to);
        Error::file(&to, e)
    })?;
    Ok(true)
}

fn remove_if_dir(dir: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(dir).is_ok() {
        true => std::fs::remove_dir_all(dir),
        false => Ok(()),
    }
}

/// Deletes the oldest kept copies until the store is under `cap_megabytes`; the copy of the
/// same Workshop time as now is never at stake, so pruning cannot undo the newest keep.
pub fn prune(store: &Path, cap_megabytes: u64) -> Result<usize> {
    let mut all: Vec<(u64, PathBuf)> = std::fs::read_dir(store)
        .into_iter()
        .flatten()
        .flatten()
        .flat_map(|entry| {
            std::fs::read_dir(entry.path())
                .into_iter()
                .flatten()
                .flatten()
                .map(|inside| inside.path())
                .collect::<Vec<_>>()
        })
        .filter_map(|copy| Some((copy.file_name()?.to_str()?.parse::<u64>().ok()?, copy)))
        .collect();
    let mut gone = 0;
    let mut total: u64 = all.iter().map(|(_, copy)| size_of(copy)).sum();
    all.sort_unstable();
    for (_, copy) in all {
        if total <= cap_megabytes.saturating_mul(1_000_000) {
            break;
        }
        let size = size_of(&copy);
        remove_if_dir(&copy)
            .map_err(|e| Error::caused(Kind::File, said!("a kept copy could not be deleted"), e))?;
        total = total.saturating_sub(size);
        gone += 1;
    }
    Ok(gone)
}

/// There is no merge or replace: restoring is a plain copy into the game's Mods folder, which
/// then wins over the Workshop's, so a folder that is already there keeps it back.
pub fn restore(
    store: &Path,
    workshop_id: &str,
    updated: u64,
    local_mods: &Path,
) -> Result<PathBuf> {
    let from = store.join(workshop_id).join(updated.to_string());
    if !from.is_dir() {
        return Err(Error::refused(said!("That kept copy is gone")));
    }
    let to = local_mods.join(workshop_id);
    if to.exists() {
        return Err(Error::refused(said!("{} is there already", to.display())));
    }
    walk::copy(&from, &to).map_err(|e| {
        let _ = remove_if_dir(&to);
        Error::file(&to, e)
    })?;
    // The game and other tools read the item from here, as with a SteamCMD copy.
    let noted = to.join("About").join("PublishedFileId.txt");
    if !noted.exists() {
        std::fs::write(&noted, workshop_id).map_err(|e| Error::file(&noted, e))?;
    }
    Ok(to)
}

/// The stamps a restore menu offers: newest first.
pub fn offered(store: &Path, workshop_id: &str) -> Vec<u64> {
    let mut times = kept(store, workshop_id);
    times.reverse();
    times
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mod_with(root: &Path, id: &str, what: &str) -> PathBuf {
        let at = root.join("workshop").join(id);
        std::fs::create_dir_all(at.join("About")).unwrap();
        std::fs::write(at.join("About/About.xml"), what).unwrap();
        at
    }

    #[test]
    fn only_a_mod_the_workshop_has_newer_than_its_newest_kept_copy_is_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let store = store(tmp.path());
        let fresh = mod_with(tmp.path(), "100", "new");
        let now = [("100".to_string(), 20_u64), ("200".to_string(), 5)];
        assert_eq!(to_keep(&now, |_| None).len(), 2);
        assert!(keep(&fresh, &store, "100", 20).unwrap());
        assert!(keep(&mod_with(tmp.path(), "100", "old"), &store, "100", 10).unwrap());
        assert_eq!(
            to_keep(&now, |id| kept(&store, id).last().copied()),
            vec!["200".to_string()]
        );
        // The stamp of now is kept, so the same launch keeps nothing twice.
        assert_eq!(
            to_keep(&[("100".to_string(), 20)], |id| kept(&store, id)
                .last()
                .copied()),
            Vec::<String>::new()
        );
        assert_eq!(offered(&store, "100"), vec![20, 10]);
    }

    #[test]
    fn a_kept_copy_restores_into_the_mods_folder_which_then_wins_but_never_over() {
        let tmp = tempfile::tempdir().unwrap();
        let store = store(tmp.path());
        let fresh = mod_with(tmp.path(), "100", "before the update");
        keep(&fresh, &store, "100", 20).unwrap();
        let local = tmp.path().join("Mods");
        std::fs::create_dir_all(&local).unwrap();
        let to = restore(&store, "100", 20, &local).unwrap();
        assert_eq!(to, local.join("100"));
        assert_eq!(
            std::fs::read_to_string(to.join("About/About.xml")).unwrap(),
            "before the update"
        );
        assert_eq!(
            std::fs::read_to_string(to.join("About/PublishedFileId.txt")).unwrap(),
            "100"
        );
        assert!(
            restore(&store, "100", 20, &local).is_err(),
            "the copy that won stays"
        );
        assert!(restore(&store, "100", 9, &local).is_err());
    }

    #[test]
    fn the_oldest_kept_copies_go_once_over_the_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let store = store(tmp.path());
        keep(&mod_with(tmp.path(), "100", "old"), &store, "100", 10).unwrap();
        keep(&mod_with(tmp.path(), "100", "new"), &store, "100", 20).unwrap();
        assert_eq!(kept(&store, "100"), vec![10, 20]);
        assert_eq!(prune(&store, 0).unwrap(), 2);
        assert!(kept(&store, "100").is_empty());
        keep(&mod_with(tmp.path(), "100", "new"), &store, "100", 20).unwrap();
        assert_eq!(prune(&store, 10).unwrap(), 0);
        assert_eq!(kept(&store, "100"), vec![20]);
    }
}
