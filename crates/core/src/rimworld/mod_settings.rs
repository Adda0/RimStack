//! The game keeps a mod's settings in `Config/Mod_<folder>_<class>.xml`, after the mod's
//! folder (the Workshop id for a Workshop mod) and the class that holds them.

use std::path::{Path, PathBuf};

/// Every `Mod_*.xml` file of the game's `Config` folder, by name; none when it cannot be read.
pub fn files(config: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(config.join("Config")) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && inner(path).is_some())
        .collect();
    found.sort();
    found
}

fn inner(file: &Path) -> Option<&str> {
    let name = file.file_name()?.to_str()?;
    name.strip_prefix("Mod_")?.strip_suffix(".xml")
}

/// The installed folder a file is named after: `Foo` and `Foo_Bar` both fit
/// `Mod_Foo_Bar_Settings.xml`, and the longest takes it.
pub fn owner(file: &Path, installed: impl Fn(&str) -> bool) -> Option<&str> {
    let inner = inner(file)?;
    inner
        .rmatch_indices('_')
        .map(|(at, _)| &inner[..at])
        .filter(|folder| !folder.is_empty() && folder.len() + 1 < inner.len())
        .find(|folder| installed(folder))
}

/// `installed` holds `folder` and every other installed folder, as a longer one may take a
/// file that starts like this one's.
pub fn of(config: &Path, folder: &str, installed: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let mut found = files(config);
    found.retain(|file| {
        inner(file).is_some_and(|name| name.starts_with(folder))
            && owner(file, &installed) == Some(folder)
    });
    found
}

/// The files no installed mod's folder names.
pub fn orphans(config: &Path, installed: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let mut found = files(config);
    found.retain(|file| owner(file, &installed).is_none());
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(names: &[&str]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("Config");
        std::fs::create_dir_all(&config).unwrap();
        for name in names {
            std::fs::write(config.join(name), "<SettingsBlock/>").unwrap();
        }
        tmp
    }

    fn names(files: &[PathBuf]) -> Vec<String> {
        files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    fn installed(folder: &str) -> bool {
        ["1713367505", "Foo", "Foo_Bar", "Choose Your Medicine"].contains(&folder)
    }

    #[test]
    fn a_mod_s_settings_are_found_by_its_folder_and_the_longest_folder_wins() {
        let tmp = config(&[
            "Mod_1713367505_RimSaves.xml",
            "Mod_Foo_Settings.xml",
            "Mod_foo_Settings.xml",
            "Mod_Foo_Bar_Settings.xml",
            "Mod_Foo_.xml",
            "Mod_Choose Your Medicine_Settings_ChooseYourMedicine.xml",
            "ModsConfig.xml",
            "Prefs.xml",
        ]);
        let of = |folder| names(&of(tmp.path(), folder, installed));
        assert_eq!(of("1713367505"), ["Mod_1713367505_RimSaves.xml"]);
        assert_eq!(of("Foo"), ["Mod_Foo_Settings.xml"]);
        assert_eq!(of("Foo_Bar"), ["Mod_Foo_Bar_Settings.xml"]);
        assert_eq!(
            of("Choose Your Medicine"),
            ["Mod_Choose Your Medicine_Settings_ChooseYourMedicine.xml"]
        );
    }

    #[test]
    fn orphans_are_the_settings_no_installed_mod_s_folder_names() {
        let tmp = config(&[
            "Mod_1713367505_RimSaves.xml",
            "Mod_Foo_Bar_Settings.xml",
            "Mod_2009463077_HarmonySettings.xml",
            "Mod_Gone_Settings.xml",
            "ModsConfig.xml",
        ]);
        std::fs::create_dir(tmp.path().join("Config/Mod_Dir_NotAFile.xml")).unwrap();
        assert_eq!(
            names(&orphans(tmp.path(), installed)),
            [
                "Mod_2009463077_HarmonySettings.xml",
                "Mod_Gone_Settings.xml"
            ]
        );
        assert!(orphans(&tmp.path().join("nowhere"), installed).is_empty());
    }
}
