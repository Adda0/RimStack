use super::Mods;
use crate::ui::made_up::made_up;
use crate::ui::shell::Shell;
use crate::ui::theme::Theme;
use crate::ui::{Options, install};
use crate::workspace::load_order::Side;
use gpui::{AppContext, Entity, HeadlessAppContext, size};
use rimstack_core::mods::library::ModId;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn opened() -> (tempfile::TempDir, HeadlessAppContext, Entity<Mods>) {
    opened_with(|_| {})
}

pub(super) fn opened_with(
    prepare: impl FnOnce(&std::path::Path),
) -> (tempfile::TempDir, HeadlessAppContext, Entity<Mods>) {
    opened_at(None, prepare)
}

/// As `opened_with`, the text at `text_scale`.
pub(super) fn opened_at(
    text_scale: Option<f32>,
    prepare: impl FnOnce(&std::path::Path),
) -> (tempfile::TempDir, HeadlessAppContext, Entity<Mods>) {
    let (tmp, app) = made_up();
    prepare(tmp.path());
    let text = crate::ui::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(crate::ui::assets::Assets));
    let options = Options {
        text_scale,
        ..Options::default()
    };
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shape = cx.update(|cx| {
        let _ = cx.global::<Theme>();
        size(crate::ui::WINDOW.0, crate::ui::WINDOW.1)
    });
    let shell = cx
        .open_window(shape, |window, cx| {
            cx.new(|cx| Shell::new(&options, window, cx))
        })
        .unwrap();
    let mods = cx
        .update(|cx| shell.read(cx).map(|shell| shell.sections.mods.clone()))
        .unwrap();
    let started = Instant::now();
    while cx.update(|cx| {
        let mods = mods.read(cx);
        mods.list.read(cx).count(Side::Active) == 0 || !mods.list.read(cx).files_read
    }) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the made-up mods were not read"
        );
        crate::ui::made_up::step(&cx);
    }
    (tmp, cx, mods)
}

fn names(mods: &Mods, side: Side, cx: &gpui::App) -> Vec<String> {
    mods.list
        .read(cx)
        .order
        .list(side)
        .mods()
        .iter()
        .map(|m| m.name.clone())
        .collect()
}

#[test]
fn a_mod_moved_across_comes_back_with_undo_and_goes_again_with_redo() {
    let (_tmp, mut cx, mods) = opened();
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let (active, inactive) = (
                names(mods, Side::Active, cx),
                names(mods, Side::Inactive, cx),
            );
            let last = active.len() - 1;
            let name = active[last].clone();
            mods.take(Side::Active, last, None, cx);
            assert!(!names(mods, Side::Active, cx).contains(&name));
            assert!(names(mods, Side::Inactive, cx).contains(&name));
            assert!(crate::ui::app(cx).session().is_dirty());
            mods.undo(cx);
            assert_eq!(names(mods, Side::Active, cx), active);
            assert_eq!(names(mods, Side::Inactive, cx), inactive);
            mods.redo(cx);
            assert!(names(mods, Side::Inactive, cx).contains(&name));
            mods.discard_changes(cx);
            assert_eq!(names(mods, Side::Active, cx), active);
        })
    });
}

#[test]
fn a_drop_moves_one_mod_or_the_mods_chosen_together() {
    let (_tmp, mut cx, mods) = opened();
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let before = names(mods, Side::Active, cx);
            let count = before.len();
            mods.drop_on(
                (Side::Active, count - 1, Vec::new()),
                (Side::Active, count - 3),
                cx,
            );
            let after = names(mods, Side::Active, cx);
            assert_eq!(after[count - 3], before[count - 1]);
            mods.undo(cx);
            let ids: Vec<ModId> = mods
                .list
                .read(cx)
                .order
                .list(Side::Inactive)
                .mods()
                .iter()
                .filter_map(|m| m.installed)
                .take(2)
                .collect();
            assert_eq!(ids.len(), 2, "two inactive mods to take along");
            mods.drop_on((Side::Inactive, 0, ids), (Side::Active, count), cx);
            assert_eq!(names(mods, Side::Active, cx).len(), count + 2);
        })
    });
}

