use super::{id, qobject};
use crate::jobs::Lane;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use std::time::Duration;

const PAUSE: Duration = Duration::from_millis(600);
const SHOWN: Duration = Duration::from_millis(1800);

/// Edits of the About tab not yet stored, each with the mod it was made for.
#[derive(Default)]
pub(super) struct Drafts {
    pause: Lane,
    fade: Lane,
    note: Option<(i32, String)>,
}

impl qobject::ModsData {
    fn drafts_mut(self: Pin<&mut Self>) -> &mut Drafts {
        &mut self.rust_mut().get_mut().drafts
    }

    pub(super) fn draft_note(mut self: Pin<&mut Self>, mod_id: i32, note: &QString) {
        let draft = (mod_id, note.to_string());
        self.as_mut().drafts_mut().note = Some(draft);
        self.wait_for_drafts();
    }

    fn wait_for_drafts(mut self: Pin<&mut Self>) {
        let thread = self.qt_thread();
        self.as_mut().drafts_mut().pause.run(
            "note draft",
            thread,
            |turn| turn.after(PAUSE),
            |me, quiet| {
                if quiet {
                    me.keep_drafts();
                }
            },
        );
    }

    /// Stores what is waiting; says "Saved" only when a note really changed.
    pub(super) fn keep_drafts(mut self: Pin<&mut Self>) {
        let drafts = self.as_mut().drafts_mut();
        drafts.pause.cancel();
        let Some((mod_id, note)) = drafts.note.take() else {
            return;
        };
        let app = self.app();
        if !id(mod_id).is_some_and(|id| app.set_note_text(id, &note)) {
            return;
        }
        self.as_mut().order_mut().sync(&app);
        self.as_mut().show();
        self.as_mut().set_notes_saved(true);
        let thread = self.qt_thread();
        self.drafts_mut().fade.run(
            "saved mark",
            thread,
            |turn| turn.after(SHOWN),
            |mut me, quiet| {
                if quiet {
                    me.as_mut().set_notes_saved(false);
                }
            },
        );
    }
}
