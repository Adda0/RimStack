#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_uncreatable]
        /// `{ name, mods, modified, settings }`.
        #[qproperty(QVariant, saved)]
        /// `{ name, title }`.
        #[qproperty(QVariant, snapshots)]
        /// Newest first; `{ time, said, restore, bringBack }`.
        #[qproperty(QVariant, history)]
        type ListsData = super::ListsDataRust;
    }

    impl cxx_qt::Threading for ListsData {}

    extern "RustQt" {
        #[qinvokable]
        fn keep(self: &ListsData, name: &QString, settings: bool) -> QString;

        /// The list's kept mod settings take the current ones' place; empty text when done.
        #[qinvokable]
        #[cxx_name = "useListSettings"]
        fn use_list_settings(self: &ListsData, name: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "keepIds"]
        fn keep_ids(self: &ListsData, name: &QString, ids: &QVariant) -> QString;

        #[qinvokable]
        fn remove(self: &ListsData, name: &QString);

        /// `None` for no such list.
        #[qinvokable]
        fn named(self: &ListsData, name: &QString) -> QVariant;

        #[qinvokable]
        #[cxx_name = "snapshotNamed"]
        fn snapshot_named(self: &ListsData, name: &QString) -> QVariant;

        #[qinvokable]
        #[cxx_name = "idsInText"]
        fn ids_in_text(self: &ListsData, text: &QString) -> QVariant;

        /// The ids of a RimSort/RimPy XML, JSON or text list; `{ ids, problem }`, `problem` empty when read.
        #[qinvokable]
        #[cxx_name = "idsInFile"]
        fn ids_in_file(self: &ListsData, chosen: &QUrl) -> QVariant;

        #[qinvokable]
        fn original(self: &ListsData) -> QVariant;
        /// `{ onlySaved, onlyCurrent, moved: [{ id, name }], addable: [modId] }`;
        /// all empty for no such list.
        #[qinvokable]
        #[cxx_name = "compareList"]
        fn compare_list(self: &ListsData, name: &QString) -> QVariant;
        /// Writes the load order as a lockfile there; empty text when it was written.
        #[qinvokable]
        #[cxx_name = "writeLock"]
        fn write_lock(self: &ListsData, file: &QString) -> QString;
        /// The check of the load order against a lockfile, or the file's not being one:
        /// `{ same, present, ids, said }` or `{ problem }`.
        #[qinvokable]
        #[cxx_name = "checkLock"]
        fn check_lock(self: &ListsData, file: &QString) -> QVariant;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[cxx_name = "make_unique"]
        #[namespace = "rust::cxxqtlib1"]
        fn make_lists_data() -> UniquePtr<ListsData>;
    }
}

use crate::App;
use crate::changes::Changes;
use crate::qml::values::{from_qml, to_qml};
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl, QVariant};
use rimstack_core::base::error::Error;
use rimstack_core::say;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub struct ListsDataRust {
    saved: QVariant,
    snapshots: QVariant,
    history: QVariant,
    app: crate::hub::Given,
}

fn said(done: Result<(), Error>) -> QString {
    crate::words::why_not(&done)
}

impl qobject::ListsData {
    fn app(&self) -> Arc<App> {
        self.app.app()
    }

    pub(crate) fn connect(mut self: Pin<&mut Self>, app: Arc<App>, demo: bool) {
        self.as_mut().rust_mut().get_mut().app = crate::hub::Given::of(app.clone());
        if demo {
            return;
        }
        crate::hub::hear(&app, self.qt_thread(), Self::heard);
        self.heard(Changes::LISTS);
    }

    fn heard(mut self: Pin<&mut Self>, changes: Changes) {
        if !changes.contains(Changes::LISTS | Changes::LANGUAGE) {
            return;
        }
        let app = self.app();
        self.as_mut().set_saved(to_qml(&app.saved_lists()));
        self.as_mut().set_snapshots(to_qml(&app.recent_snapshots()));
        self.set_history(to_qml(&app.history()));
    }

    fn keep(&self, name: &QString, settings: bool) -> QString {
        said(self.app().keep_list(&name.to_string(), None, settings))
    }

    fn use_list_settings(&self, name: &QString) -> QString {
        crate::words::why_not(&self.app().use_list_settings(&name.to_string()))
    }

    fn keep_ids(&self, name: &QString, ids: &QVariant) -> QString {
        match from_qml::<Vec<String>>(ids) {
            Some(ids) => said(self.app().keep_list(&name.to_string(), Some(ids), false)),
            None => crate::words::said(&say!("That is not a list of mods.")),
        }
    }

    fn remove(&self, name: &QString) {
        self.app().delete_list(&name.to_string());
    }

    fn named(&self, name: &QString) -> QVariant {
        to_qml(&self.app().list_named(&name.to_string()))
    }

    fn snapshot_named(&self, name: &QString) -> QVariant {
        to_qml(&self.app().snapshot_named(&name.to_string()))
    }

    fn ids_in_text(&self, text: &QString) -> QVariant {
        to_qml(&rimstack_core::mods::share::ids_in(&text.to_string()))
    }

    fn ids_in_file(&self, chosen: &QUrl) -> QVariant {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct From {
            ids: Vec<String>,
            /// Empty when the file read as a list.
            problem: rimstack_core::base::sentence::Sentence,
        }
        let Some(path) = chosen.to_local_file() else {
            return to_qml(&From {
                ids: Vec::new(),
                problem: say!("That is not a file here."),
            });
        };
        let found =
            rimstack_core::mods::share_file::file(&std::path::PathBuf::from(path.to_string()));
        let answer = From {
            ids: found.as_ref().map_or_else(|_| Vec::new(), Clone::clone),
            problem: found
                .err()
                .map_or_else(rimstack_core::base::sentence::Sentence::default, |e| {
                    e.sentence().clone()
                }),
        };
        to_qml(&answer)
    }

    fn original(&self) -> QVariant {
        to_qml(&self.app().original_list())
    }
    fn compare_list(&self, name: &QString) -> QVariant {
        to_qml(&self.app().compare_saved(&name.to_string()))
    }
    fn write_lock(&self, file: &QString) -> QString {
        said(self.app().write_lockfile(&PathBuf::from(file.to_string())))
    }

    fn check_lock(&self, file: &QString) -> QVariant {
        let checked = self.app().check_lockfile(&PathBuf::from(file.to_string()));
        to_qml(&crate::reports::Answer::from(checked))
    }
}