#[test]
fn shift_ctrl_and_invert_choose_several_mods_in_one_list() {
    let (_tmp, mut cx, mods) = opened();
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let ids: Vec<ModId> = mods
                .list
                .read(cx)
                .order
                .list(Side::Active)
                .mods()
                .iter()
                .filter_map(|m| m.installed)
                .collect();
            mods.select(Some(ids[1]), true, cx);
            mods.select_through(ids[4], true, cx);
            assert_eq!(mods.list.read(cx).selection.len(), 4);
            mods.select_also(ids[2], true, cx);
            assert_eq!(mods.list.read(cx).selection.len(), 3);
            assert_eq!(mods.selected_id(cx), Some(ids[1]));
            mods.invert_selection(true, cx);
            assert_eq!(mods.list.read(cx).selection.len(), ids.len() - 3);
        })
    });
}

#[test]
fn sort_is_shown_before_it_is_kept_and_save_writes_the_list() {
    let (tmp, mut cx, mods) = opened();
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let before = names(mods, Side::Active, cx);
            mods.preview_sort(cx);
            assert!(mods.list.read(cx).order.previewing());
            mods.end_sort(false, cx);
            assert!(!mods.list.read(cx).order.previewing());
            assert_eq!(names(mods, Side::Active, cx), before);
            let last = before.len() - 1;
            mods.take(Side::Active, last, None, cx);
            mods.save(None, cx);
        })
    });
    let started = Instant::now();
    while cx.update(|cx| mods.read(cx).list.read(cx).saving) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the save did not end"
        );
        crate::ui::made_up::step(&cx);
    }
    cx.update(|cx| {
        let mods = mods.read(cx);
        assert!(
            mods.status.save_problem.is_empty(),
            "{}",
            mods.status.save_problem
        );
        assert!(!crate::ui::app(cx).session().is_dirty());
    });
    let written =
        std::fs::read_to_string(tmp.path().join("fx/config/Config/ModsConfig.xml")).unwrap();
    let count = cx.update(|cx| mods.read(cx).list.read(cx).count(Side::Active));
    assert_eq!(
        written.matches("<li>").count() - known_expansions(&written),
        count
    );
}

fn known_expansions(written: &str) -> usize {
    written
        .split("<knownExpansions>")
        .nth(1)
        .and_then(|rest| rest.split("</knownExpansions>").next())
        .map_or(0, |known| known.matches("<li>").count())
}

#[test]
fn sort_counts_the_moves_it_would_make_once_the_files_are_read() {
    let (_tmp, mut cx, mods) = opened();
    let started = Instant::now();
    while cx.update(|cx| mods.read(cx).list.read(cx).sort_pending == 0) {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "Sort never counted its moves"
        );
        crate::ui::made_up::step(&cx);
    }
}

#[test]
fn a_category_folded_stays_folded_when_a_mod_leaves_the_folders() {
    let (_tmp, mut cx, mods) = opened();
    let (folded, gone) = cx.update(|cx| {
        let inactive = mods
            .read(cx)
            .list
            .read(cx)
            .order
            .list(Side::Inactive)
            .mods();
        let gone = inactive.last().and_then(|m| m.installed).unwrap();
        let loaded = crate::ui::app(cx).library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        (
            inactive[0].category,
            library.get(gone).unwrap().path.clone(),
        )
    });
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.toggle_category(Side::Inactive, folded, cx)
        })
    });
    let before = cx.update(|cx| mods.read(cx).list.read(cx).count(Side::Inactive));
    std::fs::remove_dir_all(gone).unwrap();
    cx.update(|cx| crate::ui::app(cx).rescan_soon());
    let started = Instant::now();
    while cx.update(|cx| mods.read(cx).list.read(cx).count(Side::Inactive) == before) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the mods were not read again"
        );
        crate::ui::made_up::step(&cx);
    }
    let heading = cx.update(|cx| {
        let rows = mods.read(cx).inactive.rows.clone();
        rows.iter()
            .find(|r| r.is_header && r.category == folded)
            .cloned()
    });
    assert!(
        heading.is_some_and(|h| h.collapsed),
        "{folded:?} was unfolded"
    );
}

