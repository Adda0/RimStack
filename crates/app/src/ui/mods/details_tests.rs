use super::Mods;
use super::edit_tests::opened_with;
use super::notes::Picking;
use crate::workspace::load_order::Side;
use gpui::{Entity, Focusable, HeadlessAppContext};
use rimstack_core::mods::library::ModId;

fn active_id(mods: &Mods, nth: usize) -> ModId {
    mods.active
        .rows
        .iter()
        .filter(|r| !r.is_header)
        .nth(nth)
        .unwrap()
        .mod_id
        .unwrap()
}

fn chosen(cx: &mut HeadlessAppContext, mods: &Entity<Mods>, nth: usize) -> ModId {
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let id = active_id(mods, nth);
            mods.choose(Side::Active, id, cx);
            id
        })
    })
}

pub(super) fn shared_textures(root: &std::path::Path) {
    for (n, shade) in [("035", 60u8), ("038", 200u8)] {
        let folder = root.join(format!("fx/game/Mods/fixture-{n}/Textures/Things"));
        std::fs::create_dir_all(&folder).unwrap();
        for name in ["Shared", "Wall"] {
            image::RgbaImage::from_pixel(4, 4, image::Rgba([shade, 90, 40, 255]))
                .save(folder.join(format!("{name}.png")))
                .unwrap();
        }
    }
}

#[test]
fn picking_a_mod_while_asked_adds_a_rule_of_your_own() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let id = chosen(&mut cx, &mods, 5);
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.start_picking(Some(Picking::After), cx);
            let other = active_id(mods, 9);
            mods.choose(Side::Active, other, cx);
            assert!(
                mods.chosen_reads.picking.is_none(),
                "a pick ends the picking"
            );
            assert_eq!(
                mods.selected_id(cx),
                Some(id),
                "the mod chosen stays chosen"
            );
            let rules = &mods.details.as_ref().unwrap().your_rules;
            assert_eq!(rules.len(), 1);
            assert_eq!((rules[0].first, rules[0].then), (other, id));
        })
    });
}

#[test]
fn tags_and_a_note_typed_are_kept_for_the_mod_they_were_typed_for() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let id = chosen(&mut cx, &mods, 4);
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let app = crate::ui::app(cx);
            app.add_tags(&[id], "worn");
            mods.draft_note("First line\nsecond".into(), cx);
            mods.keep_drafts(cx);
            let details = mods.details.as_ref().unwrap();
            assert_eq!(details.tags, "worn");
            assert_eq!(details.note, "First line\nsecond");
            assert!(mods.chosen_reads.notes_saved);
            mods.chosen_reads.notes_saved = false;
            mods.keep_drafts(cx);
            assert!(
                !mods.chosen_reads.notes_saved,
                "nothing waiting is not said saved"
            );
        })
    });
}

#[test]
fn keeping_all_textures_from_the_earlier_mod_sets_the_later_ones_aside() {
    let (_tmp, mut cx, mods) = opened_with(shared_textures);
    cx.update(|cx| mods.update(cx, |mods, cx| mods.compare_first(cx)));
    let view = cx
        .update(|cx| mods.read(cx).comparing.clone())
        .expect("a comparison is shown");
    let comparison = cx.update(|cx| view.read(cx).comparison()).unwrap();
    assert_eq!((comparison.total, comparison.kept_earlier), (2, 0));
    cx.update(|cx| mods.update(cx, |mods, cx| mods.keep_all_compared(true, cx)));
    cx.run_until_parked();
    let comparison = cx.update(|cx| view.read(cx).comparison()).unwrap();
    assert_eq!(comparison.kept_earlier, 2);
    let told = cx.update(|cx| mods.read(cx).compared.clone()).unwrap();
    assert_eq!(told.kept_earlier, 2, "the details tell what was kept");
    cx.update(|cx| mods.update(cx, |mods, cx| mods.close_compare(cx)));
    assert!(cx.update(|cx| mods.read(cx).comparing.is_none()));
}

