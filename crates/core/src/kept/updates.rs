use crate::rimworld::paths::Source;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Kept in `updates.json`; all empty before the first check.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LastCheck {
    /// Seconds since 1970 when Steam last answered.
    pub checked: Option<u64>,
    pub asked: Option<u64>,
    /// Seconds since 1970 when each Workshop item was last updated, by Workshop id.
    pub now: BTreeMap<String, u64>,
    pub checkouts: BTreeMap<String, Remote>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Remote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

impl LastCheck {
    /// A missing or damaged file is no check yet.
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

/// Workshop folders belong to Steam, so a checkout there is asked only on request.
pub fn asks_git(source: Source, checkout: bool, git_for_workshop: bool) -> bool {
    checkout
        && match source {
            Source::Local => true,
            Source::Workshop => git_for_workshop,
            Source::Official => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_is_kept_and_read_back_and_an_older_file_still_reads() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("updates.json");
        assert_eq!(LastCheck::read(&file), LastCheck::default());
        let check = LastCheck {
            checked: Some(10),
            asked: Some(2),
            now: BTreeMap::from([("100".to_string(), 7)]),
            checkouts: BTreeMap::from([
                (
                    "/mods/a".to_string(),
                    Remote {
                        behind: Some(3),
                        problem: None,
                    },
                ),
                (
                    "/mods/b".to_string(),
                    Remote {
                        behind: None,
                        problem: Some("offline".into()),
                    },
                ),
            ]),
        };
        check.write(&file).unwrap();
        assert_eq!(LastCheck::read(&file), check);
        std::fs::write(
            &file,
            r#"{"checked":5,"asked":1,"now":{"9":4},"checkouts":{"/m":{"behind":0}}}"#,
        )
        .unwrap();
        let older = LastCheck::read(&file);
        assert_eq!(
            (older.checked, older.now["9"], older.checkouts["/m"].behind),
            (Some(5), 4, Some(0))
        );
    }

    #[test]
    fn a_local_checkout_is_asked_and_a_workshop_one_only_on_request() {
        assert!(asks_git(Source::Local, true, false));
        assert!(!asks_git(Source::Workshop, true, false));
        assert!(asks_git(Source::Workshop, true, true));
    }

    #[test]
    fn nothing_but_a_checkout_of_a_mod_is_asked() {
        for on in [false, true] {
            assert!(!asks_git(Source::Local, false, on));
            assert!(!asks_git(Source::Workshop, false, on));
            assert!(!asks_git(Source::Official, true, on));
        }
    }
}
