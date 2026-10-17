//! Folders are found from the mod's id; the window never names a path to act on.

use super::App;
use rimstack_core::base::error::Error;
use rimstack_core::mods::library::{Installed, ModId};
use rimstack_core::rimworld::paths::Source;
use rimstack_core::said;
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
