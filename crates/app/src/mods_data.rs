#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;

        include!("rimstack-app/src/mod_rows.cxxqt.h");
        type ModRows = crate::mod_rows::qobject::ModRows;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_uncreatable]
        #[qproperty(*mut ModRows, active, READ, CONSTANT)]
        #[qproperty(*mut ModRows, inactive, READ, CONSTANT)]
        #[qproperty(bool, dirty)]
        /// Empty for none.
        #[qproperty(QString, unsaved)]
        /// Removing a switched-off mod in the newest save; empty for nothing said.
        #[qproperty(QString, left_save, cxx_name = "leftSave")]
        #[qproperty(bool, can_undo, cxx_name = "canUndo")]
        #[qproperty(bool, can_redo, cxx_name = "canRedo")]
        #[qproperty(bool, previewing)]
        #[qproperty(QString, sort_note, cxx_name = "sortNote")]
        #[qproperty(QString, sort_more, cxx_name = "sortMore")]
        #[qproperty(i32, sort_pending, cxx_name = "sortPending")]
        #[qproperty(i32, sort_move_count, cxx_name = "sortMoveCount")]
        #[qproperty(i32, errors)]
        #[qproperty(i32, warnings)]
        #[qproperty(i32, troubled)]
        #[qproperty(i32, conflicts)]
        #[qproperty(bool, files_read, cxx_name = "filesRead")]
        #[qproperty(bool, rescanning)]
        #[qproperty(bool, notes_saved, cxx_name = "notesSaved")]
        /// Empty for none.
        #[qproperty(QString, problem)]
        #[qproperty(QString, status_active, cxx_name = "statusActive")]
        #[qproperty(QString, status_inactive, cxx_name = "statusInactive")]
        /// Ids held from before a change mean nothing.
        #[qproperty(i32, library_version, cxx_name = "libraryVersion")]
        #[qproperty(QVariant, overview)]
        #[qproperty(QVariant, summary)]
        /// -1 for none.
        #[qproperty(i32, selected)]
        #[qproperty(QVariant, details)]
        /// `{ count, ddsMade }`, once its folder is walked.
        #[qproperty(QVariant, textures)]
        #[qproperty(QVariant, fresh)]
        #[qproperty(bool, headings)]
        /// "category", "name", "updated", "downloaded" or "supports".
        #[qproperty(QString, inactive_order, cxx_name = "inactiveOrder")]
        /// The inactive order runs the other way: oldest, lowest or Z first.
        #[qproperty(bool, inactive_reversed, cxx_name = "inactiveReversed")]
        /// `{ chosen, next, nextText, all: [{ key, text, chosen }], reversed, direction, directionIcon }`.
        #[qproperty(QVariant, inactive_orders, cxx_name = "inactiveOrders")]
        #[qproperty(QString, filter)]
        /// Empty for none.
        #[qproperty(QString, save_problem, cxx_name = "saveProblem")]
        #[qproperty(bool, saving)]
        #[qproperty(bool, file_changed, cxx_name = "fileChanged")]
        #[qproperty(QString, upkeep_problem, cxx_name = "upkeepProblem")]
        type ModsData = super::ModsDataRust;
    }

    impl cxx_qt::Threading for ModsData {}

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[cxx_name = "make_unique"]
        #[namespace = "rust::cxxqtlib1"]
        fn make_mods_data() -> UniquePtr<ModsData>;
    }

    extern "RustQt" {
        #[qsignal]
        fn done(self: Pin<&mut ModsData>, said: QString);
        /// Tone is "ok" or "err".
        #[qsignal]
        fn told(self: Pin<&mut ModsData>, said: QString, tone: QString);
        #[qsignal]
        fn saved(self: Pin<&mut ModsData>, ok: bool);
    }

    extern "RustQt" {
        #[qinvokable]
        #[cxx_name = "moveMod"]
        fn move_mod(self: Pin<&mut ModsData>, active: bool, from: i32, to: i32);
        /// At -1 `to`, where its category and rules put it.
        #[qinvokable]
        fn take(self: Pin<&mut ModsData>, from_active: bool, index: i32, to: i32);
        #[qinvokable]
        #[cxx_name = "takeAll"]
        fn take_all(self: Pin<&mut ModsData>, from_active: bool, ids: &QVariant);
        #[qinvokable]
        #[cxx_name = "toEdge"]
        fn to_edge(self: Pin<&mut ModsData>, ids: &QVariant, top: bool);
        /// Moves each mod `by` places, keeping their order; false when none moved.
        #[qinvokable]
        fn nudge(self: Pin<&mut ModsData>, active: bool, ids: &QVariant, by: i32) -> bool;
        /// Folds the innermost open heading over the mod, or opens the outermost folded one.
        #[qinvokable]
        fn fold(self: Pin<&mut ModsData>, active: bool, mod_id: i32, open: bool);
        #[qinvokable]
        #[cxx_name = "toggleCategory"]
        fn toggle_category(self: Pin<&mut ModsData>, active: bool, category: i32);
        #[qinvokable]
        #[cxx_name = "toggleSub"]
        fn toggle_sub(self: Pin<&mut ModsData>, active: bool, row: i32);
        #[qinvokable]
        #[cxx_name = "removeSub"]
        fn remove_sub(self: Pin<&mut ModsData>, active: bool, row: i32);
        #[qinvokable]
        #[cxx_name = "showBy"]
        fn show_by(self: Pin<&mut ModsData>, column: &QString, descending: bool);
        #[qinvokable]
        fn undo(self: Pin<&mut ModsData>);
        #[qinvokable]
        fn redo(self: Pin<&mut ModsData>);
        #[qinvokable]
        #[cxx_name = "discardChanges"]
        fn discard_changes(self: Pin<&mut ModsData>);
        #[qinvokable]
        #[cxx_name = "useList"]
        fn use_list(self: Pin<&mut ModsData>, ids: &QVariant, quiet: bool);
        /// Every active mod with an installed replacement gives its place to it.
        #[qinvokable]
        #[cxx_name = "useReplacements"]
        fn use_replacements(self: Pin<&mut ModsData>);
        #[qinvokable]
        #[cxx_name = "keepFile"]
        fn keep_file(self: Pin<&mut ModsData>, take: bool);
        #[qinvokable]
        fn save(self: Pin<&mut ModsData>) -> bool;
        #[qinvokable]
        #[cxx_name = "previewSort"]
        fn preview_sort(self: Pin<&mut ModsData>);
        #[qinvokable]
        #[cxx_name = "endSort"]
        fn end_sort(self: Pin<&mut ModsData>, keep: bool);
        #[qinvokable]
        fn rescan(self: Pin<&mut ModsData>);
        #[qinvokable]
        fn trust(self: Pin<&mut ModsData>, ids: &QVariant, yes: bool);
        #[qinvokable]
        #[cxx_name = "ignoreProblem"]
        fn ignore_problem(self: Pin<&mut ModsData>, package_id: &QString, key: &QString, yes: bool);
        #[qinvokable]
        #[cxx_name = "warnAgain"]
        fn warn_again(self: Pin<&mut ModsData>, ids: &QVariant);
        #[qinvokable]
        #[cxx_name = "keepNote"]
        fn keep_note(self: Pin<&mut ModsData>, mod_id: i32, tags: &QString, note: &QString);
        /// Stored after a pause in the typing, or by `keepDrafts`; the mod is the one the edit was made for.
        #[qinvokable]
        #[cxx_name = "draftNote"]
        fn draft_note(self: Pin<&mut ModsData>, mod_id: i32, note: &QString);
        /// Takes the tag off every mod grouped under it.
        #[qinvokable]
        #[cxx_name = "deleteGroup"]
        fn delete_group(self: Pin<&mut ModsData>, name: &QString);
        /// Paints the rows of these mods with a swatch, by its name; "" takes the colour off.
        #[qinvokable]
        fn paint(self: Pin<&mut ModsData>, ids: &QVariant, name: &QString);
        /// Sort leaves these mods where they are, or no longer.
        #[qinvokable]
        fn pin(self: Pin<&mut ModsData>, ids: &QVariant, on: bool);
        #[qinvokable]
        #[cxx_name = "keepDrafts"]
        fn keep_drafts(self: Pin<&mut ModsData>);
        #[qinvokable]
        #[cxx_name = "addTag"]
        fn add_tag(self: Pin<&mut ModsData>, ids: &QVariant, tag: &QString);
        /// Each of the comma-separated `tags` the mod has not yet.
        #[qinvokable]
        #[cxx_name = "addTags"]
        fn add_tags(self: Pin<&mut ModsData>, mod_id: i32, tags: &QString);
        #[qinvokable]
        #[cxx_name = "removeTag"]
        fn remove_tag(self: Pin<&mut ModsData>, mod_id: i32, tag: &QString);
        #[qinvokable]
        #[cxx_name = "clearTags"]
        fn clear_tags(self: Pin<&mut ModsData>, mod_id: i32);
        /// An empty name takes them out.
        #[qinvokable]
        fn group(self: Pin<&mut ModsData>, ids: &QVariant, name: &QString);
        #[qinvokable]
        #[cxx_name = "fileUnder"]
        fn file_under(self: Pin<&mut ModsData>, mod_id: i32, label: &QString);
        #[qinvokable]
        #[cxx_name = "addRule"]
        fn add_rule(self: Pin<&mut ModsData>, first: i32, then: i32);
        #[qinvokable]
        #[cxx_name = "removeRule"]
        fn remove_rule(self: Pin<&mut ModsData>, first: i32, then: i32);
        #[qinvokable]
        #[cxx_name = "winOver"]
        fn win_over(self: Pin<&mut ModsData>, mod_id: i32, other: i32);
        #[qinvokable]
        fn fix(self: Pin<&mut ModsData>, mod_id: i32, action: &QString, target: &QString) -> bool;
        #[qinvokable]
        #[cxx_name = "fixAll"]
        fn fix_all(self: Pin<&mut ModsData>);
        #[qinvokable]
        #[cxx_name = "activateFresh"]
        fn activate_fresh(self: Pin<&mut ModsData>);
        /// Takes the way to delete it: the whole mod, its files but the DDS textures,
        /// the DDS textures alone, or the whole mod and a subscription again.
        /// What the selection offers for cleanup:
        /// `{ unsubscribe, unsubscribeBytes, bin, binBytes }`.
        #[qinvokable]
        fn cleanup(self: &ModsData, ids: &QVariant) -> QVariant;
        /// Every selected mod that may go goes to the bin.
        #[qinvokable]
        #[cxx_name = "binMany"]
        fn bin_many(self: Pin<&mut ModsData>, ids: &QVariant);
        #[qinvokable]
        #[cxx_name = "binKind"]
        fn bin_kind(self: Pin<&mut ModsData>, mod_id: i32, kind: i32);
        #[qinvokable]
        #[cxx_name = "binTitle"]
        fn bin_title(self: &ModsData, mod_id: i32, kind: i32) -> QString;
        #[qinvokable]
        #[cxx_name = "binNote"]
        fn bin_note(self: &ModsData, mod_id: i32, kind: i32) -> QString;
        #[qinvokable]
        #[cxx_name = "binLabel"]
        fn bin_label(self: &ModsData, kind: i32) -> QString;
        #[qinvokable]
        #[cxx_name = "copyToLocal"]
        fn copy_to_local(self: Pin<&mut ModsData>, mod_id: i32);
        /// The chosen mods, whose copies then win over Steam's in the Workshop folder.
        #[qinvokable]
        #[cxx_name = "copyManyToLocal"]
        fn copy_many_to_local(self: Pin<&mut ModsData>, ids: &QVariant);
        /// The Workshop update times of the copies kept for the mod, newest first.
        #[qinvokable]
        #[cxx_name = "keptCopiesOf"]
        fn kept_copies_of(self: &ModsData, mod_id: i32) -> QVariant;
        /// The kept copy of `updated` is copied into the game's Mods folder, where it wins.
        #[qinvokable]
        #[cxx_name = "restoreKeptCopy"]
        fn restore_kept_copy(self: Pin<&mut ModsData>, mod_id: i32, updated: &QVariant);
        /// A dropped or picked .zip becomes a mod in the game's Mods folder.
        #[qinvokable]
        #[cxx_name = "installZip"]
        fn install_zip(self: Pin<&mut ModsData>, chosen: &QUrl);

        #[qinvokable]
        #[cxx_name = "resetSettings"]
        fn reset_settings(self: Pin<&mut ModsData>);
        #[qinvokable]
        #[cxx_name = "setOrphanSettingsAside"]
        fn set_orphan_settings_aside(self: Pin<&mut ModsData>);
        #[qinvokable]
        #[cxx_name = "versionWarned"]
        fn version_warned(self: &ModsData, ids: &QVariant) -> QVariant;
        #[qinvokable]
        #[cxx_name = "packageMods"]
        fn package_mods(self: &ModsData, keys: &QVariant) -> QVariant;
        /// -1 for none.
        #[qinvokable]
        #[cxx_name = "packageMod"]
        fn package_mod(self: &ModsData, active: bool, key: &QString) -> i32;
        /// `[{ name, modId, active }]`, closest first.
        #[qinvokable]
        #[cxx_name = "findMods"]
        fn find_mods(self: &ModsData, text: &QString, most: i32) -> QVariant;
        #[qinvokable]
        fn listing(self: &ModsData) -> QString;
        /// Writes the active list as RimSort's CSV, a game's ModsConfig.xml
        /// or a RimSort .rml, by the name's ending; tells how it went.
        #[qinvokable]
        #[cxx_name = "exportList"]
        fn export_list(self: Pin<&mut ModsData>, chosen: &QUrl);
        /// The load order as a share code, empty without a read library.
        #[qinvokable]
        #[cxx_name = "shareCode"]
        fn share_code(self: &ModsData) -> QString;
        /// A page line per active Workshop mod, empty without a read library.
        #[qinvokable]
        #[cxx_name = "workshopLinks"]
        fn workshop_links(self: &ModsData) -> QString;
        /// `[{ name, moves: [{ name, from, to, modId, why }] }]`.
        #[qinvokable]
        #[cxx_name = "sortMoves"]
        fn sort_moves(self: &ModsData) -> QVariant;
        #[qinvokable]
        #[cxx_name = "firstMoved"]
        fn first_moved(self: &ModsData) -> i32;
        /// -1 for none.
        #[qinvokable]
        #[cxx_name = "nextProblem"]
        fn next_problem(self: &ModsData, row: i32, forward: bool) -> i32;
        #[qinvokable]
        #[cxx_name = "problemPlace"]
        fn problem_place(self: &ModsData, row: i32) -> i32;
        #[qinvokable]
        #[cxx_name = "nameOf"]
        fn name_of(self: &ModsData, mod_id: i32) -> QString;
        /// Clamped to the ends; -1 only for a list without mods.
        #[qinvokable]
        #[cxx_name = "stepFrom"]
        fn step_from(self: &ModsData, active: bool, mod_id: i32, by: i32) -> i32;
        /// The mod a row stands for, a folded heading for the first it hides; -1 for none.
        #[qinvokable]
        #[cxx_name = "stopOn"]
        fn stop_on(self: &ModsData, active: bool, row: i32) -> i32;
        /// Its row, or that of the folded heading over it; -1 when filtered out.
        #[qinvokable]
        #[cxx_name = "placeOf"]
        fn place_of(self: &ModsData, active: bool, mod_id: i32) -> i32;
        #[qinvokable]
        #[cxx_name = "besideMod"]
        fn beside_mod(self: &ModsData, active: bool, mod_id: i32) -> i32;
        /// The installed mods with a row, top to bottom.
        #[qinvokable]
        #[cxx_name = "shownIds"]
        fn shown_ids(self: &ModsData, active: bool) -> QVariant;
        /// Null unless both are shown.
        #[qinvokable]
        #[cxx_name = "othersThrough"]
        fn others_through(self: &ModsData, active: bool, from: i32, to: i32) -> QVariant;
        /// `[earlier, later]`; null unless both are in it.
        #[qinvokable]
        #[cxx_name = "inLoadOrder"]
        fn in_load_order(self: &ModsData, a: i32, b: i32) -> QVariant;
        #[qinvokable]
        #[cxx_name = "subsBeside"]
        fn subs_beside(self: &ModsData, mod_id: i32) -> QVariant;
        /// `[{ term, note }]` for the word typed in the filter, without its `!`; `taken` the terms in it.
        #[qinvokable]
        #[cxx_name = "filterOffers"]
        fn filter_offers(
            self: &ModsData,
            partial: &QString,
            bang: &QString,
            taken: &QVariant,
            behind: i32,
        ) -> QVariant;
        /// Tags to offer for the one typed after the last comma of `text`, leaving out those `had`.
        #[qinvokable]
        #[cxx_name = "tagsFor"]
        fn tags_for(self: &ModsData, had: &QString, text: &QString) -> QVariant;
        #[qinvokable]
        #[cxx_name = "detailsOf"]
        fn details_of(self: Pin<&mut ModsData>, mod_id: i32) -> QVariant;
        /// `{ name, takeOff, packageId, fixes, resubscribe }` for the row at `index` when its mod is not installed; null otherwise.
        #[qinvokable]
        #[cxx_name = "notInstalled"]
        fn not_installed(self: &ModsData, active: bool, index: i32) -> QVariant;
        /// `{ tree, assemblies }`.
        #[qinvokable]
        #[cxx_name = "filesOf"]
        fn files_of(self: &ModsData, mod_id: i32) -> QVariant;
        #[qinvokable]
        #[cxx_name = "columnsToggled"]
        fn columns_toggled(self: &ModsData, kept: &QString, column: &QString) -> QString;
        #[qinvokable]
        #[cxx_name = "columnsMoved"]
        fn columns_moved(self: &ModsData, kept: &QString, column: &QString, to: i32) -> QString;
        #[qinvokable]
        #[cxx_name = "columnWidths"]
        fn column_widths(self: &ModsData, kept: &QString) -> QVariant;
        #[qinvokable]
        #[cxx_name = "columnSized"]
        fn column_sized(self: &ModsData, kept: &QString, column: &QString, width: f64) -> QString;
        /// `[{ column, text, mark, shown, off }]`: author, version and load.
        #[qinvokable]
        #[cxx_name = "columnChoices"]
        fn column_choices(self: &ModsData, kept: &QString, load_times: bool) -> QVariant;
        /// Why the load column is not offered; empty while load times are known.
        #[qinvokable]
        #[cxx_name = "columnNote"]
        fn column_note(self: &ModsData, load_times: bool) -> QString;
        /// `[column, descending]`; the empty column is the load order.
        #[qinvokable]
        #[cxx_name = "columnPressed"]
        fn column_pressed(
            self: &ModsData,
            by: &QString,
            descending: bool,
            asked: &QString,
        ) -> QVariant;
        #[qinvokable]
        #[cxx_name = "markReady"]
        fn mark_ready(self: &ModsData);
        /// The rows say the size their picture box is, in pixels; a change makes copies again.
        #[qinvokable]
        #[cxx_name = "pictureSized"]
        fn picture_sized(self: Pin<&mut ModsData>, wide: i32, high: i32);
    }
}

