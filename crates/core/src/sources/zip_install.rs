//! A mod lands in the game's Mods folder from a downloaded archive.

use crate::base::error::{Error, Result};
use crate::said;
use std::path::{Path, PathBuf};

/// Where the mod's About sits in the archive: at the top, or one folder down.
const ABOUT: &str = "About/About.xml";

enum Root {
    Top,
    Nested(String),
}

/// The archive is unpacked beside the target, then renamed into place: one
/// partial folder is removed on failure, and the final step cannot half-run.
pub fn install_zip(zip: &Path, mods_dir: &Path) -> Result<PathBuf> {
    let file = std::fs::File::open(zip).map_err(|e| Error::file(zip, e))?;
    let bad_zip = |e: zip::result::ZipError| Error::file(zip, e);
    let mut archive = zip::ZipArchive::new(file).map_err(bad_zip)?;
    let root = find_root(&mut archive, zip)?;
    let folder = match &root {
        Root::Top => zip
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Error::refused(said!("the zip's name has no text")))?
            .to_owned(),
        Root::Nested(name) => name.clone(),
    };
    let to = mods_dir.join(&folder);
    if to.exists() {
        return Err(Error::refused(said!("{} is there already", to.display())));
    }
    let aside = mods_dir.join(format!(".rimstack-zip-{folder}"));
    let _ = std::fs::remove_dir_all(&aside);
    std::fs::create_dir(&aside).map_err(|e| Error::file(&aside, e))?;
    extract(&mut archive, zip, &aside, &root).inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&aside);
    })?;
    std::fs::rename(&aside, &to).map_err(|e| {
        let _ = std::fs::remove_dir_all(&aside);
        Error::file(&aside, e)
    })?;
    Ok(to)
}

fn find_root(archive: &mut zip::ZipArchive<std::fs::File>, zip: &Path) -> Result<Root> {
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| Error::file(zip, e))?;
        let name = entry.name().map_err(|e| Error::file(zip, e))?;
        let Some(name) = name.strip_suffix(ABOUT) else {
            continue;
        };
        if name.is_empty() {
            return Ok(Root::Top);
        }
        let trimmed = name.trim_matches('/');
        // Exactly one normal folder name, so it can never be `..` or a drive.
        if is_one_folder(trimmed) {
            return Ok(Root::Nested(trimmed.to_owned()));
        }
    }
    Err(Error::refused(said!("the zip holds no About/About.xml")))
}

/// One piece of a path, not an anchor, not root and not `.` or `..`.
fn is_one_folder(name: &str) -> bool {
    use std::path::Component;
    let path = Path::new(name);
    path.components().count() == 1 && matches!(path.components().next(), Some(Component::Normal(_)))
}

