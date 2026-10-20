use super::App;
use crate::changes::Changes;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::{journal, lists, orders};
use std::path::PathBuf;

const SNAPSHOTS_KEPT: usize = 30;

impl App {
    pub fn snapshots_dir(&self) -> PathBuf {
        self.own_dir().join("snapshots")
    }

    pub fn lists_dir(&self) -> PathBuf {
        self.own_dir().join("lists")
    }

    pub fn journal_file(&self) -> PathBuf {
        self.own_dir().join("journal.json")
    }

    pub fn orders_file(&self) -> PathBuf {
        self.own_dir().join("orders.json")
    }

    /// A load order ModsConfig.xml held is kept for the history: the ones
    /// saved from RimStack, the ones the game or another tool wrote instead.
    pub fn record_order(&self, how: orders::How, ids: &[String]) {
        if ids.is_empty() {
            return;
        }
        let entry = orders::Entry {
            time: super::now_secs(),
            how,
            ids: ids.to_vec(),
        };
        match orders::record(&self.orders_file(), entry) {
            Ok(true) => self.publish(Changes::LISTS),
            Ok(false) => {}
            Err(problem) => rimstack_core::problem!("{problem}"),
        }
    }

    /// Named by `@kind`, local time to the second, then a counter. `None` if not kept.
    pub fn snapshot(&self, kind: &str, list: &[String]) -> Option<String> {
        let dir = self.snapshots_dir();
        let at = format!(
            "{kind} {}",
            chrono::Local::now().format("%Y-%m-%d %H.%M.%S")
        );
        let numbered = (2..).map(|n| format!("{at} {n}"));
        let name = std::iter::once(at.clone())
            .chain(numbered)
            .find(|name| !lists::taken(&dir, name))?;
        match lists::save(&dir, &name, list) {
            Ok(()) => {
                lists::prune(&dir, SNAPSHOTS_KEPT);
                self.publish(Changes::LISTS);
                Some(name)
            }
            Err(problem) => {
                rimstack_core::problem!("{problem}");
                None
            }
        }
    }

    pub fn note_change(&self, what: Sentence, snapshot: Option<&str>) {
        let revert = snapshot.map_or(String::new(), |name| format!("list {name}"));
        self.note(what, revert);
    }

    /// `revert`: "list <snapshot>", "unhide <mod folder>" or empty.
    pub fn note(&self, what: Sentence, revert: String) {
        let entry = journal::Entry {
            time: super::now_secs(),
            what: what.to_kept().to_string(),
            revert,
        };
        match journal::add(&self.journal_file(), entry) {
            Ok(()) => self.publish(Changes::LISTS),
            Err(problem) => rimstack_core::problem!("{problem}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::workspace::tests::installed;
    use rimstack_core::kept::{journal, lists, orders};

    #[test]
    fn a_snapshot_is_kept_by_its_kind_and_named_in_the_history() {
        let (_tmp, app) = installed();
        let name = app.snapshot("@saving", &["a.b".to_string()]).unwrap();
        assert!(name.starts_with("@saving 2"), "{name}");
        assert_eq!(lists::load(&app.snapshots_dir(), &name).unwrap(), ["a.b"]);
        app.note_change(
            rimstack_core::say!("Saved the load order: {}", "x"),
            Some(&name),
        );
        let kept = journal::read(&app.journal_file());
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].revert, format!("list {name}"));
    }

    #[test]
    fn the_orders_saved_and_seen_are_kept_newest_first() {
        let (_tmp, app) = installed();
        app.record_order(orders::How::Saved, &["a.b".to_string(), "c.d".to_string()]);
        app.record_order(
            orders::How::Outside,
            &["C.D".to_string(), "a.B".to_string()],
        );
        // The outside write holds the same order, only elsewhere: it is kept.
        // The same order written again is not.
        app.record_order(
            orders::How::Outside,
            &["c.d".to_string(), "a.b".to_string()],
        );
        // Kept oldest first; `past_order` counts back from the newest.
        let past = orders::read(&app.orders_file());
        assert_eq!(past.len(), 2);
        assert_eq!(past[0].how, orders::How::Saved);
        assert_eq!(past[1].how, orders::How::Outside);
        assert_eq!(past[1].ids, ["C.D", "a.B"]);
        assert_eq!(app.past_order(0).unwrap(), ["C.D", "a.B"]);
        assert_eq!(app.past_order(1).unwrap(), ["a.b", "c.d"]);
        assert!(app.past_order(2).is_none());
    }

    #[test]
    fn two_snapshots_in_one_second_are_both_kept() {
        let (_tmp, app) = installed();
        let first = app.snapshot("@saving", &["a".to_string()]).unwrap();
        let second = app.snapshot("@saving", &["b".to_string()]).unwrap();
        let dir = app.snapshots_dir();
        assert_ne!(first, second);
        assert_eq!(lists::load(&dir, &first).unwrap(), ["a"]);
        assert_eq!(lists::load(&dir, &second).unwrap(), ["b"]);
    }
}
