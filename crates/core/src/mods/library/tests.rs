use super::*;
use std::collections::HashSet;
use std::fs;

fn add_mod(root: &Path, folder: &str, about: &str, dirs: &[&str]) {
    let dir = root.join(folder);
    fs::create_dir_all(dir.join("About")).unwrap();
    fs::write(dir.join("About/About.xml"), about).unwrap();
    for d in dirs {
        fs::create_dir_all(dir.join(d)).unwrap();
    }
}

fn about(id: &str, name: &str, deps: &[&str]) -> String {
    let deps: String = deps
        .iter()
        .map(|d| format!("<li><packageId>{d}</packageId></li>"))
        .collect();
    format!(
        "<ModMetaData><packageId>{id}</packageId><name>{name}</name>\
         <author>Ann</author><modDependencies>{deps}</modDependencies></ModMetaData>"
    )
}

fn sample() -> (tempfile::TempDir, Vec<ModRoot>) {
    let tmp = tempfile::tempdir().unwrap();
    let (data, local, workshop) = (
        tmp.path().join("Data"),
        tmp.path().join("Mods"),
        tmp.path().join("workshop"),
    );
    add_mod(
        &data,
        "Core",
        &about("Ludeon.RimWorld", "Core", &[]),
        &["Defs"],
    );
    add_mod(
        &local,
        "dup",
        &about("Test.Dup", "Dup (local)", &[]),
        &["Defs"],
    );
    add_mod(&local, "broken", "<ModMetaData><name>", &[]);
    fs::create_dir_all(local.join("empty/Defs")).unwrap();
    fs::write(local.join("Place mods here.txt"), "").unwrap();
    add_mod(
        &workshop,
        "100",
        &about("Test.Dup", "Dup (workshop)", &[]),
        &["Defs"],
    );
    add_mod(
        &workshop,
        "200",
        &about("Test.Lib", "Lib", &[]),
        &["1.6/Assemblies"],
    );
    add_mod(
        &workshop,
        "300",
        &about("Test.Art", "Art", &["test.lib"]),
        &["Textures"],
    );
    add_mod(
        &workshop,
        "400",
        &about("Test.Tool", "Tool", &["Test.Lib"]),
        &["Common/Assemblies"],
    );
    let roots = vec![
        ModRoot {
            path: data,
            source: Source::Official,
        },
        ModRoot {
            path: local,
            source: Source::Local,
        },
        ModRoot {
            path: workshop,
            source: Source::Workshop,
        },
    ];
    (tmp, roots)
}

fn ids(list: &[String]) -> Vec<String> {
    list.to_vec()
}

#[test]
fn every_folder_becomes_a_mod_and_files_are_skipped() {
    let (_tmp, roots) = sample();
    let mods = scan(&roots);
    let folders: Vec<&str> = mods.iter().map(|m| m.folder.as_str()).collect();
    assert_eq!(
        folders,
        ["Core", "broken", "dup", "empty", "100", "200", "300", "400"]
    );
}

#[test]
fn a_folder_set_aside_while_a_mod_is_replaced_is_not_a_mod() {
    let (_tmp, roots) = sample();
    let local = &roots[1].path;
    add_mod(
        local,
        "dup.rimstack-old",
        &about("Test.Dup", "Dup", &[]),
        &[],
    );
    add_mod(
        local,
        "dup.rimstack-new",
        &about("Test.Dup", "Dup", &[]),
        &[],
    );
    let mods = scan(&roots);
    assert!(mods.iter().all(|m| !m.folder.contains(".rimstack-")));
}

#[test]
fn unreadable_mods_keep_their_reason() {
    let (_tmp, roots) = sample();
    let mods = scan(&roots);
    let reason = |f: &str| {
        let unread = mods.iter().find(|m| m.folder == f).unwrap();
        unread.about.clone().map_err(|problem| problem.to_string())
    };
    assert!(reason("broken").unwrap_err().starts_with("malformed XML"));
    assert_eq!(reason("empty").unwrap_err(), "no About.xml");
    assert_eq!(
        mods.iter().find(|m| m.folder == "empty").unwrap().key(),
        None
    );
}

