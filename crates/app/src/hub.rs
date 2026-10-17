#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("rimstack-app/src/settings_data.cxxqt.h");
        type SettingsData = crate::settings_data::qobject::SettingsData;
        include!("rimstack-app/src/mods_data.cxxqt.h");
        type ModsData = crate::mods_data::qobject::ModsData;
        include!("rimstack-app/src/saves_data.cxxqt.h");
        type SavesData = crate::saves_data::qobject::SavesData;
        include!("rimstack-app/src/logs_data.cxxqt.h");
        type LogsData = crate::logs_data::qobject::LogsData;
        include!("rimstack-app/src/game_data.cxxqt.h");
        type GameData = crate::game_data::qobject::GameData;
        include!("rimstack-app/src/lists_data.cxxqt.h");
        type ListsData = crate::lists_data::qobject::ListsData;
        include!("rimstack-app/src/search_data.cxxqt.h");
        type SearchData = crate::search_data::qobject::SearchData;
        include!("rimstack-app/src/textures_data.cxxqt.h");
        type TexturesData = crate::textures_data::qobject::TexturesData;
        include!("rimstack-app/src/downloads_data.cxxqt.h");
        type DownloadsData = crate::downloads_data::qobject::DownloadsData;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("rimstack-app/src/application.h");
        #[rust_name = "set_window_icon"]
        fn setWindowIcon(resource: &QString);
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(*mut SettingsData, settings, READ, CONSTANT)]
        #[qproperty(*mut ModsData, mods, READ, CONSTANT)]
        #[qproperty(*mut SavesData, saves, READ, CONSTANT)]
        #[qproperty(*mut LogsData, logs, READ, CONSTANT)]
        #[qproperty(*mut GameData, game, READ, CONSTANT)]
        #[qproperty(*mut ListsData, lists, READ, CONSTANT)]
        #[qproperty(*mut SearchData, search, READ, CONSTANT)]
        #[qproperty(*mut TexturesData, textures, READ, CONSTANT)]
        #[qproperty(*mut DownloadsData, downloads, READ, CONSTANT)]
        type Hub = super::HubRust;
    }

    impl cxx_qt::Initialize for Hub {}

    extern "RustQt" {
        #[qinvokable]
        fn given(self: &Hub, flag: &QString) -> bool;

        /// `--size=1000x560`; empty for none.
        #[qinvokable]
        fn argument(self: &Hub, flag: &QString) -> QString;

        #[qinvokable]
        fn number(self: &Hub, flag: &QString, otherwise: i32) -> i32;
    }
}

use crate::App;
use crate::changes::Changes;
use core::pin::Pin;
use cxx::UniquePtr;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;
use qobject::{
    DownloadsData, GameData, ListsData, LogsData, ModsData, SavesData, SearchData, SettingsData,
    TexturesData,
};
use rimstack_core::base::locks::Held;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

pub struct HubRust {
    settings: *mut SettingsData,
    mods: *mut ModsData,
    saves: *mut SavesData,
    logs: *mut LogsData,
    game: *mut GameData,
    lists: *mut ListsData,
    search: *mut SearchData,
    textures: *mut TexturesData,
    downloads: *mut DownloadsData,
    owned_settings: UniquePtr<SettingsData>,
    owned_mods: UniquePtr<ModsData>,
    owned_saves: UniquePtr<SavesData>,
    owned_logs: UniquePtr<LogsData>,
    owned_game: UniquePtr<GameData>,
    owned_lists: UniquePtr<ListsData>,
    owned_search: UniquePtr<SearchData>,
    owned_textures: UniquePtr<TexturesData>,
    owned_downloads: UniquePtr<DownloadsData>,
}

impl Default for HubRust {
    fn default() -> Self {
        Self {
            settings: std::ptr::null_mut(),
            mods: std::ptr::null_mut(),
            saves: std::ptr::null_mut(),
            logs: std::ptr::null_mut(),
            game: std::ptr::null_mut(),
            lists: std::ptr::null_mut(),
            search: std::ptr::null_mut(),
            textures: std::ptr::null_mut(),
            downloads: std::ptr::null_mut(),
            owned_settings: UniquePtr::null(),
            owned_mods: UniquePtr::null(),
            owned_saves: UniquePtr::null(),
            owned_logs: UniquePtr::null(),
            owned_game: UniquePtr::null(),
            owned_lists: UniquePtr::null(),
            owned_search: UniquePtr::null(),
            owned_textures: UniquePtr::null(),
            owned_downloads: UniquePtr::null(),
        }
    }
}

/// QML makes the singleton with no arguments, so the `App` waits here for the Hub.
static HANDED: OnceLock<Arc<App>> = OnceLock::new();

pub fn hand_over(app: Arc<App>) {
    if HANDED.set(app).is_err() {
        rimstack_core::problem!("the App was handed over twice");
    }
}

pub fn use_window_icon() {
    qobject::set_window_icon(&QString::from(":/qt/qml/RimStack/Ui/assets/rimstack.svg"));
}

/// `""` for the flag alone.
pub(crate) fn value(flag: &str) -> Option<String> {
    value_among(std::env::args(), flag)
}

fn value_among(args: impl IntoIterator<Item = String>, flag: &str) -> Option<String> {
    args.into_iter().find_map(|a| match a.strip_prefix(flag)? {
        "" => Some(String::new()),
        rest => rest.strip_prefix('=').map(String::from),
    })
}