mod asks;
mod commands;
mod drafts;

use crate::App;
use crate::changes::Changes;
use crate::qml::values::to_qml;
use crate::workspace::load_order::{LoadOrder, Side};
use core::pin::Pin;
use cxx::UniquePtr;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use qobject::ModRows;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::{Arrange, Arrangement};
use std::sync::Arc;

pub struct ModsDataRust {
    active: *mut ModRows,
    inactive: *mut ModRows,
    owned_active: UniquePtr<ModRows>,
    owned_inactive: UniquePtr<ModRows>,
    loading: crate::jobs::Lane,
    counting: crate::jobs::Lane,
    texturing: crate::jobs::Lane,
    picturing: crate::jobs::Lane,
    /// The size the rows show the copies at; (0, 0) until the first is made.
    picture_size: (u32, u32),
    textures_of: i32,
    dirty: bool,
    unsaved: QString,
    left_save: QString,
    can_undo: bool,
    can_redo: bool,
    previewing: bool,
    sort_note: QString,
    sort_more: QString,
    sort_pending: i32,
    sort_move_count: i32,
    errors: i32,
    warnings: i32,
    troubled: i32,
    conflicts: i32,
    files_read: bool,
    rescanning: bool,
    notes_saved: bool,
    drafts: drafts::Drafts,
    problem: QString,
    status_active: QString,
    status_inactive: QString,
    library_version: i32,
    overview: QVariant,
    summary: QVariant,
    selected: i32,
    details: QVariant,
    textures: QVariant,
    fresh: QVariant,
    headings: bool,
    inactive_order: QString,
    inactive_reversed: bool,
    inactive_orders: QVariant,
    filter: QString,
    save_problem: QString,
    saving: bool,
    file_changed: bool,
    upkeep_problem: QString,
    app: crate::hub::Given,
    order: LoadOrder,
    demo: bool,
    records: bool,
}

