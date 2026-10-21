use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::{
    foreign, journal, launches, lists, new_mods, orders, settings, steamcmd, updates,
};
use rimstack_core::mods::list::{Category, GroupedList, Mod};
use rimstack_core::mods::rules::{Kind, Rule};
use rimstack_core::mods::{bisect, hidden, lock, share_file, textures, userrules, xmlcheck};
use rimstack_core::rimworld::paths::{ModRoot, Source};
use rimstack_core::rimworld::{saves, startup, workshopdb};
use rimstack_core::sources::{git, release, replacements};
use rimstack_core::{
    base::markup, base::vdf, kept::notes, kept::owntheme, mods::community, mods::ddsaudit,
    mods::share, mods::sort, rimworld::about, rimworld::dotnet, rimworld::loadfolders,
    rimworld::logs, rimworld::modsconfig, rimworld::rimsaves, rimworld::steamworks,
    rimworld::workshop, sources::changes,
};
use serde_json::Value;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const PIECES: &[&str] = &[
    "<",
    ">",
    "</",
    "/>",
    "[",
    "]",
    "[/",
    "=",
    "\"",
    "'",
    "&",
    "&amp;",
    "#",
    ":",
    "/",
    "\\",
    ".",
    ",",
    " ",
    "\n",
    "\r\n",
    "\t",
    "\u{feff}",
    "é",
    "ß",
    "İ",
    "日本",
    "𝔘",
    "\u{202e}",
    "0",
    "7",
    "1.6",
    "42",
    "activeMods",
    "<activeMods>",
    "</activeMods>",
    "<activeMods/>",
    "li",
    "<li>",
    "</li>",
    "url",
    "[url=",
    "[/url]",
    "img",
    "[img]",
    "[/img]",
    "b",
    "i",
    "h1",
    "list",
    "*",
    "https://",
    "http://",
    "steam://",
    "javascript:",
    "github.com/a/b",
    "?id=",
    "<p id=\"",
    "\">",
    "</p>",
    "<br>",
    "RimWorld ",
    "ERROR",
    "Exception",
    "  at ",
    "(wrapper",
    "[Ref ",
    "2026-10-12 19:37:35.060 ",
    "[Verse] ",
    "\"path\"",
    "\"apps\"",
    "{",
    "}",
    "loadFolders",
    "<v1.6>",
    "IfModActive",
    "tag:",
    "is:",
    "author:",
    "patches:",
    "subscribe:",
    "_steam",
    "packageId",
    "<ModMetaData>",
    "<name>",
    "modDependencies",
    "\0",
];

fn text(random: &mut Random) -> String {
    let pieces = random.below(40);
    (0..pieces)
        .map(|_| PIECES[random.below(PIECES.len())])
        .collect()
}

fn panics_on(rounds: usize, seed: u64, probe: impl Fn(&str)) -> Vec<String> {
    let mut random = Random(seed);
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let failed: Vec<String> = (0..rounds)
        .map(|_| text(&mut random))
        .filter(|made| catch_unwind(AssertUnwindSafe(|| probe(made))).is_err())
        .collect();
    std::panic::set_hook(quiet);
    failed
}

fn a_mod(name: &str, category: Category) -> Mod {
    Mod {
        id: name.to_lowercase(),
        name: name.to_string(),
        author: "someone".to_string(),
        version: "1.0".to_string(),
        game: String::new(),
        category,
        installed: None,
        tags: Vec::new(),
        local: false,
        workshop: false,
        git: false,
        code: false,
        patched: std::sync::Arc::from(Vec::<String>::new()),
        defs: std::sync::Arc::from(Vec::<String>::new()),
        sub: String::new(),
        colour: String::new(),
        pinned: false,
        load_micros: None,
        updated: None,
        downloaded: None,
        supports: Vec::new(),
        needs_dlcs: Vec::new(),
        bytes: None,
        translation: false,
    }
}

#[test]
fn no_text_parser_panics_on_thirty_thousand_made_up_texts() {
    let failed = panics_on(30_000, 0x5eed, |made| {
        let _ = modsconfig::parse(made);
        let _ = modsconfig::with_active(made, &["a.b".to_string()]);
        let _ = modsconfig::short_version(made);
        let _ = markup::rich(made);
        let _ = vdf::libraries(made);
        let _ = about::parse(made.as_bytes());
        let _ = logs::errors(made);
        let _ = logs::game_version(made);
        let _ = logs::mentioned(
            made,
            &[("İstanbul mod".to_string(), 1), ("日本 mod".into(), 2)],
        );
        let _ = changes::steam_notes(made);
        let _ = workshop::ids_in(made);
        let _ = workshop::parse_details(made);
        let _ = workshop::parse_collections(made);
        let _ = workshop::installed_times(made);
        let _ = workshop::installed_sizes(made);
        let _ = share::ids_in(made);
        let _ = loadfolders::declared(made, "1.6");
        let _ = rimsaves::parse_settings(made);
        let _ = rimsaves::parse_meta(made);
        let _ = rimsaves::split(made);
        let _ = community::parse(made);
        let _ = community::works_on(made);
        let _ = notes::tags_in(made);
        let _ = steamworks::parse(made);
        let _ = owntheme::contrast(made, "#000000");
        let _ = owntheme::contrast(&format!("#{made}"), "#000000");
        let _ = ddsaudit::problem(made.as_bytes(), made.len() as u64);
    });
    assert!(failed.is_empty(), "panicked on {:?}", failed.first());
}

