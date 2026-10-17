use crate::base::sentence::Sentence;
use crate::base::vdf;
use crate::say;
use std::path::{Path, PathBuf};

pub const APP_ID: &str = "294100";
const SETTINGS: &str = "Ludeon Studios/RimWorld by Ludeon Studios";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    /// `<game>/Data`.
    Official,
    /// `<game>/Mods`.
    Local,
    Workshop,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModRoot {
    pub path: PathBuf,
    pub source: Source,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paths {
    pub game: Option<PathBuf>,
    /// The folder holding `Config/` and `Saves/`.
    pub config: Option<PathBuf>,
    pub workshop: Option<PathBuf>,
    /// Short form, `1.6`, from `<game>/Version.txt`.
    pub game_version: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Overrides {
    pub game: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub workshop: Option<PathBuf>,
}

impl Overrides {
    pub fn from_env() -> Self {
        let var = |name| std::env::var_os(name).map(PathBuf::from);
        Self {
            game: var("RIMSTACK_GAME"),
            config: var("RIMSTACK_CONFIG"),
            workshop: var("RIMSTACK_WORKSHOP"),
        }
    }
}

impl Paths {
    pub fn mods_config(&self) -> Option<PathBuf> {
        Some(self.config.as_ref()?.join("Config").join("ModsConfig.xml"))
    }

    pub fn local_mods(&self) -> Option<PathBuf> {
        Some(self.game.as_ref()?.join("Mods"))
    }

    /// `<library>/steamapps/workshop/appworkshop_294100.acf`, two folders above the items.
    pub fn steam_record(&self) -> Option<PathBuf> {
        let workshop = self.workshop.as_ref()?.parent()?.parent()?;
        Some(workshop.join(format!("appworkshop_{APP_ID}.acf")))
    }

    pub fn roots(&self) -> Vec<ModRoot> {
        let game = self.game.as_ref();
        [
            (game.map(|g| g.join("Data")), Source::Official),
            (self.local_mods(), Source::Local),
            (self.workshop.clone(), Source::Workshop),
        ]
        .into_iter()
        .filter_map(|(path, source)| {
            Some(ModRoot {
                path: path?,
                source,
            })
        })
        .filter(|root| root.path.is_dir())
        .fold(Vec::new(), |mut roots: Vec<ModRoot>, root| {
            // A Workshop folder typed by hand may be the game's own Mods (#6).
            let same =
                |a: &Path, b: &Path| dunce::canonicalize(a).ok() == dunce::canonicalize(b).ok();
            if !roots.iter().any(|kept| same(&kept.path, &root.path)) {
                roots.push(root);
            }
            roots
        })
    }
}

/// Steam's folder is often reached through several symlinks, so paths are canonicalised.
fn existing(candidates: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for path in candidates
        .into_iter()
        .filter_map(|p| dunce::canonicalize(p).ok())
    {
        if path.is_dir() && !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Windows,
    Mac,
}

impl Os {
    pub fn running() -> Self {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        }
    }
}

pub type Env<'a> = &'a dyn Fn(&str) -> Option<PathBuf>;

/// On Windows, Steam's install path is read from the registry under this name.
pub const STEAM_INSTALL: &str = "SteamInstallPath";

pub fn from_environment(name: &str) -> Option<PathBuf> {
    if name == STEAM_INSTALL {
        return steam_in_registry();
    }
    std::env::var_os(name).map(PathBuf::from)
}

/// Runs `reg`, which every Windows has.
fn steam_in_registry() -> Option<PathBuf> {
    if Os::running() != Os::Windows {
        return None;
    }
    let keys = [
        "HKLM\\SOFTWARE\\Wow6432Node\\Valve\\Steam",
        "HKLM\\SOFTWARE\\Valve\\Steam",
    ];
    keys.iter().find_map(|key| {
        let asked = crate::base::program::command("reg")
            .args(["query", key, "/v", "InstallPath"])
            .output()
            .ok()?;
        install_path(&String::from_utf8_lossy(&asked.stdout))
    })
}

/// The value of the `InstallPath` line that `reg query` prints: name, type, value.
fn install_path(printed: &str) -> Option<PathBuf> {
    let mut lines = printed.lines();
    let line = lines.find(|line| line.trim_start().starts_with("InstallPath"))?;
    let path = line.split("REG_SZ").nth(1)?.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

pub fn home(env: Env) -> PathBuf {
    env("HOME")
        .or_else(|| env("USERPROFILE"))
        .unwrap_or_default()
}

pub fn app_dirs(os: Os, home: &Path, env: Env) -> (PathBuf, PathBuf) {
    let set = |name: &str| env(name).filter(|path| path.is_absolute());
    match os {
        Os::Linux => (
            set("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")),
            set("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share")),
        ),
        Os::Windows => (
            set("APPDATA").unwrap_or_else(|| home.join("AppData/Roaming")),
            set("LOCALAPPDATA").unwrap_or_else(|| home.join("AppData/Local")),
        ),
        Os::Mac => {
            let support = home.join("Library/Application Support");
            (support.clone(), support)
        }
    }
}

fn steam_roots(os: Os, home: &Path, env: Env) -> Vec<PathBuf> {
    let places: Vec<PathBuf> = match os {
        Os::Linux => [
            ".local/share/Steam",
            ".steam/debian-installation",
            ".steam/steam",
            ".steam/root",
            ".var/app/com.valvesoftware.Steam/.local/share/Steam",
            "snap/steam/common/.local/share/Steam",
        ]
        .map(|p| home.join(p))
        .to_vec(),
        Os::Windows => {
            let programs = ["ProgramFiles(x86)", "ProgramFiles"];
            let usual = programs.iter().filter_map(|name| env(name));
            let usual = usual.map(|programs| programs.join("Steam"));
            env(STEAM_INSTALL).into_iter().chain(usual).collect()
        }
        Os::Mac => vec![home.join("Library/Application Support/Steam")],
    };
    existing(places)
}

/// Each folder may hold the game under any name.
fn other_shops(os: Os, home: &Path, env: Env) -> Vec<PathBuf> {
    let places = match os {
        Os::Linux => vec![home.join("GOG Games"), home.join("Games")],
        Os::Windows => {
            let drive = env("SystemDrive").unwrap_or_else(|| PathBuf::from("C:"));
            let on_drive = PathBuf::from(format!("{}\\GOG Games", drive.display()));
            vec![on_drive, home.join("GOG Games"), home.join("Games")]
        }
        Os::Mac => vec![PathBuf::from("/Applications"), home.join("Applications")],
    };
    existing(places)
}

/// Found by `Data/Core`, which every copy has.
fn game_in(os: Os, place: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(place).ok()?.flatten();
    let folders = entries
        .map(|entry| entry.path())
        .filter(|path| path.is_dir());
    let mut roots = folders.map(|folder| content_root(os, &folder));
    roots.find(|root| root.join("Data").join("Core").is_dir())
}

fn on_unity(game_version: Option<&str>) -> bool {
    let parsed = game_version.and_then(|version| {
        let (major, minor) = version.split_once('.')?;
        Some((major.parse::<u32>().ok()?, minor.parse::<u32>().ok()?))
    });
    parsed.is_some_and(|version| version < (1, 7))
}

fn native_settings(os: Os, home: &Path, game_version: Option<&str>) -> Vec<PathBuf> {
    match os {
        Os::Linux => vec![
            home.join(".config/unity3d").join(SETTINGS),
            home.join(".var/app/com.valvesoftware.Steam/.config/unity3d")
                .join(SETTINGS),
        ],
        Os::Windows => vec![home.join("AppData/LocalLow").join(SETTINGS)],
        Os::Mac => {
            let support = home.join("Library/Application Support");
            let waylith = support.join("Ludeon Studios/RimWorld");
            let unity = [support.join("Rimworld"), support.join("RimWorld")];
            if on_unity(game_version) {
                unity.into_iter().chain([waylith]).collect()
            } else {
                [waylith].into_iter().chain(unity).collect()
            }
        }
    }
}

/// On macOS, the `.app` bundle or the folder it lies in, whichever has `Data`.
fn content_root(os: Os, installed: &Path) -> PathBuf {
    if os != Os::Mac {
        return installed.to_path_buf();
    }
    let entries = std::fs::read_dir(installed).into_iter().flatten().flatten();
    let is_bundle = |path: &PathBuf| path.extension().is_some_and(|x| x == "app") && path.is_dir();
    let bundles = entries.map(|entry| entry.path()).filter(is_bundle);
    let mut candidates = bundles.chain([installed.to_path_buf()]);
    let with_data = candidates.find(|folder| folder.join("Data").is_dir());
    with_data.unwrap_or_else(|| installed.to_path_buf())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folder {
    Game,
    Config,
    Workshop,
}

/// What a folder typed by hand would be used as: the sentence says either way.
pub fn judge(os: Os, kind: Folder, place: &Path) -> Result<Sentence, Sentence> {
    if !place.is_dir() {
        return Err(say!("There is no such folder"));
    }
    match kind {
        Folder::Game => {
            let root = content_root(os, place);
            if !root.join("Data").is_dir() {
                return Err(say!(
                    "This folder has no Data folder, so it is not RimWorld"
                ));
            }
            let version = std::fs::read_to_string(root.join("Version.txt"))
                .ok()
                .and_then(|v| crate::rimworld::modsconfig::short_version(&v));
            Ok(match version {
                Some(version) => say!("RimWorld {} found", version),
                None => say!("RimWorld found"),
            })
        }
        Folder::Config => {
            if ["Config", "Saves"].iter().any(|d| place.join(d).is_dir()) {
                Ok(say!("Settings and saves found"))
            } else {
                Err(say!("This folder holds neither Config nor Saves"))
            }
        }
        Folder::Workshop => Ok(say!("Workshop folder found")),
    }
}

pub fn detect(home: &Path, overrides: &Overrides) -> Paths {
    detect_on(Os::running(), home, &from_environment, overrides)
}

pub fn detect_on(os: Os, home: &Path, env: Env, overrides: &Overrides) -> Paths {
    let roots = steam_roots(os, home, env);
    let listed: Vec<vdf::Library> = roots
        .iter()
        .filter_map(|r| {
            // Steam keeps the list in both places; an install may lack one.
            let lists = ["steamapps/libraryfolders.vdf", "config/libraryfolders.vdf"];
            lists
                .iter()
                .find_map(|list| std::fs::read_to_string(r.join(list)).ok())
        })
        .flat_map(|text| vdf::libraries(&text))
        .collect();
    // Libraries that list the game come first, so a stale copy elsewhere
    // does not win.
    let (with_app, without): (Vec<_>, Vec<_>) = listed
        .iter()
        .partition(|l| l.apps.iter().any(|a| a == APP_ID));
    let libraries = existing(
        with_app
            .into_iter()
            .chain(without)
            .map(|l| l.path.clone())
            .chain(roots.iter().cloned()),
    );

    let found_game = libraries
        .iter()
        .map(|l| (l, content_root(os, &l.join("steamapps/common/RimWorld"))))
        .find(|(_, game)| game.is_dir());
    let workshop_in = |l: &PathBuf| {
        Some(l.join("steamapps/workshop/content").join(APP_ID)).filter(|p| p.is_dir())
    };

    let game = overrides
        .game
        .clone()
        .or_else(|| found_game.as_ref().map(|(_, g)| g.clone()))
        .or_else(|| {
            let shops = other_shops(os, home, env);
            shops.iter().find_map(|place| game_in(os, place))
        });
    let workshop = overrides.workshop.clone().or_else(|| {
        // A game set by hand has no Workshop folder.
        if overrides.game.is_some() {
            return None;
        }
        found_game
            .as_ref()
            .and_then(|(l, _)| workshop_in(l))
            .or_else(|| libraries.iter().find_map(workshop_in))
    });
    let game_version = game
        .as_ref()
        .and_then(|g| std::fs::read_to_string(g.join("Version.txt")).ok())
        .and_then(|v| crate::rimworld::modsconfig::short_version(&v));
    let config = overrides.config.clone().or_else(|| {
        let native = native_settings(os, home, game_version.as_deref());
        // Under Proton the settings live inside the game's Windows prefix.
        let under_proton = libraries.iter().filter(|_| os == Os::Linux);
        let proton = under_proton.map(|l| {
            l.join("steamapps/compatdata")
                .join(APP_ID)
                .join("pfx/drive_c/users/steamuser/AppData/LocalLow")
                .join(SETTINGS)
        });
        let windows_build = game.as_ref().is_some_and(|g| {
            ["RimWorldWin64.exe", "RimWorld.exe"]
                .iter()
                .any(|exe| g.join(exe).is_file())
        });
        let ordered: Vec<PathBuf> = if windows_build {
            proton.chain(native).collect()
        } else {
            native.into_iter().chain(proton).collect()
        };
        ordered.into_iter().find(|p| p.is_dir())
    });

    Paths {
        game,
        config,
        workshop,
        game_version,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const NATIVE_CONFIG: &str = ".config/unity3d/Ludeon Studios/RimWorld by Ludeon Studios";

    fn library_with_game(library: &Path) {
        let game = library.join("steamapps/common/RimWorld");
        fs::create_dir_all(game.join("Data")).unwrap();
        fs::create_dir_all(game.join("Mods")).unwrap();
        fs::write(game.join("Version.txt"), "1.6.4871 rev598\n").unwrap();
        fs::write(game.join("RimWorldLinux"), "").unwrap();
        fs::create_dir_all(library.join("steamapps/workshop/content/294100")).unwrap();
    }

    fn steam_root(home: &Path, libraries: &[(&Path, bool)]) -> PathBuf {
        let root = home.join(".local/share/Steam");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        let mut vdf = String::from("\"libraryfolders\"\n{\n");
        for (i, (path, has_game)) in libraries.iter().enumerate() {
            let apps = if *has_game { "\"294100\" \"1\"" } else { "" };
            vdf += &format!(
                "\"{i}\" {{ \"path\" \"{}\" \"apps\" {{ {apps} }} }}\n",
                path.display()
            );
        }
        fs::write(root.join("steamapps/libraryfolders.vdf"), vdf + "}\n").unwrap();
        root
    }

    #[test]
    fn native_install_is_found() {
        let home = tempfile::tempdir().unwrap();
        let root = steam_root(home.path(), &[]);
        library_with_game(&root);
        fs::create_dir_all(home.path().join(NATIVE_CONFIG)).unwrap();

        let paths = detect(home.path(), &Overrides::default());
        let root = root.canonicalize().unwrap();
        assert_eq!(paths.game, Some(root.join("steamapps/common/RimWorld")));
        assert_eq!(
            paths.workshop,
            Some(root.join("steamapps/workshop/content/294100"))
        );
        assert_eq!(paths.config, Some(home.path().join(NATIVE_CONFIG)));
        assert_eq!(paths.game_version.as_deref(), Some("1.6"));
        let sources: Vec<Source> = paths.roots().iter().map(|r| r.source).collect();
        assert_eq!(sources, [Source::Official, Source::Local, Source::Workshop]);
    }

    #[cfg(unix)]
    #[test]
    fn game_in_a_second_library_is_found_and_symlinked_roots_count_once() {
        let home = tempfile::tempdir().unwrap();
        let second = home.path().join("games");
        library_with_game(&second);
        let root = steam_root(home.path(), &[(&second, true)]);
        fs::create_dir_all(home.path().join(".steam")).unwrap();
        std::os::unix::fs::symlink(&root, home.path().join(".steam/steam")).unwrap();

        assert_eq!(steam_roots(Os::Linux, home.path(), &|_| None).len(), 1);
        let paths = detect(home.path(), &Overrides::default());
        let second = second.canonicalize().unwrap();
        assert_eq!(paths.game, Some(second.join("steamapps/common/RimWorld")));
    }

    #[test]
    fn a_workshop_folder_set_to_the_games_mods_is_read_once_as_local() {
        let home = tempfile::tempdir().unwrap();
        library_with_game(home.path());
        let game = home.path().join("steamapps/common/RimWorld");
        let paths = Paths {
            workshop: Some(game.join("Mods/.")),
            game: Some(game),
            ..Paths::default()
        };
        let sources: Vec<Source> = paths.roots().iter().map(|r| r.source).collect();
        assert_eq!(sources, [Source::Official, Source::Local]);
    }

    #[test]
    fn library_listing_the_app_beats_a_stale_copy() {
        let home = tempfile::tempdir().unwrap();
        let (stale, real) = (home.path().join("old"), home.path().join("new"));
        library_with_game(&stale);
        library_with_game(&real);
        steam_root(home.path(), &[(&stale, false), (&real, true)]);

        let paths = detect(home.path(), &Overrides::default());
        let real = real.canonicalize().unwrap();
        assert_eq!(paths.game, Some(real.join("steamapps/common/RimWorld")));
    }

    #[test]
    fn proton_settings_are_found() {
        let home = tempfile::tempdir().unwrap();
        let root = steam_root(home.path(), &[]);
        library_with_game(&root);
        let game = root.join("steamapps/common/RimWorld");
        fs::remove_file(game.join("RimWorldLinux")).unwrap();
        fs::write(game.join("RimWorldWin64.exe"), "").unwrap();
        let prefix = root
            .join("steamapps/compatdata/294100/pfx/drive_c/users/steamuser/AppData/LocalLow")
            .join(SETTINGS);
        fs::create_dir_all(&prefix).unwrap();
        fs::create_dir_all(home.path().join(NATIVE_CONFIG)).unwrap();

        let paths = detect(home.path(), &Overrides::default());
        assert_eq!(paths.config, Some(prefix.canonicalize().unwrap()));
    }

    #[test]
    fn the_folders_of_windows_and_macos_are_found() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let programs = tmp.path().join("Program Files (x86)");
        let settings = home.join("AppData/LocalLow").join(SETTINGS);
        fs::create_dir_all(programs.join("Steam/steamapps/common/RimWorld")).unwrap();
        fs::create_dir_all(programs.join("Steam/steamapps/workshop/content/294100")).unwrap();
        fs::create_dir_all(&settings).unwrap();
        let env = |name: &str| (name == "ProgramFiles(x86)").then(|| programs.clone());
        let found = detect_on(Os::Windows, &home, &env, &Overrides::default());
        assert!(
            found
                .game
                .unwrap()
                .ends_with("Steam/steamapps/common/RimWorld")
        );
        assert!(found.workshop.unwrap().ends_with("workshop/content/294100"));
        assert_eq!(found.config.unwrap(), settings);

        let support = home.join("Library/Application Support");
        let installed = support.join("Steam/steamapps/common/RimWorld");
        fs::create_dir_all(installed.join("RimWorldMac.app")).unwrap();
        fs::create_dir_all(installed.join("Data")).unwrap();
        fs::create_dir_all(support.join("Rimworld")).unwrap();
        let found = detect_on(Os::Mac, &home, &|_| None, &Overrides::default());
        assert!(found.game.unwrap().ends_with("common/RimWorld"));
        assert_eq!(found.config.unwrap(), support.join("Rimworld"));
        fs::create_dir_all(installed.join("RimWorldMac.app/Data")).unwrap();
        let found = detect_on(Os::Mac, &home, &|_| None, &Overrides::default());
        assert!(found.game.unwrap().ends_with("RimWorld/RimWorldMac.app"));
    }

    #[test]
    fn macos_settings_follow_the_game_s_version_past_a_stale_unity_folder() {
        let home = tempfile::tempdir().unwrap();
        let support = home.path().join("Library/Application Support");
        let installed = support.join("Steam/steamapps/common/RimWorld");
        fs::create_dir_all(installed.join("Data")).unwrap();
        let (unity, waylith) = (
            support.join("RimWorld"),
            support.join("Ludeon Studios/RimWorld"),
        );
        fs::create_dir_all(&unity).unwrap();
        fs::create_dir_all(&waylith).unwrap();
        let config_for = |version: &str| {
            fs::write(installed.join("Version.txt"), version).unwrap();
            detect_on(Os::Mac, home.path(), &|_| None, &Overrides::default()).config
        };
        assert_eq!(config_for("1.7.4976 rev1043"), Some(waylith.clone()));
        assert_eq!(config_for("1.6.4871 rev598"), Some(unity));
        fs::remove_file(installed.join("Version.txt")).unwrap();
        let found = detect_on(Os::Mac, home.path(), &|_| None, &Overrides::default());
        assert_eq!(found.config, Some(waylith));
    }

    #[test]
    fn steam_s_folder_is_read_from_what_the_registry_prints() {
        let printed = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Valve\\Steam\r\n    InstallPath    REG_SZ    D:\\Games\\Steam\r\n";
        assert_eq!(
            install_path(printed),
            Some(PathBuf::from("D:\\Games\\Steam"))
        );
        assert_eq!(
            install_path("ERROR: The system was unable to find the key."),
            None
        );
        let tmp = tempfile::tempdir().unwrap();
        let steam = tmp.path().join("Steam");
        fs::create_dir_all(steam.join("steamapps/common/RimWorld")).unwrap();
        let env = |name: &str| (name == STEAM_INSTALL).then(|| steam.clone());
        let found = detect_on(Os::Windows, tmp.path(), &env, &Overrides::default());
        assert!(
            found
                .game
                .unwrap()
                .ends_with("Steam/steamapps/common/RimWorld")
        );
    }

    #[test]
    fn a_program_s_own_folders_follow_the_system() {
        let home = Path::new("/h");
        let none = |_: &str| None;
        assert_eq!(
            app_dirs(Os::Linux, home, &none),
            (home.join(".config"), home.join(".local/share"))
        );
        let set = |name: &str| (name == "XDG_DATA_HOME").then(|| PathBuf::from("/data"));
        assert_eq!(app_dirs(Os::Linux, home, &set).1, PathBuf::from("/data"));
        assert_eq!(
            app_dirs(Os::Windows, home, &none),
            (home.join("AppData/Roaming"), home.join("AppData/Local"))
        );
        let support = home.join("Library/Application Support");
        assert_eq!(app_dirs(Os::Mac, home, &none), (support.clone(), support));
        let windows = |name: &str| (name == "USERPROFILE").then(|| PathBuf::from("/u"));
        assert_eq!(super::home(&windows), PathBuf::from("/u"));
    }

    #[test]
    fn a_copy_from_another_shop_is_found_by_its_core_and_has_no_workshop() {
        let home = tempfile::tempdir().unwrap();
        let game = home.path().join("GOG Games/RimWorld 1.6");
        fs::create_dir_all(game.join("Data/Core")).unwrap();
        fs::create_dir_all(home.path().join("GOG Games/Another Game/Data")).unwrap();
        fs::write(game.join("Version.txt"), "1.6.4871 rev600").unwrap();
        let paths = detect_on(Os::Linux, home.path(), &|_| None, &Overrides::default());
        assert_eq!(paths.game, Some(game.canonicalize().unwrap()));
        assert_eq!(
            (paths.workshop, paths.game_version.as_deref()),
            (None, Some("1.6"))
        );

        let mac = tempfile::tempdir().unwrap();
        let bundle = mac.path().join("Applications/RimWorld.app");
        fs::create_dir_all(bundle.join("Data/Core")).unwrap();
        let paths = detect_on(Os::Mac, mac.path(), &|_| None, &Overrides::default());
        assert_eq!(paths.game, Some(bundle.canonicalize().unwrap()));
    }

    #[test]
    fn steam_s_list_of_libraries_is_read_from_either_place_it_keeps_it() {
        let home = tempfile::tempdir().unwrap();
        let second = home.path().join("games");
        library_with_game(&second);
        let root = steam_root(home.path(), &[(&second, true)]);
        fs::create_dir_all(root.join("config")).unwrap();
        let (usual, other) = ("steamapps/libraryfolders.vdf", "config/libraryfolders.vdf");
        fs::rename(root.join(usual), root.join(other)).unwrap();
        let paths = detect(home.path(), &Overrides::default());
        let second = second.canonicalize().unwrap();
        assert_eq!(paths.game, Some(second.join("steamapps/common/RimWorld")));
    }

    #[test]
    fn nothing_installed_finds_nothing() {
        let home = tempfile::tempdir().unwrap();
        let paths = detect(home.path(), &Overrides::default());
        assert_eq!(paths, Paths::default());
        assert!(paths.roots().is_empty());
        assert_eq!(paths.mods_config(), None);
    }

    #[test]
    fn a_game_set_by_hand_wins_and_has_no_workshop() {
        let home = tempfile::tempdir().unwrap();
        let root = steam_root(home.path(), &[]);
        library_with_game(&root);
        let copy = home.path().join("drm-free");
        fs::create_dir_all(copy.join("Mods")).unwrap();

        let overrides = Overrides {
            game: Some(copy.clone()),
            ..Overrides::default()
        };
        let paths = detect(home.path(), &overrides);
        assert_eq!(paths.game, Some(copy));
        assert_eq!(paths.workshop, None);
        assert_eq!(paths.roots().len(), 1);
    }

    #[test]
    fn a_folder_typed_by_hand_is_judged_by_what_it_holds() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        let judged = |kind, place: &Path| judge(Os::Linux, kind, place).map_err(|e| e.to_string());
        assert!(judged(Folder::Game, &game).is_err());
        fs::create_dir_all(&game).unwrap();
        assert!(judged(Folder::Game, &game).unwrap_err().contains("Data"));
        fs::create_dir_all(game.join("Data")).unwrap();
        fs::write(game.join("Version.txt"), "1.6.4871 rev598\n").unwrap();
        assert_eq!(
            judged(Folder::Game, &game).unwrap().to_string(),
            "RimWorld 1.6 found"
        );
        assert!(judged(Folder::Config, &game).is_err());
        fs::create_dir_all(game.join("Saves")).unwrap();
        assert!(judged(Folder::Config, &game).is_ok());
        assert!(judged(Folder::Workshop, &game).is_ok());
    }
}
