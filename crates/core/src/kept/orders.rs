//! The load orders ModsConfig.xml held over time: each one RimStack saved,
//! and each one the game or another tool wrote there.

use crate::base::error::Error;
use crate::rimworld::modsconfig::same_order;
use serde::{Deserialize, Serialize};
use std::path::Path;

const KEPT: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum How {
    Saved,
    /// Found in the file, written by the game or another tool.
    Outside,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Seconds since 1970.
    pub time: u64,
    pub how: How,
    pub ids: Vec<String>,
}

/// Oldest first. A missing or damaged file reads as empty.
pub fn read(file: &Path) -> Vec<Entry> {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

/// `false` when the newest entry already holds this order, which is then not
/// kept twice. Past the limit the oldest is dropped.
pub fn record(file: &Path, entry: Entry) -> Result<bool, Error> {
    let mut entries = read(file);
    if entries
        .last()
        .is_some_and(|last| same_order(&last.ids, &entry.ids))
    {
        return Ok(false);
    }
    entries.push(entry);
    let from = entries.len().saturating_sub(KEPT);
    let text = serde_json::to_string(&entries[from..]).map_err(|e| Error::format(e.to_string()))?;
    crate::base::atomic_file::write(file, text)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(time: u64, ids: &[&str]) -> Entry {
        Entry {
            time,
            how: How::Saved,
            ids: ids.iter().map(|id| id.to_string()).collect(),
        }
    }

    #[test]
    fn orders_are_kept_oldest_first_and_the_oldest_dropped_past_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kept/orders.json");
        assert!(read(&file).is_empty());
        for time in 0..KEPT as u64 + 3 {
            let id = format!("mod.{time}");
            assert!(record(&file, entry(time, &["ludeon.rimworld", &id])).unwrap());
        }
        let kept = read(&file);
        assert_eq!(kept.len(), KEPT);
        assert_eq!(kept[0].time, 3);
        assert_eq!(kept[0].ids, ["ludeon.rimworld", "mod.3"]);
        assert_eq!(kept.last().unwrap().time, KEPT as u64 + 2);
    }

    #[test]
    fn the_same_order_twice_in_a_row_is_kept_once() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("orders.json");
        assert!(record(&file, entry(1, &["a.b", "c.d"])).unwrap());
        // The game writes ids in its own case; the order is the same.
        let outside = Entry {
            how: How::Outside,
            ..entry(2, &["A.b", "c.D"])
        };
        assert!(!record(&file, outside).unwrap());
        assert_eq!(read(&file), [entry(1, &["a.b", "c.d"])]);
    }

    #[test]
    fn an_order_seen_before_comes_back_as_a_new_entry() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("orders.json");
        record(&file, entry(1, &["a.b", "c.d"])).unwrap();
        record(&file, entry(2, &["c.d", "a.b"])).unwrap();
        assert!(record(&file, entry(3, &["a.b", "c.d"])).unwrap());
        let times: Vec<u64> = read(&file).iter().map(|e| e.time).collect();
        assert_eq!(times, [1, 2, 3]);
    }

    #[test]
    fn a_damaged_file_reads_as_empty_and_is_written_over() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("orders.json");
        std::fs::write(&file, "not json").unwrap();
        assert!(read(&file).is_empty());
        let outside = Entry {
            how: How::Outside,
            ..entry(7, &["a.b"])
        };
        record(&file, outside.clone()).unwrap();
        assert_eq!(read(&file), [outside]);
    }
}
