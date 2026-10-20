use super::*;
use crate::workspace::tests::installed;
use rimstack_core::mods::list::Row;

fn roads_made_for_an_older_game(tmp: &tempfile::TempDir) {
    let about = "<ModMetaData><packageId>Ann.Roads</packageId><name>Ann.Roads</name><supportedVersions><li>1.5</li></supportedVersions></ModMetaData>";
    std::fs::write(tmp.path().join("game/Mods/roads/About/About.xml"), about).unwrap();
}

fn roads(order: &LoadOrder, side: Side) -> ModId {
    let list = order.list(side);
    let m = list.mods().iter().find(|m| m.id == "ann.roads").unwrap();
    m.installed.unwrap()
}

#[test]
fn the_two_lists_are_the_load_order_and_the_rest() {
    let (_tmp, app) = installed();
    let order = LoadOrder::read(&app, true, Arrange::Category);
    assert!(order.problem.is_empty() && order.is_installed());
    let ids = |side| -> Vec<String> {
        order
            .list(side)
            .mods()
            .iter()
            .map(|m| m.id.clone())
            .collect()
    };
    assert_eq!(ids(Side::Active), ["ludeon.rimworld"]);
    assert_eq!(ids(Side::Inactive), ["ann.roads"]);
    assert!(order.status[0].to_string().contains('1'));
}

#[test]
fn the_load_column_is_offered_only_with_load_times_and_says_why_not() {
    let shown = |kept, times| -> Vec<(&str, bool, bool)> {
        column_choices(kept, times)
            .iter()
            .map(|c| (c.column, c.shown, c.off))
            .collect()
    };
    assert_eq!(
        shown("author,load", false),
        [
            ("author", true, false),
            ("version", false, false),
            ("game", false, false),
            ("size", false, false),
            ("load", false, true)
        ]
    );
    assert_eq!(
        shown("author,load", true),
        [
            ("author", true, false),
            ("version", false, false),
            ("game", false, false),
            ("size", false, false),
            ("load", true, false)
        ]
    );
    assert!(column_note(false).to_string().contains("Loading Progress"));
    assert!(column_note(true).is_empty());
}

#[test]
fn a_side_term_filters_its_own_list_and_leaves_the_other_whole() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    let state = |order: &LoadOrder| {
        let side = |side| {
            let list = order.list(side);
            (list.matching(), list.beyond_filter())
        };
        (side(Side::Active), side(Side::Inactive))
    };
    order.set_filter("is:inactive");
    assert_eq!(state(&order), ((1, true), (1, false)));
    order.set_filter("is:active");
    assert_eq!(state(&order), ((1, false), (1, true)));
    order.set_filter("");
    assert_eq!(state(&order), ((1, false), (1, false)));
}

#[test]
fn mods_that_could_not_be_read_give_empty_lists_and_the_reason() {
    let (tmp, app) = installed();
    std::fs::remove_dir_all(tmp.path().join("game")).unwrap();
    let order = LoadOrder::read(&app, true, Arrange::Category);
    assert!(!order.problem.is_empty() && !order.is_installed());
    assert!(order.list(Side::Active).mods().is_empty());
}

#[test]
fn a_mod_taken_across_is_one_step_that_undo_takes_back() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    let id = roads(&order, Side::Inactive);
    order.take_all(&app, Side::Inactive, &[id]);
    assert_eq!(app.session().current(), ["ludeon.rimworld", "ann.roads"]);
    assert!(order.list(Side::Inactive).mods().is_empty());
    assert!(app.session().is_dirty());
    assert!(order.undo(&app));
    assert_eq!(app.session().current(), ["ludeon.rimworld"]);
    assert_eq!(order.list(Side::Inactive).mods().len(), 1);
    assert!(order.redo(&app));
    assert_eq!(order.list(Side::Active).mods().len(), 2);
}

