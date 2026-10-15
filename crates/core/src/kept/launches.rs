use crate::base::error::Error;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const KEPT: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    /// Seconds since 1970.
    pub time: u64,
    pub ids: Vec<String>,
    /// `None` until a log written after the launch is read.
    pub errors: Option<usize>,
}

/// Oldest first. A missing or damaged file reads as none.
pub fn read(path: &Path) -> Vec<Launch> {
    let Some(Value::Array(items)) = crate::base::atomic_file::read_json(path) else {
        return Vec::new();
    };
    let launch = |item: &Value| -> Option<Launch> {
        let ids = item.get("ids")?.as_array()?;
        Some(Launch {
            time: item.get("time")?.as_u64()?,
            ids: ids
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            errors: item
                .get("errors")
                .and_then(Value::as_u64)
                .map(|n| n as usize),
        })
    };
    items.iter().filter_map(launch).collect()
}

pub fn write(path: &Path, launches: &[Launch]) -> Result<(), Error> {
    let from = launches.len().saturating_sub(KEPT);
    let items: Vec<Value> = launches[from..]
        .iter()
        .map(|l| json!({ "time": l.time, "ids": l.ids, "errors": l.errors }))
        .collect();
    crate::base::atomic_file::write(path, Value::Array(items).to_string())
}

pub fn record(path: &Path, ids: &[String], now: u64) -> Result<(), Error> {
    let mut launches = read(path);
    launches.push(Launch {
        time: now,
        ids: ids.to_vec(),
        errors: None,
    });
    write(path, &launches)
}

pub fn note_outcome(launches: &mut [Launch], log_written: u64, errors: usize) -> bool {
    match launches.last_mut() {
        Some(last) if last.errors.is_none() && log_written >= last.time => {
            last.errors = Some(errors);
            true
        }
        _ => false,
    }
}

