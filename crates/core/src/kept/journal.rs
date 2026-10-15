use crate::base::error::Error;
use serde::{Deserialize, Serialize};
use std::path::Path;

const KEPT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Seconds since 1970.
    pub time: u64,
    pub what: String,
    /// Format: "list <name>" or "unhide <library folder>"; empty for what cannot be taken back.
    #[serde(default)]
    pub revert: String,
}

/// A missing or damaged file reads as empty.
pub fn read(file: &Path) -> Vec<Entry> {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn add(file: &Path, entry: Entry) -> Result<(), Error> {
    let mut entries = read(file);
    entries.push(entry);
    let from = entries.len().saturating_sub(KEPT);
    let text = serde_json::to_string(&entries[from..]).map_err(|e| Error::format(e.to_string()))?;
    crate::base::atomic_file::write(file, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(time: u64, what: &str) -> Entry {
        Entry {
            time,
            what: what.to_string(),
            revert: String::new(),
        }
    }

    #[test]
    fn entries_are_kept_in_order_and_the_oldest_dropped_past_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kept/journal.json");
        assert!(read(&file).is_empty());
        for time in 0..KEPT as u64 + 3 {
            add(&file, entry(time, "Saved")).unwrap();
        }
        let kept = read(&file);
        assert_eq!(kept.len(), KEPT);
        assert_eq!(kept[0].time, 3);
        assert_eq!(kept.last().unwrap().time, KEPT as u64 + 2);
    }

    #[test]
    fn a_damaged_file_reads_as_empty_and_is_written_over() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("journal.json");
        std::fs::write(&file, "not json").unwrap();
        assert!(read(&file).is_empty());
        let with_revert = Entry {
            revert: "list Before sorting".to_string(),
            ..entry(7, "Sorted")
        };
        add(&file, with_revert.clone()).unwrap();
        assert_eq!(read(&file), [with_revert]);
    }
}
