use super::*;
use crate::workspace::tests::installed;

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
    let order = LoadOrder::read(&app, true, false);
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
fn mods_that_could_not_be_read_give_empty_lists_and_the_reason() {
    let (tmp, app) = installed();
    std::fs::remove_dir_all(tmp.path().join("game")).unwrap();
    let order = LoadOrder::read(&app, true, false);
    assert!(!order.problem.is_empty() && !order.is_installed());
    assert!(order.list(Side::Active).mods().is_empty());
}

#[test]
fn a_mod_taken_across_is_one_step_that_undo_takes_back() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, false);
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
fn an_error_ignored_is_gone_until_warned_again() {
    let (tmp, app) = installed();
    let about = "<ModMetaData><packageId>Ann.Roads</packageId><name>Ann.Roads</name><modDependencies><li><packageId>gone.lib</packageId></li></modDependencies></ModMetaData>";
    std::fs::write(tmp.path().join("game/Mods/roads/About/About.xml"), about).unwrap();
    let mut order = LoadOrder::read(&app, true, false);
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
fn a_mod_made_for_another_game_version_offers_to_trust_it_once_active() {
    let (tmp, app) = installed();
    roads_made_for_an_older_game(&tmp);
    let mut order = LoadOrder::read(&app, true, false);
    let id = roads(&order, Side::Inactive);
    let shown = |order: &mut LoadOrder| serde_json::to_value(order.details(&app, id)).unwrap();
    assert_eq!(shown(&mut order)["offerTrust"], false);
    order.take(&app, Side::Inactive, 0, None);
    let details = shown(&mut order);
    assert_eq!(details["offerTrust"], true, "{}", details["problems"]);
    assert_eq!(details["name"], "Ann.Roads");
}

#[test]
fn saving_writes_the_load_order_and_marks_it_saved() {
    let (tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, false);
    order.take(&app, Side::Inactive, 0, None);
    order.save(&app).unwrap();
    let written = std::fs::read_to_string(tmp.path().join("config/Config/ModsConfig.xml")).unwrap();
    assert!(written.contains("<li>ann.roads</li>"));
    assert!(!app.session().is_dirty());
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
    let mut order = LoadOrder::read(&app, true, false);
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
    let mut order = LoadOrder::read(&app, true, false);
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
    let mut order = LoadOrder::read(&app, true, false);
    order.take(&app, Side::Inactive, 0, None);
    let rows = order.rows(&app, Side::Active);
    let keys: std::collections::HashSet<_> = rows.iter().map(|r| r.key.clone()).collect();
    assert_eq!(keys.len(), rows.len());
    let roads = rows
        .iter()
        .find(|r| r.name.to_string() == "Ann.Roads")
        .unwrap();
    assert!(roads.mod_id >= 0 && roads.number == 2 && roads.trouble > 0);
}

#[test]
fn a_sort_of_a_list_in_order_moves_nothing_and_says_so() {
    let (_tmp, app) = installed();
    let mut order = LoadOrder::read(&app, true, false);
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

fn nothing_found() -> crate::workspace::Conflicts {
    crate::workspace::Conflicts {
        members: Vec::new(),
        overlaps: rimstack_core::mods::conflicts::overlaps(&[]),
        scanned: Default::default(),
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
    let mut order = LoadOrder::read(&app, true, false);
    order.take(&app, Side::Inactive, 0, None);
    let game = tmp.path().join("game/RimWorld");
    std::fs::copy("/bin/sleep", &game).unwrap();
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
