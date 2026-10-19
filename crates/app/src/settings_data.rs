#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("rimstack-app/src/application.h");
        #[rust_name = "set_font_family"]
        fn setFontFamily(family: &QString) -> QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_uncreatable]
        #[qproperty(QVariant, report)]
        #[qproperty(QVariant, folder_checks, cxx_name = "folderChecks")]
        #[qproperty(QVariant, databases)]
        #[qproperty(QVariant, languages)]
        #[qproperty(QVariant, words)]
        #[qproperty(QString, theme)]
        #[qproperty(QVariant, own_colours, cxx_name = "ownColours")]
        #[qproperty(bool, community_busy, cxx_name = "communityBusy")]
        #[qproperty(bool, community_current, cxx_name = "communityCurrent")]
        #[qproperty(QString, community_problem, cxx_name = "communityProblem")]
        #[qproperty(QVariant, release)]
        #[qproperty(bool, release_busy, cxx_name = "releaseBusy")]
        #[qproperty(QString, problem)]
        type SettingsData = super::SettingsDataRust;
    }

    impl cxx_qt::Threading for SettingsData {}

    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");
        #[cxx_name = "make_unique"]
        #[namespace = "rust::cxxqtlib1"]
        fn make_settings_data() -> UniquePtr<SettingsData>;
    }

    extern "RustQt" {
        #[qinvokable]
        fn keep(self: Pin<&mut SettingsData>, name: &QString, value: &QString) -> bool;

        /// Judges a typed folder after a pause in the typing, off the window's thread, and uses it when it is fine.
        #[qinvokable]
        #[cxx_name = "checkFolder"]
        fn check_folder(self: Pin<&mut SettingsData>, name: &QString, typed: &QString);

        /// The same judgement for a folder chosen in a dialog.
        #[qinvokable]
        #[cxx_name = "chooseFolder"]
        fn choose_folder(self: Pin<&mut SettingsData>, name: &QString, chosen: &QUrl);

        /// Stores a typed setting after a pause in the typing.
        #[qinvokable]
        #[cxx_name = "keepLater"]
        fn keep_later(self: Pin<&mut SettingsData>, name: &QString, value: &QString);

        #[qinvokable]
        #[cxx_name = "useDatabase"]
        fn use_database(self: Pin<&mut SettingsData>, id: &QString, on: bool);

        /// 0 warn about the game version, 1 not for the version installed, 2 never.
        #[qinvokable]
        #[cxx_name = "versionWarnings"]
        fn version_warnings(self: Pin<&mut SettingsData>, choice: i32);

        #[qinvokable]
        #[cxx_name = "useLanguage"]
        fn use_language(self: Pin<&mut SettingsData>, name: &QString);

        #[qinvokable]
        fn counted(
            self: &SettingsData,
            n: i32,
            one: &QString,
            many: &QString,
            locale: &QString,
        ) -> QString;

        /// `{ file }` or `{ problem }`.
        #[qinvokable]
        #[cxx_name = "startLanguage"]
        fn start_language(self: &SettingsData, name: &QString) -> QVariant;

        /// `{ file }` or `{ problem }`.
        #[qinvokable]
        #[cxx_name = "startOwnTheme"]
        fn start_own_theme(self: &SettingsData, colours: &QVariant) -> QVariant;

        /// `[{ what, found, needed }]`.
        #[qinvokable]
        #[cxx_name = "shortContrast"]
        fn short_contrast(self: &SettingsData, colours: &QVariant) -> QVariant;

        #[qinvokable]
        #[cxx_name = "useFont"]
        fn use_font(self: Pin<&mut SettingsData>, family: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "downloadCommunity"]
        fn download_community(self: Pin<&mut SettingsData>);

        #[qinvokable]
        #[cxx_name = "checkRelease"]
        fn check_release(self: Pin<&mut SettingsData>);

        #[qinvokable]
        #[cxx_name = "importRimSort"]
        fn import_rimsort(self: &SettingsData) -> QString;

        #[qinvokable]
        #[cxx_name = "importRimPy"]
        fn import_rimpy(self: &SettingsData) -> QString;

        #[qinvokable]
        #[cxx_name = "setModSettingsAside"]
        fn set_mod_settings_aside(self: &SettingsData) -> QString;

        #[qinvokable]
        #[cxx_name = "filesChanged"]
        fn files_changed(self: &SettingsData);
    }
}

