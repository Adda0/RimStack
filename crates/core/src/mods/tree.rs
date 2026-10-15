use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn folder_tree(root: &Path) -> Vec<(String, usize)> {
    let shown = |name: &str| !name.starts_with('.');
    let mut inner: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut files: BTreeMap<(String, Option<String>), usize> = BTreeMap::new();
    for entry in crate::base::walk::tree(root, usize::MAX) {
        let parts: Vec<String> = entry
            .within
            .iter()
            .map(|part| part.to_string_lossy().into_owned())
            .collect();
        match (entry.is_dir, parts.as_slice()) {
            (true, [top]) if shown(top) => {
                inner.entry(top.clone()).or_default();
            }
            (true, [top, folder]) if shown(top) && shown(folder) => {
                inner.entry(top.clone()).or_default().insert(folder.clone());
            }
            (false, [top, rest @ ..]) if !rest.is_empty() => {
                *files.entry((top.clone(), None)).or_default() += 1;
                if rest.len() > 1 {
                    let key = (top.clone(), Some(rest[0].clone()));
                    *files.entry(key).or_default() += 1;
                }
            }
            _ => {}
        }
    }
    let count = |top: &str, folder: Option<&String>| {
        let key = (top.to_string(), folder.cloned());
        files.get(&key).copied().unwrap_or(0)
    };
    let mut lines = Vec::new();
    for (top, folders) in &inner {
        if folders.is_empty() || folders.len() > 12 {
            lines.push((format!("{top}/"), count(top, None)));
        } else {
            for folder in folders {
                lines.push((format!("{top}/{folder}/"), count(top, Some(folder))));
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn folders_are_listed_two_deep_with_their_files_counted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        for file in [
            "About/About.xml",
            "Textures/Things/a.png",
            "Textures/Things/Deep/b.png",
            "Textures/UI/c.png",
            ".git/config",
            "readme.txt",
        ] {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        assert_eq!(
            folder_tree(root),
            [
                ("About/".to_string(), 1),
                ("Textures/Things/".to_string(), 2),
                ("Textures/UI/".to_string(), 1)
            ]
        );
        assert!(folder_tree(&root.join("nowhere")).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_back_is_not_counted_again() {
        let tmp = tempfile::tempdir().unwrap();
        let textures = tmp.path().join("Textures");
        fs::create_dir_all(&textures).unwrap();
        fs::write(textures.join("one.png"), "").unwrap();
        std::os::unix::fs::symlink(&textures, textures.join("again")).unwrap();
        assert_eq!(folder_tree(tmp.path()), [("Textures/".to_string(), 1)]);
    }
}