#[test]
fn content_folders_are_found_one_level_down() {
    let (_tmp, roots) = sample();
    let mods = scan(&roots);
    let kinds = |f: &str| mods.iter().find(|m| m.folder == f).unwrap().kinds;
    assert!(kinds("200").assemblies && !kinds("200").defs);
    assert!(kinds("300").textures);
    assert!(kinds("400").assemblies);
}

#[test]
fn active_ids_resolve_in_order_whatever_their_case() {
    let (_tmp, roots) = sample();
    let mods = scan(&roots);
    let active = ids(&[
        "test.lib".into(),
        "LUDEON.RIMWORLD".into(),
        "gone.mod".into(),
    ]);
    let names: Vec<String> = resolve_active(&active, &mods)
        .iter()
        .map(|r| match r {
            Resolved::Found(m) => m.display_name().to_string(),
            Resolved::Missing(id) => format!("missing {id}"),
        })
        .collect();
    assert_eq!(names, ["Lib", "Core", "missing gone.mod"]);

    let unnamed = |source| Installed {
        path: "/x/Royalty".into(),
        source,
        folder: "Royalty".into(),
        about: about::parse(b"<a><packageId>Ludeon.RimWorld.Royalty</packageId></a>"),
        kinds: Kinds::default(),
        code: Code::default(),
        preview: None,
        git: false,
        published: true,
        named: None,
        downloaded: None,
    };
    assert_eq!(unnamed(Source::Official).display_name(), "Royalty");
    assert_eq!(
        unnamed(Source::Local).display_name(),
        "Ludeon.RimWorld.Royalty"
    );
}

#[test]
fn duplicate_id_prefers_local_and_steam_suffix_picks_workshop() {
    let (_tmp, roots) = sample();
    let mods = scan(&roots);
    let active = ids(&["test.dup".into(), "test.dup_steam".into()]);
    let sources: Vec<Source> = resolve_active(&active, &mods)
        .iter()
        .map(|r| match r {
            Resolved::Found(m) => m.source,
            Resolved::Missing(_) => panic!("missing"),
        })
        .collect();
    assert_eq!(sources, [Source::Local, Source::Workshop]);
}

#[test]
fn library_splits_active_and_inactive() {
    let (_tmp, roots) = sample();
    let mods = scan(&roots);
    let active = ids(&[
        "ludeon.rimworld".into(),
        "test.dup".into(),
        "gone.mod".into(),
    ]);
    let library = build(mods, &active, "1.6");

    let names = |list: &[Mod]| list.iter().map(|m| m.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&library.active), ["Core", "Dup (local)", "gone.mod"]);
    assert_eq!(library.missing, 1);
    assert_eq!(library.unreadable, 2);
    assert_eq!(
        names(&library.inactive),
        ["Lib", "broken", "Dup (workshop)", "empty", "Tool", "Art"]
    );
}

#[test]
fn categories_come_from_source_contents_and_dependents() {
    let (_tmp, roots) = sample();
    let mods = scan(&roots);
    let library = build(mods, &[], "1.6");
    let category = |name: &str| {
        library
            .inactive
            .iter()
            .find(|m| m.name == name)
            .unwrap()
            .category
    };
    assert_eq!(category("Core"), Category::Game);
    assert_eq!(category("Lib"), Category::Libraries);
    assert_eq!(category("Art"), Category::Textures);
    assert_eq!(category("Tool"), Category::Content);
    assert_eq!(category("Dup (local)"), Category::Content);
}

