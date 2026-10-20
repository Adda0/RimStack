#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        #[qobject]
        type QAbstractListModel;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
    }

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qml_uncreatable]
        #[qproperty(bool, active, READ, CONSTANT)]
        #[qproperty(i32, count)]
        #[qproperty(i32, matching)]
        /// Where a mod dropped below the last row goes, in the whole list.
        #[qproperty(i32, end_place, cxx_name = "endPlace")]
        /// The filter only applies to the other side; everything here shows, unfiltered.
        #[qproperty(bool, beyond_filter, cxx_name = "beyondFilter")]
        type ModRows = super::ModRowsRust;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[cxx_name = "make_unique"]
        #[namespace = "rust::cxxqtlib1"]
        fn make_mod_rows() -> UniquePtr<ModRows>;
    }

    extern "RustQt" {
        #[qinvokable]
        #[cxx_override]
        fn data(self: &ModRows, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &ModRows, parent: &QModelIndex) -> i32;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &ModRows) -> QHash_i32_QByteArray;

        /// `{}` past the end.
        #[qinvokable]
        fn at(self: &ModRows, row: i32) -> QVariant;

        #[qinvokable]
        fn rows(self: &ModRows) -> i32;

        /// -1 when filtered out or under a folded heading.
        #[qinvokable]
        #[cxx_name = "rowOf"]
        fn row_of(self: &ModRows, position: i32) -> i32;

        /// -1 when it is not in it.
        #[qinvokable]
        #[cxx_name = "indexOf"]
        fn index_of(self: &ModRows, mod_id: i32) -> i32;

        /// -1 for a heading or none.
        #[qinvokable]
        #[cxx_name = "modIdAt"]
        fn mod_id_at(self: &ModRows, row: i32) -> i32;
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut ModRows>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );

        #[inherit]
        fn index(self: &ModRows, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
    }

    extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut ModRows>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut ModRows>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows(
            self: Pin<&mut ModRows>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows(self: Pin<&mut ModRows>);
        #[inherit]
        #[cxx_name = "beginMoveRows"]
        unsafe fn begin_move_rows(
            self: Pin<&mut ModRows>,
            source_parent: &QModelIndex,
            source_first: i32,
            source_last: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;
        #[inherit]
        #[cxx_name = "endMoveRows"]
        unsafe fn end_move_rows(self: Pin<&mut ModRows>);
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut ModRows>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut ModRows>);
    }
}

use crate::qml::diff::{Step, steps};
use crate::qml::values::to_qml;
use crate::workspace::load_order::RowView;
use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant, QVector,
};
use rimstack_core::base::sentence::Sentence;
use serde_json::{Value, json};

#[derive(Default)]
pub struct ModRowsRust {
    active: bool,
    count: i32,
    matching: i32,
    end_place: i32,
    /// The filter only applies to the other side; everything here shows, unfiltered.
    beyond_filter: bool,
    shown: Vec<RowView>,
}

/// From the first role Qt leaves free.
const ROLES: [&str; 32] = [
    "isHeader",
    "isSub",
    "again",
    "name",
    "author",
    "version",
    "number",
    "count",
    "collapsed",
    "category",
    "modIndex",
    "modId",
    "movedFrom",
    "trouble",
    "troubleText",
    "tags",
    "local",
    "picture",
    "preview",
    "loadTime",
    "loadPart",
    "versionIgnored",
    "fresh",
    "source",
    "whyMoved",
    "groupSize",
    "twin",
    "outdated",
    "madeFor",
    "measure",
    "colour",
    "game",
];
const FIRST_ROLE: i32 = 257;

enum Cell<'a> {
    Flag(bool),
    Number(i32),
    Part(f64),
    Text(&'a str),
    Said(&'a Sentence),
}

pub(crate) fn fields_of(row: &RowView) -> serde_json::Map<String, Value> {
    ROLES
        .iter()
        .enumerate()
        .filter_map(|(at, name)| Some((name.to_string(), cell(row, at)?.value())))
        .collect()
}

fn cell(row: &RowView, field: usize) -> Option<Cell<'_>> {
    use Cell::*;
    Some(match field {
        0 => Flag(row.is_header),
        1 => Flag(row.is_sub),
        2 => Flag(row.again),
        3 => Said(&row.name),
        4 => Text(&row.author),
        5 => Text(&row.version),
        6 => Number(row.number),
        7 => Number(row.count),
        8 => Flag(row.collapsed),
        9 => Number(row.category),
        10 => Number(row.mod_index),
        11 => Number(row.mod_id),
        12 => Number(row.moved_from),
        13 => Number(row.trouble),
        14 => Said(&row.trouble_text),
        15 => Text(&row.tags),
        16 => Flag(row.local),
        17 => Text(&row.picture),
        18 => Text(&row.preview),
        19 => Text(&row.load_time),
        20 => Part(row.load_part),
        21 => Flag(row.version_ignored),
        22 => Flag(row.fresh),
        23 => Number(row.source),
        24 => Said(&row.why_moved),
        25 => Number(row.group_size),
        26 => Flag(row.twin),
        27 => Flag(row.outdated),
        28 => Text(&row.made_for),
        29 => Said(&row.measure),
        30 => Text(&row.colour),
        31 => Text(&row.game),
        _ => return None,
    })
}