#[test]
fn a_game_folder_chosen_reads_the_mods_from_it() {
    let (tmp, mut cx, mods) = opened();
    let gone = cx.update(|cx| {
        let inactive = mods
            .read(cx)
            .list
            .read(cx)
            .order
            .list(Side::Inactive)
            .mods();
        let gone = inactive.last().and_then(|m| m.installed).unwrap();
        let loaded = crate::ui::app(cx).library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        library.get(gone).unwrap().path.clone()
    });
    let before = cx.update(|cx| mods.read(cx).list.read(cx).count(Side::Inactive));
    std::fs::remove_dir_all(gone).unwrap();
    let game = tmp.path().join("fx/game");
    cx.update(|cx| {
        let app = crate::ui::app(cx);
        app.publish(crate::changes::Changes::LIBRARY);
        app.keep_setting("game", game.to_str().unwrap())
    })
    .unwrap();
    cx.run_until_parked();
    let list = cx.update(|cx| mods.read(cx).list.clone());
    cx.update(|cx| {
        let list = list.read(cx);
        assert!(!list.rescanning);
        assert_eq!(list.count(Side::Inactive), before - 1);
    });
}

#[test]
fn home_end_and_the_page_keys_walk_the_list() {
    let (_tmp, mut cx, mods) = opened();
    let window = cx.update(|cx| cx.windows()[0]);
    let chosen = |cx: &mut HeadlessAppContext| {
        cx.update(|cx| {
            let order = mods.read(cx).list.read(cx).order.list(Side::Active);
            (0..order.rows().len())
                .filter_map(|row| order.installed_on(row))
                .collect::<Vec<_>>()
        })
    };
    let selected =
        |cx: &mut HeadlessAppContext| cx.update(|cx| mods.read(cx).list.read(cx).selected_id());
    let keys = |keys: &'static str, cx: &mut HeadlessAppContext| {
        cx.update_window(window, |_, window, cx| {
            let parsed = gpui::Keystroke::parse(keys).unwrap();
            window.dispatch_keystroke(parsed, cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    cx.update_window(window, |_, window, cx| {
        let handle = mods.read(cx).list(Side::Active).read(cx).focus_handle();
        window.focus(&handle, cx);
    })
    .unwrap();
    cx.run_until_parked();
    let list = chosen(&mut cx);
    let (first, last) = (*list.first().unwrap(), *list.last().unwrap());
    keys("end", &mut cx);
    assert_eq!(selected(&mut cx), Some(last), "End selects the last mod");
    keys("home", &mut cx);
    assert_eq!(selected(&mut cx), Some(first), "Home selects the first mod");
    keys("down", &mut cx);
    let second = selected(&mut cx);
    keys("pageup", &mut cx);
    assert_eq!(selected(&mut cx), Some(first), "Page Up stops at the first");
    let page = cx.update(|cx| {
        let scroll = mods.read(cx).list(Side::Active).read(cx).scroll().clone();
        crate::ui::rows::page_of(&scroll, cx.global::<Theme>().row())
    });
    keys("pagedown", &mut cx);
    let at = selected(&mut cx).unwrap();
    let row = |id: ModId, cx: &mut HeadlessAppContext| {
        cx.update(|cx| {
            let order = mods.read(cx).list.read(cx).order.list(Side::Active);
            order.position_of(id).and_then(|at| order.row_of(at))
        })
    };
    let (from, to) = (
        row(second.unwrap(), &mut cx).unwrap(),
        row(at, &mut cx).unwrap(),
    );
    assert!(
        to + 1 >= from + page,
        "Page Down steps a page of rows: {from} -> {to}, a page being {page}"
    );
}

#[test]
fn esc_in_an_empty_filter_and_down_give_the_list_the_keys() {
    let (_tmp, mut cx, mods) = opened();
    let window = cx.update(|cx| cx.windows()[0]);
    let keys = |keys: &'static str, cx: &mut HeadlessAppContext| {
        for key in keys.split(' ') {
            cx.update_window(window, |_, window, cx| {
                let parsed = gpui::Keystroke::parse(key).unwrap();
                window.dispatch_keystroke(parsed, cx);
            })
            .unwrap();
            cx.run_until_parked();
        }
    };
    let on_list = |cx: &mut HeadlessAppContext| {
        cx.update_window(window, |_, window, cx| {
            let mods = mods.read(cx);
            let list = mods.list(Side::Active).read(cx).focus_handle();
            (list.is_focused(window), mods.list.read(cx).selected_id())
        })
        .unwrap()
    };
    let first = cx.update(|cx| {
        let order = mods.read(cx).list.read(cx).order.list(Side::Active);
        (0..order.rows().len()).find_map(|row| order.installed_on(row))
    });
    cx.update_window(window, |_, window, cx| {
        let handle = mods.read(cx).list(Side::Active).read(cx).focus_handle();
        window.focus(&handle, cx);
    })
    .unwrap();
    cx.run_until_parked();
    keys("ctrl-f", &mut cx);
    assert_eq!(
        on_list(&mut cx),
        (false, None),
        "Ctrl+F gives the filter the keys"
    );
    keys("escape", &mut cx);
    assert_eq!(
        on_list(&mut cx),
        (true, first),
        "Esc in the empty filter goes to the list and chooses its first mod"
    );
    keys("ctrl-f z z z q escape", &mut cx);
    assert!(!on_list(&mut cx).0, "Esc first empties a filter typed in");
    keys("z z z q down", &mut cx);
    assert_eq!(
        on_list(&mut cx),
        (true, first),
        "Down with nothing offered goes to the list"
    );
    let note = cx.update(|cx| mods.read(cx).details_view.read(cx).note.clone());
    cx.update_window(window, |_, window, cx| {
        window.focus(&gpui::Focusable::focus_handle(&note, cx), cx);
        note.update(cx, |field, cx| field.set_text("kept", cx));
    })
    .unwrap();
    cx.run_until_parked();
    keys("escape", &mut cx);
    assert!(on_list(&mut cx).0, "Esc in the note goes back to the list");
    assert_eq!(cx.update(|cx| note.read(cx).text().to_string()), "kept");
}

#[test]
fn f6_goes_round_the_panes_and_ctrl_page_keys_round_the_tabs() {
    let (_tmp, mut cx, mods) = opened();
    let window = cx.update(|cx| cx.windows()[0]);
    let keys = |keys: &'static str, cx: &mut HeadlessAppContext| {
        for key in keys.split(' ') {
            cx.update_window(window, |_, window, cx| {
                let parsed = gpui::Keystroke::parse(key).unwrap();
                window.dispatch_keystroke(parsed, cx);
            })
            .unwrap();
            cx.run_until_parked();
        }
    };
    let pane = |cx: &mut HeadlessAppContext| {
        cx.update_window(window, |_, window, cx| {
            let mods = mods.read(cx);
            [
                mods.list(Side::Inactive).read(cx).focus_handle(),
                mods.list(Side::Active).read(cx).focus_handle(),
                mods.details_view.read(cx).focus.clone(),
            ]
            .iter()
            .position(|pane| pane.is_focused(window))
        })
        .unwrap()
    };
    let tab = |cx: &mut HeadlessAppContext| cx.update(|cx| mods.read(cx).details_view.read(cx).tab);
    cx.update_window(window, |_, window, cx| {
        let handle = mods.read(cx).list(Side::Active).read(cx).focus_handle();
        window.focus(&handle, cx);
    })
    .unwrap();
    cx.run_until_parked();
    keys("home", &mut cx);
    keys("f6", &mut cx);
    assert_eq!(
        pane(&mut cx),
        Some(2),
        "F6 goes from the load order to the details"
    );
    keys("f6", &mut cx);
    assert_eq!(pane(&mut cx), Some(0), "and round to the inactive mods");
    keys("shift-f6", &mut cx);
    assert_eq!(pane(&mut cx), Some(2), "Shift+F6 goes back");
    keys("f6 f6", &mut cx);
    assert_eq!(pane(&mut cx), Some(1));
    assert_eq!(tab(&mut cx), 0);
    keys("ctrl-pagedown", &mut cx);
    assert_eq!(tab(&mut cx), 1, "Ctrl+Page Down shows the next tab");
    keys("ctrl-pageup ctrl-pageup", &mut cx);
    assert_eq!(tab(&mut cx), 3, "Ctrl+Page Up goes round to the last");
    assert_eq!(pane(&mut cx), Some(1), "the list keeps the keys");
}

#[test]
fn shift_ctrl_and_ctrl_a_choose_from_the_keyboard() {
    let (_tmp, mut cx, mods) = opened();
    let window = cx.update(|cx| cx.windows()[0]);
    let keys = |keys: &'static str, cx: &mut HeadlessAppContext| {
        cx.update_window(window, |_, window, cx| {
            let parsed = gpui::Keystroke::parse(keys).unwrap();
            window.dispatch_keystroke(parsed, cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    let chosen =
        |cx: &mut HeadlessAppContext| cx.update(|cx| mods.read(cx).list.read(cx).selection.clone());
    let first =
        |cx: &mut HeadlessAppContext| cx.update(|cx| mods.read(cx).list.read(cx).selected_id());
    let cursor = |cx: &mut HeadlessAppContext| cx.update(|cx| mods.read(cx).list.read(cx).cursor());
    cx.update_window(window, |_, window, cx| {
        let handle = mods.read(cx).list(Side::Active).read(cx).focus_handle();
        window.focus(&handle, cx);
    })
    .unwrap();
    cx.run_until_parked();
    keys("home", &mut cx);
    let top = first(&mut cx).unwrap();
    keys("shift-down", &mut cx);
    keys("shift-down", &mut cx);
    assert_eq!(chosen(&mut cx).len(), 3, "Shift with Down selects through");
    keys("shift-up", &mut cx);
    assert_eq!(chosen(&mut cx).len(), 2, "Shift with Up takes one back");
    assert_eq!(first(&mut cx), Some(top), "and the first mod stays the one");
    keys("ctrl-down", &mut cx);
    assert_eq!(chosen(&mut cx).len(), 2, "Ctrl with Down selects nothing");
    let below = cursor(&mut cx).unwrap();
    keys("ctrl-space", &mut cx);
    assert_eq!(chosen(&mut cx).len(), 3, "Ctrl + Space adds the mod");
    assert!(
        chosen(&mut cx).contains(&below),
        "the mod the cursor went to is chosen"
    );
    keys("ctrl-a", &mut cx);
    let shown = cx.update(|cx| {
        mods.read(cx)
            .list
            .read(cx)
            .order
            .list(Side::Active)
            .shown_installed()
            .len()
    });
    assert_eq!(
        chosen(&mut cx).len(),
        shown,
        "Ctrl + A chooses every mod shown"
    );
    assert_eq!(first(&mut cx), Some(top), "keeping the first one chosen");
}

#[test]
fn alt_moves_the_selected_mods_together_and_enter_takes_them_across() {
    let (_tmp, mut cx, mods) = opened();
    let window = cx.update(|cx| cx.windows()[0]);
    let keys = |keys: &'static str, cx: &mut HeadlessAppContext| {
        cx.update_window(window, |_, window, cx| {
            let parsed = gpui::Keystroke::parse(keys).unwrap();
            window.dispatch_keystroke(parsed, cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    let place = |id: ModId, cx: &mut HeadlessAppContext| {
        cx.update(|cx| {
            mods.read(cx)
                .list
                .read(cx)
                .order
                .list(Side::Active)
                .position_of(id)
        })
    };
    let chosen =
        |cx: &mut HeadlessAppContext| cx.update(|cx| mods.read(cx).list.read(cx).selection.clone());
    let count = |side: Side, cx: &mut HeadlessAppContext| {
        cx.update(|cx| mods.read(cx).list.read(cx).count(side))
    };
    cx.update_window(window, |_, window, cx| {
        let handle = mods.read(cx).list(Side::Active).read(cx).focus_handle();
        window.focus(&handle, cx);
    })
    .unwrap();
    cx.run_until_parked();
    keys("home", &mut cx);
    keys("shift-down", &mut cx);
    let pair = chosen(&mut cx);
    let places = |pair: &[ModId], cx: &mut HeadlessAppContext| {
        pair.iter().map(|id| place(*id, cx)).collect::<Vec<_>>()
    };
    let (before, total) = (places(&pair, &mut cx), count(Side::Active, &mut cx));
    keys("alt-down", &mut cx);
    let moved = places(&pair, &mut cx);
    assert!(
        moved
            .iter()
            .zip(&before)
            .enumerate()
            .all(|(at, (now, was))| {
                let (Some(now), Some(was)) = (now, was) else {
                    return false;
                };
                *now == was + 1 || moved[..at].contains(&Some(*now))
            }),
        "Alt + Down moves every chosen mod one place, keeping their order"
    );
    keys("alt-home", &mut cx);
    let edged = places(&pair, &mut cx);
    assert!(
        edged
            .iter()
            .zip(moved.iter())
            .all(|(at, was)| { at.is_some_and(|at| was.is_some_and(|was| at <= was)) }),
        "Alt + Home takes them up without splitting them"
    );
    keys("enter", &mut cx);
    assert_eq!(
        count(Side::Active, &mut cx),
        total - 2,
        "Enter moves every chosen mod across"
    );
    assert!(
        pair.iter().all(|id| place(*id, &mut cx).is_none()),
        "and they leave the active list"
    );
}

#[test]
fn menu_key_and_shift_f10_open_the_selected_mods_menu() {
    let (_tmp, mut cx, mods) = opened();
    let window = cx.update(|cx| cx.windows()[0]);
    let keys = |keys: &'static str, cx: &mut HeadlessAppContext| {
        cx.update_window(window, |_, window, cx| {
            let parsed = gpui::Keystroke::parse(keys).unwrap();
            window.dispatch_keystroke(parsed, cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    let entries = |cx: &mut HeadlessAppContext| cx.update(|cx| crate::ui::menu::texts(cx));
    cx.update_window(window, |_, window, cx| {
        let handle = mods.read(cx).list(Side::Active).read(cx).focus_handle();
        window.focus(&handle, cx);
    })
    .unwrap();
    cx.run_until_parked();
    keys("menu", &mut cx);
    assert!(
        entries(&mut cx).is_empty(),
        "the Menu key opens nothing with no mod chosen"
    );
    keys("end", &mut cx);
    keys("menu", &mut cx);
    let said = crate::words::put(&rimstack_core::say!("Copy name"));
    assert!(
        entries(&mut cx).contains(&said),
        "the Menu key opens the chosen mod's menu, Copy name among its entries"
    );
    keys("escape", &mut cx);
    for _ in 0..12 {
        cx.advance_clock(Duration::from_millis(16));
        cx.run_until_parked();
    }
    assert!(
        entries(&mut cx).is_empty(),
        "Escape closes it, so the entries go"
    );
    keys("shift-f10", &mut cx);
    assert!(
        entries(&mut cx).contains(&said),
        "Shift + F10 opens the menu again"
    );
}

#[test]
fn left_folds_the_chosen_mods_heading_and_the_arrows_stop_on_it() {
    let (_tmp, mut cx, mods) = opened();
    let window = cx.update(|cx| cx.windows()[0]);
    let keys = |keys: &'static str, cx: &mut HeadlessAppContext| {
        cx.update_window(window, |_, window, cx| {
            let parsed = gpui::Keystroke::parse(keys).unwrap();
            window.dispatch_keystroke(parsed, cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    let shown = |cx: &mut HeadlessAppContext| {
        cx.update(|cx| mods.read(cx).list(Side::Active).read(cx).shown().clone())
    };
    let chosen =
        |cx: &mut HeadlessAppContext| cx.update(|cx| mods.read(cx).list.read(cx).selected_id());
    cx.update_window(window, |_, window, cx| {
        let handle = mods.read(cx).list(Side::Active).read(cx).focus_handle();
        window.focus(&handle, cx);
    })
    .unwrap();
    cx.run_until_parked();
    keys("home", &mut cx);
    let mod_id = chosen(&mut cx).expect("Home chooses the first mod");
    let rows = shown(&mut cx).rows.len();
    keys("left", &mut cx);
    let folded = shown(&mut cx);
    assert!(
        folded.rows.len() < rows,
        "Left folds the heading over the chosen mod, hiding the mods under it"
    );
    let place = folded.place.expect("the chosen mod's place");
    assert!(
        folded.rows[place].is_header && folded.rows[place].collapsed,
        "its place is the folded heading"
    );
    keys("down", &mut cx);
    assert_ne!(
        chosen(&mut cx),
        Some(mod_id),
        "Down steps off the folded heading onto the next mod"
    );
    keys("up", &mut cx);
    assert_eq!(
        chosen(&mut cx),
        Some(mod_id),
        "Up steps onto the folded heading, which stands for the mod it hides"
    );
    assert_eq!(
        shown(&mut cx).place,
        Some(place),
        "the heading stands for it in the same place"
    );
    keys("right", &mut cx);
    assert_eq!(
        shown(&mut cx).rows.len(),
        rows,
        "Right shows its mods again"
    );
}