fn extract(
    archive: &mut zip::ZipArchive<std::fs::File>,
    zip: &Path,
    into: &Path,
    root: &Root,
) -> Result<()> {
    let prefix: &str = match root {
        Root::Top => "",
        Root::Nested(name) => name,
    };
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| Error::file(zip, e))?;
        let safe = entry
            .enclosed_name()
            .ok_or_else(|| Error::refused(said!("the zip reaches outside its folder")))?;
        if !safe.starts_with(prefix) {
            continue;
        }
        let inner = safe.strip_prefix(prefix).unwrap();
        if inner.as_os_str().is_empty() {
            continue;
        }
        let to = into.join(inner);
        if entry.is_dir() {
            std::fs::create_dir_all(&to).map_err(|e| Error::file(&to, e))?;
            continue;
        }
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::file(dir, e))?;
        }
        let mut out = std::fs::File::create(&to).map_err(|e| Error::file(&to, e))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| Error::file(&to, e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::install_zip;
    use crate::base::error::Kind;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use zip::write::SimpleFileOptions;

    fn zip_with(dir: &Path, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join("mod.zip");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, bytes) in entries {
            let options = match name.ends_with('/') {
                true => options.unix_permissions(0o755),
                false => options,
            };
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn a_nested_mod_lands_under_its_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = zip_with(
            tmp.path(),
            &[
                ("MyMod/", b""),
                ("MyMod/About/", b""),
                ("MyMod/About/About.xml", b"<ModMetaData/>"),
                ("MyMod/Defs/", b""),
                ("MyMod/Defs/a.xml", b"<defs/>"),
            ],
        );
        let dir = tmp.path().join("Mods");
        std::fs::create_dir(&dir).unwrap();
        let landed = install_zip(&zip_path, &dir).unwrap();
        assert_eq!(landed, dir.join("MyMod"));
        assert!(landed.join("About/About.xml").is_file());
        assert!(landed.join("Defs/a.xml").is_file());
    }

    #[test]
    fn a_top_level_mod_lands_under_the_zips_name() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = zip_with(
            tmp.path(),
            &[("About/About.xml", b"<ModMetaData/>"), ("readme.md", b"hi")],
        );
        let dir = tmp.path().join("Mods");
        std::fs::create_dir(&dir).unwrap();
        let landed = install_zip(&zip_path, &dir).unwrap();
        assert_eq!(landed, dir.join("mod"));
        assert!(landed.join("About/About.xml").is_file());
    }

    #[test]
    fn a_zip_reaching_outside_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = zip_with(
            tmp.path(),
            &[
                ("MyMod/About/About.xml", b"<ModMetaData/>"),
                ("../outside.txt", b"bad"),
            ],
        );
        let dir = tmp.path().join("Mods");
        std::fs::create_dir(&dir).unwrap();
        let err = install_zip(&zip_path, &dir).unwrap_err();
        assert_eq!(err.kind(), Kind::Refused);
        assert!(!dir.join("MyMod").exists());
        assert!(!dir.join("outside.txt").exists());
        assert!(!tmp.path().join("outside.txt").exists());
    }

    #[test]
    fn a_zip_without_about_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = zip_with(tmp.path(), &[("readme.md", b"hi")]);
        let dir = tmp.path().join("Mods");
        std::fs::create_dir(&dir).unwrap();
        let err = install_zip(&zip_path, &dir).unwrap_err();
        assert_eq!(err.kind(), Kind::Refused);
        assert!(!dir.join("mod").exists());
    }

    #[test]
    fn folders_written_as_zip_entries_stay_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = zip_with(
            tmp.path(),
            &[
                ("MyMod/", b""),
                ("MyMod/About/", b""),
                ("MyMod/About/About.xml", b"<ModMetaData/>"),
            ],
        );
        let dir = tmp.path().join("Mods");
        std::fs::create_dir(&dir).unwrap();
        let landed = install_zip(&zip_path, &dir).unwrap();
        assert!(landed.join("About").is_dir());
        assert!(landed.join("About/About.xml").is_file());
    }

    #[test]
    fn a_dot_dot_folder_never_becomes_a_mod() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = zip_with(
            tmp.path(),
            &[
                ("../About/About.xml", b"<ModMetaData/>"),
                ("mod.txt", b"hi"),
            ],
        );
        let dir = tmp.path().join("Mods");
        std::fs::create_dir(&dir).unwrap();
        let err = install_zip(&zip_path, &dir).unwrap_err();
        assert_eq!(err.kind(), Kind::Refused);
        assert!(!tmp.path().join("About").exists());
        assert!(std::fs::read_dir(&dir).unwrap().next().is_none());
    }

    #[test]
    fn an_installed_zip_is_found_by_the_mods_scan() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = zip_with(
            tmp.path(),
            &[(
                "Hats/About/About.xml",
                b"<ModMetaData><packageId>Ann.Hats</packageId><name>Hats</name></ModMetaData>",
            )],
        );
        let dir = tmp.path().join("Mods");
        std::fs::create_dir(&dir).unwrap();
        install_zip(&zip_path, &dir).unwrap();
        let roots = [crate::rimworld::paths::ModRoot {
            path: dir.clone(),
            source: crate::rimworld::paths::Source::Local,
        }];
        let found = crate::mods::library::scan(&roots);
        let hats = found.iter().find(|m| m.folder == "Hats").unwrap();
        let about = hats.about.as_ref().unwrap();
        assert_eq!(about.package_id, "Ann.Hats");
    }

    #[test]
    fn an_existing_mod_folder_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let zip_path = zip_with(tmp.path(), &[("MyMod/About/About.xml", b"<ModMetaData/>")]);
        let dir = tmp.path().join("Mods");
        std::fs::create_dir(&dir).unwrap();
        let existing = dir.join("MyMod");
        std::fs::create_dir_all(&existing).unwrap();
        std::fs::write(existing.join("sentinel.txt"), "stays").unwrap();
        let err = install_zip(&zip_path, &dir).unwrap_err();
        assert_eq!(err.kind(), Kind::Refused);
        assert!(existing.is_dir());
        assert!(existing.join("sentinel.txt").is_file());
    }
}