#[test]
fn a_mod_that_asks_to_load_before_the_game_is_a_loader_and_so_is_one_ahead_of_it() {
    let tmp = tempfile::tempdir().unwrap();
    let (data, local) = (tmp.path().join("Data"), tmp.path().join("Mods"));
    let with = |id: &str, name: &str, before: &str| {
        format!(
            "<ModMetaData><packageId>{id}</packageId><name>{name}</name>\
             <loadBefore><li>{before}</li></loadBefore></ModMetaData>"
        )
    };
    add_mod(
        &data,
        "Core",
        &about("Ludeon.RimWorld", "Core", &[]),
        &["Defs"],
    );
    add_mod(
        &local,
        "h",
        &with("brrainz.harmony", "Harmony", "ludeon.rimworld"),
        &["Assemblies"],
    );
    add_mod(
        &local,
        "p",
        &with("zetrith.prepatcher", "Prepatcher", "Brrainz.Harmony"),
        &["Assemblies"],
    );
    add_mod(
        &local,
        "o",
        &with("some.other", "Other", "some.mod"),
        &["Assemblies"],
    );
    let roots = [
        ModRoot {
            path: data,
            source: Source::Official,
        },
        ModRoot {
            path: local,
            source: Source::Local,
        },
    ];
    let library = build(scan(&roots), &[], "1.6");
    let category = |name: &str| {
        let found = library.inactive.iter().find(|m| m.name == name);
        found.unwrap().category
    };
    assert_eq!(category("Core"), Category::Game);
    assert_eq!(category("Harmony"), Category::Loaders);
    assert_eq!(category("Prepatcher"), Category::Loaders);
    assert_eq!(category("Other"), Category::Content);
}

#[test]
fn a_mod_made_for_a_newer_game_supports_at_most_the_installed_one() {
    let tmp = tempfile::tempdir().unwrap();
    let made_for = |id: &str, versions: &[&str]| {
        let versions: String = versions.iter().map(|v| format!("<li>{v}</li>")).collect();
        format!(
            "<ModMetaData><packageId>{id}</packageId><name>{id}</name>\
             <supportedVersions>{versions}</supportedVersions></ModMetaData>"
        )
    };
    add_mod(tmp.path(), "new", &made_for("a.new", &["1.6", "2.0"]), &[]);
    add_mod(tmp.path(), "old", &made_for("a.old", &["1.5"]), &[]);
    add_mod(tmp.path(), "none", &made_for("a.none", &[]), &[]);
    let roots = [ModRoot {
        path: tmp.path().to_path_buf(),
        source: Source::Local,
    }];
    let supports = |game: &str| {
        let library = build(scan(&roots), &[], game);
        let mut found: Vec<_> = (library.inactive.iter())
            .map(|m| (m.name.clone(), m.supports.clone()))
            .collect();
        found.sort();
        found
    };
    let row = |name: &str, v: &[u64]| (name.to_string(), v.to_vec());
    assert_eq!(
        supports("1.7"),
        [
            row("a.new", &[1, 7]),
            row("a.none", &[]),
            row("a.old", &[1, 5])
        ]
    );
    // Without a known game nothing is cut.
    assert_eq!(
        supports(""),
        [
            row("a.new", &[2, 0]),
            row("a.none", &[]),
            row("a.old", &[1, 5])
        ]
    );
}

#[test]
fn arranging_by_the_ids_of_any_split_gives_the_same_split_back() {
    let (_tmp, roots) = sample();
    let library = build(scan(&roots), &["test.art".to_string()], "1.6");
    let mut ids: Vec<String> = library.active.iter().map(|m| m.id.clone()).collect();
    ids.extend(
        library
            .inactive
            .iter()
            .map(|m| m.id.clone())
            .filter(|id| !id.is_empty()),
    );
    ids.push("not.installed".to_string());
    let all = library.arrange(&ids);
    let back: Vec<&str> = all.active.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(back, ids);
    assert_eq!(all.missing, 1);
    for (m, id) in all.active.iter().zip(&ids) {
        if let Some(i) = m.installed {
            assert_eq!(library.written_id(i.index()), *id);
        }
    }
    let folders: HashSet<ModId> = all.active.iter().filter_map(|m| m.installed).collect();
    assert_eq!(folders.len(), ids.len() - 1);
}

#[test]
fn assemblies_are_found_at_the_root_and_one_folder_down() {
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "Assemblies/Main.dll",
        "1.6/Assemblies/Main.dll",
        "1.6/Assemblies/Other.DLL",
        "Assemblies/notes.txt",
    ] {
        let path = dir.path().join("mod").join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }
    fs::create_dir_all(dir.path().join("mod/About")).unwrap();
    fs::write(
        dir.path().join("mod/About/About.xml"),
        about("t.t", "T", &[]),
    )
    .unwrap();
    let roots = [ModRoot {
        path: dir.path().to_path_buf(),
        source: Source::Local,
    }];
    assert_eq!(scan(&roots)[0].assemblies(), ["main", "other"]);
}

