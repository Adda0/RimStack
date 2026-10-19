//! The window's side of the search inside the mods' files: options in, hits streamed out.

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
        #[qproperty(QString, text)]
        #[qproperty(bool, regex_mode, cxx_name = "regexMode")]
        #[qproperty(bool, match_case, cxx_name = "matchCase")]
        #[qproperty(bool, xml_only, cxx_name = "xmlOnly")]
        #[qproperty(bool, all_mods, cxx_name = "allMods")]
        #[qproperty(bool, busy)]
        /// The progress while looking, the outcome after.
        #[qproperty(QString, progress)]
        #[qproperty(QString, summary)]
        /// `[{ modId, modName, folder, file, line, text }]`, streamed in batches.
        #[qproperty(QVariant, hits)]
        type FindData = super::FindDataRust;
    }

    impl cxx_qt::Threading for FindData {}

    extern "RustQt" {
        /// Remembers `text` and searches; empties the hits before.
        #[qinvokable]
        fn go(self: Pin<&mut FindData>, text: &QString);
        #[qinvokable]
        fn start(self: Pin<&mut FindData>);
        #[qinvokable]
        fn stop(self: &FindData);
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[cxx_name = "make_unique"]
        #[namespace = "rust::cxxqtlib1"]
        fn make_find_data() -> UniquePtr<FindData>;
    }
}

use crate::App;
use crate::qml::values::to_qml;
use crate::workspace::find::{self, Hit};
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct FindDataRust {
    text: QString,
    regex_mode: bool,
    match_case: bool,
    xml_only: bool,
    all_mods: bool,
    busy: bool,
    progress: QString,
    summary: QString,
    hits: QVariant,
    app: crate::hub::Given,
    all: Vec<Hit>,
    stop: Arc<AtomicBool>,
}

impl qobject::FindData {
    fn app(&self) -> Arc<App> {
        self.app.app()
    }

    pub(crate) fn connect(mut self: Pin<&mut Self>, app: Arc<App>) {
        let me = self.as_mut().rust_mut().get_mut();
        me.app = crate::hub::Given::of(app);
        me.xml_only = true;
        self.as_mut().set_hits(to_qml(&Vec::<Hit>::new()));
    }

    fn go(mut self: Pin<&mut Self>, text: &QString) {
        self.as_mut().set_text(text.clone());
        self.start();
    }

    fn start(mut self: Pin<&mut Self>) {
        if self.busy {
            return;
        }
        let text = self.text.to_string();
        let needle = find::Needle::make(&text, self.regex_mode, self.match_case);
        let needle = match (text.trim().is_empty(), needle) {
            (true, _) => {
                self.as_mut().set_summary(QString::default());
                return;
            }
            (false, Ok(needle)) => needle,
            (false, Err(why)) => {
                self.as_mut().set_summary(crate::words::said(&why));
                return;
            }
        };
        self.as_mut().rust_mut().get_mut().all = Vec::new();
        self.as_mut().set_hits(to_qml(&Vec::<Hit>::new()));
        self.as_mut().set_progress(QString::default());
        self.as_mut().set_summary(QString::default());
        self.as_mut().set_busy(true);
        self.stop.store(false, Ordering::Relaxed);
        let thread = self.qt_thread();
        let more = thread.clone();
        let roots = self.app().file_search_roots(self.all_mods);
        let (stop, xml_only) = (self.stop.clone(), self.xml_only);
        crate::jobs::run(
            "find in files",
            self.qt_thread(),
            move || {
                let batch = move |hits: &[Hit]| {
                    let hits = hits.to_vec();
                    let _ = thread.queue(move |me| me.took(hits));
                };
                let on_file = move |files: usize| {
                    let said = crate::words::said(&find::searching(files));
                    let _ = more.queue(move |me| me.set_progress(said));
                };
                find::files(&roots, &needle, xml_only, &stop, &batch, &on_file)
            },
            |mut me, found| {
                me.as_mut().set_progress(QString::default());
                me.as_mut()
                    .set_summary(crate::words::said(&find::said_of(&found)));
                me.as_mut().set_busy(false);
            },
        );
    }

    fn took(mut self: Pin<&mut Self>, batch: Vec<Hit>) {
        let me = self.as_mut().rust_mut().get_mut();
        me.all.extend(batch);
        let all = me.all.clone();
        self.set_hits(to_qml(&all));
    }

    fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
