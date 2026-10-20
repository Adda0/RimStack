use super::{id, ids, qobject};
use crate::App;
use crate::workspace::load_order::Side;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl, QVariant};
use rimstack_core::base::error::Result;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::rimworld::modsconfig;
use rimstack_core::{say, say_n};

impl qobject::ModsData {
    fn refuse_list(self: Pin<&mut Self>) {
        let said = crate::words::said(&say!("That is not a list of mods."));
        self.told(said, QString::from("err"));
    }

    fn done_said(mut self: Pin<&mut Self>, said: Sentence) {
        if self.records {
            self.as_mut().done(crate::words::said(&said));
        }
    }

    pub(super) fn order_mut(self: Pin<&mut Self>) -> &mut crate::workspace::load_order::LoadOrder {
        &mut self.rust_mut().get_mut().order
    }

    pub(super) fn move_mod(mut self: Pin<&mut Self>, active: bool, from: i32, to: i32) {
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        let app = self.app();
        self.as_mut()
            .order_mut()
            .move_mod(&app, Side::of(active), from, to);
        self.changed();
    }

    pub(super) fn take(mut self: Pin<&mut Self>, from_active: bool, index: i32, to: i32) {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let from = Side::of(from_active);
        let name = self
            .order
            .list(from)
            .mods()
            .get(index)
            .map(|m| m.name.clone());
        let Some(name) = name else {
            return;
        };
        let app = self.app();
        let to = usize::try_from(to).ok();
        self.as_mut().order_mut().take(&app, from, index, to);
        self.as_mut().changed();
        let said = match from {
            Side::Inactive => say!("Activated {}", name),
            Side::Active => say!("Deactivated {}", name),
        };
        self.done_said(said);
    }

