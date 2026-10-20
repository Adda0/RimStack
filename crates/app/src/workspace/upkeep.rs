//! Folders are found from the mod's id; the window never names a path to act on.

use super::App;
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::{Installed, ModId};
use rimstack_core::rimworld::paths::Source;
use rimstack_core::{said, say, say_n};
use serde::Serialize;
use std::path::{Path, PathBuf};

impl App {
    fn installed_mod<T>(&self, id: ModId, with: impl FnOnce(&Installed) -> T) -> Result<T, Error> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let found = library.get(id).ok_or_else(|| {
            Error::refused(said!(
                "That mod is no longer where it was read; read the folders again"
            ))
        })?;
        Ok(with(found))
    }

    pub fn bin_mod(&self, id: ModId) -> Result<(), Error> {
        let folder = self.bin_target(id)?;
        trash::delete(&folder).map_err(|problem| Error::file(&folder, problem))
    }

    /// The DDS textures stay where they are, so a resubscribed Workshop mod reads them
    /// again without converting anything.
    pub fn bin_except_dds(&self, id: ModId) -> Result<usize, Error> {
        let folder = self.bin_target(id)?;
        let files = rimstack_core::sources::maintenance::files_except_dds(&folder);
        let gone = files.len();
        trash::delete_all(&files).map_err(|problem| Error::file(&folder, problem))?;
        Ok(gone)
    }

    /// Only the DDS files RimStack made: the game goes back to loading the PNGs.
    pub fn remove_dds(&self, id: ModId) -> Result<usize, Error> {
        let folder = self.installed_mod(id, |m| m.path.clone())?;
        let file = self.texture_manifest_file();
        let mut manifest = rimstack_core::mods::textures::Manifest::read(&file);
        let removed = rimstack_core::mods::textures::remove(&folder, &mut manifest);
        manifest
            .write(&file)
            .map_err(|problem| Error::file(&file, problem))?;
        self.textures_changed();
        Ok(removed)
    }

    /// Steam drops the copy in its own hands only after the unsubscription, so that
    /// comes first; the subscription it is sent next downloads the mod anew.
    pub fn bin_and_resubscribe(&self, id: ModId) -> Result<(), Error> {
        let workshop = self.workshop_id(id).ok_or_else(|| {
            Error::refused(said!("Only a Workshop mod is downloaded again by Steam"))
        })?;
        let folder = self.bin_target(id)?;
        trash::delete(&folder).map_err(|problem| Error::file(&folder, problem))?;
        let ids = std::slice::from_ref(&workshop);
        let unsubscribed = self
            .ask_steam(super::fetch::SteamAsk::Unsubscribe, ids)
            .note;
        std::thread::sleep(std::time::Duration::from_secs(1));
        let subscribed = self.ask_steam(super::fetch::SteamAsk::Subscribe, ids).note;
        unsubscribed?;
        subscribed.map(|_| ())
    }

    /// The game's own mods and those in the load order being edited are refused, but for a
    /// copy whose place for the same order entry another copy of its id then takes.
    pub(crate) fn bin_target(&self, id: ModId) -> Result<PathBuf, Error> {
        let active = self.session().current().to_vec();
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let loaded_as = library.loaded_as(id, &active);
        let replaced = loaded_as.is_some_and(|entry| library.another_loads(id, entry));
        let in_use = loaded_as.is_some();
        self.installed_mod(id, |m| match (m.source, in_use && !replaced) {
            (Source::Official, _) => Err(Error::refused(said!("The game's own mods stay"))),
            (_, true) => Err(Error::refused(said!("Deactivate it first"))),
            _ => Ok(m.path.clone()),
        })?
    }

    /// A dropped or picked archive lands in the game's Mods folder; never
    /// while RimWorld runs, or it would read a mod it was not told about.
    pub fn install_zip(&self, zip: &Path) -> Result<PathBuf, Error> {
        if self.game_running() {
            return Err(Error::refused(said!(
                "RimWorld is running. Close the game before installing a mod."
            )));
        }
        let local = self
            .paths()
            .local_mods()
            .ok_or_else(|| Error::refused(said!("The game's folder is not known")))?;
        rimstack_core::sources::zip_install::install_zip(zip, &local)
    }

    pub fn copy_to_local(&self, id: ModId) -> Result<PathBuf, Error> {
        let local = self
            .paths()
            .local_mods()
            .ok_or_else(|| Error::refused(said!("The game's folder is not known")))?;
        let from = self.installed_mod(id, |m| m.path.clone())?;
        rimstack_core::sources::maintenance::copy_to_local(&from, &local)
    }

    /// Off the window's thread: many Workshop folders are gigabytes of copying.
    pub fn copy_many_to_local(
        &self,
        ids: &[ModId],
    ) -> Result<rimstack_core::sources::maintenance::Copied, Error> {
        let local = self
            .paths()
            .local_mods()
            .ok_or_else(|| Error::refused(said!("The game's folder is not known")))?;
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let roots: Vec<PathBuf> = ids
            .iter()
            .filter_map(|&id| library.get(id).map(|m| m.path.clone()))
            .collect();
        rimstack_core::sources::maintenance::copy_many_to_local(&roots, &local)
    }

    /// The Workshop update times of the copies kept for this mod, newest first.
    pub fn kept_copies_of(&self, id: ModId) -> Vec<u64> {
        let folder = self
            .installed_mod(id, |m| {
                (m.source == Source::Workshop).then(|| m.folder.clone())
            })
            .ok()
            .flatten();
        match folder {
            Some(folder) => rimstack_core::sources::fallback::offered(
                &rimstack_core::sources::fallback::store(self.data_dir()),
                &folder,
            ),
            None => Vec::new(),
        }
    }

    /// Rolling back a Workshop update: the kept copy is what goes into the game's Mods
    /// folder, where it then wins, so restoring does nothing while a folder of that item
    /// is already there.
    pub fn restore_kept_copy(&self, id: ModId, updated: u64) -> Result<PathBuf, Error> {
        let local = self
            .paths()
            .local_mods()
            .ok_or_else(|| Error::refused(said!("The game's folder is not known")))?;
        let folder = self.installed_mod(id, |m| {
            (m.source == Source::Workshop).then(|| m.folder.clone())
        })?;
        let Some(folder) = folder else {
            return Err(Error::refused(said!("Not a Workshop mod")));
        };
        rimstack_core::sources::fallback::restore(
            &rimstack_core::sources::fallback::store(self.data_dir()),
            &folder,
            updated,
            &local,
        )
    }

    /// Every one allowed to go; the count says how many folders were moved.
    /// Stops at the first folder the bin refuses.
    pub fn bin_mods(&self, ids: &[ModId]) -> Result<usize, Error> {
        let mut folders = Vec::new();
        for &id in ids {
            if let Ok(folder) = self.bin_target(id) {
                folders.push(folder);
            }
        }
        let binned = folders.len();
        for folder in folders {
            trash::delete(&folder).map_err(|problem| Error::file(&folder, problem))?;
        }
        Ok(binned)
    }

    /// The title and the note of the prompt that confirms `kind` for this mod.
    pub fn bin_prompt(&self, id: ModId, kind: i32) -> (Sentence, Sentence) {
        let made = self
            .installed_mod(id, |m| self.texture_manifest().made_in(&m.path))
            .unwrap_or(0);
        let name = self
            .installed_mod(id, |m| m.display_name().to_string())
            .unwrap_or_default();
        (bin_title(kind, &name), bin_note(kind, made))
    }
}