#[test]
fn mods_are_found_by_words_of_their_name_the_closest_first() {
    let (_tmp, roots) = sample();
    let library = build(scan(&roots), &[], "1.6");
    let names = |text: &str, most: usize| -> Vec<&str> {
        let found = library.find(text, most);
        found
            .into_iter()
            .map(|i| library.get(i).unwrap().display_name())
            .collect()
    };
    assert_eq!(names("du", 9), ["Dup (local)", "Dup (workshop)"]);
    assert_eq!(names("DUP work", 9), ["Dup (workshop)"]);
    assert_eq!(
        names("o", 9),
        ["Core", "Tool", "broken", "Dup (local)", "Dup (workshop)"]
    );
    assert_eq!(names("o", 2), ["Core", "Tool"]);
    assert_eq!(names("test.art", 9), ["Art"]);
    assert!(names("", 9).is_empty());
    assert!(names("zzz", 9).is_empty());
}

#[test]
fn a_languages_only_mod_translates_what_it_names_or_the_game() {
    let (_tmp, roots) = sample();
    let workshop = roots[2].path.clone();
    add_mod(
        &workshop,
        "500",
        &about(
            "Test.LibRu",
            "Lib Russian",
            &["Ludeon.RimWorld.Royalty", "Test.Lib"],
        ),
        &["Languages"],
    );
    add_mod(
        &workshop,
        "600",
        "<ModMetaData><packageId>Test.GoneCz</packageId><name>Gone Czech</name>\
         <loadAfter><li>Gone.Mod</li></loadAfter></ModMetaData>",
        &["Languages"],
    );
    add_mod(
        &workshop,
        "700",
        &about("Test.CoreEs", "Core Spanish", &[]),
        &["Languages"],
    );
    add_mod(
        &workshop,
        "800",
        &about("Test.BothDe", "Both German", &["Test.Lib"]),
        &["Languages", "Defs"],
    );
    let library = build(scan(&roots), &[], "1.6");
    let says = |folder: &str| {
        let found = library.inactive.iter().find_map(|m| {
            let installed = library.get(m.installed?)?;
            (installed.folder == folder).then(|| (m.translation, library.translated(installed)))
        });
        found.unwrap_or_else(|| panic!("no mod in {folder}"))
    };
    assert_eq!(says("500"), (true, Some(("Lib".to_string(), true))));
    // Only named after the game: it still translates the game and its expansions.
    assert_eq!(says("600"), (true, Some(("Gone.Mod".to_string(), false))));
    assert_eq!(says("700"), (true, Some(("Core".to_string(), true))));
    // More than translations alone is an ordinary mod; so is one without a Languages folder.
    assert!(!says("800").0);
    assert!(!says("300").0);
}

#[test]
fn rows_point_back_at_their_mod_and_requirements_are_named() {
    let (_tmp, roots) = sample();
    let library = build(scan(&roots), &["test.art".to_string()], "1.6");
    let art = library.get(library.active[0].installed.unwrap()).unwrap();
    assert_eq!(art.display_name(), "Art");
    assert_eq!(art.contains().to_string(), "textures");
    assert_eq!(art.source_label(), "Workshop");
    assert_eq!(library.requirements(art), [("Lib".to_string(), true)]);
    for m in &library.inactive {
        assert_eq!(
            library.get(m.installed.unwrap()).unwrap().display_name(),
            m.name
        );
    }
}

#[test]
fn loading_without_a_game_says_what_to_set() {
    let nothing_found = load(&Paths::default(), None).unwrap_err();
    assert_eq!(nothing_found.kind(), crate::base::error::Kind::Refused);
    assert!(nothing_found.to_string().contains("RIMSTACK_GAME"));
}

