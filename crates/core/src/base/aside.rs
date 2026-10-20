use crate::base::error::{Error, Result};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub const OLD: &str = ".rimstack-old";
pub const NEW: &str = ".rimstack-new";

/// Appended to the whole name: `with_extension` would replace a dotted part of it.
pub fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

pub fn left_beside(name: &OsStr) -> Option<&str> {
    let name = name.to_str()?;
    let owner = name.strip_suffix(OLD).or_else(|| name.strip_suffix(NEW))?;
    Some(owner).filter(|owner| !owner.is_empty())
}

/// The given settings files, folder too: one bad entry moves none. Every file is copied
/// before any is removed; `aside` must not exist yet.
pub fn set_aside(config: &Path, aside: &Path, settings: &[PathBuf]) -> Result<usize> {
    let failed = |e: std::io::Error| Error::file(aside, e);
    if settings.is_empty() {
        return Ok(0);
    }
    if settings.iter().any(|file| !file.is_file()) {
        let e = std::io::Error::new(std::io::ErrorKind::NotADirectory, "not a settings file");
        return Err(Error::file(config, e));
    }
    if let Some(parent) = aside.parent() {
        std::fs::create_dir_all(parent).map_err(failed)?;
    }
    std::fs::create_dir(aside).map_err(failed)?;
    let copied = settings.iter().try_for_each(|file| {
        let to = aside.join(file.file_name().unwrap_or_default());
        std::fs::copy(file, to).map(drop)
    });
    if let Err(problem) = copied {
        let _ = std::fs::remove_dir_all(aside);
        return Err(failed(problem));
    }
    for (moved, file) in settings.iter().enumerate() {
        if let Err(problem) = std::fs::remove_file(file) {
            let left = settings.len() - moved;
            return Err(Error::caused(
                crate::base::error::Kind::File,
                crate::said!(
                    "Moved {} of {} settings files to {}; {} are still in Config",
                    moved,
                    settings.len(),
                    aside.display(),
                    left
                ),
                problem,
            ));
        }
    }
    Ok(settings.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_asked_settings_are_set_aside() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("Config");
        std::fs::create_dir_all(&config).unwrap();
        for name in ["Mod_1_A.xml", "Mod_2_B.xml", "Mod_3_C.xml"] {
            std::fs::write(config.join(name), name).unwrap();
        }
        let aside = tmp.path().join("aside");
        assert_eq!(
            set_aside(
                tmp.path(),
                &aside,
                &[config.join("Mod_1_A.xml"), config.join("Mod_2_B.xml")],
            ),
            Ok(2)
        );
        assert!(config.join("Mod_3_C.xml").is_file() && !aside.join("Mod_3_C.xml").exists());
        assert_eq!(
            std::fs::read_to_string(aside.join("Mod_1_A.xml")).unwrap(),
            "Mod_1_A.xml"
        );
    }
}