/// What a multi-selection offers for cleanup, each with the folders' bytes.
#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Cleanup {
    pub(crate) unsubscribe: usize,
    pub(crate) unsubscribe_bytes: u64,
    pub(crate) bin: usize,
    pub(crate) bin_bytes: u64,
}

pub const BIN_WHOLE: i32 = 0;
pub const BIN_KEEP_DDS: i32 = 1;
pub const BIN_DDS_ONLY: i32 = 2;
pub const BIN_RESUBSCRIBE: i32 = 3;

/// One way to delete a mod, as its menu offers it; `kind` is what the window sends back.
#[derive(Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BinChoice {
    kind: i32,
    text: Sentence,
    icon: &'static str,
}

fn choice(kind: i32, text: Sentence, icon: &'static str) -> BinChoice {
    BinChoice { kind, text, icon }
}

/// The whole mod is offered only where it may go; the DDS files only where some were made.
pub fn bin_choices(made: usize, workshop: bool, whole: bool) -> Vec<BinChoice> {
    let mut choices = Vec::new();
    if whole {
        choices.push(choice(BIN_WHOLE, say!("Move to the bin…"), "trash-2"));
        choices.push(choice(
            BIN_KEEP_DDS,
            say!("Delete its files, but keep the .dds textures…"),
            "trash-2",
        ));
    }
    if made > 0 {
        choices.push(choice(
            BIN_DDS_ONLY,
            say!("Delete the .dds textures…"),
            "image",
        ));
    }
    if whole && workshop {
        choices.push(choice(
            BIN_RESUBSCRIBE,
            say!("Delete and subscribe again…"),
            "download",
        ));
    }
    choices
}

pub fn bin_title(kind: i32, name: &str) -> Sentence {
    match kind {
        BIN_KEEP_DDS => say!("Delete the files of {} but keep its .dds textures", name),
        BIN_DDS_ONLY => say!("Delete the .dds textures of {}", name),
        BIN_RESUBSCRIBE => say!("Delete {} and subscribe to it again", name),
        _ => say!("Move to the bin: {}", name),
    }
}

pub fn bin_note(kind: i32, made: usize) -> Sentence {
    match kind {
        BIN_KEEP_DDS => say!(
            "Everything but its .dds textures goes to the bin, so the mod stops being listed; the textures stay for the copy Steam downloads again."
        ),
        BIN_DDS_ONLY => say_n!(
            made,
            "{} texture RimStack made is removed, and the game loads the .png instead.",
            "{} textures RimStack made are removed, and the game loads the .png instead."
        ),
        BIN_RESUBSCRIBE => {
            say!("Its folder goes to the bin, and Steam downloads the mod again.")
        }
        _ => say!("Steam downloads a Workshop mod again while you stay subscribed to it."),
    }
}

