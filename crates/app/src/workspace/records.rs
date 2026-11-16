use super::App;
use crate::changes::Changes;
use rimstack_core::base::locks::Held;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::{journal, lists, orders};
use std::path::PathBuf;

const SNAPSHOTS_KEPT: usize = 30;

impl App {
    pub fn snapshots_dir(&self) -> PathBuf {
        self.data_dir().join("snapshots")
    }

    pub fn lists_dir(&self) -> PathBuf {
        self.data_dir().join("lists")
    }

    pub fn orders_file(&self) -> PathBuf {
        self.data_dir().join("orders.json")
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

    pub fn journal_file(&self) -> PathBuf {
        self.data_dir().join("journal.json")
    }

    /// Named by `@kind`, local time to the second, then a counter; written off this thread, in
    /// the order asked, and told as `LISTS` once written. `None` when no name is free.
    pub fn snapshot(&self, kind: &str, list: &[String]) -> Option<String> {
        let dir = self.snapshots_dir();
        let at = format!(
            "{kind} {}",
            chrono::Local::now().format("%Y-%m-%d %H.%M.%S")
        );
        let numbered = (2..).map(|n| format!("{at} {n}"));
        let name = {
            let mut named = self.snapshots_named.held();
            let name = std::iter::once(at.clone())
                .chain(numbered)
                .find(|name| !named.contains(name) && !lists::taken(&dir, name))?;
            named.insert(name.clone());
            name
        };
        let (name_kept, list) = (name.clone(), list.to_vec());
        let (named, bus) = (self.snapshots_named.clone(), self.bus.clone());
        self.in_order(move || {
            if let Err(problem) = lists::save(&dir, &name_kept, &list) {
                rimstack_core::problem!("{problem}");
            }
            named.held().remove(&name_kept);
            lists::prune(&dir, SNAPSHOTS_KEPT);
            bus.publish(Changes::LISTS);
        });
        Some(name)
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
        let (file, bus) = (self.journal_file(), self.bus.clone());
        self.in_order(move || match journal::add(&file, entry) {
            Ok(()) => bus.publish(Changes::LISTS),
            Err(problem) => rimstack_core::problem!("{problem}"),
        });
    }

    /// `write` off this thread, after every write asked before it.
    fn in_order(&self, write: impl FnOnce() + Send + 'static) {
        self.records_waiting.held().push_back(Box::new(write));
        let waiting = self.records_waiting.clone();
        self.records_writes.ask("history written", (), move |()| {
            loop {
                let Some(write) = waiting.held().pop_front() else {
                    return;
                };
                write();
            }
        });
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
        app.settle_writes();
        assert!(name.starts_with("@saving 2"), "{name}");
        assert_eq!(lists::load(&app.snapshots_dir(), &name).unwrap(), ["a.b"]);
        app.note_change(
            rimstack_core::say!("Saved the load order: {}", "x"),
            Some(&name),
        );
        app.settle_writes();
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
    fn an_order_the_game_wrote_as_it_started_is_named_as_its_own() {
        let (_tmp, app) = installed();
        app.record_order(orders::How::Game, &["x.y".to_string()]);
        app.settle_writes();
        let past = app.past_orders();
        assert_eq!(past.len(), 1);
        assert_eq!(
            past[0].said,
            rimstack_core::say!("Rewritten by RimWorld as it started")
        );
        assert_eq!(past[0].mods, 1);
    }

    #[test]
    fn two_snapshots_in_one_second_are_both_kept() {
        let (_tmp, app) = installed();
        let first = app.snapshot("@saving", &["a".to_string()]).unwrap();
        let second = app.snapshot("@saving", &["b".to_string()]).unwrap();
        app.settle_writes();
        let dir = app.snapshots_dir();
        assert_ne!(first, second);
        assert_eq!(lists::load(&dir, &first).unwrap(), ["a"]);
        assert_eq!(lists::load(&dir, &second).unwrap(), ["b"]);
    }
}
