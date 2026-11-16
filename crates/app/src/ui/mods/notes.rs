use super::Mods;
use crate::App;
use crate::ui::menu::{self, Entry};
use crate::ui::notices;
use crate::words::put;
use crate::workspace::load_order::{LoadOrder, Side};
use gpui::{Context, Pixels, Point, Window};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::{say, say_n};
use std::time::Duration;

const PAUSE: Duration = Duration::from_millis(600);
const SHOWN: Duration = Duration::from_millis(1800);

/// The other mod of a rule of your own being chosen: the chosen one loads after it, or before.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Picking {
    After,
    Before,
}

#[derive(Default)]
pub struct Drafts {
    pause: crate::jobs::Lane,
    fade: crate::jobs::Lane,
    note: Option<(ModId, String)>,
}

impl Mods {
    pub(super) fn start_picking(&mut self, picking: Option<Picking>, cx: &mut Context<Self>) {
        self.chosen_reads.picking = picking;
        self.sync_lists(cx);
        self.sync_details(cx);
        cx.notify();
    }

    pub(super) fn pick(&mut self, other: ModId, cx: &mut Context<Self>) {
        let Some(chosen) = self.selected_id(cx) else {
            return;
        };
        let app = super::super::app(cx);
        match self.chosen_reads.picking.take() {
            Some(Picking::After) => app.change_rule(other, chosen, true),
            Some(Picking::Before) => app.change_rule(chosen, other, true),
            None => return,
        }
        self.show(cx);
    }

    pub(super) fn remove_rule(&mut self, first: ModId, then: ModId, cx: &mut Context<Self>) {
        super::super::app(cx).change_rule(first, then, false);
        self.show(cx);
    }

    pub(super) fn win_over(&mut self, other: ModId, cx: &mut Context<Self>) {
        let Some(id) = self.list.read(cx).selected_id() else {
            return;
        };
        let app = super::super::app(cx);
        app.change_rule(other, id, true);
        self.edit_list(cx, |list, _| {
            if let Some((from, to)) = list.order.list(Side::Active).below(id, other) {
                list.order.move_mod(&app, Side::Active, from, to);
            }
        });
        self.changed(cx);
    }

    pub(super) fn compare_with(&mut self, other: ModId, cx: &mut Context<Self>) {
        let list = self.list.read(cx);
        let Some(id) = list.selected_id() else {
            return;
        };
        if let Some((earlier, later)) = list.order.list(Side::Active).in_load_order(id, other) {
            self.open_compare(earlier, later, cx);
        }
    }

    pub(super) fn warn_again(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id(cx) else {
            return;
        };
        let app = super::super::app(cx);
        app.warn_again(&[id]);
        self.edit_list(cx, |list, _| list.order.recheck(&app));
        self.show(cx);
    }

    pub(super) fn pin(&mut self, on: bool, cx: &mut Context<Self>) {
        let ids = self.list.read(cx).selection.clone();
        if ids.is_empty() {
            return;
        }
        let app = super::super::app(cx);
        if app.set_pinned(&ids, on) {
            self.describe(cx);
        }
        self.show(cx);
    }

    pub(super) fn draft_note(&mut self, note: String, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_id(cx) {
            self.chosen_reads.drafts.note = Some((id, note));
            self.wait_for_drafts(cx);
        }
    }