pub fn bin_label(kind: i32) -> Sentence {
    match kind {
        BIN_KEEP_DDS => say!("Delete the files"),
        BIN_DDS_ONLY => say!("Delete the textures"),
        BIN_RESUBSCRIBE => say!("Delete and subscribe again"),
        _ => say!("Move to the bin"),
    }
}

#[cfg(test)]
mod tests {
    use crate::workspace::tests::installed;
    use rimstack_core::base::error::Kind;
    use rimstack_core::mods::library::ModId;

    fn by_folder(app: &crate::App, folder: &str) -> ModId {
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();

        library.ids().find(|(_, m)| m.folder == folder).unwrap().0
    }

    #[test]
    fn the_game_s_own_mods_and_active_ones_are_never_binned() {
        let (tmp, app) = installed();
        let core = by_folder(&app, "Core");
        assert_eq!(app.bin_mod(core).unwrap_err().kind(), Kind::Refused);
        assert!(tmp.path().join("game/Data/Core").is_dir());
        app.session()
            .record(vec!["ludeon.rimworld".into(), "ann.roads".into()]);
        let roads = by_folder(&app, "roads");
        assert_eq!(app.bin_mod(roads).unwrap_err().kind(), Kind::Refused);
        assert!(tmp.path().join("game/Mods/roads").is_dir());
        assert_eq!(
            app.bin_mod(ModId::new(0, 99)).unwrap_err().kind(),
            Kind::Refused
        );
    }

    #[test]
    fn an_active_local_copy_may_be_binned_when_its_workshop_subscription_takes_its_place() {
        let (tmp, app) = installed();
        std::fs::create_dir_all(tmp.path().join("workshop/123/About")).unwrap();
        std::fs::write(
            tmp.path().join("workshop/123/About/About.xml"),
            "<ModMetaData><packageId>Ann.Roads</packageId></ModMetaData>",
        )
        .unwrap();
        app.rescan();
        app.session()
            .record(vec!["ludeon.rimworld".into(), "ann.roads".into()]);
        let roads = by_folder(&app, "roads");
        assert_eq!(
            app.bin_target(roads).unwrap(),
            tmp.path().join("game/Mods/roads")
        );
        // An order entry names only the local copy, so the Workshop one may go without
        // deactivating anything first.
        let subscribed = by_folder(&app, "123");
        assert_eq!(
            app.bin_target(subscribed).unwrap(),
            tmp.path().join("workshop/123")
        );
    }

    #[test]
    fn a_workshop_copy_named_by_its_steam_entry_is_refused() {
        let (tmp, app) = installed();
        std::fs::create_dir_all(tmp.path().join("workshop/123/About")).unwrap();
        std::fs::write(
            tmp.path().join("workshop/123/About/About.xml"),
            "<ModMetaData><packageId>Ann.Roads</packageId></ModMetaData>",
        )
        .unwrap();
        app.rescan();
        app.session()
            .record(vec!["ludeon.rimworld".into(), "ann.roads_steam".into()]);
        let subscribed = by_folder(&app, "123");
        assert_eq!(
            app.bin_target(subscribed).unwrap_err().kind(),
            Kind::Refused
        );
        // The local copy it stands in for, by the plain entry, may go.
        let roads = by_folder(&app, "roads");
        assert_eq!(
            app.bin_target(roads).unwrap(),
            tmp.path().join("game/Mods/roads")
        );
    }

    #[test]
    fn a_mod_is_copied_into_the_game_s_folder_once() {
        let (tmp, app) = installed();
        std::fs::create_dir_all(tmp.path().join("workshop/123/About")).unwrap();
        std::fs::write(
            tmp.path().join("workshop/123/About/About.xml"),
            "<ModMetaData><packageId>Ann.Hats</packageId></ModMetaData>",
        )
        .unwrap();
        app.rescan();
        let hats = by_folder(&app, "123");
        let copy = app.copy_to_local(hats).unwrap();
        assert_eq!(copy, tmp.path().join("game/Mods/123"));
        assert_eq!(app.copy_to_local(hats).unwrap_err().kind(), Kind::Refused);
    }

    #[test]
    fn a_zip_lands_in_the_game_s_mods_folder_once_and_shows_after_a_rescan() {
        let (tmp, app) = installed();
        let zip_path = tmp.path().join("hats.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(
            "Hats/About/About.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        std::io::Write::write_all(
            &mut zip,
            b"<ModMetaData><packageId>Ann.Hats</packageId></ModMetaData>",
        )
        .unwrap();
        zip.finish().unwrap();
        let landed = app.install_zip(&zip_path).unwrap();
        assert_eq!(landed, tmp.path().join("game/Mods/Hats"));
        assert!(landed.join("About/About.xml").is_file());
        assert_eq!(
            app.install_zip(&zip_path).unwrap_err().kind(),
            Kind::Refused
        );
        app.rescan();
        let _ = by_folder(&app, "Hats");
    }
}
