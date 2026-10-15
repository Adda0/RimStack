//! The record is one per process, so this test is a binary of its own.

use rimstack_core::base::error::Error;
use rimstack_core::{base::diagnostics, note, problem, said};

#[test]
fn notes_problems_and_a_panic_on_another_thread_are_all_in_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("RimStack").join("diagnostics.log");
    note!("before the file is open: only printed");
    diagnostics::start(&file).unwrap();
    diagnostics::record_panics();
    note!("scan_ms={}", 12);
    problem!("{}: denied", "notes.json");
    // In English in the record, whatever the language on screen.
    let refused = Error::refused(said!("{} is not a setting", "gameDir"));
    problem!("{refused}");
    let crashed = std::thread::spawn(|| {
        let rows: Vec<u8> = Vec::new();
        rows[3]
    })
    .join();
    assert!(crashed.is_err());

    let kept = std::fs::read_to_string(&file).unwrap();
    let lines: Vec<&str> = kept.lines().collect();
    assert!(lines[0].ends_with("Z note scan_ms=12"), "{kept}");
    assert!(lines[1].ends_with("Z problem notes.json: denied"), "{kept}");
    assert!(
        lines[2].ends_with("Z problem gameDir is not a setting"),
        "{kept}"
    );
    assert!(lines[3].contains("Z crash index out of bounds"), "{kept}");
    assert!(lines[3].contains("tests/diagnostics.rs:"), "{kept}");
    assert!(lines.len() > 5 && lines[4].starts_with("    "), "{kept}");
}