impl qobject::Hub {
    fn given(&self, flag: &QString) -> bool {
        given(&flag.to_string())
    }

    fn argument(&self, flag: &QString) -> QString {
        QString::from(&value(&flag.to_string()).unwrap_or_default())
    }

    fn number(&self, flag: &QString, otherwise: i32) -> i32 {
        let given = value(&flag.to_string());
        given.and_then(|n| n.parse().ok()).unwrap_or(otherwise)
    }
}

pub(crate) fn given(flag: &str) -> bool {
    value(flag).is_some()
}

fn watch_every(demo: bool) -> Option<Duration> {
    let check = value("--check");
    let live = check.as_deref() == Some("live");
    let still = demo || (check.is_some() && !live) || (given("--shot") && !given("--live"));
    match (still, live) {
        (true, _) => None,
        (false, true) => Some(Duration::from_millis(500)),
        (false, false) => Some(Duration::from_secs(2)),
    }
}

/// Valid while the Hub lives.
pub(crate) fn address<T>(owned: &mut UniquePtr<T>) -> *mut T
where
    T: cxx::memory::UniquePtrTarget,
{
    owned.as_mut().map_or(std::ptr::null_mut(), |pinned| {
        // SAFETY: the address is only handed to QML; the object stays
        // pinned inside the `UniquePtr` the Hub keeps.
        unsafe { Pin::get_unchecked_mut(pinned) as *mut T }
    })
}

impl cxx_qt::Initialize for qobject::Hub {
    fn initialize(mut self: Pin<&mut Self>) {
        let app = HANDED
            .get()
            .cloned()
            .expect("main hands the App over before QML is loaded");
        let demo = given("--demo") || given("--selftest");
        let records = !given("--check");
        let mut hub = self.as_mut().rust_mut();
        (hub.settings, hub.owned_settings) =
            made(crate::settings_data::qobject::make_settings_data(), |it| {
                it.connect(app.clone())
            });
        (hub.mods, hub.owned_mods) = made(crate::mods_data::qobject::make_mods_data(), |it| {
            it.connect(app.clone(), demo, records)
        });
        (hub.saves, hub.owned_saves) = made(crate::saves_data::qobject::make_saves_data(), |it| {
            it.connect(app.clone())
        });
        (hub.logs, hub.owned_logs) = made(crate::logs_data::qobject::make_logs_data(), |it| {
            it.connect(app.clone(), !demo)
        });
        (hub.game, hub.owned_game) = made(crate::game_data::qobject::make_game_data(), |it| {
            it.connect(app.clone(), demo, watch_every(demo))
        });
        (hub.lists, hub.owned_lists) = made(crate::lists_data::qobject::make_lists_data(), |it| {
            it.connect(app.clone(), demo)
        });
        (hub.search, hub.owned_search) =
            made(crate::search_data::qobject::make_search_data(), |it| {
                it.connect(app.clone(), demo, records)
            });
        (hub.textures, hub.owned_textures) =
            made(crate::textures_data::qobject::make_textures_data(), |it| {
                it.connect(app.clone())
            });
        (hub.downloads, hub.owned_downloads) = made(
            crate::downloads_data::qobject::make_downloads_data(),
            |it| it.connect(app.clone(), demo),
        );
    }
}

fn made<T>(mut object: UniquePtr<T>, connect: impl FnOnce(Pin<&mut T>)) -> (*mut T, UniquePtr<T>)
where
    T: cxx::memory::UniquePtrTarget,
{
    if let Some(object) = object.as_mut() {
        connect(object);
    }
    (address(&mut object), object)
}

/// cxx-qt makes the adapter before the `App` exists; the Hub gives it the `App` after.
#[derive(Default)]
pub(crate) struct Given(Option<Arc<App>>);

impl Given {
    pub(crate) fn of(app: Arc<App>) -> Self {
        Self(Some(app))
    }

    pub(crate) fn app(&self) -> Arc<App> {
        self.0
            .clone()
            .expect("connected by the Hub before QML sees it")
    }
}

/// Runs on the adapter's own thread.
pub(crate) fn hear<T>(app: &Arc<App>, thread: CxxQtThread<T>, heard: fn(Pin<&mut T>, Changes))
where
    T: Threading + 'static,
{
    let thread = Mutex::new(thread);
    let speaking = Arc::downgrade(app);
    app.listen(move |changes| {
        let speaking = speaking.clone();
        let _ = thread.held().queue(move |me| {
            if let Some(app) = speaking
                .upgrade()
                .filter(|_| changes.contains(Changes::LANGUAGE))
            {
                crate::words::show_in(crate::words::Words::of(&app.speak()));
            }
            heard(me, changes)
        });
    });
}

#[cfg(test)]
mod tests {
    use super::value_among;

    #[test]
    fn a_flag_is_found_with_its_value_and_not_by_a_longer_one() {
        let args = || ["rimstack", "--check", "--section=3", "--shot-delay=6000"].map(String::from);
        assert_eq!(value_among(args(), "--check").as_deref(), Some(""));
        assert_eq!(value_among(args(), "--section").as_deref(), Some("3"));
        assert_eq!(value_among(args(), "--shot"), None);
        assert_eq!(value_among(args(), "--first"), None);
    }
}