#[test]
fn a_rule_the_order_breaks_is_a_warning_its_fix_moves_the_mod_for() {
    use rimstack_core::mods::describe::Fix;
    let (tmp, app) = installed();
    let jeep = tmp.path().join("game/Mods/jeep/About");
    std::fs::create_dir_all(&jeep).unwrap();
    let about = "<ModMetaData><packageId>ann.jeep</packageId><name>Jeep</name><loadAfter><li>Ann.Roads</li></loadAfter></ModMetaData>";
    std::fs::write(jeep.join("About.xml"), about).unwrap();
    app.rescan();
    app.session().record(vec![
        "ludeon.rimworld".into(),
        "ann.jeep".into(),
        "ann.roads".into(),
    ]);
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    // [Core, Jeep, Roads]: Jeep's own rule wants it after Roads, so it warns.
    let trouble_at = |order: &LoadOrder, at: usize| {
        order.troubles()[at]
            .iter()
            .find(|t| t.key == "order:ann.roads")
            .cloned()
    };
    let Some(t) = trouble_at(&order, 1) else {
        panic!("{:?}", order.troubles()[1])
    };
    assert!(!t.error);
    assert_eq!(
        t.words.to_string(),
        "Should load after Ann.Roads, by its own rule."
    );
    // The fix names the other mod's place and the side; one move settles it.
    let jeep_id = order.list(Side::Active).mods()[1].installed.unwrap();
    assert_eq!(t.fix, Fix::Move(2, true));
    order.move_next_to(&app, jeep_id, 2, true);
    assert_eq!(
        order.list(Side::Active).mods()[2].id,
        "ann.jeep",
        "{:?}",
        order.list(Side::Active).mods()
    );
    assert!(trouble_at(&order, 2).is_none());
    // And it is one undo step, not a move for each rule.
    assert!(order.undo(&app));
    assert_eq!(order.list(Side::Active).mods()[1].id, "ann.jeep");
    assert!(trouble_at(&order, 1).is_some());
    // A rule no longer broken stops warning even before the next move.
    app.ignore_problem("Ann.Jeep", "order:ann.roads", true);
    order.recheck(&app);
    assert!(trouble_at(&order, 1).is_none());
}

#[test]
fn an_error_ignored_is_gone_until_warned_again() {
    let (tmp, app) = installed();
    let about = "<ModMetaData><packageId>Ann.Roads</packageId><name>Ann.Roads</name><modDependencies><li><packageId>gone.lib</packageId></li></modDependencies></ModMetaData>";
    std::fs::write(tmp.path().join("game/Mods/roads/About/About.xml"), about).unwrap();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    let id = roads(&order, Side::Inactive);
    order.take(&app, Side::Inactive, 0, None);
    let errors = |order: &LoadOrder| {
        order
            .troubles()
            .iter()
            .flatten()
            .filter(|t| t.error)
            .count()
    };
    assert_eq!(errors(&order), 1);
    let key = order.troubles()[1][0].key.clone();
    app.ignore_problem("Ann.Roads", &key, true);
    order.recheck(&app);
    assert_eq!(errors(&order), 0);
    let details = serde_json::to_value(order.details(&app, id)).unwrap();
    assert_eq!(details["problemsIgnored"], 1);
    app.warn_again(&[id]);
    order.recheck(&app);
    assert_eq!(errors(&order), 1);
}

