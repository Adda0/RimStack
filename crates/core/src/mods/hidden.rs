//! A hidden file is renamed inside its mod's folder, never edited or deleted,
//! and every rename is recorded so it can be put back.

use crate::base::error::Error;
use crate::base::path_text;
use crate::said;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SUFFIX: &str = ".rimstack-hidden";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hidden {
    files: BTreeMap<PathBuf, u64>,
}

fn hidden_name(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(SUFFIX);
    PathBuf::from(name)
}

/// `None` for a file that is not hidden.
pub fn original_of(hidden: &Path) -> Option<PathBuf> {
    let name = hidden.file_name()?;
    let original = match name.to_str() {
        Some(text) => std::ffi::OsString::from(text.strip_suffix(SUFFIX)?),
        None => without_suffix_bytes(name)?,
    };
    Some(hidden.with_file_name(original))
}

#[cfg(unix)]
fn without_suffix_bytes(name: &std::ffi::OsStr) -> Option<std::ffi::OsString> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = name.as_bytes().strip_suffix(SUFFIX.as_bytes())?;
    Some(std::ffi::OsStr::from_bytes(bytes).to_os_string())
}

#[cfg(not(unix))]
fn without_suffix_bytes(_name: &std::ffi::OsStr) -> Option<std::ffi::OsString> {
    None
}

impl Hidden {
    /// A damaged file is nothing hidden. Files already renamed stay renamed;
    /// `strays` finds them.
    pub fn read(path: &Path) -> Self {
        let Some(Value::Object(entries)) = crate::base::atomic_file::read_json(path) else {
            return Self::default();
        };
        let files = entries
            .iter()
            .filter_map(|(k, v)| Some((path_text::from_text(k), v.as_u64()?)))
            .collect();
        Self { files }
    }

    pub fn write(&self, path: &Path) -> Result<(), Error> {
        let entries: serde_json::Map<String, Value> = self
            .files
            .iter()
            .map(|(k, len)| (path_text::to_text(k), json!(len)))
            .collect();
        crate::base::atomic_file::write(path, Value::Object(entries).to_string())
    }

    #[cfg(test)]
    fn is_hidden(&self, texture: &Path) -> bool {
        self.files.contains_key(texture)
    }

    pub fn in_mod(&self, mod_root: &Path) -> Vec<&Path> {
        let inside = |p: &&PathBuf| p.starts_with(mod_root);
        self.files
            .keys()
            .filter(inside)
            .map(PathBuf::as_path)
            .collect()
    }