pub fn last_clean(launches: &[Launch]) -> Option<&Launch> {
    launches.iter().rev().find(|l| l.errors == Some(0))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Shown {
    pub log: Option<String>,
    pub time: u64,
    pub mods: usize,
    pub errors: Option<usize>,
    pub ids: Vec<String>,
    pub removed: usize,
    pub added: usize,
    pub reordered: bool,
}

pub fn shown(record: &[Launch], list: &[String], kept_logs: &[(u64, PathBuf)]) -> Vec<Shown> {
    let one = |(at, launch): (usize, &Launch)| {
        let difference = crate::rimworld::saves::compare(&launch.ids, list);
        let next = record.get(at + 1).map(|after| after.time);
        let log = crate::rimworld::logs::kept_for(kept_logs, launch.time, next);
        Shown {
            log: log.map(|path| path.to_string_lossy().into_owned()),
            time: launch.time,
            mods: launch.ids.len(),
            errors: launch.errors,
            ids: launch.ids.clone(),
            removed: difference.missing.len(),
            added: difference.added.len(),
            reordered: difference.reordered,
        }
    };
    record.iter().enumerate().rev().map(one).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SinceClean {
    pub time: u64,
    pub ids: Vec<String>,
    pub removed: Vec<String>,
    pub added: Vec<String>,
    pub reordered: bool,
}

pub fn since_clean(
    record: &[Launch],
    list: &[String],
    name_of: &dyn Fn(&str) -> String,
) -> Option<SinceClean> {
    let clean = last_clean(record)?;
    let difference = crate::rimworld::saves::compare(&clean.ids, list);
    let named = |ids: &[String], places: &[usize]| -> Vec<String> {
        places.iter().map(|&i| name_of(&ids[i])).collect()
    };
    Some(SinceClean {
        time: clean.time,
        ids: clean.ids.clone(),
        removed: named(&clean.ids, &difference.missing),
        added: named(list, &difference.added),
        reordered: difference.reordered,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct LastStart {
    /// Seconds since 1970.
    pub time: u64,
    pub errors: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogRead {
    pub written: u64,
    pub previous: bool,
    pub errors: usize,
}

/// `log_written` is in seconds since 1970, 0 for never.
/// The log may show starts not recorded here.
pub fn last_start(
    recorded: Option<&Launch>,
    read: Option<LogRead>,
    log_written: u64,
) -> Option<LastStart> {
    const SAME_START: u64 = 120;
    let recorded_at = recorded.map(|r| r.time);
    if let Some(read) = read.filter(|r| r.written > 0 && !r.previous) {
        if recorded_at.is_none_or(|at| read.written >= at) {
            return Some(LastStart {
                time: read.written,
                errors: Some(read.errors),
            });
        }
    }
    if log_written > 0 && recorded_at.is_none_or(|at| log_written > at + SAME_START) {
        return Some(LastStart {
            time: log_written,
            errors: None,
        });
    }
    recorded.map(|r| LastStart {
        time: r.time,
        errors: r.errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_start_is_the_newest_that_is_known() {
        let recorded = Launch {
            time: 1_000,
            ids: Vec::new(),
            errors: Some(2),
        };
        let read = |written, previous| {
            Some(LogRead {
                written,
                previous,
                errors: 5,
            })
        };
        let start = |time, errors| Some(LastStart { time, errors });
        assert_eq!(
            last_start(Some(&recorded), read(1_000, false), 0),
            start(1_000, Some(5))
        );
        assert_eq!(
            last_start(Some(&recorded), read(2_000, true), 0),
            start(1_000, Some(2))
        );
        assert_eq!(last_start(Some(&recorded), None, 1_121), start(1_121, None));
        assert_eq!(
            last_start(Some(&recorded), None, 1_120),
            start(1_000, Some(2))
        );
        assert_eq!(last_start(None, None, 7), start(7, None));
        assert_eq!(last_start(None, None, 0), None);
    }

    #[test]
    fn launches_are_shown_newest_first_each_with_its_log_and_what_changed_since() {
        let launch = |time, ids: &[&str], errors| Launch {
            time,
            ids: ids.iter().map(|id| id.to_string()).collect(),
            errors,
        };
        let record = [
            launch(100, &["a", "b"], Some(0)),
            launch(200, &["a", "b", "c"], Some(4)),
        ];
        let list: Vec<String> = ["b", "a", "d"].map(String::from).to_vec();
        let kept = [
            (150, PathBuf::from("/logs/150.log")),
            (250, PathBuf::from("/logs/250.log")),
        ];
        let rows = shown(&record, &list, &kept);
        assert_eq!(
            (rows[0].time, rows[0].log.as_deref()),
            (200, Some("/logs/250.log"))
        );
        assert_eq!(
            (rows[0].removed, rows[0].added, rows[0].reordered),
            (1, 1, true)
        );
        assert_eq!(rows[1].log.as_deref(), Some("/logs/150.log"));

        let upper = |id: &str| id.to_uppercase();
        let since = since_clean(&record, &list, &upper).unwrap();
        assert_eq!((since.time, since.removed.len()), (100, 0));
        assert_eq!(since.added, ["D"]);
        assert_eq!(since_clean(&record[1..], &list, &upper), None);
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn launches_are_recorded_and_read_back_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data").join("launches.json");
        assert!(read(&path).is_empty());
        record(&path, &ids(&["a"]), 10).unwrap();
        record(&path, &ids(&["a", "b"]), 20).unwrap();
        let launches = read(&path);
        assert_eq!(launches.len(), 2);
        assert_eq!(launches[1].ids, ids(&["a", "b"]));
        assert_eq!(launches[1].errors, None);
    }

    #[test]
    fn only_the_newest_launches_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("launches.json");
        for time in 0..KEPT as u64 + 5 {
            record(&path, &ids(&["a"]), time).unwrap();
        }
        let launches = read(&path);
        assert_eq!(launches.len(), KEPT);
        assert_eq!(launches[0].time, 5);
    }

    #[test]
    fn an_outcome_goes_to_the_newest_launch_once_and_only_from_a_later_log() {
        let launch = |time, errors| Launch {
            time,
            ids: Vec::new(),
            errors,
        };
        let mut launches = vec![launch(10, Some(0)), launch(20, None)];
        assert!(!note_outcome(&mut launches, 15, 3));
        assert!(note_outcome(&mut launches, 25, 3));
        assert_eq!(launches[1].errors, Some(3));
        assert!(!note_outcome(&mut launches, 30, 0));
        assert_eq!(last_clean(&launches).unwrap().time, 10);
        assert!(!note_outcome(&mut [], 30, 0));
    }
}
