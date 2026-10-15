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
