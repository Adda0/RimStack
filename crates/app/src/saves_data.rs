#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_uncreatable]
        #[qproperty(QVariant, saves)]
        #[qproperty(QVariant, folders)]
        #[qproperty(QString, current_folder, cxx_name = "currentFolder")]
        #[qproperty(bool, rimsaves)]
        #[qproperty(QString, heading)]
        /// Empty when none does.
        #[qproperty(QString, all_from, cxx_name = "allFrom")]
        #[qproperty(bool, pictured)]
        #[qproperty(bool, busy)]
        /// Empty for every one.
        #[qproperty(QString, folder)]
        /// Keeps the saves whose name has it.
        #[qproperty(QString, filter)]
        /// True when the folder holds enough saves to filter.
        #[qproperty(bool, searchable)]
        #[qproperty(bool, wanted)]
        type SavesData = super::SavesDataRust;
    }

    impl cxx_qt::Threading for SavesData {}

    extern "RustQt" {
        #[qinvokable]
        fn reread(self: Pin<&mut SavesData>);

        #[qinvokable]
        #[cxx_name = "backUp"]
        fn back_up(self: &SavesData);
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[cxx_name = "make_unique"]
        #[namespace = "rust::cxxqtlib1"]
        fn make_saves_data() -> UniquePtr<SavesData>;
    }
}

use crate::changes::Changes;
use crate::qml::values::to_qml;
use crate::workspace::saves::{shown, SavesReport};
use crate::App;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use std::sync::Arc;

#[derive(Default)]
pub struct SavesDataRust {
    saves: QVariant,
    folders: QVariant,
    current_folder: QString,
    rimsaves: bool,
    heading: QString,
    all_from: QString,
    pictured: bool,
    busy: bool,
    folder: QString,
    filter: QString,
    searchable: bool,
    wanted: bool,
    app: crate::hub::Given,
    report: SavesReport,
    stale: bool,
    reading: crate::jobs::Lane,
}

impl qobject::SavesData {
    fn app(&self) -> Arc<App> {
        self.app.app()
    }

    pub(crate) fn connect(mut self: Pin<&mut Self>, app: Arc<App>) {
        crate::hub::hear(&app, self.qt_thread(), Self::heard);
        let me = self.as_mut().rust_mut().get_mut();
        me.app = crate::hub::Given::of(app);
        me.stale = true;
        self.as_mut().on_wanted_changed(|me| me.read()).release();
        self.as_mut().on_folder_changed(|me| me.show()).release();
        self.as_mut().on_filter_changed(|me| me.show()).release();
    }

    fn reread(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().get_mut().stale = true;
        self.read();
    }

    fn back_up(&self) {
        let app = self.app();
        crate::jobs::spawn("save backup", move || {
            if let Err(problem) = app.back_up_newest_save("save-backups", 5) {
                rimstack_core::problem!("save not backed up: {problem}");
            }
        });
    }

    fn heard(mut self: Pin<&mut Self>, changes: Changes) {
        if changes.contains(Changes::LIBRARY | Changes::LOAD_ORDER | Changes::SAVES) {
            self.as_mut().rust_mut().get_mut().stale = true;
            self.read();
        } else if changes.contains(Changes::LANGUAGE) {
            self.show();
        }
    }

    fn read(mut self: Pin<&mut Self>) {
        if !self.wanted || !self.stale {
            return;
        }
        self.as_mut().rust_mut().get_mut().stale = false;
        self.as_mut().set_busy(true);
        let app = self.app();
        self.reading.run(
            "saves",
            self.qt_thread(),
            move |_| app.saves_report(),
            |mut me, report| {
                me.as_mut().rust_mut().get_mut().report = report;
                me.as_mut().set_busy(false);
                me.show();
            },
        );
    }

    fn show(mut self: Pin<&mut Self>) {
        let report = &self.report;
        let on_show = shown(report, &self.folder.to_string(), &self.filter.to_string());
        let saves = to_qml(&on_show.saves);
        let heading = crate::words::said(&on_show.heading);
        let pictured = on_show.pictured;
        let searchable = on_show.searchable;
        let folders = to_qml(&report.folders);
        let current = QString::from(report.current_folder.as_deref().unwrap_or_default());
        let all_from = QString::from(report.all_from.as_deref().unwrap_or_default());
        let rimsaves = report.rimsaves;
        self.as_mut().set_saves(saves);
        self.as_mut().set_heading(heading);
        self.as_mut().set_all_from(all_from);
        self.as_mut().set_pictured(pictured);
        self.as_mut().set_searchable(searchable);
        self.as_mut().set_folders(folders);
        self.as_mut().set_current_folder(current);
        self.set_rimsaves(rimsaves);
    }
}
