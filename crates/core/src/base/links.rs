use std::path::{Path, PathBuf};

/// The path with each link Windows refuses to pass replaced by the folder it names:
/// a symlink made as a file (`mklink` without `/D`) leads to a folder Explorer opens,
/// yet every path through it is not found (#3). Elsewhere the path comes back as given.
pub fn through(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        out.push(part);
        if out.is_dir() {
            continue;
        }
        let Ok(target) = std::fs::read_link(&out) else {
            continue;
        };
        let target = out
            .parent()
            .map_or(target.clone(), |above| above.join(&target));
        if target.is_dir() {
            out = dunce::simplified(&target).to_path_buf();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_without_such_a_link_is_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let folder = tmp.path().join("a/b");
        std::fs::create_dir_all(&folder).unwrap();
        assert_eq!(through(&folder), folder);
        let missing = tmp.path().join("a/none/c");
        assert_eq!(through(&missing), missing);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_works_is_kept_as_named() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("real/inner")).unwrap();
        std::os::unix::fs::symlink(tmp.path().join("real"), tmp.path().join("link")).unwrap();
        let named = tmp.path().join("link/inner");
        assert_eq!(through(&named), named);
    }

    #[cfg(windows)]
    #[test]
    fn a_file_symlink_to_a_folder_is_passed() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        std::fs::create_dir_all(real.join("inner")).unwrap();
        let link = tmp.path().join("link");
        if std::os::windows::fs::symlink_file(&real, &link).is_err() {
            // Creating a symlink needs Developer Mode or an administrator.
            return;
        }
        assert!(
            !link.join("inner").is_dir(),
            "Windows follows a file symlink to a folder itself, so `through` is not needed"
        );
        let passed = through(&link.join("inner"));
        assert!(passed.is_dir());
        assert_eq!(passed, dunce::simplified(&real.join("inner")));
    }
}