#[test]
fn the_filter_box_takes_any_text() {
    let mods: Vec<Mod> = ["Harmony", "İstanbul", "日本 mod", "Core"]
        .iter()
        .map(|name| a_mod(name, Category::Content))
        .collect();
    let failed = panics_on(20_000, 0xf117e4, |made| {
        let mut list = GroupedList::new(mods.clone());
        list.set_filter(made);
        let _ = list.matching();
        let _ = list.rows().len();
    });
    assert!(failed.is_empty(), "panicked on {:?}", failed.first());
}

#[test]
#[ignore = "needs RIMSTACK_DLL: a mod's assembly"]
fn a_damaged_real_assembly_is_refused_or_read_and_never_panics() {
    let path = std::env::var_os("RIMSTACK_DLL").expect("RIMSTACK_DLL names a mod's assembly");
    let sound = std::fs::read(path).unwrap();
    assert!(dotnet::Assembly::read(&sound).is_some());
    let mut random = Random(0xd11);
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let (mut panicked, mut read, mut slowest) = (0, 0, Duration::ZERO);
    for round in 0..4_000 {
        let mut dll = sound.clone();
        match round % 4 {
            0 => {
                let cut = random.below(dll.len());
                dll.truncate(cut);
            }
            1 => {
                // Counts and offsets are four bytes.
                for _ in 0..1 + random.below(4) {
                    let at = random.below(dll.len() - 4);
                    dll[at..at + 4].copy_from_slice(&[0xff, 0xff, 0xff, 0x7f]);
                }
            }
            _ => {
                for _ in 0..1 + random.below(24) {
                    let span = if random.below(3) == 0 {
                        dll.len()
                    } else {
                        dll.len() / 16
                    };
                    let at = random.below(span.max(1));
                    dll[at] = random.next() as u8;
                }
            }
        }
        let started = Instant::now();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let Some(assembly) = dotnet::Assembly::read(&dll) else {
                return false;
            };
            let _ = assembly.references();
            let _ = assembly.types();
            let _ = assembly.patches();
            let _ = assembly.hand_applied();
            let _ = assembly.looked_up();
            true
        }));
        slowest = slowest.max(started.elapsed());
        match outcome {
            Ok(true) => read += 1,
            Ok(false) => {}
            Err(_) => panicked += 1,
        }
    }
    std::panic::set_hook(quiet);
    println!(
        "assembly fuzz: 4000 damaged files, {read} still read, {panicked} panics, slowest {slowest:?}"
    );
    assert_eq!(panicked, 0);
    assert!(
        slowest < Duration::from_secs(2),
        "one reading took {slowest:?}"
    );
}

#[test]
#[ignore = "needs RIMSTACK_DLLS: a file listing installed assemblies"]
fn installed_assemblies_are_read() {
    let listing =
        std::env::var_os("RIMSTACK_DLLS").expect("RIMSTACK_DLLS names a list of assemblies");
    let listing = std::fs::read_to_string(listing).unwrap();
    let (mut read, mut refused, mut panicked) = (0, Vec::new(), Vec::new());
    let started = Instant::now();
    for path in listing.lines().filter(|line| !line.is_empty()) {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let assembly = dotnet::Assembly::read(&bytes)?;
            Some((assembly.types().len(), assembly.patches().len()))
        }));
        match outcome {
            Ok(Some(_)) => read += 1,
            // A native library has no .NET header.
            Ok(None) => refused.push(path),
            Err(_) => panicked.push(path),
        }
    }
    println!(
        "assemblies: {read} read, {} refused, {} panicked, in {:?}",
        refused.len(),
        panicked.len(),
        started.elapsed()
    );
    for path in refused.iter().take(8) {
        println!("  refused: {path}");
    }
    assert!(panicked.is_empty(), "panicked on {:?}", panicked.first());
}

#[test]
fn a_contradiction_among_many_rules_still_sorts_in_reasonable_time() {
    let count = 2_300;
    let mut rules: Vec<Rule> = (0..count - 1)
        .map(|m| Rule {
            first: m,
            then: m + 1,
            kind: Kind::LoadAfter,
        })
        .collect();
    for bottom in 0..5 {
        rules.extend((5..count).map(|above| Rule {
            first: above,
            then: bottom,
            kind: Kind::CommunityBottom,
        }));
    }
    let started = Instant::now();
    let sorted = sort::sort(count, &rules);
    let took = started.elapsed();
    println!(
        "sort with a loop: {count} mods, {} rules, {} dropped, {took:?}",
        rules.len(),
        sorted.dropped.len()
    );
    assert_eq!(sorted.order.len(), count);
    assert!(took < Duration::from_secs(5), "took {took:?}");
}

/// A line, a count and a nesting no file of a mod, of Steam, of the game or of RimStack's own
/// should hold, and the time a reader may take over one on a debug build: far above what a
/// reader that is linear in its file needs, far below what a quadratic one does.
const HUGE: usize = 50 << 20;
const MANY: usize = 1_000_000;
const DEEP: usize = 100_000;
const BOUND: Duration = Duration::from_secs(120);

