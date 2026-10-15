//! A line is `2026-10-14 19:37:35Z problem the text`;
//! later lines of a text are indented under it.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Past this the file is set aside as `<name>.old`, replacing the one before.
const MOST_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Note,
    Problem,
    Crash,
}

impl Level {
    fn word(self) -> &'static str {
        match self {
            Level::Note => "note",
            Level::Problem => "problem",
            Level::Crash => "crash",
        }
    }
}

fn time_in_words(secs: u64) -> String {
    let at = i64::try_from(secs)
        .ok()
        .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0));
    at.map_or_else(
        || secs.to_string(),
        |at| at.format("%Y-%m-%d %H:%M:%SZ").to_string(),
    )
}

fn line(secs: u64, level: Level, text: &str) -> String {
    let text = text.trim_end().replace('\n', "\n    ");
    format!("{} {} {text}\n", time_in_words(secs), level.word())
}

fn set_aside(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".old");
    PathBuf::from(name)
}

struct Record {
    file: File,
}

impl Record {
    fn open(path: &Path, most: u64) -> std::io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        if std::fs::metadata(path).is_ok_and(|file| file.len() > most) {
            std::fs::rename(path, set_aside(path))?;
        }
        let file = File::options().create(true).append(true).open(path)?;
        Ok(Self { file })
    }

    fn write(&mut self, secs: u64, level: Level, text: &str) -> std::io::Result<()> {
        self.file.write_all(line(secs, level, text).as_bytes())
    }
}

static RECORD: Mutex<Option<Record>> = Mutex::new(None);

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Until this succeeds, `note` only prints.
pub fn start(path: &Path) -> std::io::Result<()> {
    let record = Record::open(path, MOST_BYTES)?;
    *RECORD.lock().unwrap_or_else(crate::base::locks::quietly) = Some(record);
    Ok(())
}

pub fn note(level: Level, text: &str) {
    eprintln!("{text}");
    if let Some(record) = RECORD
        .lock()
        .unwrap_or_else(crate::base::locks::quietly)
        .as_mut()
    {
        // Nowhere left to say that the record itself cannot be written.
        let _ = record.write(now(), level, text);
    }
}

fn crash_in_words(what: &str, at: Option<(&str, u32)>, trace: &str) -> String {
    let place = at.map_or_else(String::new, |(file, line)| format!(" at {file}:{line}"));
    format!("{what}{place}\n{trace}")
}

/// A panic on the window's thread ends the program once written down; on any
/// other thread it is written down and the program goes on.
pub fn record_panics() {
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let said = panic.payload();
        let what = said
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| said.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("a panic that says nothing");
        let at = panic.location().map(|at| (at.file(), at.line()));
        let trace = std::backtrace::Backtrace::force_capture().to_string();
        // `try_lock`, not `lock`: a panic while a line is written holds the record,
        // so waiting would hang.
        let mut held = match RECORD.try_lock() {
            Ok(held) => Some(held),
            Err(std::sync::TryLockError::Poisoned(held)) => Some(held.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) => None,
        };
        if let Some(record) = held.as_mut().and_then(|held| held.as_mut()) {
            let _ = record.write(now(), Level::Crash, &crash_in_words(what, at, &trace));
        }
        before(panic);
    }));
}

#[macro_export]
macro_rules! note {
    ($($part:tt)+) => {
        $crate::base::diagnostics::note(
            $crate::base::diagnostics::Level::Note,
            &format!($($part)+),
        )
    };
}

#[macro_export]
macro_rules! problem {
    ($($part:tt)+) => {
        $crate::base::diagnostics::note(
            $crate::base::diagnostics::Level::Problem,
            &format!($($part)+),
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_is_written_as_its_date_and_hour() {
        assert_eq!(time_in_words(0), "1970-01-01 00:00:00Z");
        assert_eq!(time_in_words(951_782_400), "2000-02-29 00:00:00Z");
        assert_eq!(time_in_words(1_791_833_855), "2026-10-12 19:37:35Z");
        assert_eq!(time_in_words(4_107_542_399), "2100-02-28 23:59:59Z");
    }

    #[test]
    fn a_line_names_its_time_and_kind_and_keeps_later_lines_under_it() {
        assert_eq!(
            line(0, Level::Problem, "notes.json: denied\n"),
            "1970-01-01 00:00:00Z problem notes.json: denied\n"
        );
        assert_eq!(
            line(0, Level::Crash, "index out of range\n0: main\n1: start"),
            "1970-01-01 00:00:00Z crash index out of range\n    0: main\n    1: start\n"
        );
    }

    #[test]
    fn the_file_is_added_to_and_set_aside_once_it_is_large() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("RimStack").join("diagnostics.log");
        let mut record = Record::open(&path, 60).unwrap();
        record.write(0, Level::Note, "first").unwrap();
        drop(record);
        let mut record = Record::open(&path, 60).unwrap();
        record
            .write(1, Level::Note, "second and long enough")
            .unwrap();
        drop(record);
        let kept = std::fs::read_to_string(&path).unwrap();
        assert_eq!(kept.lines().count(), 2);

        let mut record = Record::open(&path, 60).unwrap();
        record.write(2, Level::Note, "third").unwrap();
        let fresh = std::fs::read_to_string(&path).unwrap();
        assert!(fresh.ends_with("note third\n") && fresh.lines().count() == 1);
        assert_eq!(std::fs::read_to_string(set_aside(&path)).unwrap(), kept);
    }
}
