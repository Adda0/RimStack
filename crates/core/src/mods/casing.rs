//! Folders LoadFolders.xml names in another letter case than on disk. The game joins the name
//! to the mod's folder as written, so where the file system tells case apart it loads nothing
//! from them, silently; elsewhere the path is found and nothing is told.

use crate::mods::describe::{Fix, Trouble, TroubleKind};
use crate::rimworld::loadfolders::Miscased;
use crate::{say, say_n};

/// Ignorable by `Trouble::key`, like the rest.
pub fn troubles(miscased: &[Miscased]) -> Vec<Trouble> {
    if miscased.is_empty() {
        return Vec::new();
    }
    let folders = (miscased.iter())
        .map(|m| say!("{} (on disk {})", m.named, m.on_disk))
        .collect();
    vec![Trouble {
        error: false,
        words: say_n!(
            miscased.len(),
            "LoadFolders.xml names {} folder in other letter case than on disk, so on this system the game loads nothing from it:",
            "LoadFolders.xml names {} folders in other letter case than on disk, so on this system the game loads nothing from them:"
        ),
        items: folders,
        short: say!("LoadFolders.xml letter case"),
        fix: Fix::None,
        key: "loadfolders-case".to_string(),
        kind: TroubleKind::LoadFoldersCase,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_folder_is_named_with_its_case_on_disk() {
        let said = troubles(&[Miscased {
            named: "common".to_string(),
            on_disk: "Common".to_string(),
        }]);
        assert_eq!(said.len(), 1);
        assert_eq!(
            said[0].words.to_string(),
            "LoadFolders.xml names 1 folder in other letter case than on disk, so on this system the game loads nothing from it:"
        );
        assert_eq!(said[0].items.len(), 1);
        assert_eq!(said[0].items[0].to_string(), "common (on disk Common)");
        assert!(troubles(&[]).is_empty());
    }
}