#[test]
fn a_local_mod_naming_no_workshop_item_is_flagged_until_its_warning_is_ignored() {
    let (tmp, app) = installed();
    // The fixture's local mod has no About/PublishedFileId.txt.
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    let id = roads(&order, Side::Inactive);
    order.set_filter("is:incomplete");
    assert_eq!(order.list(Side::Inactive).matching(), 1);
    // In its details it is a problem that can be ignored by the same page.
    let details = serde_json::to_value(order.details(&app, id)).unwrap();
    assert_eq!(details["problems"][0]["ignore"], "no-workshop-id");
    // Once active, the missing property is a warning of its own until ignored.
    order.take(&app, Side::Inactive, 0, None);
    let own = &order.troubles()[1];
    assert_eq!(own.len(), 1, "{own:?}");
    assert_eq!(own[0].key, "no-workshop-id");
    assert_eq!(order.list(Side::Active).matching(), 1);
    app.ignore_problem("Ann.Roads", "no-workshop-id", true);
    order.recheck(&app);
    order.set_filter("is:incomplete");
    assert!(order.troubles()[1].is_empty(), "{:?}", order.troubles()[1]);
    assert_eq!(order.list(Side::Active).matching(), 0);
    // The ignore is kept in RimStack's notes: a new app on the same folder stays quiet.
    drop(order);
    drop(app);
    let again = installed_app_again(&tmp);
    let mut order = LoadOrder::read(&again, true, Arrange::Category);
    order.set_filter("is:incomplete");
    assert_eq!(
        order.list(Side::Active).matching() + order.list(Side::Inactive).matching(),
        0
    );
}