/// Runs `probe` on a thread of its own: `Err` when it panics or is still running after `limit`.
/// A probe that overruns is left running; the test fails anyway.
fn bounded(limit: Duration, probe: impl FnOnce() + Send + 'static) -> Result<Duration, String> {
    let (done, finished) = std::sync::mpsc::channel();
    let started = Instant::now();
    std::thread::spawn(move || {
        let outcome = catch_unwind(AssertUnwindSafe(probe)).map_err(|panic| {
            let said = panic.downcast_ref::<&str>().map(|s| s.to_string());
            said.or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_default()
        });
        let _ = done.send(outcome);
    });
    match finished.recv_timeout(limit) {
        Ok(Ok(())) => Ok(started.elapsed()),
        Ok(Err(said)) => Err(format!("panicked: {said}")),
        Err(_) => Err(format!("still running after {limit:?}")),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Format {
    Xml,
    Json,
    Text,
}

/// A file read at start, where the reader looks for it inside the folder it is given.
struct Reader {
    name: &'static str,
    file: &'static str,
    format: Format,
    /// A well-formed file of `entries`.
    made: fn(&[String]) -> String,
    read: fn(&Path),
}

fn lis(entries: &[String]) -> String {
    entries.iter().map(|e| format!("<li>{e}</li>")).collect()
}

fn quoted(entries: &[String]) -> String {
    let each: Vec<String> = entries.iter().map(|e| format!("\"{e}\"")).collect();
    each.join(",")
}

fn keyed(entries: &[String], value: &str) -> String {
    let each: Vec<String> = entries.iter().map(|e| format!("\"{e}\":{value}")).collect();
    each.join(",")
}

fn objects(entries: &[String], shape: impl Fn(&String) -> String) -> String {
    let each: Vec<String> = entries.iter().map(shape).collect();
    each.join(",")
}

fn lines(entries: &[String], shape: impl Fn(&String) -> String) -> String {
    let each: Vec<String> = entries.iter().map(shape).collect();
    each.join("\n")
}

fn in_mods(dir: &Path) -> Vec<rimstack_core::mods::library::Installed> {
    let root = ModRoot {
        path: dir.join("Mods"),
        source: Source::Local,
    };
    rimstack_core::mods::library::scan(&[root])
}

fn text_of(file: &Path) -> String {
    std::fs::read_to_string(file).unwrap_or_default()
}

fn readers() -> Vec<Reader> {
    vec![
        Reader {
            name: "About.xml",
            file: "Mods/m/About/About.xml",
            format: Format::Xml,
            made: |e| {
                format!(
                    "<?xml version=\"1.0\" encoding=\"utf-8\"?><ModMetaData><packageId>a.b</packageId>\
                     <name>{}</name><description>{}</description><supportedVersions>{}</supportedVersions>\
                     <modDependencies>{}</modDependencies><loadAfter>{}</loadAfter>\
                     <modDependenciesByVersion><v1.6>{}</v1.6></modDependenciesByVersion></ModMetaData>",
                    e[0],
                    e[0],
                    lis(e),
                    objects(e, |e| format!(
                        "<li><packageId>{e}</packageId><alternativePackageIds>{}</alternativePackageIds></li>",
                        lis(std::slice::from_ref(e))
                    )),
                    lis(e),
                    objects(e, |e| format!("<li><packageId>{e}</packageId></li>")),
                )
            },
            read: |dir| {
                let mods = in_mods(dir);
                let active: Vec<String> = vec!["a.b".into()];
                let _ = rimstack_core::mods::library::build(mods, &active, "1.6");
            },
        },
        Reader {
            name: "PublishedFileId.txt",
            file: "Mods/m/About/PublishedFileId.txt",
            format: Format::Text,
            made: |e| e.join("\n"),
            read: |dir| {
                let _ = about::published_file_id(&dir.join("Mods/m"));
            },
        },
        Reader {
            name: "LoadFolders.xml",
            file: "Mods/m/LoadFolders.xml",
            format: Format::Xml,
            made: |e| {
                let marked: String = e
                    .iter()
                    .map(|e| format!("<li IfModActive=\"{e}\">{e}</li>"))
                    .collect();
                format!(
                    "<loadFolders><v1.6>{marked}</v1.6><default>{}</default></loadFolders>",
                    lis(e)
                )
            },
            read: |dir| {
                let _ = loadfolders::checked(&dir.join("Mods/m"), "1.6", &|_| true);
            },
        },
        Reader {
            name: "ModsConfig.xml",
            file: "ModsConfig.xml",
            format: Format::Xml,
            made: |e| {
                format!(
                    "<?xml version=\"1.0\" encoding=\"utf-8\"?><ModsConfigData><version>1.6.4871 rev600</version>\
                     <activeMods>{}</activeMods><knownExpansions>{}</knownExpansions></ModsConfigData>",
                    lis(e),
                    lis(e)
                )
            },
            read: |dir| {
                if let Ok(config) = modsconfig::read(&dir.join("ModsConfig.xml")) {
                    let _ = modsconfig::short_version(config.version.as_deref().unwrap_or(""));
                    let _ = rimstack_core::mods::share_file::modsconfig_xml(&config.active, "1.6");
                }
            },
        },
        Reader {
            name: "a save's header",
            file: "Saves/a.rws",
            format: Format::Xml,
            made: |e| {
                format!(
                    "<?xml version=\"1.0\" encoding=\"utf-8\"?><savegame><meta><gameVersion>{}</gameVersion>\
                     <modIds>{}</modIds><modNames>{}</modNames></meta><game/></savegame>",
                    e[0],
                    lis(e),
                    lis(e)
                )
            },
            read: |dir| {
                let _ = saves::list(&dir.join("Saves"));
            },
        },
        Reader {
            name: "RimSaves' settings",
            file: "Config/Mod_1_RimSaves.xml",
            format: Format::Xml,
            made: |e| {
                format!(
                    "<SettingsBlock><ModSettings><folders>{}</folders><curFolder>{}</curFolder></ModSettings></SettingsBlock>",
                    lis(e),
                    e[0]
                )
            },
            read: |dir| {
                let _ = rimsaves::settings(dir);
            },
        },
        Reader {
            name: "RimSaves' save facts",
            file: "RimSaves/Meta/a.dat",
            format: Format::Text,
            made: |e| objects(e, |e| format!("{e}:%41{e}")),
            read: |dir| {
                let _ = rimsaves::facts(&rimsaves::meta(dir, "a"));
            },
        },
        Reader {
            name: "a RimSort list (.json)",
            file: "list.json",
            format: Format::Json,
            made: |e| format!("{{\"activeMods\":[{}]}}", quoted(e)),
            read: |dir| {
                let _ = share_file::file(&dir.join("list.json"));
            },
        },
        Reader {
            name: "a mod list (.rml)",
            file: "list.rml",
            format: Format::Xml,
            made: |e| {
                format!(
                    "<savedModList><meta><modIds>{}</modIds></meta></savedModList>",
                    lis(e)
                )
            },
            read: |dir| {
                let _ = share_file::file(&dir.join("list.rml"));
            },
        },
        Reader {
            name: "a RimPy list (.xml)",
            file: "list.xml",
            format: Format::Xml,
            made: |e| {
                format!(
                    "<ModsConfigData><activeMods>{}</activeMods></ModsConfigData>",
                    lis(e)
                )
            },
            read: |dir| {
                let _ = share_file::file(&dir.join("list.xml"));
            },
        },
        Reader {
            name: "a list as text (.txt)",
            file: "list.txt",
            format: Format::Text,
            made: |e| lines(e, |e| format!("{e} [{e}]")),
            read: |dir| {
                let _ = share_file::file(&dir.join("list.txt"));
            },
        },
        Reader {
            name: "a share code",
            file: "code.txt",
            format: Format::Text,
            made: |e| {
                let entries: Vec<share::Entry> = e
                    .iter()
                    .map(|id| share::Entry {
                        name: id.clone(),
                        package_id: id.clone(),
                        workshop_id: Some("2009463077".into()),
                        url: None,
                    })
                    .collect();
                share::code(&entries)
            },
            read: |dir| {
                let _ = share::ids_in(&text_of(&dir.join("code.txt")));
            },
        },
        Reader {
            name: "Player.log",
            file: "Player.log",
            format: Format::Text,
            made: |e| {
                lines(e, |e| {
                    format!("Exception in {e}.Patch: {e}\n  at {e}.Type.Method ()")
                })
            },
            read: |dir| {
                let Ok(text) = logs::read(&dir.join("Player.log")) else {
                    return;
                };
                let _ = logs::game_version(&text);
                let _ = logs::levels(&text);
                let owners = std::collections::HashMap::from([("author.mod".to_string(), 1)]);
                for group in logs::errors(&text) {
                    let _ = logs::mods_in(&group, &owners);
                }
            },
        },
        Reader {
            name: "Steam's appworkshop_294100.acf",
            file: "appworkshop_294100.acf",
            format: Format::Text,
            made: |e| {
                let items: String = e
                    .iter()
                    .map(|e| format!("\"{e}\" {{ \"size\" \"10\" \"timeupdated\" \"20\" }}\n"))
                    .collect();
                format!("\"AppWorkshop\" {{ \"WorkshopItemsInstalled\" {{ {items} }} }}")
            },
            read: |dir| {
                let text = text_of(&dir.join("appworkshop_294100.acf"));
                let _ = workshop::installed_times(&text);
                let _ = workshop::installed_sizes(&text);
            },
        },
        Reader {
            name: "Steam's libraryfolders.vdf",
            file: "libraryfolders.vdf",
            format: Format::Text,
            made: |e| {
                let items = lines(e, |e| {
                    format!("\"{e}\" {{ \"path\" \"/{e}\" \"apps\" {{ \"294100\" \"1\" }} }}")
                });
                format!("\"libraryfolders\" {{ {items} }}")
            },
            read: |dir| {
                let _ = vdf::libraries(&text_of(&dir.join("libraryfolders.vdf")));
            },
        },
        Reader {
            name: "workshopNames.tsv",
            file: "workshopNames.tsv",
            format: Format::Text,
            made: |e| lines(e, |e| format!("{e}\t123\t{e}")),
            read: |dir| {
                let _ =
                    workshopdb::read_with_own(&dir.join("workshopNames.tsv"), &dir.join("own.tsv"));
            },
        },
        Reader {
            name: "community rules",
            file: "communityRules.json",
            format: Format::Json,
            made: |e| {
                let rules = objects(e, |e| {
                    format!(
                        "\"{e}\":{{\"loadAfter\":{{{}}},\"loadBottom\":{{\"value\":true}}}}",
                        keyed(std::slice::from_ref(e), "{}")
                    )
                });
                format!("{{\"timestamp\":1,\"rules\":{{{rules}}}}}")
            },
            read: |dir| {
                if let Ok(Some(rules)) = community::read(&dir.join("communityRules.json")) {
                    let _ = rules.pairs();
                    let _ = rules.rules(&[]);
                }
            },
        },
        Reader {
            name: "NoVersionWarning's list",
            file: "worksOn-1.6.xml",
            format: Format::Xml,
            made: |e| format!("<ModIdsToFix>{}</ModIdsToFix>", lis(e)),
            read: |dir| {
                let _ = community::read_works_on(&dir.join("worksOn-1.6.xml"));
            },
        },
        Reader {
            name: "StartupImpactData.xml",
            file: "StartupImpactData.xml",
            format: Format::Xml,
            made: |e| {
                let mods: String = e
                    .iter()
                    .map(|e| format!("<li><modPackageId>{e}</modPackageId><totalImpact>12.5</totalImpact></li>"))
                    .collect();
                format!(
                    "<StartupImpact><loadingTime>1000</loadingTime><mods>{mods}</mods></StartupImpact>"
                )
            },
            read: |dir| {
                let _ = startup::read(dir);
            },
        },
        Reader {
            name: "a mod's .git/config",
            file: "Mods/m/.git/config",
            format: Format::Text,
            made: |e| {
                format!(
                    "[remote \"origin\"]\n{}",
                    lines(e, |e| format!("url = https://{e}"))
                )
            },
            read: |dir| {
                let _ = git::page(&dir.join("Mods/m"));
            },
        },
        Reader {
            name: "a mod's patches",
            file: "Mods/m/Patches/a.xml",
            format: Format::Xml,
            made: |e| {
                let ops: String = e
                    .iter()
                    .map(|e| format!(
                        "<li Class=\"PatchOperationFindMod\"><mods><li>{e}</li></mods><match Class=\"PatchOperationReplace\">\
                         <xpath>Defs/ThingDef[defName=\"{e}\"]/label</xpath></match></li>"
                    ))
                    .collect();
                format!(
                    "<Patch><Operation Class=\"PatchOperationSequence\"><operations>{ops}</operations></Operation></Patch>"
                )
            },
            read: |dir| {
                let file = dir.join("Mods/m/Patches/a.xml");
                let mut read = xmlcheck::Read::default();
                xmlcheck::read_patches(&text_of(&file), &file, &mut read);
                xmlcheck::read_defs(&text_of(&file), &file, &mut read);
            },
        },
        Reader {
            name: "RimPy's config.ini",
            file: "config.ini",
            format: Format::Text,
            made: |e| format!("[Colors]\n{}", lines(e, |e| format!("{e} = #ff00ff"))),
            read: |dir| {
                let _ = foreign::rimpy(&text_of(&dir.join("config.ini")));
            },
        },
        Reader {
            name: "orders.json",
            file: "orders.json",
            format: Format::Json,
            made: |e| format!("[{{\"time\":1,\"how\":\"saved\",\"ids\":[{}]}}]", quoted(e)),
            read: |dir| {
                let _ = orders::read(&dir.join("orders.json"));
            },
        },
        Reader {
            name: "journal.json",
            file: "journal.json",
            format: Format::Json,
            made: |e| {
                let entries = objects(e, |e| {
                    format!("{{\"time\":1,\"what\":\"[\\\"{e}\\\"]\",\"revert\":\"list {e}\"}}")
                });
                format!("[{entries}]")
            },
            read: |dir| {
                for entry in journal::read(&dir.join("journal.json")) {
                    let _ = Sentence::from_kept(&entry.what).to_string();
                }
            },
        },
        Reader {
            name: "notes.json",
            file: "notes.json",
            format: Format::Json,
            made: |e| {
                let notes = objects(e, |e| {
                    format!(
                        "\"{e}\":{{\"tags\":[\"{e}\"],\"note\":\"{e}\",\"versionOk\":true,\"category\":\"{e}\",\"ignored\":[\"{e}\"],\"colour\":\"#ff00ff\",\"pinned\":true,\"sub\":\"{e}\"}}"
                    )
                });
                format!("{{{notes}}}")
            },
            read: |dir| {
                let notes = notes::Notes::read(&dir.join("notes.json"));
                let _ = notes.tags_for("a", &[], 10);
            },
        },
        Reader {
            name: "settings.json",
            file: "settings.json",
            format: Format::Json,
            made: |e| format!("{{{}}}", objects(e, |e| format!("\"{e}\":\"{e}\""))),
            read: |dir| {
                let _ = settings::Settings::read(&dir.join("settings.json"));
            },
        },
        Reader {
            name: "launches.json",
            file: "launches.json",
            format: Format::Json,
            made: |e| {
                format!(
                    "[{}]",
                    objects(e, |e| format!(
                        "{{\"time\":1,\"ids\":[\"{e}\"],\"errors\":2}}"
                    ))
                )
            },
            read: |dir| {
                let record = launches::read(&dir.join("launches.json"));
                let _ = launches::last_clean(&record);
            },
        },
        Reader {
            name: "updates.json",
            file: "updates.json",
            format: Format::Json,
            made: |e| {
                format!(
                    "{{\"checked\":1,\"asked\":2,\"now\":{{{}}},\"gone\":[{}],\"checkouts\":{{{}}}}}",
                    keyed(e, "5"),
                    quoted(e),
                    keyed(e, "{\"behind\":1,\"problem\":\"x\"}")
                )
            },
            read: |dir| {
                let _ = updates::LastCheck::read(&dir.join("updates.json"));
            },
        },
        Reader {
            name: "steamcmd.json",
            file: "steamcmd.json",
            format: Format::Json,
            made: |e| format!("{{\"at\":{{{}}}}}", keyed(e, "5")),
            read: |dir| {
                let _ = steamcmd::Downloaded::read(&dir.join("steamcmd.json"));
            },
        },
        Reader {
            name: "textures.json",
            file: "textures.json",
            format: Format::Json,
            made: |e| {
                format!(
                    "{{{}}}",
                    keyed(e, "{\"len\":1,\"modified\":2,\"output\":\"ff\"}")
                )
            },
            read: |dir| {
                let _ = textures::Manifest::read(&dir.join("textures.json"));
            },
        },
        Reader {
            name: "hidden.json",
            file: "hidden.json",
            format: Format::Json,
            made: |e| format!("{{{}}}", keyed(e, "5")),
            read: |dir| {
                let _ = hidden::Hidden::read(&dir.join("hidden.json"));
            },
        },
        Reader {
            name: "rules.json",
            file: "rules.json",
            format: Format::Json,
            made: |e| {
                format!(
                    "[{}]",
                    objects(e, |e| format!("{{\"first\":\"{e}\",\"then\":\"b.{e}\"}}"))
                )
            },
            read: |dir| {
                let rules = userrules::UserRules::read(&dir.join("rules.json"));
                let _ = rules.rules(&[]);
            },
        },
        Reader {
            name: "a saved list",
            file: "lists/a.json",
            format: Format::Json,
            made: |e| format!("{{\"name\":\"a\",\"ids\":[{}]}}", quoted(e)),
            read: |dir| {
                let _ = lists::all(&dir.join("lists"));
                let _ = lists::load(&dir.join("lists"), "a");
            },
        },
        Reader {
            name: "a saved list's settings",
            file: "lists/a.settings/settings.json",
            format: Format::Json,
            made: |e| {
                let files = objects(e, |e| {
                    format!("{{\"file\":\"Mod_{e}_S.xml\",\"folder\":\"{e}\",\"mod\":\"{e}\"}}")
                });
                format!("{{\"settings\":[{files}]}}")
            },
            read: |dir| {
                let _ = lists::kept_settings(&dir.join("lists"), "a");
            },
        },
        Reader {
            name: "bisect.json",
            file: "bisect.json",
            format: Format::Json,
            made: |e| {
                let at: Vec<String> = (0..e.len()).map(|at| at.to_string()).collect();
                let none = vec!["[]"; e.len()].join(",");
                format!(
                    "{{\"original\":[{}],\"suspects\":[{}],\"seeded\":[{}],\"requires\":[{none}]}}",
                    quoted(e),
                    at.join(","),
                    at.join(",")
                )
            },
            read: |dir| {
                // What the app shows of a search under way as it starts.
                if let Some(search) = bisect::Bisect::read(&dir.join("bisect.json")) {
                    let _ = (search.found(), search.rounds_left(), search.seeded());
                    let _ = (search.is_stuck(), search.list());
                }
            },
        },
        Reader {
            name: "changes.json",
            file: "changes.json",
            format: Format::Json,
            made: |e| format!("{{\"time\":1,\"workshop\":{{{}}}}}", keyed(e, "5")),
            read: |dir| {
                let _ = changes::AtLaunch::read(&dir.join("changes.json"));
            },
        },
        Reader {
            name: "a lockfile",
            file: "rimstack.lock.json",
            format: Format::Json,
            made: |e| {
                let mods = objects(e, |e| {
                    format!(
                        "{{\"packageId\":\"{e}\",\"source\":\"workshop\",\"workshop\":\"1\",\"updated\":2,\"version\":\"{e}\"}}"
                    )
                });
                format!(
                    "{{\"game\":\"1.6\",\"dlcs\":[{}],\"mods\":[{mods}]}}",
                    quoted(e)
                )
            },
            read: |dir| {
                let _ = lock::read(&dir.join("rimstack.lock.json"));
            },
        },
        Reader {
            name: "seen.txt",
            file: "seen.txt",
            format: Format::Text,
            made: |e| e.join("\n"),
            read: |dir| {
                let _ = new_mods::Seen::read(&dir.join("seen.txt")).is_new("a");
            },
        },
        Reader {
            name: "replacements.json.gz",
            file: "replacements.json.gz",
            format: Format::Json,
            made: |e| {
                let rules = objects(e, |e| {
                    format!(
                        "{{\"oldWorkshopId\":\"{e}\",\"newWorkshopId\":\"1\",\"newName\":\"{e}\",\"newVersions\":[\"1.6\"]}}"
                    )
                });
                format!("{{\"rules\":[{rules}]}}")
            },
            read: |dir| {
                let _ = replacements::read(&dir.join("replacements.json.gz"));
            },
        },
        Reader {
            name: "own translations",
            file: "words.json",
            format: Format::Json,
            made: |e| format!("{{{}}}", objects(e, |e| format!("\"{e}\":\"{e}\""))),
            read: |dir| {
                let _ =
                    rimstack_core::base::translate::table("{}", &text_of(&dir.join("words.json")));
            },
        },
    ]
}

/// Every type of `value` turned into another, one way per `how`.
fn swapped(value: &Value, how: usize) -> Value {
    match (value, how) {
        (Value::Array(items), 2) => Value::Object(
            items
                .iter()
                .enumerate()
                .map(|(at, v)| (at.to_string(), swapped(v, how)))
                .collect(),
        ),
        (Value::Object(pairs), 3) => {
            Value::Array(pairs.values().map(|v| swapped(v, how)).collect())
        }
        (Value::Array(items), _) => Value::Array(items.iter().map(|v| swapped(v, how)).collect()),
        (Value::Object(pairs), _) => Value::Object(
            pairs
                .iter()
                .map(|(k, v)| (k.clone(), swapped(v, how)))
                .collect(),
        ),
        (Value::String(_), 0) => Value::from(7),
        (Value::Number(n), 1) => Value::from(n.to_string()),
        (_, 4) => Value::Null,
        (Value::Number(_), 5) => Value::from(-1),
        (Value::Number(_), 6) => Value::from(1e300),
        (Value::Number(_), 7) => Value::from(u32::MAX),
        (Value::Bool(b), 8) => Value::from(u8::from(*b)),
        (other, _) => other.clone(),
    }
}

fn packed(plain: &[u8]) -> Vec<u8> {
    let mut packed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    std::io::Write::write_all(&mut packed, plain).unwrap();
    packed.finish().unwrap()
}

enum Put {
    Missing,
    Folder,
    Bytes(Vec<u8>),
}

/// What a damaged, odd or hostile copy of `reader`'s file holds.
fn damaged(reader: &Reader) -> Vec<(String, Put)> {
    let sample: Vec<String> = ["first.mod", "second.BROKEN", "third.mod"]
        .map(String::from)
        .into();
    let good = (reader.made)(&sample);
    let invalid: &[u8] = &[0xFF, 0xC3, 0x28];
    let not_utf8 = match good.split_once("BROKEN") {
        Some((before, after)) => [before.as_bytes(), invalid, after.as_bytes()].concat(),
        None => [good.as_bytes(), invalid].concat(),
    };
    let utf16 = good.encode_utf16().flat_map(u16::to_le_bytes);
    let mut cases: Vec<(String, Put)> = vec![
        ("missing".into(), Put::Missing),
        ("a folder".into(), Put::Folder),
        ("empty".into(), Put::Bytes(Vec::new())),
        (
            "truncated".into(),
            Put::Bytes(good.as_bytes()[..good.len() / 2].to_vec()),
        ),
        (
            "only a byte order mark".into(),
            Put::Bytes(vec![0xEF, 0xBB, 0xBF]),
        ),
        ("nothing but zeros".into(), Put::Bytes(vec![0; 4096])),
        ("not UTF-8".into(), Put::Bytes(not_utf8)),
        (
            "UTF-16 with a byte order mark".into(),
            Put::Bytes([0xFF, 0xFE].into_iter().chain(utf16).collect()),
        ),
    ];
    // From here on what a packed file holds, packed: the damage above is to the file itself.
    let content = cases.len();
    let deep = match reader.format {
        Format::Xml => (reader.made)(&[format!(
            "{}x{}",
            "<li Class=\"PatchOperationSequence\"><operations>".repeat(DEEP),
            "</operations></li>".repeat(DEEP)
        )]),
        Format::Json => format!("{}{}", "[{\"a\":".repeat(DEEP), "}]".repeat(DEEP)),
        Format::Text => (reader.made)(&["[{<(".repeat(DEEP)]),
    };
    cases.push(("deep".into(), Put::Bytes(deep.into_bytes())));
    let huge = (reader.made)(&[format!("huge.{}", "x".repeat(HUGE))]);
    cases.push(("a huge line".into(), Put::Bytes(huge.into_bytes())));
    let many: Vec<String> = (0..MANY).map(|n| format!("author{n}.mod{n}")).collect();
    cases.push((
        "a million entries".into(),
        Put::Bytes((reader.made)(&many).into_bytes()),
    ));
    if reader.format == Format::Json {
        let shaped: Value = serde_json::from_str(&good).expect(reader.name);
        for how in 0..9 {
            let wrong = swapped(&shaped, how).to_string();
            cases.push((format!("wrong types {how}"), Put::Bytes(wrong.into_bytes())));
        }
        for whole in [
            "null",
            "0",
            "\"x\"",
            "[]",
            "{}",
            "[[]]",
            "{\"\":{}}",
            "1e999",
        ] {
            cases.push((
                format!("only {whole}"),
                Put::Bytes(whole.as_bytes().to_vec()),
            ));
        }
    }
    if reader.file.ends_with(".gz") {
        for (_, put) in &mut cases[content..] {
            if let Put::Bytes(plain) = put {
                *plain = packed(plain);
            }
        }
        let whole = packed(good.as_bytes());
        cases.push((
            "truncated once packed".into(),
            Put::Bytes(whole[..whole.len() / 2].to_vec()),
        ));
    }
    cases
}

#[test]
fn every_file_read_at_start_takes_damage_without_panicking_or_hanging() {
    let readers = readers();
    let next = AtomicUsize::new(0);
    let failed: Vec<String> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    let mut failed = Vec::new();
                    while let Some(reader) = readers.get(next.fetch_add(1, Ordering::Relaxed)) {
                        for (case, put) in damaged(reader) {
                            let dir = tempfile::tempdir().unwrap();
                            let file = dir.path().join(reader.file);
                            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                            match put {
                                Put::Missing => {}
                                Put::Folder => std::fs::create_dir(&file).unwrap(),
                                Put::Bytes(bytes) => std::fs::write(&file, bytes).unwrap(),
                            }
                            let read = reader.read;
                            let at = dir.path().to_path_buf();
                            match bounded(BOUND, move || read(&at)) {
                                Ok(took) => println!("{}, {case}: {took:?}", reader.name),
                                Err(problem) => {
                                    failed.push(format!("{}, {case}: {problem}", reader.name));
                                    // The probe may still be reading the folder.
                                    std::mem::forget(dir);
                                }
                            }
                        }
                    }
                    failed
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect()
    });
    assert!(failed.is_empty(), "{failed:#?}");
}