impl Cell<'_> {
    fn variant(&self) -> QVariant {
        match *self {
            Cell::Flag(b) => QVariant::from(&b),
            Cell::Number(n) => QVariant::from(&n),
            Cell::Part(p) => QVariant::from(&p),
            Cell::Text(t) => QVariant::from(&QString::from(t)),
            Cell::Said(s) => QVariant::from(&crate::words::said(s)),
        }
    }

    fn value(&self) -> Value {
        match *self {
            Cell::Flag(b) => json!(b),
            Cell::Number(n) => json!(n),
            Cell::Part(p) => json!(p),
            Cell::Text(t) => json!(t),
            Cell::Said(s) => json!(crate::words::put(s)),
        }
    }
}

fn held(found: Option<usize>) -> i32 {
    found.map_or(-1, |at| at as i32)
}

impl qobject::ModRows {
    pub(crate) fn set_side(mut self: Pin<&mut Self>, active: bool) {
        self.as_mut().rust_mut().active = active;
    }

    fn row(&self, row: i32) -> Option<&RowView> {
        self.shown.get(usize::try_from(row).ok()?)
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let field = usize::try_from(role - FIRST_ROLE).ok();
        let found = self
            .row(index.row())
            .zip(field)
            .and_then(|(row, f)| cell(row, f));
        found.map_or_else(QVariant::default, |found| found.variant())
    }

    fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.shown.len() as i32
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut names = QHash::<QHashPair_i32_QByteArray>::default();
        for (at, name) in ROLES.iter().enumerate() {
            names.insert(FIRST_ROLE + at as i32, QByteArray::from(*name));
        }
        names
    }

    fn at(&self, row: i32) -> QVariant {
        let Some(row) = self.row(row) else {
            return to_qml(&json!({}));
        };
        to_qml(&fields_of(row))
    }

    fn rows(&self) -> i32 {
        self.shown.len() as i32
    }

    fn row_of(&self, position: i32) -> i32 {
        held(
            self.shown
                .iter()
                .position(|r| !r.is_header && r.mod_index == position),
        )
    }

    fn index_of(&self, mod_id: i32) -> i32 {
        let found = self
            .shown
            .iter()
            .find(|r| mod_id >= 0 && r.mod_id == mod_id);
        found.map_or(-1, |r| r.mod_index)
    }

    fn mod_id_at(&self, row: i32) -> i32 {
        self.row(row).map_or(-1, |r| r.mod_id)
    }

    /// Told before a change is made, or the list resets past too many steps.
    pub(crate) fn show(
        mut self: Pin<&mut Self>,
        rows: Vec<RowView>,
        count: usize,
        matching: usize,
        end_place: usize,
        beyond_filter: bool,
    ) {
        let before: Vec<_> = self.shown.iter().map(|r| r.key.clone()).collect();
        let after: Vec<_> = rows.iter().map(|r| r.key.clone()).collect();
        match steps(&before, &after) {
            Some(steps) => {
                self.as_mut().step(&steps, &rows);
                self.as_mut().changed_rows(rows);
            }
            None => self.as_mut().renew(rows),
        }
        self.as_mut().set_count(count as i32);
        self.as_mut().set_matching(matching as i32);
        self.as_mut().set_end_place(end_place as i32);
        self.as_mut().set_beyond_filter(beyond_filter);
    }

    fn renew(mut self: Pin<&mut Self>, rows: Vec<RowView>) {
        // SAFETY: the reset ends below, after the rows changed.
        unsafe { self.as_mut().begin_reset_model() };
        self.as_mut().rust_mut().shown = rows;
        // SAFETY: ends the reset begun above.
        unsafe { self.as_mut().end_reset_model() };
    }

    fn step(mut self: Pin<&mut Self>, steps: &[Step], rows: &[RowView]) {
        let root = QModelIndex::default();
        for step in steps {
            // SAFETY: each change is announced before `shown` changes and
            // ended right after, with the rows it names.
            unsafe {
                match *step {
                    Step::Remove { first, last } => {
                        self.as_mut()
                            .begin_remove_rows(&root, first as i32, last as i32);
                        self.as_mut().rust_mut().shown.drain(first..=last);
                        self.as_mut().end_remove_rows();
                    }
                    Step::Insert { first, last } => {
                        self.as_mut()
                            .begin_insert_rows(&root, first as i32, last as i32);
                        let coming = rows[first..=last].iter().cloned();
                        self.as_mut().rust_mut().shown.splice(first..first, coming);
                        self.as_mut().end_insert_rows();
                    }
                    Step::Move { from, to } => {
                        // Qt names the place as the row it goes before, counted before the move.
                        let before = if to > from { to + 1 } else { to };
                        let (from, before) = (from as i32, before as i32);
                        self.as_mut()
                            .begin_move_rows(&root, from, from, &root, before);
                        let row = self.as_mut().rust_mut().shown.remove(from as usize);
                        self.as_mut().rust_mut().shown.insert(to, row);
                        self.as_mut().end_move_rows();
                    }
                }
            }
        }
    }

    fn changed_rows(mut self: Pin<&mut Self>, rows: Vec<RowView>) {
        let changed: Vec<usize> = (0..rows.len())
            .filter(|&at| self.shown.get(at) != Some(&rows[at]))
            .collect();
        self.as_mut().rust_mut().shown = rows;
        if changed.is_empty() {
            return;
        }
        let (first, last) = (changed[0] as i32, changed[changed.len() - 1] as i32);
        let root = QModelIndex::default();
        let (top, bottom) = (self.index(first, 0, &root), self.index(last, 0, &root));
        self.data_changed(&top, &bottom, &QVector::default());
    }
}