impl Default for ModsDataRust {
    fn default() -> Self {
        Self {
            active: std::ptr::null_mut(),
            inactive: std::ptr::null_mut(),
            owned_active: UniquePtr::null(),
            owned_inactive: UniquePtr::null(),
            loading: crate::jobs::Lane::default(),
            counting: crate::jobs::Lane::default(),
            texturing: crate::jobs::Lane::default(),
            picturing: crate::jobs::Lane::default(),
            picture_size: (0, 0),
            textures_of: -1,
            dirty: false,
            unsaved: QString::default(),
            left_save: QString::default(),
            can_undo: false,
            can_redo: false,
            previewing: false,
            sort_note: QString::default(),
            sort_more: QString::default(),
            sort_pending: 0,
            sort_move_count: 0,
            errors: 0,
            warnings: 0,
            troubled: 0,
            conflicts: 0,
            files_read: false,
            rescanning: false,
            notes_saved: false,
            drafts: drafts::Drafts::default(),
            problem: QString::default(),
            status_active: QString::default(),
            status_inactive: QString::default(),
            library_version: 0,
            overview: QVariant::default(),
            summary: QVariant::default(),
            selected: -1,
            details: QVariant::default(),
            textures: QVariant::default(),
            fresh: QVariant::default(),
            headings: true,
            inactive_order: QString::default(),
            inactive_reversed: false,
            inactive_orders: QVariant::default(),
            filter: QString::default(),
            save_problem: QString::default(),
            saving: false,
            file_changed: false,
            upkeep_problem: QString::default(),
            app: crate::hub::Given::default(),
            order: LoadOrder::demo(0, 0),
            demo: false,
            records: true,
        }
    }
}

