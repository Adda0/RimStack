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
        #[qproperty(bool, running, READ, NOTIFY)]
        /// `{ time, errors }`, or null.
        #[qproperty(QVariant, last_start, cxx_name = "lastStart")]
        #[qproperty(QVariant, reset_by_game, cxx_name = "resetByGame")]
        #[qproperty(bool, held)]
        #[qproperty(bool, watching)]
        #[qproperty(bool, starting)]
        type GameData = super::GameDataRust;
    }

    impl cxx_qt::Threading for GameData {}

    extern "RustQt" {
        #[qsignal]
        #[cxx_name = "writtenOutside"]
        fn written_outside(self: Pin<&mut GameData>, ids: QVariant);

        #[qsignal]
        #[cxx_name = "openSteam"]
        fn open_steam(self: Pin<&mut GameData>, address: QString);

        #[qsignal]
        #[cxx_name = "notStarted"]
        fn not_started(self: Pin<&mut GameData>, why: QString);

        #[qinvokable]
        fn play(self: Pin<&mut GameData>, round: bool);
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[cxx_name = "make_unique"]
        #[namespace = "rust::cxxqtlib1"]
        fn make_game_data() -> UniquePtr<GameData>;
    }
}

use crate::changes::Changes;
use crate::qml::values::to_qml;
use crate::App;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use rimstack_core::base::watch::{Stamps, Watcher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
pub struct GameDataRust {
    running: bool,
    last_start: QVariant,
    reset_by_game: QVariant,
    held: bool,
    watching: bool,
    starting: bool,
    app: crate::hub::Given,
    watcher: Watcher,
    /// Read by the thread that looks at the disk.
    looking: Arc<AtomicBool>,
    demo: bool,
}

impl qobject::GameData {
    fn app(&self) -> Arc<App> {
        self.app.app()
    }

    /// `None` never looks.
    pub(crate) fn connect(
        mut self: Pin<&mut Self>,
        app: Arc<App>,
        demo: bool,
        every: Option<Duration>,
    ) {
        crate::hub::hear(&app, self.qt_thread(), Self::heard);
        let me = self.as_mut().rust_mut().get_mut();
        me.app = crate::hub::Given::of(app.clone());
        me.demo = demo;
        let looking = me.looking.clone();
        self.as_mut()
            .on_watching_changed(|me| me.looking.store(me.watching, Ordering::Relaxed))
            .release();
        self.as_mut().heard(Changes::LAUNCHES);
        let Some(every) = every else {
            return;
        };
        let thread = self.qt_thread();
        let at_start = app.clone();
        crate::jobs::run(
            "emptied list",
            self.qt_thread(),
            move || at_start.list_reset_by_game(),
            |me, ids| {
                if let Some(ids) = ids {
                    me.set_reset_by_game(to_qml(&ids));
                }
            },
        );
        crate::jobs::spawn("watch", move || loop {
            std::thread::sleep(every);
            let running = app.game_running();
            let stamps = looking.load(Ordering::Relaxed).then(|| app.stamps());
            if thread.queue(move |me| me.looked(running, stamps)).is_err() {
                break;
            }
        });
    }

    fn heard(mut self: Pin<&mut Self>, changes: Changes) {
        if changes.contains(Changes::LAUNCHES | Changes::LOGS | Changes::FOLDERS) && !self.demo {
            let last = to_qml(&self.app().last_start());
            self.as_mut().set_last_start(last);
        }
    }

    fn looked(mut self: Pin<&mut Self>, running: bool, stamps: Option<Stamps>) {
        let app = self.app();
        if running != self.running {
            self.as_mut().rust_mut().get_mut().running = running;
            self.as_mut().running_changed();
            if running {
                let app = app.clone();
                crate::jobs::spawn("versions at start", move || app.record_versions());
            }
        }
        let Some(stamps) = stamps else {
            return;
        };
        let held = self.held;
        let noticed = self
            .as_mut()
            .rust_mut()
            .get_mut()
            .watcher
            .look(stamps, running, held);
        let mut changes = Changes::NONE;
        for (seen, change) in [
            (noticed.mods, Changes::MOD_FILES),
            (noticed.saves, Changes::SAVES),
            (noticed.log, Changes::LOGS),
            (noticed.theme, Changes::THEME),
            (noticed.language, Changes::LANGUAGE),
        ] {
            if seen {
                changes |= change;
            }
        }
        if !changes.is_empty() {
            app.publish(changes);
        }
        // Never taken unasked: the game empties the file when it first
        // starts on a new version.
        if noticed.mods_config {
            if let Some(ids) = app.mods_config_list() {
                self.written_outside(to_qml(&ids));
            }
        }
    }

    fn play(mut self: Pin<&mut Self>, round: bool) {
        if self.starting {
            return;
        }
        self.as_mut().set_starting(true);
        let app = self.app();
        let record = !self.demo && !round;
        crate::jobs::run(
            "game start",
            self.qt_thread(),
            move || app.launch(record),
            move |mut me, started| {
                me.as_mut().set_starting(false);
                match started {
                    Ok(address) => {
                        if let Some(address) = address {
                            me.as_mut().open_steam(QString::from(address));
                        }
                        if record {
                            me.app().publish(Changes::LAUNCHES);
                        }
                    }
                    Err(problem) => {
                        rimstack_core::problem!("game not started: {problem}");
                        me.not_started(crate::words::said_of(&problem));
                    }
                }
            },
        );
    }
}