use crate::App;
use crate::changes::Changes;
use crate::qml::values::{from_qml, to_qml};
use crate::reports::{Answer, Written};
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl, QVariant};
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
pub struct SettingsDataRust {
    report: QVariant,
    folder_checks: QVariant,
    typing: std::collections::HashMap<String, crate::jobs::Lane>,
    folder_verdicts: std::collections::BTreeMap<String, crate::reports::FolderVerdict>,
    databases: QVariant,
    languages: QVariant,
    words: QVariant,
    theme: QString,
    own_colours: QVariant,
    community_busy: bool,
    community_current: bool,
    community_problem: QString,
    release: QVariant,
    release_busy: bool,
    problem: QString,
    app: crate::hub::Given,
}

fn written(file: Result<std::path::PathBuf, rimstack_core::base::error::Error>) -> QVariant {
    let file = file.map(|file| Written {
        file: file.to_string_lossy().into_owned(),
    });
    to_qml(&Answer::from(file))
}

const FOLDER_PAUSE: Duration = Duration::from_millis(400);
const SETTING_PAUSE: Duration = Duration::from_millis(600);

impl qobject::SettingsData {
    fn typing_lane(self: Pin<&mut Self>, name: &str) -> &mut crate::jobs::Lane {
        self.rust_mut()
            .get_mut()
            .typing
            .entry(name.to_string())
            .or_default()
    }

    fn app(&self) -> Arc<App> {
        self.app.app()
    }

    pub(crate) fn connect(mut self: Pin<&mut Self>, app: Arc<App>) {
        crate::hub::hear(&app, self.qt_thread(), Self::heard);
        self.as_mut().rust_mut().app = crate::hub::Given::of(app);
        self.as_mut().on_theme_changed(|me| me.recolour()).release();
        self.heard(Changes::SETTINGS | Changes::LANGUAGE | Changes::THEME | Changes::COMMUNITY);
    }

    fn heard(mut self: Pin<&mut Self>, changes: Changes) {
        let app = self.app();
        let settings = Changes::SETTINGS
            | Changes::FOLDERS
            | Changes::LIBRARY
            | Changes::LOGS
            | Changes::MOD_SETTINGS;
        if changes.contains(settings) {
            self.as_mut().set_report(to_qml(&app.settings_report()));
        }
        if changes.contains(Changes::SETTINGS | Changes::COMMUNITY | Changes::LANGUAGE) {
            self.as_mut().set_databases(to_qml(&app.databases()));
        }
        if changes.contains(Changes::LANGUAGE) {
            self.as_mut().set_languages(to_qml(&app.languages()));
            self.as_mut().set_words(to_qml(&app.speak()));
        }
        if changes.contains(Changes::THEME) {
            self.recolour();
        }
    }

    fn recolour(mut self: Pin<&mut Self>) {
        let colours = self.app().own_colours(&self.theme.to_string());
        self.as_mut().set_own_colours(to_qml(&colours));
    }

    fn keep(mut self: Pin<&mut Self>, name: &QString, value: &QString) -> bool {
        let kept = self
            .app()
            .keep_setting(&name.to_string(), &value.to_string());
        self.as_mut().set_problem(crate::words::why_not(&kept));
        kept.is_ok()
    }

    fn check_folder(mut self: Pin<&mut Self>, name: &QString, typed: &QString) {
        let (name, typed) = (name.to_string(), typed.to_string().trim().to_string());
        let thread = self.qt_thread();
        let lane = self.as_mut().typing_lane(&name);
        let judged = (name.clone(), typed.clone());
        lane.run(
            "folder check",
            thread,
            move |turn| {
                turn.after(FOLDER_PAUSE)
                    .then(|| crate::workspace::judge_folder(&judged.0, &judged.1))
            },
            move |mut me, verdict| {
                let Some(verdict) = verdict else {
                    return;
                };
                if me.app().settings().get(&name).unwrap_or_default() == typed {
                    me.as_mut()
                        .rust_mut()
                        .get_mut()
                        .folder_verdicts
                        .remove(&name);
                } else {
                    if verdict.ok {
                        me.as_mut()
                            .keep(&QString::from(&name), &QString::from(&typed));
                    }
                    me.as_mut()
                        .rust_mut()
                        .get_mut()
                        .folder_verdicts
                        .insert(name, verdict);
                }
                let shown = to_qml(&me.folder_verdicts);
                me.as_mut().set_folder_checks(shown);
            },
        );
    }

