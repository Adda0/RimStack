use super::qobject;
use crate::qml::values::to_qml;
use crate::workspace::changed::{notes_asked, NotesState, MOST_ASKED, MOST_NOTES};
use crate::workspace::now_secs;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QVariant;
use rimstack_core::mods::library::ModId;
use rimstack_core::sources::changes::{self, Fetching};

impl qobject::DownloadsData {
    fn fetch(mut self: Pin<&mut Self>, asked: Vec<(String, u64)>) {
        if self.notes_busy || self.demo || asked.is_empty() {
            return;
        }
        let several = asked.len() > 1;
        let asked: Vec<(String, u64)> = asked
            .into_iter()
            .filter(|(id, _)| !(several && self.fetched.contains_key(id)))
            .take(MOST_ASKED)
            .collect();
        self.as_mut().set_notes_busy(true);
        let thread = self.qt_thread();
        crate::jobs::spawn("change notes", move || {
            let state = |done: usize, resume: u64, refused: bool| {
                let state = NotesState {
                    done,
                    asked: asked.len(),
                    resume,
                    refused,
                };
                let _ = thread.queue(move |me| me.set_notes_state(to_qml(&state)));
            };
            let mut tell = |fetching| match fetching {
                Fetching::Under { done } => state(done, 0, false),
                Fetching::Waiting { done, resume } => state(done, resume, false),
                Fetching::GaveUp { done } => state(done, 0, true),
                Fetching::Fetched { id, notes } => {
                    let _ = thread.queue(move |mut me| {
                        me.as_mut().rust_mut().get_mut().fetched.insert(id, notes);
                        let all = to_qml(&me.fetched);
                        me.as_mut().set_notes(all);
                        me.show_since();
                    });
                }
            };
            changes::fetch_notes(
                &asked,
                MOST_NOTES,
                &changes::fetch_steam_notes,
                &std::thread::sleep,
                &now_secs,
                &mut tell,
            );
            let _ = thread.queue(|me| me.set_notes_busy(false));
        });
    }

    pub(super) fn fetch_since_notes(self: Pin<&mut Self>) {
        let asked = notes_asked(&self.changed);
        self.fetch(asked);
    }

    pub(super) fn fetch_notes(self: Pin<&mut Self>, mod_id: i32) {
        let workshop = ModId::from_qml(mod_id).and_then(|id| self.app().workshop_id(id));
        if let Some(id) = workshop {
            self.fetch(vec![(id, 0)]);
        }
    }

    pub(super) fn commits_of(&self, mod_id: i32) -> QVariant {
        match ModId::from_qml(mod_id) {
            Some(id) => to_qml(&self.app().commits_of(id)),
            None => QVariant::default(),
        }
    }
}