#[test]
fn keys_typed_in_the_filter_of_a_comparison_keep_no_texture() {
    let (_tmp, mut cx, mods) = opened_with(shared_textures);
    cx.update(|cx| mods.update(cx, |mods, cx| mods.compare_first(cx)));
    let view = cx.update(|cx| mods.read(cx).comparing.clone()).unwrap();
    let window = cx.update(|cx| cx.windows()[0]);
    use gpui::Focusable;
    cx.update_window(window, |_, window, cx| {
        let field = view.read(cx).filter.clone();
        window.focus(&field.focus_handle(cx), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.dispatch_keystroke(gpui::Keystroke::parse("2").unwrap(), cx);
    })
    .unwrap();
    cx.run_until_parked();
    let comparison = cx.update(|cx| view.read(cx).comparison()).unwrap();
    assert_eq!(
        comparison.yours, 0,
        "a key typed in the filter chose nothing"
    );
    assert_eq!(
        cx.update(|cx| view.read(cx).filter.read(cx).text().to_string()),
        "2"
    );
}

#[test]
fn a_comparison_holds_the_watcher_and_ends_when_the_mods_are_read_again() {
    let (_tmp, mut cx, mods) = opened_with(shared_textures);
    cx.update(|cx| mods.update(cx, |mods, cx| mods.compare_first(cx)));
    assert!(cx.update(|cx| mods.read(cx).held(cx)));
    let app = cx.update(|cx| crate::ui::app(cx));
    crate::jobs::waiting_here(|| app.rescan());
    cx.run_until_parked();
    assert!(cx.update(|cx| mods.read(cx).comparing.is_none()));
    assert!(!cx.update(|cx| mods.read(cx).held(cx)));
}

#[test]
fn tags_given_to_any_mod_are_offered_while_tagging_and_not_the_one_typed_or_taken() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let id = chosen(&mut cx, &mods, 4);
    let other = chosen(&mut cx, &mods, 9);
    let third = chosen(&mut cx, &mods, 12);
    let offered = |typed: &str, cx: &mut HeadlessAppContext| -> Vec<String> {
        let view = cx.update(|cx| mods.read(cx).details_view.clone());
        let window = cx.update(|cx| cx.windows()[0]);
        cx.update_window(window, |_, window, cx| {
            let field = view.read(cx).tags.clone();
            window.focus(&field.focus_handle(cx), cx);
            field.update(cx, |field, cx| field.set_text(typed, cx));
            let had = mods
                .read(cx)
                .details
                .as_ref()
                .map(|d| d.tags.clone())
                .unwrap_or_default();
            crate::ui::app(cx).tag_offers(&had, typed)
        })
        .unwrap()
        .into_iter()
        .map(|offer| offer.tag)
        .collect()
    };
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let app = crate::ui::app(cx);
            assert!(app.add_tags(&[other], "medieval, mechs"));
            assert!(app.add_tags(&[third], "medieval, keep"));
            mods.choose(Side::Active, id, cx);
        })
    });
    assert_eq!(
        offered("me", &mut cx),
        ["medieval", "mechs"],
        "the most used tag is offered first and the one typed is not"
    );
    assert_eq!(
        offered("medieval, ", &mut cx),
        ["keep", "mechs"],
        "the tags already typed are not offered again"
    );
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            assert!(crate::ui::app(cx).add_tags(&[id], "medieval"));
            mods.choose(Side::Active, id, cx);
        })
    });
    assert_eq!(
        offered("me", &mut cx),
        ["mechs"],
        "a tag the mod shown already has is not offered"
    );
}

#[gpui::test]
fn tags_typed_in_the_add_field_are_added_once_a_chip_takes_one_off_and_the_bin_takes_them_all() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let id = chosen(&mut cx, &mods, 4);
    cx.update(|cx| {
        crate::ui::app(cx).set_note_text(id, "a note");
    });
    let tags = |cx: &mut HeadlessAppContext| -> String {
        cx.update(|cx| {
            mods.read(cx)
                .details
                .as_ref()
                .map(|d| d.tags.clone())
                .unwrap_or_default()
        })
    };
    let view = cx.update(|cx| mods.read(cx).details_view.clone());
    let window = cx.update(|cx| cx.windows()[0]);
    cx.update_window(window, |_, window, cx| {
        let field = view.read(cx).tags.clone();
        window.focus(&field.focus_handle(cx), cx);
        field.update(cx, |field, cx| field.set_text("second, Third", cx));
        window.dispatch_keystroke(gpui::Keystroke::parse("enter").unwrap(), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        tags(&mut cx),
        "second, Third",
        "the tags typed are added once, whatever their case"
    );
    cx.update_window(window, |_, window, cx| {
        let field = view.read(cx).tags.clone();
        assert_eq!(field.read(cx).text(), "", "the add field is cleared");
        field.update(cx, |field, cx| field.set_text("SECOND", cx));
        window.dispatch_keystroke(gpui::Keystroke::parse("enter").unwrap(), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        tags(&mut cx),
        "second, Third",
        "a tag the mod already has is not added again"
    );
    cx.update(|cx| mods.update(cx, |mods, cx| mods.remove_tag("SECOND", cx)));
    assert_eq!(
        tags(&mut cx),
        "Third",
        "one tag is taken off alone, the note stays"
    );
    cx.update(|cx| mods.update(cx, |mods, cx| mods.clear_tags(cx)));
    assert_eq!(tags(&mut cx), "", "all tags are taken off");
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            mods.show(cx);
            let note = mods.details.as_ref().unwrap().note.clone();
            assert_eq!(note, "a note", "the note stays when the tags go");
        })
    });
}