    pub(super) fn take_all(mut self: Pin<&mut Self>, from_active: bool, given: &QVariant) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        if ids.is_empty() {
            return;
        }
        let from = Side::of(from_active);
        let app = self.app();
        self.as_mut().order_mut().take_all(&app, from, &ids);
        self.as_mut().changed();
        let said = match from {
            Side::Inactive => say_n!(ids.len(), "Activated {} mod", "Activated {} mods"),
            Side::Active => say_n!(ids.len(), "Deactivated {} mod", "Deactivated {} mods"),
        };
        self.done_said(said);
    }

    pub(super) fn to_edge(mut self: Pin<&mut Self>, given: &QVariant, top: bool) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        self.as_mut().order_mut().to_edge(&app, &ids, top);
        self.as_mut().changed();
        self.done_said(say_n!(ids.len(), "Moved {} mod", "Moved {} mods"));
    }

    pub(super) fn nudge(mut self: Pin<&mut Self>, active: bool, given: &QVariant, by: i32) -> bool {
        let Some(ids) = ids(given) else {
            self.refuse_list();
            return false;
        };
        let app = self.app();
        let moved = self
            .as_mut()
            .order_mut()
            .nudge(&app, Side::of(active), &ids, by as isize);
        if moved {
            self.changed();
        }
        moved
    }

    pub(super) fn toggle_category(mut self: Pin<&mut Self>, active: bool, category: i32) {
        let found = usize::try_from(category).ok();
        let Some(&category) = found.and_then(|at| rimstack_core::mods::list::Category::ALL.get(at))
        else {
            return;
        };
        let app = self.app();
        self.as_mut()
            .order_mut()
            .toggle_category(&app, Side::of(active), category);
        self.show();
    }

    pub(super) fn fold(mut self: Pin<&mut Self>, active: bool, mod_id: i32, open: bool) {
        let Some(id) = id(mod_id) else {
            return;
        };
        let app = self.app();
        if self
            .as_mut()
            .order_mut()
            .fold(&app, Side::of(active), id, open)
        {
            self.show();
        }
    }

    pub(super) fn toggle_sub(mut self: Pin<&mut Self>, active: bool, row: i32) {
        let Ok(row) = usize::try_from(row) else {
            return;
        };
        let app = self.app();
        self.as_mut()
            .order_mut()
            .toggle_sub(&app, Side::of(active), row);
        self.show();
    }

    pub(super) fn remove_sub(mut self: Pin<&mut Self>, active: bool, row: i32) {
        let group = usize::try_from(row)
            .ok()
            .and_then(|row| self.order.group_on(Side::of(active), row));
        let Some((tag, ids)) = group else {
            return;
        };
        let app = self.app();
        app.remove_tag(&ids, &tag);
        self.as_mut().order_mut().sync(&app);
        self.show();
    }

    pub(super) fn show_by(mut self: Pin<&mut Self>, column: &QString, descending: bool) {
        self.as_mut()
            .order_mut()
            .show_by(&column.to_string(), descending);
        self.show();
    }

    pub(super) fn undo(mut self: Pin<&mut Self>) {
        let app = self.app();
        if self.as_mut().order_mut().undo(&app) {
            self.changed();
        }
    }

    pub(super) fn redo(mut self: Pin<&mut Self>) {
        let app = self.app();
        if self.as_mut().order_mut().redo(&app) {
            self.changed();
        }
    }

    pub(super) fn discard_changes(mut self: Pin<&mut Self>) {
        let app = self.app();
        self.as_mut().order_mut().discard_changes(&app);
        self.as_mut().changed();
        self.done_said(say!("Back to the saved list"));
    }

    pub(super) fn use_list(mut self: Pin<&mut Self>, given: &QVariant, quiet: bool) {
        let Some(ids) = crate::qml::values::from_qml::<Vec<String>>(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        let keep = self.records && !quiet;
        let before = keep
            .then(|| app.snapshot("@list", app.session().current()))
            .flatten();
        let count = ids.len();
        self.as_mut().order_mut().use_list(&app, ids);
        self.as_mut().changed();
        if keep {
            let taken = say_n!(
                count,
                "Another list taken: {} mod",
                "Another list taken: {} mods"
            );
            app.note_change(taken, before.as_deref());
            let said = say_n!(count, "Using a list of {} mod", "Using a list of {} mods");
            self.done_said(said);
        }
    }

    pub(super) fn use_replacements(mut self: Pin<&mut Self>) {
        let app = self.app();
        let Some((ids, count)) = app.with_replacements() else {
            return;
        };
        let keep = self.records;
        let before = keep
            .then(|| app.snapshot("@list", app.session().current()))
            .flatten();
        self.as_mut().order_mut().use_list(&app, ids);
        self.as_mut().changed();
        let said = say_n!(
            count,
            "{} mod swapped for its replacement",
            "{} mods swapped for their replacements"
        );
        if keep {
            app.note_change(said.clone(), before.as_deref());
        }
        self.done_said(said);
    }

    pub(super) fn export_list(mut self: Pin<&mut Self>, chosen: &QUrl) {
        let Some(given) = chosen.to_local_file() else {
            let said = crate::words::said(&say!("That is not a file here."));
            return self.as_mut().told(said, QString::from("err"));
        };
        let app = self.app();
        let loaded = app.library();
        let Ok((library, _)) = &*loaded else {
            let said = crate::words::said(&say!("The mods are not read yet."));
            return self.as_mut().told(said, QString::from("err"));
        };
        let rows: Vec<rimstack_core::mods::share_file::Row> = self
            .rust()
            .order
            .list(Side::Active)
            .mods()
            .iter()
            .filter_map(|m| {
                let installed = library.get(m.installed?)?;
                let about = installed.about.as_ref().ok();
                Some(rimstack_core::mods::share_file::Row {
                    name: installed.display_name().to_string(),
                    author: about.map(|a| a.authors.join(", ")).unwrap_or_default(),
                    package_id: about.map(|a| a.package_id.clone()).unwrap_or_default(),
                    file_id: if installed.source == rimstack_core::rimworld::paths::Source::Workshop
                    {
                        installed.folder.clone()
                    } else {
                        String::new()
                    },
                    supported: about
                        .map(|a| a.supported_versions.clone())
                        .unwrap_or_default(),
                    downloaded: installed.downloaded,
                    updated: m.updated,
                    source: installed.source_label().to_string(),
                    path: installed.path.to_string_lossy().into_owned(),
                    url: about.and_then(|a| a.url.clone()).unwrap_or_default(),
                })
            })
            .collect();
        let mut path = std::path::PathBuf::from(given.to_string());
        if path.extension().is_none() {
            path.set_extension("csv");
        }
        let ids: Vec<String> = rows
            .iter()
            .map(|row| row.package_id.clone())
            .filter(|id| !id.is_empty())
            .collect();
        let kind = path
            .extension()
            .and_then(|end| end.to_str())
            .map(str::to_lowercase);
        let (text, what) = match kind.as_deref() {
            Some("rml") => (
                rimstack_core::mods::share_file::rml_xml(&ids),
                say!("a RimSort mod list"),
            ),
            Some("xml") => (
                rimstack_core::mods::share_file::modsconfig_xml(&ids, &library.game_version),
                say!("a game's ModsConfig"),
            ),
            _ => (rimstack_core::mods::share_file::csv(&rows), say!("CSV")),
        };
        let said = rimstack_core::base::atomic_file::write(&path, text);
        match said {
            Ok(()) => {
                let count = say_n!(rows.len(), "One active mod", "{} active mods");
                let note = say!(
                    "{} as {} in {}",
                    count.inside(),
                    what.inside(),
                    path.display()
                );
                let said = crate::words::said(&note);
                self.as_mut().told(said, QString::from("ok"));
            }
            Err(problem) => {
                let said = crate::words::said_of(&problem);
                self.as_mut().told(said, QString::from("err"));
            }
        }
    }

    pub(super) fn keep_file(mut self: Pin<&mut Self>, take: bool) {
        let app = self.app();
        self.as_mut().order_mut().keep_file(&app, take);
        self.as_mut().set_save_problem(QString::default());
        self.as_mut().set_file_changed(false);
        self.as_mut().changed();
    }

    pub(super) fn save(mut self: Pin<&mut Self>) -> bool {
        if self.saving {
            return false;
        }
        if self.order.previewing() {
            self.as_mut().end_sort(true);
        }
        let app = self.app();
        let saving = match self.as_mut().order_mut().to_save(&app) {
            Ok(saving) => saving,
            Err(problem) => {
                self.as_mut()
                    .set_save_problem(crate::words::said_of(&problem));
                self.as_mut().saved(false);
                return false;
            }
        };
        let records = self.records;
        let changes = app.session().changes().said();
        let before = app.session().saved().to_vec();
        self.as_mut().set_saving(true);
        let worker = app.clone();
        crate::jobs::run(
            "save",
            self.qt_thread(),
            move || {
                let snapshot = records
                    .then(|| worker.snapshot("@saving", &before))
                    .flatten();
                let written = worker.write_list(&saving.file, &saving.written, &saving.expected);
                if written.is_ok() {
                    worker.record_order(rimstack_core::kept::orders::How::Saved, &saving.written);
                }
                if written.is_ok() && records {
                    let said = say!("Saved the load order: {}", changes.inside());
                    worker.note_change(said, snapshot.as_deref());
                }
                (saving.list, written)
            },
            move |mut me, (list, written)| {
                if written.is_ok() {
                    me.as_mut().order_mut().saved(&app, list);
                }
                me.as_mut()
                    .set_save_problem(crate::words::why_not(&written));
                let outside = written.as_ref().err() == Some(&modsconfig::changed_outside());
                me.as_mut().set_file_changed(outside);
                me.as_mut().set_saving(false);
                me.as_mut().changed();
                me.saved(written.is_ok());
            },
        );
        true
    }

    pub(super) fn preview_sort(mut self: Pin<&mut Self>) {
        if !self.files_read {
            return;
        }
        let app = self.app();
        self.as_mut().order_mut().preview_sort(&app);
        self.show();
    }

    pub(super) fn end_sort(mut self: Pin<&mut Self>, keep: bool) {
        let app = self.app();
        if !keep {
            self.as_mut().order_mut().drop_sort(&app);
            return self.show();
        }
        if !self.order.previewing() {
            return;
        }
        let moves = self.order.sort_move_count();
        let before = self
            .records
            .then(|| app.snapshot("@sorting", app.session().current()))
            .flatten();
        self.as_mut().order_mut().keep_sort(&app);
        self.as_mut().changed();
        if self.records {
            let sorted = say_n!(moves, "Sorted: {} mod moved", "Sorted: {} mods moved");
            app.note_change(sorted.clone(), before.as_deref());
            self.done_said(sorted);
        }
    }

    pub(super) fn count_sort_moves(self: Pin<&mut Self>) {
        const QUIET: std::time::Duration = std::time::Duration::from_millis(400);
        let count = self.order.sort_count().filter(|_| self.files_read);
        let Some(count) = count else {
            self.counting.cancel();
            self.set_sort_pending(0);
            return;
        };
        let app = self.app();
        self.counting.run(
            "sort count",
            self.qt_thread(),
            move |turn| {
                std::thread::sleep(QUIET);
                turn.current().then(|| count.moves(&app))
            },
            |me, moves| {
                if let Some(moves) = moves {
                    me.set_sort_pending(i32::try_from(moves).unwrap_or(i32::MAX));
                }
            },
        );
    }

    pub(super) fn rescan(mut self: Pin<&mut Self>) {
        if self.rescanning || self.demo {
            return;
        }
        self.as_mut().set_rescanning(true);
        let app = self.app();
        crate::jobs::run("rescan", self.qt_thread(), move || app.rescan(), |_, ()| {});
    }

    pub(super) fn trust(mut self: Pin<&mut Self>, given: &QVariant, yes: bool) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        app.set_version_ok(&ids, yes);
        self.as_mut().order_mut().recheck(&app);
        self.show();
    }

    pub(super) fn ignore_problem(
        mut self: Pin<&mut Self>,
        package_id: &QString,
        key: &QString,
        yes: bool,
    ) {
        let app = self.app();
        app.ignore_problem(&package_id.to_string(), &key.to_string(), yes);
        self.as_mut().order_mut().recheck(&app);
        self.show();
    }

    pub(super) fn warn_again(mut self: Pin<&mut Self>, given: &QVariant) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        app.warn_again(&ids);
        self.as_mut().order_mut().recheck(&app);
        self.show();
    }

    pub(super) fn keep_note(mut self: Pin<&mut Self>, mod_id: i32, tags: &QString, note: &QString) {
        let Some(id) = id(mod_id) else {
            return;
        };
        let app = self.app();
        app.set_note(id, &tags.to_string(), &note.to_string());
        self.as_mut().order_mut().sync(&app);
        self.show();
    }

    pub(super) fn add_tag(mut self: Pin<&mut Self>, given: &QVariant, tag: &QString) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let (app, tag) = (self.app(), tag.to_string());
        app.add_tag(&ids, &tag);
        self.as_mut().order_mut().sync(&app);
        self.as_mut().show();
        let said = say_n!(ids.len(), "Tagged {} mod: {}", "Tagged {} mods: {}", tag);
        self.told(crate::words::said(&said), QString::from("ok"));
    }

    pub(super) fn add_tags(self: Pin<&mut Self>, mod_id: i32, tags: &QString) {
        let tags = tags.to_string();
        self.change_tags(mod_id, |app, id| app.add_tags(id, &tags));
    }

    pub(super) fn remove_tag(self: Pin<&mut Self>, mod_id: i32, tag: &QString) {
        let tag = tag.to_string();
        self.change_tags(mod_id, |app, id| app.remove_tag(&[id], &tag));
    }

    pub(super) fn clear_tags(self: Pin<&mut Self>, mod_id: i32) {
        self.change_tags(mod_id, |app, id| app.clear_tags(id));
    }

    fn change_tags(
        mut self: Pin<&mut Self>,
        mod_id: i32,
        change: impl FnOnce(&App, ModId) -> bool,
    ) {
        let Some(id) = id(mod_id) else {
            return;
        };
        let app = self.app();
        if change(&app, id) {
            self.as_mut().order_mut().sync(&app);
            self.show();
        }
    }

    pub(super) fn group(mut self: Pin<&mut Self>, given: &QVariant, name: &QString) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        app.set_group(&ids, &name.to_string());
        self.as_mut().order_mut().sync(&app);
        self.show();
    }

    pub(super) fn delete_group(mut self: Pin<&mut Self>, name: &QString) {
        let app = self.app();
        if app.delete_group(&name.to_string()) {
            self.as_mut().order_mut().sync(&app);
            self.show();
        }
    }

    pub(super) fn paint(mut self: Pin<&mut Self>, given: &QVariant, name: &QString) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        if app.set_colour(&ids, &name.to_string()) {
            self.as_mut().order_mut().sync(&app);
            self.show();
        }
    }

    pub(super) fn pin(mut self: Pin<&mut Self>, given: &QVariant, on: bool) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        if app.set_pinned(&ids, on) {
            self.as_mut().order_mut().sync(&app);
            self.show();
        }
    }

    pub(super) fn file_under(mut self: Pin<&mut Self>, mod_id: i32, label: &QString) {
        let Some(id) = id(mod_id) else {
            return;
        };
        let app = self.app();
        app.set_category(id, &label.to_string());
        self.as_mut().order_mut().sync(&app);
        self.show();
    }

    pub(super) fn add_rule(self: Pin<&mut Self>, first: i32, then: i32) {
        self.change_rule(first, then, true);
    }

    pub(super) fn remove_rule(self: Pin<&mut Self>, first: i32, then: i32) {
        self.change_rule(first, then, false);
    }

    fn change_rule(self: Pin<&mut Self>, first: i32, then: i32, add: bool) {
        if let (Some(first), Some(then)) = (id(first), id(then)) {
            self.app().change_rule(first, then, add);
            self.show();
        }
    }

    pub(super) fn win_over(mut self: Pin<&mut Self>, mod_id: i32, other: i32) {
        let (Some(id), Some(other)) = (id(mod_id), id(other)) else {
            return;
        };
        let app = self.app();
        app.change_rule(other, id, true);
        if let Some((from, to)) = self.order.list(Side::Active).below(id, other) {
            self.as_mut()
                .order_mut()
                .move_mod(&app, Side::Active, from, to);
        }
        self.changed();
    }

    pub(super) fn use_copy(mut self: Pin<&mut Self>, mod_id: i32, written: &QString) {
        let Some(id) = id(mod_id) else {
            return;
        };
        if self.order.previewing() {
            return;
        }
        let app = self.app();
        if !self
            .as_mut()
            .order_mut()
            .use_copy(&app, id, written.to_string())
        {
            self.done_said(say!("That copy is no longer there"));
            return;
        }
        self.as_mut().changed();
        self.done_said(say!("The load order now names this copy"));
    }

    pub(super) fn fix(
        mut self: Pin<&mut Self>,
        mod_id: i32,
        action: &QString,
        target: &QString,
    ) -> bool {
        if self.order.previewing() {
            return true;
        }
        let target = target.to_string();
        match action.to_string().as_str() {
            "activate" => {
                let found = self.package_mod(false, &QString::from(&target));
                if let Some(at) =
                    id(found).and_then(|i| self.order.list(Side::Inactive).position_of(i))
                {
                    self.take(false, at as i32, -1);
                }
                true
            }
            "deactivate" => {
                let at = target.parse::<i32>().unwrap_or(-1);
                self.take(true, at, -1);
                true
            }
            "trust" => {
                self.as_mut()
                    .trust(&crate::qml::values::to_qml(&[mod_id]), true);
                true
            }
            "move" => {
                let Some((at, side)) = target.split_once(':') else {
                    return true;
                };
                let (Ok(other), Some(id)) = (at.parse::<usize>(), id(mod_id)) else {
                    return true;
                };
                let app = self.app();
                self.as_mut()
                    .order_mut()
                    .move_next_to(&app, id, other, side == "after");
                self.as_mut().changed();
                true
            }
            _ => false,
        }
    }

    pub(super) fn fix_all(mut self: Pin<&mut Self>) {
        if self.order.previewing() {
            return;
        }
        let needed = self.order.needed();
        let ids = self.needed_ids(&needed);
        let app = self.app();
        self.as_mut()
            .order_mut()
            .take_all(&app, Side::Inactive, &ids);
        self.as_mut().changed();
        self.done_said(say!("Activated the needed mods"));
    }

    pub(super) fn activate_fresh(mut self: Pin<&mut Self>) {
        let app = self.app();
        let fresh: Vec<ModId> = self.order.fresh(&app, Side::Inactive);
        if fresh.is_empty() || self.order.previewing() {
            return;
        }
        self.as_mut()
            .order_mut()
            .take_all(&app, Side::Inactive, &fresh);
        self.as_mut().changed();
        self.done_said(say_n!(
            fresh.len(),
            "Activated {} new mod",
            "Activated {} new mods"
        ));
    }

    pub(super) fn bin_kind(mut self: Pin<&mut Self>, mod_id: i32, kind: i32) {
        use crate::workspace::upkeep::{BIN_DDS_ONLY, BIN_KEEP_DDS, BIN_RESUBSCRIBE};
        let Some(id) = id(mod_id) else {
            return;
        };
        let app = self.app();
        let done = match kind {
            BIN_KEEP_DDS => app.bin_except_dds(id).map(|_| ()),
            BIN_DDS_ONLY => app.remove_dds(id).map(|_| ()),
            BIN_RESUBSCRIBE => app.bin_and_resubscribe(id),
            _ => app.bin_mod(id),
        };
        self.as_mut()
            .set_upkeep_problem(crate::words::why_not(&done));
        if done.is_ok() {
            self.rescan();
        }
    }

    pub(super) fn bin_title(&self, mod_id: i32, kind: i32) -> QString {
        self.bin_words(mod_id, kind, |title, _| title)
    }

    pub(super) fn bin_note(&self, mod_id: i32, kind: i32) -> QString {
        self.bin_words(mod_id, kind, |_, note| note)
    }

    fn bin_words(
        &self,
        mod_id: i32,
        kind: i32,
        pick: impl Fn(Sentence, Sentence) -> Sentence,
    ) -> QString {
        let Some(id) = id(mod_id) else {
            return QString::default();
        };
        let (title, note) = self.app().bin_prompt(id, kind);
        crate::words::said(&pick(title, note))
    }

    pub(super) fn bin_label(&self, kind: i32) -> QString {
        crate::words::said(&crate::workspace::upkeep::bin_label(kind))
    }

    /// What the selected installed mods offer for cleanup:
    /// `{ unsubscribe, unsubscribeBytes, bin, binBytes }`.
    pub(super) fn cleanup(&self, given: &QVariant) -> QVariant {
        let app = self.app();
        let wanted = ids(given).unwrap_or_default();
        crate::qml::values::to_qml(&self.rust().order.cleanup(&app, &wanted))
    }

    /// Every selected mod allowed to go goes to the bin, Steam's folders first
    /// aside only for a local copy replacing one.
    pub(super) fn bin_many(mut self: Pin<&mut Self>, given: &QVariant) {
        let Some(ids) = ids(given) else {
            return self.refuse_list();
        };
        let app = self.app();
        let done = app.bin_mods(&ids).map(drop);
        self.as_mut()
            .set_upkeep_problem(crate::words::why_not(&done));
        if done.is_ok() {
            self.rescan();
        }
    }

    pub(super) fn copy_to_local(mut self: Pin<&mut Self>, mod_id: i32) {
        let Some(id) = id(mod_id) else {
            return;
        };
        self.as_mut().set_upkeep_problem(QString::default());
        let app = self.app();
        crate::jobs::run(
            "copy to the game folder",
            self.qt_thread(),
            move || app.copy_to_local(id).err(),
            |mut me, problem| match problem {
                Some(problem) => me.set_upkeep_problem(crate::words::said_of(&problem)),
                None => me.as_mut().rescan(),
            },
        );
    }

    pub(super) fn copy_many_to_local(mut self: Pin<&mut Self>, given: &QVariant) {
        let Some(ids) = ids(given) else {
            return;
        };
        if ids.is_empty() {
            return;
        }
        self.as_mut().set_upkeep_problem(QString::default());
        let app = self.app();
        crate::jobs::run(
            "copy to the game folder",
            self.qt_thread(),
            move || app.copy_many_to_local(&ids),
            |mut me, done: Result<rimstack_core::sources::maintenance::Copied>| match done {
                Ok(copies) => {
                    let said = match copies.present {
                        0 => say_n!(
                            copies.copied,
                            "{} mod copied to the game's Mods folder",
                            "{} mods copied to the game's Mods folder"
                        ),
                        _ => say!(
                            "{} copied, {} were there already and stay",
                            copies.copied,
                            copies.present
                        ),
                    };
                    me.as_mut()
                        .told(crate::words::said(&said), QString::from("ok"));
                    me.as_mut().rescan();
                }
                Err(problem) => me.set_upkeep_problem(crate::words::said_of(&problem)),
            },
        );
    }

    pub(super) fn kept_copies_of(&self, mod_id: i32) -> QVariant {
        let Some(id) = id(mod_id) else {
            return QVariant::default();
        };
        crate::qml::values::to_qml(&self.app().kept_copies_of(id))
    }

    pub(super) fn restore_kept_copy(mut self: Pin<&mut Self>, mod_id: i32, updated: &QVariant) {
        let (Some(id), Some(updated)) = (id(mod_id), crate::qml::values::from_qml(updated)) else {
            return;
        };
        self.as_mut().set_upkeep_problem(QString::default());
        let app = self.app();
        crate::jobs::run(
            "restore a kept copy",
            self.qt_thread(),
            move || app.restore_kept_copy(id, updated).err(),
            |mut me, problem| match problem {
                Some(problem) => me.set_upkeep_problem(crate::words::said_of(&problem)),
                None => {
                    let said = crate::words::said(&say!(
                        "Restored; this copy now wins over the Workshop's one"
                    ));
                    me.as_mut().told(said, QString::from("ok"));
                    me.as_mut().rescan();
                }
            },
        );
    }

    pub(super) fn install_zip(self: Pin<&mut Self>, chosen: &QUrl) {
        // A file:// URL and a bare path are taken; anything else is refused.
        let file = chosen.to_local_file().map(|local| local.to_string());
        let file = match file.filter(|local| !local.is_empty()) {
            Some(local) => local,
            None if chosen.scheme().is_none_or(|scheme| scheme.is_empty()) => {
                chosen.to_display_string().to_string()
            }
            None => {
                self.told(
                    crate::words::said(&say!(
                        "{} is not a file on this computer",
                        chosen.to_display_string()
                    )),
                    QString::from("err"),
                );
                return;
            }
        };
        // A .rar or folder? The unpacker says no; cheaper than a full open.
        if !file.to_lowercase().ends_with(".zip") {
            self.told(
                crate::words::said(&say!("{} is not a .zip", file)),
                QString::from("err"),
            );
            return;
        }
        let path = std::path::PathBuf::from(file);
        let app = self.app();
        crate::jobs::run(
            "install a mod from a zip",
            self.qt_thread(),
            move || app.install_zip(&path),
            |mut me, landed| match landed {
                Ok(folder) => {
                    let name = folder
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    me.as_mut().told(
                        crate::words::said(&say!("{} is now with the local mods", name)),
                        QString::from("ok"),
                    );
                    me.as_mut().rescan();
                }
                Err(problem) => me.told(crate::words::said_of(&problem), QString::from("err")),
            },
        );
    }

    pub(super) fn reset_settings(mut self: Pin<&mut Self>) {
        let done = id(self.selected).map(|id| self.app().reset_mod_settings(id));
        self.as_mut().told_result(done);
    }

    pub(super) fn set_orphan_settings_aside(mut self: Pin<&mut Self>) {
        let done = self.app().set_orphan_settings_aside();
        self.as_mut().told_result(Some(done));
    }

    fn told_result(mut self: Pin<&mut Self>, done: Option<Result<Sentence>>) {
        let Some(done) = done else {
            return;
        };
        let tone = QString::from(if done.is_err() { "err" } else { "ok" });
        let said = crate::words::said_or_why(&done);
        self.as_mut().told(said, tone);
    }
}