#[test]
fn a_local_copy_has_the_page_of_the_workshop_item_it_is_known_by() {
    use rimstack_core::rimworld::workshopdb;
    let (tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    let id = roads(&order, Side::Inactive);
    let page = |order: &mut LoadOrder| {
        let said = serde_json::to_value(order.details(&app, id)).unwrap();
        (said["workshopPage"].clone(), said["steamPage"].clone())
    };
    assert_eq!(
        page(&mut order),
        (serde_json::Value::Null, serde_json::Value::Null)
    );
    // Known to the Workshop database by its package id.
    std::fs::write(workshopdb::file_in(app.data_dir()), "ann.roads\t555\tRoads").unwrap();
    assert_eq!(
        page(&mut order),
        (
            "https://steamcommunity.com/sharedfiles/filedetails/?id=555".into(),
            "steam://url/CommunityFilePage/555".into()
        )
    );
    // The item the copy names itself wins.
    let named = tmp.path().join("game/Mods/roads/About/PublishedFileId.txt");
    std::fs::write(named, "777\n").unwrap();
    assert_eq!(
        page(&mut order).0,
        "https://steamcommunity.com/sharedfiles/filedetails/?id=777"
    );
}

fn installed_app_again(tmp: &tempfile::TempDir) -> App {
    App::new(
        tmp.path().join("home"),
        &tmp.path().join("settings"),
        &tmp.path().join("data"),
        rimstack_core::rimworld::paths::Overrides {
            game: Some(tmp.path().join("game")),
            config: Some(tmp.path().join("config")),
            workshop: Some(tmp.path().join("workshop")),
        },
    )
}

#[test]
fn a_mod_made_for_another_game_version_offers_to_trust_it_once_active() {
    let (tmp, app) = installed();
    roads_made_for_an_older_game(&tmp);
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    let id = roads(&order, Side::Inactive);
    let shown = |order: &mut LoadOrder| serde_json::to_value(order.details(&app, id)).unwrap();
    assert_eq!(shown(&mut order)["offerTrust"], false);
    order.take(&app, Side::Inactive, 0, None);
    let details = shown(&mut order);
    assert_eq!(details["offerTrust"], true, "{}", details["problems"]);
    assert_eq!(details["name"], "Ann.Roads");
}

#[test]
fn a_mod_made_for_another_game_version_is_marked_on_either_side_until_trusted() {
    let (tmp, app) = installed();
    roads_made_for_an_older_game(&tmp);
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    let id = roads(&order, Side::Inactive);
    let marked = |order: &LoadOrder| {
        let rows = order.rows(&app, Side::Inactive);
        rows.iter()
            .any(|r| r.mod_id == id.to_qml() && r.outdated && r.made_for == "1.5")
    };
    assert!(marked(&order));
    app.set_version_ok(&[id], true);
    order.recheck(&app);
    assert!(!marked(&order));
    app.set_version_ok(&[id], false);
    order.recheck(&app);
    order.take(&app, Side::Inactive, 0, None);
    let active = order.rows(&app, Side::Active);
    assert!(
        active
            .iter()
            .any(|r| r.mod_id == id.to_qml() && r.made_for == "1.5")
    );
}

#[test]
fn saving_writes_the_load_order_and_marks_it_saved() {
    let (tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    order.take(&app, Side::Inactive, 0, None);
    order.save(&app).unwrap();
    let written = std::fs::read_to_string(tmp.path().join("config/Config/ModsConfig.xml")).unwrap();
    assert!(written.contains("<li>ann.roads</li>"));
    assert!(!app.session().is_dirty());
}

#[test]
fn a_list_saved_here_is_never_taken_for_one_written_outside() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    order.take(&app, Side::Inactive, 0, None);
    let saving = order.to_save(&app).unwrap();

    app.write_list(&saving.file, &saving.written, &saving.expected)
        .unwrap();
    assert_eq!(app.stamps().mods_config, 0);
    order.saved(&app, saving.list);

    assert_eq!(app.stamps().mods_config, 0);
}

fn written_outside(tmp: &tempfile::TempDir, ids: &str) -> (std::path::PathBuf, String) {
    let file = tmp.path().join("config/Config/ModsConfig.xml");
    let text = format!("<ModsConfigData><activeMods>{ids}</activeMods></ModsConfigData>");
    std::fs::write(&file, &text).unwrap();
    (file, text)
}

#[test]
fn a_file_written_outside_is_not_saved_over_until_its_list_or_ours_is_kept() {
    let (tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    order.take(&app, Side::Inactive, 0, None);
    let (file, text) = written_outside(&tmp, "<li>ludeon.rimworld</li><li>other.mod</li>");

    let refused = order.save(&app).unwrap_err();

    assert_eq!(
        refused,
        rimstack_core::rimworld::modsconfig::changed_outside()
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    assert!(app.session().is_dirty());

    order.keep_file(&app, false);
    assert_eq!(app.session().current(), ["ludeon.rimworld", "ann.roads"]);
    order.save(&app).unwrap();
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(written.contains("ann.roads") && !written.contains("other.mod"));
}

#[test]
fn taking_the_list_of_a_file_written_outside_leaves_nothing_to_save() {
    let (tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    order.take(&app, Side::Inactive, 0, None);
    let (file, text) = written_outside(&tmp, "<li>ludeon.rimworld</li><li>other.mod</li>");

    assert_ne!(app.stamps().mods_config, 0);

    order.keep_file(&app, true);

    assert_eq!(app.session().current(), ["ludeon.rimworld", "other.mod"]);
    assert!(!app.session().is_dirty());
    assert_eq!(app.stamps().mods_config, 0);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}

#[test]
fn every_row_has_a_key_of_its_own_and_a_mod_row_its_id() {
    let (tmp, app) = installed();
    roads_made_for_an_older_game(&tmp);
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    order.take(&app, Side::Inactive, 0, None);
    let rows = order.rows(&app, Side::Active);
    let keys: std::collections::HashSet<_> = rows.iter().map(|r| r.key.clone()).collect();
    assert_eq!(keys.len(), rows.len());
    let roads = rows
        .iter()
        .find(|r| r.name.to_string() == "Ann.Roads")
        .unwrap();
    assert!(roads.mod_id >= 0 && roads.number == 2 && roads.outdated);
}

#[test]
fn a_sort_of_a_list_in_order_moves_nothing_and_says_so() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    order.preview_sort(&app);
    assert!(!order.previewing());
    assert!(!order.sort_note.is_empty());
    assert_eq!(order.sort_count().map(|count| count.moves(&app)), Some(0));
}

#[test]
fn made_up_lists_have_rows_and_no_problems() {
    let (_tmp, app) = installed();
    let order = LoadOrder::demo(40, 10);
    assert_eq!(order.list(Side::Active).mods().len(), 40);
    assert!(order.rows(&app, Side::Active).len() >= 40);
    assert_eq!(order.errors_and_warnings(), (0, 0));
}

#[test]
fn a_mod_of_the_newest_save_switched_off_says_what_removal_would_do() {
    let (tmp, app) = installed();
    // A textures-only mod, and one defining things, both active and in the newest save.
    std::fs::create_dir_all(tmp.path().join("game/Mods/roads/Defs")).unwrap();
    std::fs::write(
        tmp.path().join("game/Mods/roads/Defs/Things.xml"),
        "<Defs><ThingDef><defName>Rock</defName></ThingDef></Defs>",
    )
    .unwrap();
    let about = "<ModMetaData><packageId>Ann.Pale</packageId><name>Pale</name></ModMetaData>";
    std::fs::create_dir_all(tmp.path().join("game/Mods/pale/About")).unwrap();
    std::fs::create_dir_all(tmp.path().join("game/Mods/pale/Textures")).unwrap();
    std::fs::write(tmp.path().join("game/Mods/pale/About/About.xml"), about).unwrap();
    std::fs::write(
        tmp.path().join("game/Mods/pale/Textures/rock.png"),
        [0, 0, 0, 0],
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("config/Config/ModsConfig.xml"),
        "<ModsConfigData><activeMods><li>ludeon.rimworld</li><li>ann.roads</li><li>ann.pale</li></activeMods></ModsConfigData>",
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("config/Saves")).unwrap();
    std::fs::write(
        tmp.path().join("config/Saves/Colony.rws"),
        "<savegame><meta><gameVersion>1.6.4871 rev600</gameVersion><modIds><li>ludeon.rimworld</li><li>ann.roads</li><li>ann.pale</li></modIds><modNames><li>Core</li><li>Ann.Roads</li><li>Pale</li></modNames></meta><game/></savegame>",
    )
    .unwrap();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    order.settle(&app);
    let members = order.files_to_read(&app).unwrap();
    app.read_active_files(members);
    // While it stands on the active list its details say what removing it would do.
    use rimstack_core::mods::removal::Removal;
    let roads = roads(&order, Side::Active);
    assert_eq!(
        app.conflicts()
            .reading()
            .as_ref()
            .map(|found| found.removal.get(&roads)),
        Some(Some(&Removal::Defs(vec!["ThingDef".to_string()]))),
    );
    let said = order.details(&app, roads);
    let as_json = crate::qml::values::as_shown(&said);
    assert_eq!(
        as_json.get("removal").and_then(|v| v.as_str()),
        Some("Removing it mid-save likely damages the save, which keeps what it defines: ThingDef"),
    );
    assert_eq!(
        as_json.get("removalHarms").and_then(|v| v.as_bool()),
        Some(true),
    );
    // Switched off, taking both out of the save is put in words, the damage first.
    let all: Vec<ModId> = (order.list(Side::Active).mods().iter())
        .filter(|m| m.id != "ludeon.rimworld")
        .filter_map(|m| m.installed)
        .collect();
    order.take_all(&app, Side::Active, &all);
    if let Some(members) = order.files_to_read(&app) {
        app.read_active_files(members);
    }
    let library = (*app.library())
        .as_ref()
        .ok()
        .map(|(l, _)| l.clone())
        .unwrap();
    assert_eq!(
        order.left_save(&app, &library).to_string(),
        "Taking Ann.Roads out of the newest save is likely to damage it. Taking Pale out of it is likely safe.",
    );
}

fn nothing_found() -> crate::workspace::Conflicts {
    crate::workspace::Conflicts {
        members: Vec::new(),
        overlaps: rimstack_core::mods::conflicts::overlaps(&[]),
        def_collisions: Vec::new(),
        scanned: Default::default(),
        xml_found: Default::default(),
        assemblies: Default::default(),
        miscased: Default::default(),
        removal: Default::default(),
    }
}

#[test]
fn a_scan_overtaken_by_a_change_of_textures_writes_nothing() {
    let (_tmp, app) = installed();
    let generation = app.conflicts_generation();
    app.textures_changed();
    app.keep_files_read(generation, nothing_found());
    assert!(app.conflicts().reading().is_none());
}

#[test]
fn a_scan_nothing_overtook_is_kept() {
    let (_tmp, app) = installed();
    app.keep_files_read(app.conflicts_generation(), nothing_found());
    assert!(app.conflicts().reading().is_some());
}

#[cfg(target_os = "linux")]
#[test]
fn nothing_is_saved_while_the_game_runs() {
    let (tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    order.take(&app, Side::Inactive, 0, None);
    let game = tmp.path().join("game/RimWorld");
    // A script: NixOS has no /bin/sleep. Its path stays on the shell's command line.
    std::fs::write(&game, "#!/bin/sh\nwhile :; do sleep 1; done\n").unwrap();
    let mut mode = std::fs::metadata(&game).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut mode, 0o755);
    std::fs::set_permissions(&game, mode).unwrap();
    let mut running = (0..50)
        .find_map(
            |_| match std::process::Command::new(&game).arg("30").spawn() {
                Err(e) if e.raw_os_error() == Some(26) => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    None
                }
                other => Some(other.unwrap()),
            },
        )
        .unwrap();
    let file = tmp.path().join("config/Config/ModsConfig.xml");
    let before = std::fs::read_to_string(&file).unwrap();

    let refused = order.save(&app);

    running.kill().unwrap();
    running.wait().unwrap();
    assert_eq!(
        refused.unwrap_err().kind(),
        rimstack_core::base::error::Kind::Refused
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    order.save(&app).unwrap();
}

fn with_a_mod_gone(workshop_names: &str) -> (tempfile::TempDir, App, LoadOrder) {
    let (tmp, app) = installed();
    std::fs::write(
        tmp.path().join("config/Config/ModsConfig.xml"),
        "<ModsConfigData><activeMods><li>gone.mod</li><li>ludeon.rimworld</li></activeMods></ModsConfigData>",
    )
    .unwrap();
    std::fs::create_dir_all(app.data_dir()).unwrap();
    let names = rimstack_core::rimworld::workshopdb::file_in(app.data_dir());
    std::fs::write(names, workshop_names).unwrap();
    let order = LoadOrder::read(&app, true, Arrange::Category);
    assert_eq!(order.list(Side::Active).mods()[0].id, "gone.mod");
    (tmp, app, order)
}

#[test]
fn a_mod_on_the_list_but_not_installed_gives_its_menu_its_id() {
    let (_tmp, app, order) = with_a_mod_gone("");
    let said = serde_json::to_value(order.not_installed(&app, Side::Active, 0)).unwrap();
    assert_eq!(said["packageId"], "gone.mod");
    assert_eq!(said["fixes"][0]["action"], "search");
    assert_eq!(said["resubscribe"], serde_json::json!([]));
    assert!(order.not_installed(&app, Side::Active, 1).is_none());
    assert!(order.not_installed(&app, Side::Inactive, 0).is_none());
}

#[test]
fn a_mod_not_installed_the_workshop_knows_offers_its_page_and_to_subscribe_again() {
    let (_tmp, app, order) = with_a_mod_gone("gone.mod\t123\tGone");
    let said = serde_json::to_value(order.not_installed(&app, Side::Active, 0)).unwrap();
    assert_eq!(said["name"], "Gone");
    assert_eq!(said["fixes"][0]["action"], "subscribe");
    assert_eq!(said["fixes"][0]["target"], "123");
    assert_eq!(said["resubscribe"], serde_json::json!(["123"]));
}

#[test]
fn a_mod_not_installed_taken_off_the_list_is_gone_from_both() {
    let (_tmp, app, mut order) = with_a_mod_gone("");
    order.take(&app, Side::Active, 0, None);
    assert_eq!(app.session().current(), ["ludeon.rimworld"]);
    let inactive: Vec<&str> = order
        .list(Side::Inactive)
        .mods()
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(inactive, ["ann.roads"]);
    assert!(order.undo(&app));
    assert_eq!(app.session().current(), ["gone.mod", "ludeon.rimworld"]);
}

#[test]
fn an_inactive_mod_in_no_save_no_list_and_no_kept_order_is_unplayed() {
    let (tmp, app) = installed();
    let add = |folder: &str, id: &str| {
        let about = tmp.path().join("game/Mods").join(folder).join("About");
        std::fs::create_dir_all(&about).unwrap();
        std::fs::write(
            about.join("About.xml"),
            format!("<ModMetaData><packageId>{id}</packageId><name>{id}</name></ModMetaData>"),
        )
        .unwrap();
    };
    add("b", "a.b");
    add("c", "a.c");
    add("d", "a.d");
    std::fs::write(
        tmp.path().join("config/Config/ModsConfig.xml"),
        "<ModsConfigData><activeMods><li>ludeon.rimworld</li></activeMods></ModsConfigData>",
    )
    .unwrap();
    // One save names a.b (the game folds the `_steam` away), a kept list a.c,
    // the order history a.d.
    std::fs::create_dir_all(tmp.path().join("config/Saves")).unwrap();
    std::fs::write(
        tmp.path().join("config/Saves/old.rws"),
        "<savegame><meta><modIds><li>ludeon.rimworld</li><li>a.b_steam</li></modIds></meta></savegame>",
    )
    .unwrap();
    app.rescan();
    app.record_order(
        rimstack_core::kept::orders::How::Outside,
        &["ludeon.rimworld".to_string(), "a.d".to_string()],
    );
    app.keep_list("mine", Some(vec!["a.c".to_string()]), false)
        .unwrap();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    let shown = |order: &LoadOrder, side| -> Vec<String> {
        let list = order.list(side);
        let id = |r: &Row| match r {
            Row::Mod { index } => Some(list.mods()[*index].id.clone()),
            _ => None,
        };
        list.rows().iter().filter_map(id).collect()
    };
    order.set_filter("is:unplayed");
    assert_eq!(shown(&order, Side::Inactive), ["ann.roads"]);
    assert!(!shown(&order, Side::Inactive).contains(&"a.b".to_string()));
    // A mod whose list is gone is offered too, from then on.
    app.delete_list("mine");
    order.sync(&app);
    order.set_filter("is:unplayed");
    assert_eq!(shown(&order, Side::Inactive), ["a.c", "ann.roads"]);
}

#[test]
fn a_load_folder_named_in_other_letter_case_is_warned_about_where_case_counts() {
    let (tmp, app) = installed();
    std::fs::create_dir_all(tmp.path().join("game/Mods/roads/Common")).unwrap();
    std::fs::write(
        tmp.path().join("game/Mods/roads/LoadFolders.xml"),
        "<loadFolders><v1.6><li>/</li><li>common</li></v1.6></loadFolders>",
    )
    .unwrap();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    order.take(&app, Side::Inactive, 0, None);
    let members = order.files_to_read(&app).unwrap();
    app.read_active_files(members);
    order.recheck(&app);
    let told = order.troubles()[1]
        .iter()
        .any(|t| t.key == "loadfolders-case");
    let case_counts = !tmp.path().join("game/Mods/roads/common").is_dir();
    assert_eq!(told, case_counts, "{:?}", order.troubles()[1]);
}

#[test]
fn an_active_mod_bundling_harmony_is_warned_about_and_filtered() {
    let (tmp, app) = installed();
    std::fs::create_dir_all(tmp.path().join("game/Mods/roads/Assemblies")).unwrap();
    std::fs::write(
        tmp.path().join("game/Mods/roads/Assemblies/0Harmony.dll"),
        "not an assembly",
    )
    .unwrap();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    order.take(&app, Side::Inactive, 0, None);
    let members = order.files_to_read(&app).unwrap();
    app.read_active_files(members);
    order.recheck(&app);
    let own = &order.troubles()[1];
    assert!(own.iter().any(|t| t.key == "assembly-harmony"), "{own:?}");
    order.set_filter("is:assemblies");
    assert_eq!(order.list(Side::Active).matching(), 1);
}

#[test]
fn cleanup_names_the_workshop_mods_for_steam_and_the_rest_for_the_bin() {
    let (tmp, app) = installed();
    // A Workshop copy of an inactive mod, and one local one.
    std::fs::create_dir_all(tmp.path().join("workshop/42/About")).unwrap();
    std::fs::write(
        tmp.path().join("workshop/42/About/About.xml"),
        "<ModMetaData><packageId>Ann.Hats</packageId></ModMetaData>",
    )
    .unwrap();
    std::fs::write(tmp.path().join("workshop/42/file.pdb"), "x").unwrap();
    app.rescan();
    let order = LoadOrder::read(&app, true, Arrange::Category);
    let about = |folder: &str, active: bool| {
        order
            .list(match active {
                true => Side::Active,
                false => Side::Inactive,
            })
            .mods()
            .iter()
            .find(|m| m.id == folder)
            .and_then(|m| m.installed)
    };
    let wanted: Vec<ModId> = [
        about("ann.hats", false),
        about("ann.roads", false),
        about("ludeon.rimworld", true),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert_eq!(wanted.len(), 3);
    let cleanup = order.cleanup(&app, &wanted);
    // The workshop mod counts for Steam, the active core is not binnable, the
    // inactive local mod is.
    assert_eq!(cleanup.unsubscribe, 1);
    assert_eq!(cleanup.bin, 1, "{cleanup:?}");
    assert!(cleanup.unsubscribe_bytes > 0 && cleanup.bin_bytes > 0);
}

#[test]
fn a_library_no_active_mod_needs_is_unused_until_one_does() {
    let (tmp, app) = installed();
    let add = |folder: &str, id: &str, needs: &str| {
        let about = tmp.path().join("game/Mods").join(folder).join("About");
        std::fs::create_dir_all(&about).unwrap();
        let deps = match needs {
            "" => String::new(),
            id => {
                format!("<modDependencies><li><packageId>{id}</packageId></li></modDependencies>")
            }
        };
        let xml = format!(
            "<ModMetaData><packageId>{id}</packageId><name>{id}</name>{deps}</ModMetaData>"
        );
        std::fs::write(about.join("About.xml"), xml).unwrap();
    };
    // Two mods needing it make it a library.
    add("lib", "a.lib", "");
    add("b", "a.b", "a.lib");
    add("c", "a.c", "a.lib");
    std::fs::write(
        tmp.path().join("config/Config/ModsConfig.xml"),
        "<ModsConfigData><activeMods><li>ludeon.rimworld</li><li>a.lib</li></activeMods></ModsConfigData>",
    )
    .unwrap();
    app.rescan();
    let mut order = LoadOrder::read(&app, true, Arrange::Category);
    let shown = |order: &LoadOrder, side| -> Vec<String> {
        let list = order.list(side);
        let id = |r: &Row| match r {
            Row::Mod { index } => Some(list.mods()[*index].id.clone()),
            _ => None,
        };
        list.rows().iter().filter_map(id).collect()
    };
    order.set_filter("is:unused");
    assert_eq!(shown(&order, Side::Active), ["a.lib"]);
    let b = order
        .list(Side::Inactive)
        .mods()
        .iter()
        .position(|m| m.id == "a.b");
    order.take(&app, Side::Inactive, b.unwrap(), None);
    order.set_filter("is:unused");
    assert!(shown(&order, Side::Active).is_empty());
}