/// `None` for -1 and below.
fn id(held: i32) -> Option<ModId> {
    ModId::from_qml(held)
}

fn ids(given: &QVariant) -> Option<Vec<ModId>> {
    crate::qml::values::mod_ids(given)
}

impl qobject::ModsData {
    fn app(&self) -> Arc<App> {
        self.app.app()
    }

    fn arrangement(&self) -> Arrangement {
        Arrangement {
            by: Arrange::from_name(&self.inactive_order.to_string()),
            reversed: self.inactive_reversed,
        }
    }

    fn rearrange_inactive(mut self: Pin<&mut Self>) {
        let (app, how) = (self.app(), self.arrangement());
        self.as_mut()
            .rust_mut()
            .get_mut()
            .order
            .set_inactive_order(&app, how);
        self.as_mut().show_orders();
        self.show();
    }

    fn rows(self: Pin<&mut Self>, side: Side) -> Option<Pin<&mut ModRows>> {
        let me = self.rust_mut().get_mut();
        match side {
            Side::Active => me.owned_active.as_mut(),
            Side::Inactive => me.owned_inactive.as_mut(),
        }
    }

    pub(crate) fn connect(mut self: Pin<&mut Self>, app: Arc<App>, demo: bool, records: bool) {
        let mut active = crate::mod_rows::qobject::make_mod_rows();
        let mut inactive = crate::mod_rows::qobject::make_mod_rows();
        if let Some(rows) = active.as_mut() {
            rows.set_side(true);
        }
        if let Some(rows) = inactive.as_mut() {
            rows.set_side(false);
        }
        {
            let me = self.as_mut().rust_mut().get_mut();
            me.active = crate::hub::address(&mut active);
            me.inactive = crate::hub::address(&mut inactive);
            me.owned_active = active;
            me.owned_inactive = inactive;
            me.app = crate::hub::Given::of(app.clone());
            me.demo = demo;
            me.records = records && !demo;
        }
        crate::hub::hear(&app, self.qt_thread(), Self::heard);
        self.as_mut()
            .on_selected_changed(|me| me.describe())
            .release();
        self.as_mut()
            .on_headings_changed(|mut me| {
                let (app, headings) = (me.app(), me.headings);
                me.as_mut()
                    .rust_mut()
                    .get_mut()
                    .order
                    .set_headings(&app, headings);
                me.show();
            })
            .release();
        self.as_mut()
            .on_inactive_order_changed(|me| me.rearrange_inactive())
            .release();
        self.as_mut()
            .on_inactive_reversed_changed(|me| me.rearrange_inactive())
            .release();
        self.as_mut()
            .on_unsaved_changed(|me| me.count_sort_moves())
            .release();
        self.as_mut()
            .on_files_read_changed(|me| me.count_sort_moves())
            .release();
        self.as_mut()
            .on_previewing_changed(|me| me.count_sort_moves())
            .release();
        self.as_mut()
            .on_filter_changed(|mut me| {
                let text = me.filter.to_string();
                me.as_mut().rust_mut().get_mut().order.set_filter(&text);
                me.show();
            })
            .release();
        self.as_mut().show_orders();
        if demo {
            self.as_mut().rust_mut().get_mut().order = LoadOrder::demo(2000, 300);
            self.show();
        } else {
            self.as_mut().set_summary(to_qml(&app.summary()));
            self.load(false);
        }
    }

