//! Folders are found from the mod's id; the window never names a path to act on.

use super::App;
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::{Installed, ModId};
use rimstack_core::rimworld::paths::Source;
use rimstack_core::{said, say, say_n};
use serde::Serialize;
use std::path::PathBuf;

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
    /// local copy whose Workshop subscription then takes its place.
    fn bin_target(&self, id: ModId) -> Result<PathBuf, Error> {
        let active = self.session().current().to_vec();
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let replaced = library.copies(id).is_some_and(|c| c.local == id);
        self.installed_mod(id, |m| {
            let in_use = m
                .key()
                .is_some_and(|key| active.iter().any(|a| a.eq_ignore_ascii_case(&key)));
            match (m.source, in_use && !replaced) {
                (Source::Official, _) => Err(Error::refused(said!("The game's own mods stay"))),
                (_, true) => Err(Error::refused(said!("Deactivate it first"))),
                _ => Ok(m.path.clone()),
            }
        })?
    }

    pub fn copy_to_local(&self, id: ModId) -> Result<PathBuf, Error> {
        let local = self
            .paths()
            .local_mods()
            .ok_or_else(|| Error::refused(said!("The game's folder is not known")))?;
        let from = self.installed_mod(id, |m| m.path.clone())?;
        rimstack_core::sources::maintenance::copy_to_local(&from, &local)
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
        let subscribed = by_folder(&app, "123");
        assert_eq!(
            app.bin_target(subscribed).unwrap_err().kind(),
            Kind::Refused
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
}
