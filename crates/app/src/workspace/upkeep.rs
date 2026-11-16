//! Folders are found from the mod's id; the window never names a path to act on.

use super::App;
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::{Installed, ModId};
use rimstack_core::rimworld::paths::Source;
use rimstack_core::sources::maintenance;
use rimstack_core::{said, say, say_n};
use serde::Serialize;
use std::path::{Path, PathBuf};

impl App {
    fn installed_mod<T>(&self, id: ModId, with: impl FnOnce(&Installed) -> T) -> Result<T, Error> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let found = library.get(id).ok_or_else(|| {
            Error::refused(said!(
                "That mod has moved since it was read; read the folders again"
            ))
        })?;
        Ok(with(found))
    }

    pub fn bin_mod(&self, id: ModId) -> Result<(), Error> {
        let folder = self.bin_target(id)?;
        trash::delete(&folder).map_err(|problem| Error::file(&folder, problem))
    }

    /// The way `kind` deletes the mod, as its row menu asked.
    pub fn bin_as(&self, id: ModId, kind: BinKind) -> Result<(), Error> {
        match kind {
            BinKind::Whole => self.bin_mod(id),
            BinKind::KeepDds => self.bin_except_dds(id).map(|_| ()),
            BinKind::DdsOnly => self.remove_dds(id).map(|_| ()),
            BinKind::Resubscribe => self.bin_and_resubscribe(id),
        }
    }

    /// The DDS textures stay where they are, so a resubscribed Workshop mod reads them
    /// again without converting anything.
    pub fn bin_except_dds(&self, id: ModId) -> Result<usize, Error> {
        let folder = self.bin_target(id)?;
        let files = maintenance::files_except_dds(&folder);
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

    /// The Workshop mods among `ids` that have no copy in the game's Mods folder yet, with the
    /// words to ask before copying them.
    pub fn copy_offer(&self, ids: &[ModId]) -> CopyOffer {
        let loaded = self.library();
        let (Ok((library, _)), Some(local)) = (loaded.as_ref(), self.paths().local_mods()) else {
            return CopyOffer::default();
        };
        let copyable: Vec<(ModId, &Installed)> = library
            .to_copy(ids)
            .into_iter()
            .filter_map(|(id, m)| Some((id, m.ok()?)))
            .collect();
        let into = Line::path(&local);
        let (title, lines) = match copyable.as_slice() {
            [(_, one)] => (
                say!("Copy {} to the game's Mods folder", one.display_name()),
                vec![
                    Line::Said(say!("From")),
                    Line::path(&one.path),
                    Line::Said(say!("Into")),
                    into,
                    Line::Said(say!(
                        "The Workshop copy stays where it is; Steam keeps updating that one, not this."
                    )),
                ],
            ),
            many => (
                say_n!(
                    many.len(),
                    "Copy {} mod to the game's Mods folder",
                    "Copy {} mods to the game's Mods folder"
                ),
                vec![
                    Line::Said(say!("Into")),
                    into,
                    Line::Said(say!(
                        "The Workshop copies stay where they are; Steam keeps updating those, not these."
                    )),
                ],
            ),
        };
        CopyOffer {
            ids: copyable.iter().map(|(id, _)| *id).collect(),
            title,
            lines,
        }
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

    /// Each is tried, whatever became of the others.
    pub fn copy_to_local(&self, ids: &[ModId]) -> Result<(), Error> {
        let local = self
            .paths()
            .local_mods()
            .ok_or_else(|| Error::refused(said!("The game's folder is not known")))?;
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let failed: Vec<Error> = library
            .to_copy(ids)
            .into_iter()
            .filter_map(|(_, m)| {
                m.and_then(|m| maintenance::copy_to_local(&m.path, &m.folder, &local))
                    .err()
            })
            .collect();
        match failed.first() {
            None => Ok(()),
            Some(only) if ids.len() == 1 => Err(only.clone()),
            Some(first) => Err(Error::new(
                first.kind(),
                said!(
                    "{} of {} not copied: {}",
                    failed.len(),
                    ids.len(),
                    first.within()
                ),
            )),
        }
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
}

/// The ways a mod's row menu offers to delete it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BinKind {
    Whole,
    KeepDds,
    DdsOnly,
    Resubscribe,
}

impl BinKind {
    /// The whole mod only where it may go; the DDS files only where some were made.
    pub fn offered(made: usize, workshop: bool, whole: bool) -> Vec<BinKind> {
        let mut kinds = Vec::new();
        if whole {
            kinds.extend([BinKind::Whole, BinKind::KeepDds]);
        }
        if made > 0 {
            kinds.push(BinKind::DdsOnly);
        }
        if whole && workshop {
            kinds.push(BinKind::Resubscribe);
        }
        kinds
    }

    pub fn entry(self) -> (Sentence, &'static str) {
        match self {
            BinKind::Whole => (say!("Move to the bin…"), "trash-2"),
            BinKind::KeepDds => (
                say!("Delete its files, but keep the .dds textures…"),
                "trash-2",
            ),
            BinKind::DdsOnly => (say!("Delete the .dds textures…"), "image"),
            BinKind::Resubscribe => (say!("Delete and subscribe again…"), "download"),
        }
    }

    pub fn title(self, name: &str) -> Sentence {
        match self {
            BinKind::KeepDds => say!("Delete the files of {} but keep its .dds textures", name),
            BinKind::DdsOnly => say!("Delete the .dds textures of {}", name),
            BinKind::Resubscribe => say!("Delete {} and subscribe to it again", name),
            BinKind::Whole => say!("Move to the bin: {}", name),
        }
    }

    pub fn note(self, made: usize) -> Sentence {
        match self {
            BinKind::KeepDds => say!(
                "Everything but its .dds textures goes to the bin, so the mod stops being listed; the textures stay for the copy Steam downloads again."
            ),
            BinKind::DdsOnly => say_n!(
                made,
                "{} texture RimStack made is removed, and the game loads the .png instead.",
                "{} textures RimStack made are removed, and the game loads the .png instead."
            ),
            BinKind::Resubscribe => {
                say!("Its folder goes to the bin, and Steam downloads the mod again.")
            }
            BinKind::Whole => {
                say!("Steam downloads a Workshop mod again while you stay subscribed to it.")
            }
        }
    }

    pub fn label(self) -> Sentence {
        match self {
            BinKind::KeepDds => say!("Delete the files"),
            BinKind::DdsOnly => say!("Delete the textures"),
            BinKind::Resubscribe => say!("Delete and subscribe again"),
            BinKind::Whole => say!("Move to the bin"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CopyOffer {
    pub ids: Vec<ModId>,
    pub title: Sentence,
    /// A text or a path, as the window's question shows them.
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Line {
    Said(Sentence),
    Path { path: String },
}

impl Line {
    fn path(path: &Path) -> Self {
        Line::Path {
            path: path.to_string_lossy().into_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::workspace::tests::installed;
    use rimstack_core::base::error::Kind;
    use rimstack_core::mods::library::ModId;
    use rimstack_core::rimworld::paths::Source;

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

    fn subscribe(root: &std::path::Path, id: &str, package_id: &str) {
        let about = root.join("workshop").join(id).join("About");
        std::fs::create_dir_all(&about).unwrap();
        let xml = format!("<ModMetaData><packageId>{package_id}</packageId></ModMetaData>");
        std::fs::write(about.join("About.xml"), xml).unwrap();
    }

    fn subscribed(app: &crate::App, folder: &str) -> ModId {
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let mut ids = library.ids();
        ids.find(|(_, m)| m.source == Source::Workshop && m.folder == folder)
            .unwrap()
            .0
    }

    #[test]
    fn a_mod_is_copied_into_the_game_s_folder_once_with_its_workshop_id() {
        let (tmp, app) = installed();
        subscribe(tmp.path(), "123", "Ann.Hats");
        app.rescan();
        let hats = subscribed(&app, "123");
        app.copy_to_local(&[hats]).unwrap();
        let copy = tmp.path().join("game/Mods/123");
        assert_eq!(
            std::fs::read_to_string(copy.join("About/PublishedFileId.txt")).unwrap(),
            "123"
        );
        assert!(
            !tmp.path()
                .join("workshop/123/About/PublishedFileId.txt")
                .exists()
        );
        assert_eq!(
            app.copy_to_local(&[hats]).unwrap_err().kind(),
            Kind::Refused
        );
        app.rescan();
        assert!(app.copy_offer(&[subscribed(&app, "123")]).ids.is_empty());
    }

    #[test]
    fn of_a_selection_only_workshop_mods_without_a_copy_are_offered_and_each_is_tried() {
        let (tmp, app) = installed();
        subscribe(tmp.path(), "123", "Ann.Hats");
        subscribe(tmp.path(), "456", "Ann.Roads");
        subscribe(tmp.path(), "789", "Ann.Boots");
        app.rescan();
        let (hats, roads, boots) = (
            subscribed(&app, "123"),
            subscribed(&app, "456"),
            subscribed(&app, "789"),
        );
        let (core, local_roads) = (by_folder(&app, "Core"), by_folder(&app, "roads"));
        let offer = app.copy_offer(&[core, local_roads, hats, roads, boots]);
        assert_eq!(offer.ids, [hats, boots]);
        assert_eq!(
            offer.title.to_string(),
            "Copy 2 mods to the game's Mods folder"
        );
        assert_eq!(
            app.copy_offer(&[hats, roads]).title.to_string(),
            "Copy Ann.Hats to the game's Mods folder"
        );
        std::fs::create_dir_all(tmp.path().join("game/Mods/789")).unwrap();
        let problem = app.copy_to_local(&[hats, boots, roads, core]).unwrap_err();
        assert_eq!(problem.kind(), Kind::Refused);
        assert!(
            problem.to_string().starts_with("3 of 4 not copied: "),
            "{problem}"
        );
        assert!(tmp.path().join("game/Mods/123/About/About.xml").is_file());
        assert!(!tmp.path().join("game/Mods/456").exists());
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