fn shared_defs(root: &std::path::Path) {
    for n in ["004", "007"] {
        let folder = root.join(format!("fx/game/Mods/fixture-{n}/Defs"));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("Shared.xml"),
            "<Defs><ThingDef><defName>Shared_Thing</defName></ThingDef></Defs>",
        )
        .unwrap();
    }
}

#[test]
fn two_mods_defining_the_same_def_are_said_to_clash_the_later_winning() {
    let (_tmp, mut cx, mods) = opened_with(shared_defs);
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            let order = &mods.list.read(cx).order;
            let list = order.list(Side::Active);
            let mut found: Vec<ModId> = Vec::new();
            let mut names: Vec<String> = Vec::new();
            for m in list.mods() {
                if m.name != "Fixture Mod 004" && m.name != "Fixture Mod 007" {
                    continue;
                }
                let Some(installed) = m.installed else {
                    continue;
                };
                found.push(installed);
                names.push(m.name.clone());
            }
            assert_eq!(names, ["Fixture Mod 007", "Fixture Mod 004"]);
            let (earlier, later) = (found[0], found[1]);
            mods.choose(Side::Active, earlier, cx);
            let details = mods.details.as_ref().unwrap();
            assert_eq!(details.defs_replaced_by.len(), 1);
            assert_eq!(details.defs_replaced_by[0].mod_id, later);
            assert_eq!(details.defs_replaced_by[0].defs, 1);
            assert_eq!(
                details.defs_replaced_by[0].some.to_string(),
                "Shared_Thing (ThingDef)"
            );
            assert!(details.defs_replaces.is_empty());
            mods.choose(Side::Active, later, cx);
            let details = mods.details.as_ref().unwrap();
            assert_eq!(details.defs_replaces.len(), 1);
            assert!(details.defs_replaced_by.is_empty());
            let flagged = mods
                .list
                .read(cx)
                .order
                .list(Side::Active)
                .flagged(rimstack_core::mods::list::flag::DEFCLASH);
            assert_eq!(flagged, 2);
        })
    });
}