/// Each piece repeated into a long text, through every parser of text from outside: one that
/// looks ahead for a closing piece from every opening one takes quadratic time.
#[test]
fn no_text_parser_takes_quadratic_time_over_a_long_repeat() {
    let length = 1 << 20;
    type Parse = fn(&str);
    let parsers: Vec<(&str, Parse)> = vec![
        ("modsconfig::parse", |t| {
            let _ = modsconfig::parse(t);
        }),
        ("markup::rich", |t| {
            let _ = markup::rich(t);
        }),
        ("vdf::libraries", |t| {
            let _ = vdf::libraries(t);
        }),
        ("about::parse", |t| {
            let _ = about::parse(t.as_bytes());
        }),
        ("logs::errors", |t| {
            let owners = std::collections::HashMap::from([("author.mod".to_string(), 1)]);
            for group in logs::errors(t) {
                let _ = logs::mods_in(&group, &owners);
            }
        }),
        ("logs::levels", |t| {
            let _ = logs::levels(t);
        }),
        ("logs::game_version", |t| {
            let _ = logs::game_version(t);
        }),
        ("logs::mentioned", |t| {
            let _ = logs::mentioned(
                t,
                &[("İstanbul mod".to_string(), 1), ("日本 mod".into(), 2)],
            );
        }),
        ("changes::steam_notes", |t| {
            let _ = changes::steam_notes(t);
        }),
        ("workshop::ids_in", |t| {
            let _ = workshop::ids_in(t);
        }),
        ("workshop::parse_details", |t| {
            let _ = workshop::parse_details(t);
        }),
        ("workshop::parse_search", |t| {
            let _ = workshop::parse_search(t);
        }),
        ("workshop::parse_collections", |t| {
            let _ = workshop::parse_collections(t);
        }),
        ("workshop::hidden_in", |t| {
            let _ = workshop::hidden_in(t);
        }),
        ("workshop::installed_times", |t| {
            let _ = workshop::installed_times(t);
        }),
        ("share::ids_in", |t| {
            let _ = share::ids_in(t);
        }),
        ("share_file::ids_from_text", |t| {
            let _ = share_file::ids_from_text(t);
        }),
        ("loadfolders::declared", |t| {
            let _ = loadfolders::declared(t, "1.6");
        }),
        ("rimsaves::parse_settings", |t| {
            let _ = rimsaves::parse_settings(t);
        }),
        ("rimsaves::parse_meta", |t| {
            let _ = rimsaves::parse_meta(t);
        }),
        ("community::parse", |t| {
            let _ = community::parse(t);
        }),
        ("community::works_on", |t| {
            let _ = community::works_on(t);
        }),
        ("notes::tags_in", |t| {
            let _ = notes::tags_in(t);
        }),
        ("steamworks::parse", |t| {
            let _ = steamworks::parse(t);
        }),
        ("startup::parse", |t| {
            let _ = startup::parse(t);
        }),
        ("foreign::rimpy", |t| {
            let _ = foreign::rimpy(t);
        }),
        ("git::links_in", |t| {
            let _ = git::links_in(t);
        }),
        ("release::stable_from", |t| {
            let _ = release::stable_from(t);
        }),
        ("replacements::parse", |t| {
            let _ = replacements::parse(t);
        }),
        ("Sentence::from_kept", |t| {
            let _ = Sentence::from_kept(t).to_string();
        }),
        ("translate::table", |t| {
            let _ = rimstack_core::base::translate::table(t, t);
        }),
        ("xmlcheck::read_patches", |t| {
            let mut read = xmlcheck::Read::default();
            xmlcheck::read_patches(t, Path::new("a.xml"), &mut read);
        }),
    ];
    let extra = [
        ("", "[url]", ""),
        ("", "<a", ""),
        ("", "/n", ""),
        ("", "rimstack-list:1:", ""),
        ("", "\"a.", ""),
        ("", "[b]", ""),
        ("[", "x", ""),
        ("Exception ", "a.", ""),
        ("2026-10-12 19:37:35.060 ERROR ", "a.", ""),
    ];
    // Each piece also with what closes a tag far away at the end.
    let texts: Vec<(String, std::sync::Arc<str>)> = PIECES
        .iter()
        .flat_map(|piece| [("", *piece, ""), ("", *piece, "]>")])
        .chain(extra)
        .map(|(start, piece, end)| {
            let text = format!("{start}{}{end}", piece.repeat(length / piece.len()));
            (
                format!("{start:?}, {piece:?} repeated, {end:?}"),
                text.into(),
            )
        })
        .collect();
    let next = AtomicUsize::new(0);
    let jobs: Vec<(usize, usize)> = (0..parsers.len())
        .flat_map(|p| (0..texts.len()).map(move |t| (p, t)))
        .collect();
    let failed: Vec<String> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    let mut failed = Vec::new();
                    while let Some(&(p, t)) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                        let (name, parse) = parsers[p];
                        let text = texts[t].1.clone();
                        let outcome = bounded(Duration::from_secs(20), move || parse(&text));
                        if let Err(problem) = outcome {
                            failed.push(format!("{name} over {}: {problem}", texts[t].0));
                        }
                    }
                    failed
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect()
    });
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn a_share_code_that_unpacks_to_a_gigabyte_is_refused_quickly() {
    // Zeros pack a thousand to one: a code of a megabyte would unpack to a gigabyte.
    let mut packed = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let zeros = vec![b'a'; 1 << 20];
    for _ in 0..1024 {
        std::io::Write::write_all(&mut packed, &zeros).unwrap();
    }
    let bytes = packed.finish().unwrap();
    let code = format!("rimstack-list:1:{}", url64(&bytes));
    let outcome = bounded(Duration::from_secs(10), move || {
        assert_eq!(share::decode(&code), None);
    });
    assert!(outcome.is_ok(), "{outcome:?}");
}

/// base64url without padding, as share codes write it.
fn url64(bytes: &[u8]) -> String {
    const LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for part in bytes.chunks(3) {
        let n = part
            .iter()
            .enumerate()
            .fold(0u32, |n, (at, b)| n | u32::from(*b) << (16 - at * 8));
        for at in 0..=part.len() {
            out.push(LETTERS[(n >> (18 - at * 6)) as usize & 63] as char);
        }
    }
    out
}
