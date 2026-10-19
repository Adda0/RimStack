use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Kept in `steamcmd.json`: the mods SteamCMD put in the game's Mods folder, and when, by
/// Workshop id. A folder named by a Workshop id may also be a copy of a Workshop mod, which only
/// this record tells apart.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Downloaded {
    /// Seconds since 1970: the Workshop's copy was no newer then.
    pub at: BTreeMap<String, u64>,
}

impl Downloaded {
    /// A missing or damaged file records nothing.
    pub fn read(file: &Path) -> Self {
        let kept = crate::base::atomic_file::read_json(file).map(serde_json::from_value);
        kept.and_then(Result::ok).unwrap_or_default()
    }

    pub fn write(&self, file: &Path) -> Result<(), crate::base::error::Error> {
        let text = serde_json::to_string(self)
            .map_err(|e| crate::base::error::Error::format(format!("{}: {e}", file.display())))?;
        crate::base::atomic_file::write(file, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_download_is_kept_and_read_back_and_a_damaged_file_records_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("steamcmd.json");
        assert_eq!(Downloaded::read(&file), Downloaded::default());
        let kept = Downloaded {
            at: BTreeMap::from([("100".to_string(), 7)]),
        };
        kept.write(&file).unwrap();
        assert_eq!(Downloaded::read(&file), kept);
        std::fs::write(&file, "{ not json").unwrap();
        assert_eq!(Downloaded::read(&file), Downloaded::default());
    }
}