#[gpui::test]
fn a_tag_offer_is_chosen_with_the_arrows_enter_adds_it_and_escape_closes_the_offers() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let id = chosen(&mut cx, &mods, 4);
    let other = chosen(&mut cx, &mods, 9);
    let type_tags = |typed: &str, cx: &mut HeadlessAppContext| {
        let view = cx.update(|cx| mods.read(cx).details_view.clone());
        let window = cx.update(|cx| cx.windows()[0]);
        cx.update_window(window, |_, window, cx| {
            let field = view.read(cx).tags.clone();
            window.focus(&field.focus_handle(cx), cx);
            field.update(cx, |field, cx| field.set_text(typed, cx));
        })
        .unwrap();
    };
    let tags = |cx: &mut HeadlessAppContext| -> String {
        cx.update(|cx| {
            mods.read(cx)
                .details
                .as_ref()
                .map(|d| d.tags.clone())
                .unwrap_or_default()
        })
    };
    let enter = |cx: &mut HeadlessAppContext| {
        let window = cx.update(|cx| cx.windows()[0]);
        cx.update_window(window, |_, window, cx| {
            window.dispatch_keystroke(gpui::Keystroke::parse("enter").unwrap(), cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    let key = |name: &'static str, cx: &mut HeadlessAppContext| {
        let window = cx.update(|cx| cx.windows()[0]);
        cx.update_window(window, |_, window, cx| {
            window.dispatch_keystroke(gpui::Keystroke::parse(name).unwrap(), cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            assert!(crate::ui::app(cx).add_tags(&[other], "pointer-pick, pointer-second"));
            mods.choose(Side::Active, id, cx);
        })
    });
    // Enter with no offer picked adds what is typed.
    type_tags("pointer-p", &mut cx);
    enter(&mut cx);
    assert_eq!(tags(&mut cx), "pointer-p");
    // Down then Enter adds the offered tag.
    type_tags("pointer-p", &mut cx);
    key("down", &mut cx);
    enter(&mut cx);
    assert_eq!(tags(&mut cx), "pointer-p, pointer-pick");
    // Escape closes the offers, so Enter then adds what is typed.
    type_tags("pointer-s", &mut cx);
    key("escape", &mut cx);
    enter(&mut cx);
    assert_eq!(
        tags(&mut cx),
        "pointer-p, pointer-pick, pointer-s",
        "with the offers closed Enter adds the typed tag, not the offered one"
    );
}

#[gpui::test]
fn copied_tags_replace_a_mods_tags_or_join_them_and_one_undo_gives_each_its_own_back() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    let id = chosen(&mut cx, &mods, 4);
    let other = chosen(&mut cx, &mods, 9);
    let tags = |cx: &mut HeadlessAppContext, of: ModId| -> String {
        cx.update(|cx| crate::ui::app(cx).tags_of(&[of]).remove(0))
    };
    let group = |cx: &mut HeadlessAppContext, of: ModId| -> String {
        cx.update(|cx| {
            let app = crate::ui::app(cx);
            let loaded = app.library();
            let key = loaded
                .as_ref()
                .as_ref()
                .ok()
                .and_then(|(library, _)| library.get(of).and_then(|m| m.key()));
            drop(loaded);
            key.and_then(|key| {
                app.notes()
                    .read()
                    .unwrap()
                    .get(&key)
                    .map(|note| note.group().to_string())
            })
            .unwrap_or_default()
        })
    };
    cx.update(|cx| {
        mods.update(cx, |mods, cx| {
            assert!(crate::ui::app(cx).add_tags(&[other], "pointer-own"));
            mods.choose_nth(Side::Active, 4, false, cx);
            mods.choose_nth(Side::Active, 9, true, cx);
        })
    });
    // Copying a mod's tags keeps them for pasting and puts them on the clipboard.
    let copied = "pointer-copied, pointer-second".to_string();
    cx.update(|cx| mods.update(cx, |mods, cx| mods.copy_tags(copied.clone(), cx)));
    assert_eq!(cx.update(|cx| mods.read(cx).copied_tags.clone()), copied);
    // Pasting gives both chosen mods the copied tags, the first one their group.
    cx.update(|cx| mods.update(cx, |mods, cx| mods.paste_tags(vec![id, other], false, cx)));
    cx.run_until_parked();
    assert_eq!(tags(&mut cx, other), copied);
    assert_eq!(group(&mut cx, other), "pointer-copied");
    assert_eq!(tags(&mut cx, id), copied);
    assert_eq!(
        cx.update(|cx| crate::ui::notices::earlier(cx))[0]
            .0
            .to_string(),
        "Tags pasted onto 2 mods"
    );
    // One Undo in the notice gives each its own tags and group back.
    let undo = cx
        .update(|cx| crate::ui::notices::newest_offer(cx))
        .expect("the paste offers Undo");
    assert_eq!(undo.0.to_string(), "Undo");
    let window = cx.update(|cx| cx.windows()[0]);
    cx.update_window(window, |_, window, cx| (undo.1)(window, cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(tags(&mut cx, other), "pointer-own");
    assert_eq!(group(&mut cx, other), "pointer-own");
    assert_eq!(tags(&mut cx, id), "");
    // Adding the copied tags keeps those a mod has, its own first.
    cx.update(|cx| mods.update(cx, |mods, cx| mods.paste_tags(vec![other], true, cx)));
    cx.run_until_parked();
    assert_eq!(
        tags(&mut cx, other),
        "pointer-own, pointer-copied, pointer-second"
    );
    assert_eq!(group(&mut cx, other), "pointer-own");
}

#[test]
fn a_short_fact_takes_one_line_rather_than_one_letter_a_line() {
    let (_tmp, mut cx, mods) = opened_with(|_| {});
    chosen(&mut cx, &mods, 3);
    let window = cx.update(|cx| cx.windows()[0]);
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    }
    let field = cx
        .update(|cx| {
            mods.read(cx)
                .details_view
                .read(cx)
                .copyable_field("fact-contains")
        })
        .expect("the mod's contents are shown");
    let (text, rows) = cx.update(|cx| {
        let field = field.read(cx);
        (field.text().to_owned(), field.rows_drawn())
    });
    assert!(text.len() > 3, "a fact long enough to tell: {text:?}");
    assert_eq!(rows, 1, "{text:?} fits one line of the details pane");
}