    /// `anew`: ids held before mean nothing.
    fn load(mut self: Pin<&mut Self>, anew: bool) {
        let app = self.app();
        let (headings, by) = (self.headings, self.arrangement());
        self.as_mut().set_rescanning(true);
        self.loading.run(
            "load order",
            self.qt_thread(),
            move |_| LoadOrder::read(&app, headings, by),
            move |mut me, mut order| {
                // The window's prefs reach `me` while the list is still read, at the start;
                // the order read with the defaults must not undo them.
                let app = me.app();
                if me.headings != headings {
                    order.set_headings(&app, me.headings);
                }
                if me.arrangement() != by {
                    order.set_inactive_order(&app, me.arrangement());
                }
                order.set_filter(&me.filter.to_string());
                me.as_mut().rust_mut().get_mut().order = order;
                me.as_mut().set_rescanning(false);
                if anew {
                    let version = me.library_version + 1;
                    me.as_mut().set_library_version(version);
                    me.as_mut().set_selected(-1);
                }
                me.as_mut().show_orders();
                me.as_mut().show();
                me.as_mut().read_files();
                me.as_mut().make_pictures();
                if let Some(started) = crate::STARTED.get() {
                    rimstack_core::note!("mods_shown_ms={}", started.elapsed().as_millis());
                }
            },
        );
    }