#[test]
fn an_id_from_an_earlier_reading_finds_nothing_in_the_next() {
    let (_tmp, roots) = sample();
    let before = build(scan(&roots), &["test.art".to_string()], "1.6");
    let art = before.active[0].installed.unwrap();
    assert_eq!(before.get(art).unwrap().display_name(), "Art");
    assert!(before.category_of(art).is_some());

    let after = build(scan(&roots), &["test.art".to_string()], "1.6");
    assert_eq!((after.get(art), after.category_of(art)), (None, None));
    let again = after.active[0].installed.unwrap();
    assert_ne!(art, again);
    assert_eq!(after.get(again).unwrap().display_name(), "Art");
}

#[test]
fn the_active_mods_needing_a_mod_are_listed_for_it() {
    let tmp = tempfile::tempdir().unwrap();
    let local = tmp.path().join("Mods");
    add_mod(&local, "lib", &about("a.lib", "Lib", &[]), &["Defs"]);
    add_mod(&local, "one", &about("a.one", "One", &["a.lib"]), &["Defs"]);
    add_mod(&local, "two", &about("a.two", "Two", &["A.Lib"]), &["Defs"]);
    add_mod(
        &local,
        "versioned",
        "<ModMetaData><packageId>a.versioned</packageId><name>Versioned</name>\
         <modDependenciesByVersion><v1.6><li><packageId>a.lib</packageId></li></v1.6></modDependenciesByVersion>\
         </ModMetaData>",
        &["Defs"],
    );
    add_mod(
        &local,
        "old",
        "<ModMetaData><packageId>a.old</packageId><name>Old</name>\
         <modDependenciesByVersion><v1.5><li><packageId>a.lib</packageId></li></v1.5></modDependenciesByVersion>\
         </ModMetaData>",
        &["Defs"],
    );
    add_mod(&local, "off", &about("a.off", "Off", &["a.lib"]), &["Defs"]);
    let roots = [ModRoot {
        path: local,
        source: Source::Local,
    }];
    let active = ["a.lib", "a.one", "a.two", "a.versioned", "a.old"].map(String::from);
    let library = build(scan(&roots), &active, "1.6");
    let lib = library
        .installed
        .iter()
        .find(|m| m.folder == "lib")
        .unwrap();
    let mut needing: Vec<String> = library
        .needed_by_active(lib)
        .into_iter()
        .map(|n| n.0)
        .collect();
    needing.sort();
    assert_eq!(
        needing,
        ["One", "Two", "Versioned"],
        "the plain and the 1.6 dependencies, not the inactive or the 1.5 one"
    );
}

#[test]
fn a_mod_knows_who_needs_it_and_switching_it_off_takes_them_along() {
    let tmp = tempfile::tempdir().unwrap();
    let local = tmp.path().join("Mods");
    add_mod(&local, "lib", &about("a.lib", "Lib", &[]), &["Defs"]);
    add_mod(&local, "mid", &about("a.mid", "Mid", &["a.lib"]), &["Defs"]);
    add_mod(&local, "top", &about("a.top", "Top", &["a.mid"]), &["Defs"]);
    add_mod(&local, "own", &about("a.own", "Own", &[]), &["Defs"]);
    add_mod(&local, "off", &about("a.off", "Off", &["a.lib"]), &["Defs"]);
    let roots = [ModRoot {
        path: local,
        source: Source::Local,
    }];
    let active = ["a.lib", "a.own", "a.mid", "a.top"].map(String::from);
    let library = build(scan(&roots), &active, "1.6");
    let lib = library
        .installed
        .iter()
        .find(|m| m.folder == "lib")
        .unwrap();
    let mut needing: Vec<String> = library.needed_by(lib).into_iter().map(|n| n.0).collect();
    needing.sort();
    assert_eq!(
        needing,
        ["Mid", "Off"],
        "inactive ones too, only direct ones"
    );
    let names = |off: Vec<bool>| -> Vec<&str> {
        (library.active.iter().zip(off))
            .filter(|(_, off)| *off)
            .map(|(m, _)| m.name.as_str())
            .collect()
    };
    let requires = library.requires_within(&library.active);
    let at = library.active.iter().position(|m| m.name == "Lib").unwrap();
    let mut off = vec![false; library.active.len()];
    off[at] = true;
    assert_eq!(names(with_needing(&requires, off)), ["Lib", "Mid", "Top"]);
}
