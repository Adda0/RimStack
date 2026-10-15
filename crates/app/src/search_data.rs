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
        #[qproperty(bool, active)]
        #[qproperty(bool, asking)]
        #[qproperty(QString, found)]
        #[qproperty(QString, found_id, cxx_name = "foundId")]
        #[qproperty(QString, stuck)]
        #[qproperty(i32, rounds)]
        #[qproperty(i32, suspects)]
        #[qproperty(QVariant, list)]
        #[qproperty(i32, everything)]
        type SearchData = super::SearchDataRust;
    }

    extern "RustQt" {
        #[qinvokable]
        fn start(self: Pin<&mut SearchData>);

        #[qinvokable]
        fn answer(self: Pin<&mut SearchData>, broken: bool);

        /// Less the mod found if `without_found`.
        #[qinvokable]
        fn end(self: Pin<&mut SearchData>, without_found: bool) -> QVariant;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[cxx_name = "make_unique"]
        #[namespace = "rust::cxxqtlib1"]
        fn make_search_data() -> UniquePtr<SearchData>;
    }
}

use crate::qml::values::to_qml;
use crate::workspace::search::Hunt;
use crate::App;
use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QVariant};
use std::sync::Arc;

#[derive(Default)]
pub struct SearchDataRust {
    active: bool,
    asking: bool,
    found: QString,
    found_id: QString,
    stuck: QString,
    rounds: i32,
    suspects: i32,
    list: QVariant,
    everything: i32,
    app: crate::hub::Given,
    records: bool,
}

fn int(n: impl TryInto<i32>) -> i32 {
    n.try_into().unwrap_or(i32::MAX)
}

impl qobject::SearchData {
    fn app(&self) -> Arc<App> {
        self.app.app()
    }

    pub(crate) fn connect(mut self: Pin<&mut Self>, app: Arc<App>, demo: bool, records: bool) {
        let hunt = match demo {
            true => Hunt::default(),
            false => app.hunt(),
        };
        let me = self.as_mut().rust_mut().get_mut();
        me.records = records && !demo;
        me.app = crate::hub::Given::of(app);
        self.show(hunt);
    }

    fn show(mut self: Pin<&mut Self>, hunt: Hunt) {
        self.as_mut().set_found(QString::from(&hunt.found));
        self.as_mut().set_found_id(QString::from(&hunt.found_id));
        self.as_mut()
            .set_stuck(QString::from(&hunt.stuck.join(", ")));
        self.as_mut().set_rounds(int(hunt.rounds));
        self.as_mut().set_suspects(int(hunt.suspects));
        self.as_mut().set_list(to_qml(&hunt.list));
        self.as_mut().set_everything(int(hunt.everything));
        self.as_mut().set_asking(hunt.asking);
        self.set_active(hunt.active);
    }

    fn start(self: Pin<&mut Self>) {
        let hunt = self.app().start_hunt(self.records);
        self.show(hunt);
    }

    fn answer(self: Pin<&mut Self>, broken: bool) {
        let hunt = self.app().answer_hunt(broken);
        self.show(hunt);
    }

    fn end(self: Pin<&mut Self>, without_found: bool) -> QVariant {
        let ids = self.app().end_hunt(without_found);
        self.show(Hunt::default());
        to_qml(&ids)
    }
}