    fn choose_folder(self: Pin<&mut Self>, name: &QString, chosen: &QUrl) {
        if let Some(path) = chosen.to_local_file() {
            self.check_folder(name, &path);
        }
    }

    fn keep_later(mut self: Pin<&mut Self>, name: &QString, value: &QString) {
        let (name, value) = (name.to_string(), value.to_string().trim().to_string());
        let thread = self.qt_thread();
        let lane = self.as_mut().typing_lane(&name);
        lane.run(
            "setting typed",
            thread,
            |turn| turn.after(SETTING_PAUSE),
            move |mut me, quiet| {
                let same = me.app().settings().get(&name).unwrap_or_default() == value;
                if quiet && !same {
                    me.as_mut()
                        .keep(&QString::from(&name), &QString::from(&value));
                }
            },
        );
    }

    fn use_database(mut self: Pin<&mut Self>, id: &QString, on: bool) {
        let kept = self.app().use_database(&id.to_string(), on);
        self.as_mut().set_problem(crate::words::why_not(&kept));
    }

    fn version_warnings(mut self: Pin<&mut Self>, choice: i32) {
        let kept = self.app().version_warnings(choice);
        self.as_mut().set_problem(crate::words::why_not(&kept));
    }

    fn counted(&self, n: i32, one: &QString, many: &QString, locale: &QString) -> QString {
        let n = usize::try_from(n).unwrap_or_default();
        let (one, many) = (one.to_string(), many.to_string());
        QString::from(&crate::words::counted(n, &one, &many, &locale.to_string()))
    }

    fn use_language(self: Pin<&mut Self>, name: &QString) {
        self.keep(&QString::from("language"), name);
    }

    fn start_language(&self, name: &QString) -> QVariant {
        written(self.app().start_language(&name.to_string()))
    }

    fn start_own_theme(&self, colours: &QVariant) -> QVariant {
        let colours = from_qml::<serde_json::Value>(colours).unwrap_or_default();
        written(
            self.app()
                .start_own_theme(&self.theme.to_string(), &colours),
        )
    }

    fn short_contrast(&self, colours: &QVariant) -> QVariant {
        let colours = from_qml::<serde_json::Map<String, serde_json::Value>>(colours);
        to_qml(&crate::workspace::short_contrast(
            &colours.unwrap_or_default(),
        ))
    }

    fn use_font(self: Pin<&mut Self>, family: &QString) -> QString {
        self.keep(&QString::from("font"), family);
        qobject::set_font_family(family)
    }

    fn download_community(mut self: Pin<&mut Self>) {
        if self.community_busy {
            return;
        }
        self.as_mut().set_community_busy(true);
        self.as_mut().set_community_problem(QString::default());
        let app = self.app();
        crate::jobs::run(
            "community download",
            self.qt_thread(),
            move || app.download_databases(),
            |mut me, done| {
                me.as_mut().set_community_current(done.is_ok());
                me.as_mut()
                    .set_community_problem(crate::words::why_not(&done));
                me.as_mut().set_community_busy(false);
            },
        );
    }

    fn check_release(mut self: Pin<&mut Self>) {
        if self.release_busy {
            return;
        }
        self.as_mut().set_release_busy(true);
        let nightly = self.app().settings().nightly_updates();
        crate::jobs::run(
            "release check",
            self.qt_thread(),
            move || crate::workspace::newest_release(nightly),
            |mut me, found| {
                me.as_mut().set_release(to_qml(&found));
                me.as_mut().set_release_busy(false);
            },
        );
    }

    fn import_rimsort(&self) -> QString {
        crate::words::said(&self.app().import_rimsort())
    }

    fn set_mod_settings_aside(&self) -> QString {
        crate::words::said_or_why(&self.app().set_mod_settings_aside())
    }

    fn import_rimpy(&self) -> QString {
        crate::words::said(&self.app().import_rimpy())
    }

    fn files_changed(&self) {
        self.app().publish(Changes::THEME | Changes::LANGUAGE);
    }
}