    fn show_orders(self: Pin<&mut Self>) {
        let orders = to_qml(&self.order.inactive_orders());
        self.set_inactive_orders(orders);
    }

    fn read_files(mut self: Pin<&mut Self>) {
        let app = self.app();
        let Some(members) = self.order.files_to_read(&app) else {
            let installed = self.order.is_installed();
            self.set_files_read(installed);
            return;
        };
        self.as_mut().set_files_read(false);
        crate::jobs::run(
            "active files",
            self.qt_thread(),
            move || app.read_active_files(members),
            |_, ()| {},
        );
    }

    fn heard(mut self: Pin<&mut Self>, changes: Changes) {
        if self.demo {
            return;
        }
        let app = self.app();
        let counted = Changes::LIBRARY
            | Changes::LISTS
            | Changes::LAUNCHES
            | Changes::SAVES
            | Changes::RULES
            | Changes::UPDATES;
        if changes.contains(counted) {
            self.as_mut().set_summary(to_qml(&app.summary()));
        }
        if changes.contains(Changes::LIBRARY | Changes::LANGUAGE) {
            return self.load(changes.contains(Changes::LIBRARY));
        }
        if changes.contains(Changes::MOD_FILES) {
            self.as_mut().rescan();
        }
        if changes.contains(Changes::FOLDERS) {
            self.as_mut().set_rescanning(true);
            crate::jobs::run(
                "folders",
                self.qt_thread(),
                move || app.reread_folders(),
                |_, ()| {},
            );
            return;
        }
        let order = &mut self.as_mut().rust_mut().get_mut().order;
        if changes.contains(Changes::NOTES) {
            order.sync(&app);
        }
        if changes.contains(Changes::PROBLEMS | Changes::COMMUNITY | Changes::FILES_READ) {
            order.recheck(&app);
        }
        if changes.contains(Changes::COMMUNITY | Changes::FILES_READ) {
            order.refile(&app);
        }
        if changes.contains(Changes::UPDATES | Changes::LAUNCHES) {
            order.settle(&app);
        }
        if changes.contains(Changes::FILES_READ) {
            self.as_mut().set_files_read(true);
        }
        if changes.contains(Changes::TEXTURES) {
            self.as_mut().read_files();
        }
        self.show();
    }