    pub fn hide(&mut self, png: &Path) -> Result<(), Error> {
        let texture = png
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("png"));
        if !texture {
            return Err(Error::refused(said!("{} is not a texture", png.display())));
        }
        let to_hide: Vec<PathBuf> = [png.to_path_buf(), png.with_extension("dds")]
            .into_iter()
            .filter(|file| !self.files.contains_key(file) && file.is_file())
            .collect();
        if let Some(taken) = to_hide.iter().find(|file| hidden_name(file).exists()) {
            return Err(Error::refused(said!(
                "A hidden copy of {} is already there; move or delete it first",
                taken.display()
            )));
        }
        for file in to_hide {
            let failed = |e: std::io::Error| Error::file(&file, e);
            let len = std::fs::metadata(&file).map_err(failed)?.len();
            std::fs::rename(&file, hidden_name(&file)).map_err(failed)?;
            self.files.insert(file, len);
        }
        Ok(())
    }

    /// Puts a hidden texture back under its own name. If a file of that
    /// name exists again (Steam restored it), the hidden copy is left
    /// where it is and the record dropped: nothing is overwritten.
    pub fn unhide(&mut self, png: &Path) -> Result<(), Error> {
        for file in [png.to_path_buf(), png.with_extension("dds")] {
            if !self.files.contains_key(&file) {
                continue;
            }
            let renamed = hidden_name(&file);
            if renamed.is_file() && !file.exists() {
                std::fs::rename(&renamed, &file).map_err(|e| Error::file(&file, e))?;
            }
            self.files.remove(&file);
        }
        Ok(())
    }

    pub fn unhide_all(&mut self, mod_root: &Path) -> Result<usize, Error> {
        let pngs: Vec<PathBuf> = self
            .in_mod(mod_root)
            .into_iter()
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")))
            .map(Path::to_path_buf)
            .collect();
        for png in &pngs {
            self.unhide(png)?;
        }
        Ok(pngs.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn textures() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("mod/Textures");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("wall.png"), "png bytes").unwrap();
        fs::write(dir.join("wall.dds"), "dds").unwrap();
        fs::write(dir.join("door.png"), "door").unwrap();
        (tmp, dir)
    }

    #[test]
    fn hiding_renames_the_png_and_its_dds_and_unhiding_puts_them_back() {
        let (_tmp, dir) = textures();
        let mut hidden = Hidden::default();
        hidden.hide(&dir.join("wall.png")).unwrap();
        assert!(!dir.join("wall.png").exists() && !dir.join("wall.dds").exists());
        assert_eq!(
            fs::read_to_string(dir.join("wall.png.rimstack-hidden")).unwrap(),
            "png bytes"
        );
        assert!(dir.join("wall.dds.rimstack-hidden").exists());
        assert!(hidden.is_hidden(&dir.join("wall.png")));
        assert!(dir.join("door.png").exists());

        hidden.hide(&dir.join("wall.png")).unwrap(); // again: nothing changes
        hidden.unhide(&dir.join("wall.png")).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("wall.png")).unwrap(),
            "png bytes"
        );
        assert_eq!(fs::read_to_string(dir.join("wall.dds")).unwrap(), "dds");
        assert!(!dir.join("wall.png.rimstack-hidden").exists());
        assert_eq!(hidden, Hidden::default());
    }

    #[test]
    fn a_whole_mod_is_unhidden_at_once_and_other_mods_are_left() {
        let (_tmp, dir) = textures();
        let (_tmp2, other) = textures();
        let mut hidden = Hidden::default();
        hidden.hide(&dir.join("wall.png")).unwrap();
        hidden.hide(&dir.join("door.png")).unwrap();
        hidden.hide(&other.join("door.png")).unwrap();
        assert_eq!(hidden.in_mod(dir.parent().unwrap()).len(), 3);
        assert_eq!(hidden.unhide_all(dir.parent().unwrap()).unwrap(), 2);
        assert!(dir.join("wall.png").exists() && dir.join("door.png").exists());
        assert!(hidden.is_hidden(&other.join("door.png")));
    }

    #[test]
    fn a_file_steam_restored_is_never_overwritten() {
        let (_tmp, dir) = textures();
        let mut hidden = Hidden::default();
        hidden.hide(&dir.join("door.png")).unwrap();
        fs::write(dir.join("door.png"), "restored by Steam").unwrap();
        hidden.unhide(&dir.join("door.png")).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("door.png")).unwrap(),
            "restored by Steam"
        );
        assert!(dir.join("door.png.rimstack-hidden").exists());
        assert!(!hidden.is_hidden(&dir.join("door.png")));
    }

    #[test]
    fn a_hidden_copy_already_there_is_never_overwritten() {
        let (_tmp, dir) = textures();
        fs::write(dir.join("wall.png.rimstack-hidden"), "older copy").unwrap();
        let mut hidden = Hidden::default();
        let refused = hidden.hide(&dir.join("wall.png")).unwrap_err();
        assert_eq!(refused.kind(), crate::base::error::Kind::Refused);
        assert_eq!(
            fs::read_to_string(dir.join("wall.png.rimstack-hidden")).unwrap(),
            "older copy"
        );
        assert_eq!(
            fs::read_to_string(dir.join("wall.png")).unwrap(),
            "png bytes"
        );
        assert_eq!(fs::read_to_string(dir.join("wall.dds")).unwrap(), "dds");
        assert_eq!(hidden, Hidden::default());
    }

    #[cfg(unix)]
    #[test]
    fn a_texture_that_cannot_be_renamed_back_stays_recorded() {
        use std::os::unix::fs::PermissionsExt;
        let (_tmp, dir) = textures();
        let mut hidden = Hidden::default();
        hidden.hide(&dir.join("door.png")).unwrap();
        let sealed = |mode| fs::set_permissions(&dir, fs::Permissions::from_mode(mode));
        sealed(0o555).unwrap();
        let result = hidden.unhide(&dir.join("door.png"));
        sealed(0o755).unwrap();
        if result.is_ok() {
            return;
        }
        assert!(hidden.is_hidden(&dir.join("door.png")));
        assert!(dir.join("door.png.rimstack-hidden").exists());
    }

    #[test]
    fn only_a_texture_is_ever_renamed() {
        let tmp = tempfile::tempdir().unwrap();
        let notes = tmp.path().join("notes.txt");
        fs::write(&notes, "mine").unwrap();
        let refused = Hidden::default().hide(&notes).unwrap_err();
        assert_eq!(refused.kind(), crate::base::error::Kind::Refused);
        assert!(notes.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_texture_with_a_name_that_is_not_utf8_comes_back() {
        use std::os::unix::ffi::OsStrExt;
        let (tmp, dir) = textures();
        let png = dir.join(std::ffi::OsStr::from_bytes(b"wall\xff.png"));
        fs::write(&png, "png").unwrap();
        let record = tmp.path().join("hidden.json");
        let mut hidden = Hidden::read(&record);
        hidden.hide(&png).unwrap();
        hidden.write(&record).unwrap();
        assert_eq!(original_of(&hidden_name(&png)), Some(png.clone()));

        let mut again = Hidden::read(&record);
        again.unhide_all(dir.parent().unwrap()).unwrap();
        assert!(png.exists());
    }

    #[test]
    fn the_record_reads_back_as_written() {
        let (tmp, dir) = textures();
        let mut hidden = Hidden::default();
        hidden.hide(&dir.join("wall.png")).unwrap();
        let file = tmp.path().join("data/hidden.json");
        hidden.write(&file).unwrap();
        assert_eq!(Hidden::read(&file), hidden);
        fs::write(&file, "x").unwrap();
        assert_eq!(Hidden::read(&file), Hidden::default());
    }
}