    /// Each tag of the comma-separated `text` the mod shown does not yet have goes last.
    pub(super) fn add_tags(&mut self, text: String, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id(cx) else {
            return;
        };
        let app = super::super::app(cx);
        if !app.add_tags(&[id], &text) {
            return;
        }
        self.edit_list(cx, |list, _| list.order.sync(&app));
        // The field that asked is being updated as this runs, so the panel is shown after it.
        let me = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = me.update(cx, |mods, cx| mods.show(cx));
        });
    }

    /// The mod's tags kept for pasting onto other mods, and on the clipboard as the Add tags
    /// box takes them; nothing copied from a mod without tags.
    pub(super) fn copy_tags(&mut self, tags: String, cx: &mut Context<Self>) {
        if tags.is_empty() {
            return;
        }
        self.copied_tags = tags.clone();
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(tags));
    }

    /// The mods take the copied tags, or with `add` gain those they lack; one Undo gives each its
    /// own back.
    pub(super) fn paste_tags(&mut self, ids: Vec<ModId>, add: bool, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        let before = app.tags_of(&ids);
        let copied = self.copied_tags.clone();
        let changed = match add {
            true => app.add_tags(&ids, &copied),
            false => app.set_tags(&ids, &copied),
        };
        if !changed {
            return;
        }
        self.edit_list(cx, |list, _| list.order.sync(&app));
        self.show(cx);
        let paired: Vec<(ModId, String)> = ids.into_iter().zip(before).collect();
        let me = cx.entity().downgrade();
        let said = say_n!(
            paired.len(),
            "Tags pasted onto {} mod",
            "Tags pasted onto {} mods"
        );
        notices::offer(
            put(&said),
            put(&say!("Undo")),
            move |_, cx| {
                let _ = me.update(cx, |mods, cx| mods.undo_tags(paired.clone(), cx));
            },
            cx,
        );
    }

    /// Each mod gets back the tags it had before the paste.
    pub(super) fn undo_tags(&mut self, pairs: Vec<(ModId, String)>, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        let mut changed = false;
        for (id, tags) in pairs {
            changed |= app.set_tags(&[id], &tags);
        }
        if !changed {
            return;
        }
        self.edit_list(cx, |list, _| list.order.sync(&app));
        self.show(cx);
    }

    /// A tag taken off the mod shown.
    pub(super) fn remove_tag(&mut self, tag: &str, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_id(cx) {
            self.change_tags(id, |app, id| app.remove_tag(&[id], tag), cx);
        }
    }

    /// Every tag taken off the mod shown, the note staying.
    pub(super) fn clear_tags(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_id(cx) {
            self.change_tags(id, |app, id| app.clear_tags(id), cx);
        }
    }

    fn change_tags(
        &mut self,
        id: ModId,
        change: impl FnOnce(&App, ModId) -> bool,
        cx: &mut Context<Self>,
    ) {
        let app = super::super::app(cx);
        if change(&app, id) {
            self.edit_list(cx, |list, _| list.order.sync(&app));
            self.show(cx);
        }
    }

    fn wait_for_drafts(&mut self, cx: &mut Context<Self>) {
        self.chosen_reads
            .drafts
            .pause
            .after(PAUSE, cx, |mods, cx| mods.keep_drafts(cx));
    }

    pub(super) fn keep_drafts(&mut self, cx: &mut Context<Self>) {
        self.chosen_reads.drafts.pause.cancel();
        let Some((id, note)) = self.chosen_reads.drafts.note.take() else {
            return;
        };
        let app = super::super::app(cx);
        if !app.set_note_text(id, &note) {
            return;
        }
        self.edit_list(cx, |list, _| list.order.sync(&app));
        self.chosen_reads.notes_saved = true;
        self.show(cx);
        self.chosen_reads.drafts.fade.after(SHOWN, cx, |mods, cx| {
            mods.chosen_reads.notes_saved = false;
            mods.sync_details(cx);
        });
    }

    /// The groups beside the mod, a new one, out of its group, and its group deleted for
    /// every mod in it: the row menu's filing entries.
    pub(super) fn group_choices(
        &mut self,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.selected_id(cx) else {
            return;
        };
        let app = super::super::app(cx);
        let entries = self
            .list
            .update(cx, |list, _| list.order.group_menu(&app, id));
        self.open_entries(entries, at, window, cx);
    }

    pub(super) fn category_choices(
        &mut self,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.selected_id(cx) else {
            return;
        };
        let chosen = self
            .details
            .as_ref()
            .map(|d| d.category_chosen.clone())
            .unwrap_or_default();
        let labels = self
            .list
            .read(cx)
            .overview
            .as_ref()
            .map(|o| o.categories.clone())
            .unwrap_or_default();
        let me = cx.entity().downgrade();
        let file = |label: &'static str| {
            let me = me.clone();
            move |_: &mut Window, cx: &mut gpui::App| {
                let _ = me.update(cx, |mods, cx| mods.file_under(id, label, cx));
            }
        };
        let mut entries: Vec<Entry> = vec![
            Entry::item(put(&say!("Automatic")), file(""))
                .icon(if chosen.is_empty() { "check" } else { "" })
                .into(),
            Entry::Line,
        ];
        for label in labels {
            let item = Entry::item(put(&Sentence::said(label, Vec::new())), file(label));
            entries.push(
                if chosen == label {
                    item.icon("check")
                } else {
                    item
                }
                .into(),
            );
        }
        menu::open_at(at, entries, window, cx);
    }

    fn file_under(&mut self, id: ModId, label: &str, cx: &mut Context<Self>) {
        let app = super::super::app(cx);
        app.set_category(id, label);
        self.edit_list(cx, |list, _| list.order.sync(&app));
        self.show(cx);
    }

    pub(super) fn read_chosen(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.details.as_ref().and(self.selected_id(cx)) else {
            self.chosen_reads.read.cancel();
            self.chosen_reads.files = None;
            return;
        };
        self.read_commits(id, cx);
        if self
            .chosen_reads
            .files
            .as_ref()
            .is_some_and(|(of, _)| *of == id)
        {
            return;
        }
        let app = super::super::app(cx);
        self.chosen_reads.read.run(
            "files of a mod",
            cx,
            move |_| {
                let textures = LoadOrder::textures(&app, id);
                let files = LoadOrder::files(&app, id);
                let hidden = app.hidden_in(id);
                (textures, files, hidden)
            },
            move |mods, read, cx| {
                mods.chosen_reads.files = Some((id, std::rc::Rc::new(read)));
                mods.sync_details(cx);
            },
        );
    }

    fn read_commits(&mut self, id: ModId, cx: &mut Context<Self>) {
        let git = self.details.as_ref().is_some_and(|d| !d.workshop && d.git);
        if !git
            || self
                .chosen_reads
                .commits
                .as_ref()
                .is_some_and(|(of, _)| *of == id)
        {
            return;
        }
        let app = super::super::app(cx);
        self.chosen_reads.commits_read.run(
            "commits of a mod",
            cx,
            move |_| (app.commits_of(id), app.repository_of(id)),
            move |mods, (commits, page), cx| {
                mods.chosen_reads.commits = Some((id, std::rc::Rc::new(commits)));
                mods.chosen_reads.repository = Some((id, page));
                mods.describe(cx);
            },
        );
    }

    pub(super) fn unhide_chosen(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id(cx) else {
            return;
        };
        let app = super::super::app(cx);
        self.status.unhide_problem = crate::words::why_not(&app.unhide_mod(id));
        self.chosen_reads.files = None;
        self.read_chosen(cx);
        self.sync_details(cx);
    }
}