    /// Known after the first row is made; copies wait for the size, which never wavers
    /// between rows while the box does.
    fn picture_sized(mut self: Pin<&mut Self>, wide: i32, high: i32) {
        let size = (wide.max(0) as u32, high.max(0) as u32);
        if size != self.rust().picture_size && size.0 > 0 && size.1 > 0 {
            self.as_mut().rust_mut().get_mut().picture_size = size;
            self.as_mut().make_pictures();
        }
    }

    /// Fills the pack with what it is missing, off the window's thread, then shows the rows
    /// again so they take their copies: until then, each is read the way it always was.
    fn make_pictures(self: Pin<&mut Self>) {
        if self.demo {
            return;
        }
        let (app, (wide, high)) = (self.app(), self.rust().picture_size);
        if (wide, high) == (0, 0) {
            return;
        }
        self.picturing.run(
            "preview copies",
            self.qt_thread(),
            move |_| {
                let wanted = crate::workspace::preview_pack::wanted(&app);
                crate::workspace::preview_pack::renew(&app, &wanted, wide, high)
            },
            |mut me, changed| {
                if changed.is_some() {
                    me.as_mut().show();
                }
            },
        );
    }

    fn show(mut self: Pin<&mut Self>) {
        let app = self.app();
        for side in [Side::Active, Side::Inactive] {
            let list = self.order.list(side);
            let (count, matching) = (list.mods().len(), list.matching());
            let end = list.end_place();
            let beyond = list.beyond_filter();
            let rows = self.order.rows(&app, side);
            if let Some(model) = self.as_mut().rows(side) {
                model.show(rows, count, matching, end, beyond);
            }
        }
        let (dirty, unsaved, undo, redo) = {
            let session = app.session();
            (
                session.is_dirty(),
                session.changes().said(),
                session.can_undo(),
                session.can_redo(),
            )
        };
        let installed = self.order.is_installed();
        let left_save = {
            match &*app.library() {
                Ok((library, _)) => crate::words::said(&self.order.left_save(&app, library)),
                Err(_) => QString::default(),
            }
        };
        self.as_mut().set_dirty(dirty && installed);
        self.as_mut().set_unsaved(crate::words::said(&unsaved));
        self.as_mut().set_left_save(left_save);
        self.as_mut().set_can_undo(undo);
        self.as_mut().set_can_redo(redo);
        let order = &self.order;
        let (errors, warnings) = order.errors_and_warnings();
        let troubled = order.troubles().iter().filter(|t| !t.is_empty()).count();
        let previewing = order.previewing();
        let (note, more) = match order.sort_note.split_first() {
            Some((first, rest)) => (
                crate::words::said(first),
                crate::words::said(&Sentence::joined(" ", rest.to_vec())),
            ),
            None => Default::default(),
        };
        let moves = order.sort_move_count() as i32;
        let conflicts = order.in_conflict(&app).len() as i32;
        let problem = crate::words::said(&order.problem);
        let status = order.status.clone();
        let overview = to_qml(&order.overview(&app));
        let fresh = to_qml(&order.fresh(&app, Side::Inactive));
        self.as_mut().set_errors(errors as i32);
        self.as_mut().set_warnings(warnings as i32);
        self.as_mut().set_troubled(troubled as i32);
        self.as_mut().set_previewing(previewing);
        self.as_mut().set_sort_note(note);
        self.as_mut().set_sort_more(more);
        self.as_mut().set_sort_move_count(moves);
        self.as_mut().set_conflicts(conflicts);
        self.as_mut().set_problem(problem);
        self.as_mut()
            .set_status_active(crate::words::said(&status[0]));
        self.as_mut()
            .set_status_inactive(crate::words::said(&status[1]));
        self.as_mut().set_overview(overview);
        self.as_mut().set_fresh(fresh);
        self.describe();
    }

    fn describe(mut self: Pin<&mut Self>) {
        let app = self.app();
        let selected = id(self.selected).filter(|_| self.order.is_installed());
        let details = match selected {
            Some(id) => to_qml(&self.as_mut().rust_mut().get_mut().order.details(&app, id)),
            None => QVariant::default(),
        };
        self.as_mut().set_details(details);
        self.count_textures(app, selected);
    }

    /// Off the window's thread.
    fn count_textures(mut self: Pin<&mut Self>, app: Arc<App>, selected: Option<ModId>) {
        if self.textures_of != self.selected {
            let selected = self.selected;
            self.as_mut().rust_mut().get_mut().textures_of = selected;
            self.as_mut().set_textures(QVariant::default());
        }
        let Some(id) = selected else {
            self.texturing.cancel();
            return;
        };
        self.texturing.run(
            "textures of a mod",
            self.qt_thread(),
            move |_| LoadOrder::textures(&app, id),
            |me, textures| me.set_textures(to_qml(&textures)),
        );
    }

    fn changed(self: Pin<&mut Self>) {
        let app = self.app();
        self.show();
        app.publish(Changes::LOAD_ORDER);
    }
}
