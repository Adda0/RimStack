use super::button::Btn;
use super::controls::{Choice, Slide, Toggle};
use super::menu::Menu;
use super::{Options, focus, install, menu, notices};
use crate::App;
use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, prelude::FluentBuilder,
};
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
struct Bench {
    first: usize,
    second: usize,
    on: bool,
    picked: usize,
    slid: f32,
    /// Keys the screen behind the controls was given.
    keys: usize,
    chosen: Option<&'static str>,
    menu: Option<Entity<menu::Menu>>,
    notices: Option<Entity<notices::Notices>>,
    focus: Option<gpui::FocusHandle>,
}

impl Render for Bench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.entity().downgrade();
        let pick = me.clone();
        let slide = me.clone();
        div()
            .size_full()
            .when_some(self.focus.clone(), |d, focus| d.track_focus(&focus))
            .on_key_down(cx.listener(|b, _: &gpui::KeyDownEvent, _, cx| {
                b.keys += 1;
                cx.notify();
            }))
            .on_action(|_: &focus::Next, window, cx| window.focus_next(cx))
            .on_action(|_: &focus::Previous, window, cx| window.focus_prev(cx))
            .child(Btn::new("first", "First").on_click(cx.listener(|b, _, _, _| b.first += 1)))
            .child(Btn::new("second", "Second").on_click(cx.listener(|b, _, _, _| b.second += 1)))
            .child(
                Toggle::new("toggle", self.on, "Toggle").on_flip(move |on, _, cx| {
                    let _ = me.update(cx, |b, cx| {
                        b.on = on;
                        cx.notify();
                    });
                }),
            )
            .child(
                Choice::new(
                    "choice",
                    vec!["A".into(), "B".into(), "C".into()],
                    self.picked,
                )
                .on_pick(move |at, _, cx| {
                    let _ = pick.update(cx, |b, cx| {
                        b.picked = at;
                        cx.notify();
                    });
                }),
            )
            .child(
                Slide::new("slide", (0., 1., 0.1), self.slid).on_move(move |at, _, cx| {
                    let _ = slide.update(cx, |b, cx| {
                        b.slid = at;
                        cx.notify();
                    });
                }),
            )
            .children(self.notices.clone())
            .children(self.menu.clone())
    }
}

fn bench(cx: &mut TestAppContext) -> (tempfile::TempDir, Entity<Bench>, &mut VisualTestContext) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    let app = App::new(
        path.join("home"),
        &path.join("settings"),
        &path.join("data"),
        Default::default(),
    );
    cx.update(|cx| {
        install(
            Arc::new(app),
            &Options {
                demo: true,
                ..Options::default()
            },
            cx,
        )
    });
    let (bench, cx) = cx.add_window_view(|_, cx| Bench {
        menu: Some(menu::install(cx)),
        notices: Some(notices::install(cx)),
        focus: Some(cx.focus_handle()),
        ..Bench::default()
    });
    cx.update(|window, cx| {
        let focus = bench.read(cx).focus.clone().unwrap();
        window.focus(&focus, cx);
    });
    cx.run_until_parked();
    (root, bench, cx)
}

/// Each key down then up, as a keyboard sends them: a control is pressed on the key's release.
pub(crate) fn press(cx: &mut VisualTestContext, keys: &'static str) {
    for key in keys.split_whitespace() {
        cx.simulate_keystrokes(key);
        let keystroke = gpui::Keystroke::parse(key).unwrap();
        cx.simulate_event(gpui::KeyUpEvent { keystroke });
    }
}

#[gpui::test]
fn tab_reaches_each_control_and_enter_or_space_presses_it(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    press(cx, "tab enter tab space");
    assert_eq!(bench.read_with(cx, |b, _| (b.first, b.second)), (1, 1));
    press(cx, "tab space");
    assert!(bench.read_with(cx, |b, _| b.on), "Space flips the toggle");
    press(cx, "tab right right");
    assert_eq!(
        bench.read_with(cx, |b, _| b.picked),
        2,
        "the arrows move the choice"
    );
    press(cx, "shift-tab space");
    assert!(!bench.read_with(cx, |b, _| b.on), "Shift+Tab goes back");
}

fn frames_asked(cx: &mut VisualTestContext) -> usize {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        window.simulate_next_frame(cx)
    })
}

#[gpui::test]
fn a_value_from_elsewhere_shows_at_once_and_a_pick_slides_to_its_place(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    cx.simulate_mouse_move(
        gpui::point(gpui::px(900.), gpui::px(900.)),
        None,
        gpui::Modifiers::none(),
    );
    while frames_asked(cx) > 0 {
        cx.executor().advance_clock(Duration::from_millis(16));
    }
    bench.update(cx, |b, cx| {
        b.picked = 2;
        b.on = true;
        cx.notify();
    });
    assert_eq!(frames_asked(cx), 0, "nothing slides that was not pressed");
    press(cx, "tab tab tab tab left");
    assert_eq!(bench.read_with(cx, |b, _| b.picked), 1);
    assert!(frames_asked(cx) > 0, "the pick slides");
    let settled = (0..60).find(|_| {
        cx.executor().advance_clock(Duration::from_millis(16));
        frames_asked(cx) == 0
    });
    assert!(settled.is_some(), "the slide comes to rest");
}

#[gpui::test]
fn a_button_pointed_at_fades_to_its_hover_colour_and_rests(cx: &mut TestAppContext) {
    let (_root, _bench, cx) = bench(cx);
    frames_asked(cx);
    cx.simulate_mouse_move(
        gpui::point(gpui::px(6.), gpui::px(6.)),
        None,
        gpui::Modifiers::none(),
    );
    assert!(frames_asked(cx) > 0, "the hover colour fades in");
    let settled = (0..60).find(|_| {
        cx.executor().advance_clock(Duration::from_millis(16));
        frames_asked(cx) == 0
    });
    assert!(settled.is_some(), "the fade comes to rest");
}

#[gpui::test]
fn a_menu_takes_the_ends_the_page_keys_space_and_letters(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    let entries = || {
        let entry = |said: &'static str| -> menu::Entry {
            let bench = bench.downgrade();
            menu::Entry::item(said, move |_, cx| {
                let _ = bench.update(cx, |b, _| b.chosen = Some(said));
            })
            .into()
        };
        vec![
            entry("Alpha"),
            entry("Beta"),
            menu::Entry::Line,
            entry("Bravo"),
            entry("Gamma"),
            entry("Delta"),
        ]
    };
    cx.update(|window, cx| {
        menu::open_at(
            gpui::point(gpui::px(10.), gpui::px(10.)),
            entries(),
            window,
            cx,
        )
    });
    // End to the last entry, Home to the first, a letter to the next entry starting with it
    // (wrapping past the end), the page keys as far as a page.
    let at = |cx: &mut VisualTestContext| cx.update(|_, cx| menu::chosen(cx));
    press(cx, "end");
    assert_eq!(at(cx), Some(5), "End reaches Delta");
    press(cx, "home");
    assert_eq!(at(cx), Some(0), "Home reaches Alpha");
    press(cx, "b");
    assert_eq!(at(cx), Some(1), "a letter reaches Beta");
    press(cx, "b");
    assert_eq!(at(cx), Some(3), "and the next one starting with it, Bravo");
    press(cx, "b");
    assert_eq!(at(cx), Some(1), "wrapping to Beta again");
    press(cx, "pagedown");
    assert_eq!(at(cx), Some(3), "Page Down moves a page along");
    press(cx, "pageup");
    assert_eq!(at(cx), Some(1), "Page Up moves a page back");
    press(cx, "enter");
    assert_eq!(
        bench.read_with(cx, |b, _| b.chosen),
        Some("Beta"),
        "Enter takes the entry the keys are on"
    );
    let open = |cx: &mut VisualTestContext| cx.update(|_, cx| menu::drawn(cx));
    let closed = (0..60).find(|_| {
        cx.executor().advance_clock(Duration::from_millis(16));
        !open(cx)
    });
    assert!(closed.is_some(), "and the menu closes");
    cx.update(|window, cx| {
        menu::open_at(
            gpui::point(gpui::px(10.), gpui::px(10.)),
            entries(),
            window,
            cx,
        )
    });
    press(cx, "down down space");
    assert_eq!(
        bench.read_with(cx, |b, _| b.chosen),
        Some("Beta"),
        "Space takes the entry too"
    );
}

#[gpui::test]
fn a_slider_takes_the_ends_and_the_page_keys_and_a_choice_the_ends(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    // Tab past the buttons, the toggle and the choice to the slider.
    press(cx, "tab tab tab tab tab");
    press(cx, "end");
    assert_eq!(
        bench.read_with(cx, |b, _| b.slid),
        1.,
        "End reaches the top"
    );
    press(cx, "home");
    assert_eq!(
        bench.read_with(cx, |b, _| b.slid),
        0.,
        "Home reaches the bottom"
    );
    press(cx, "pageup");
    assert_eq!(
        bench.read_with(cx, |b, _| b.slid),
        0.,
        "Page Up at the bottom stays"
    );
    press(cx, "pagedown");
    assert_eq!(
        bench.read_with(cx, |b, _| b.slid),
        0.3,
        "Page Down moves a quarter of the way, on a step"
    );
    press(cx, "shift-tab end");
    assert_eq!(
        bench.read_with(cx, |b, _| b.picked),
        2,
        "End takes the choice's last option"
    );
    press(cx, "home");
    assert_eq!(
        bench.read_with(cx, |b, _| b.picked),
        0,
        "Home takes the choice's first option"
    );
}

#[gpui::test]
fn a_menu_is_chosen_from_with_the_arrows_and_closes_on_escape(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    let entries = |bench: &Entity<Bench>| -> Vec<menu::Entry> {
        let (one, two) = (bench.downgrade(), bench.downgrade());
        vec![
            menu::Entry::Title("Things".into()),
            menu::Entry::item("Off", |_, _| {}).off(true).into(),
            menu::Entry::item("One", move |_, cx| {
                let _ = one.update(cx, |b, _| b.chosen = Some("one"));
            })
            .into(),
            menu::Entry::Line,
            menu::Entry::item("Two", move |_, cx| {
                let _ = two.update(cx, |b, _| b.chosen = Some("two"));
            })
            .into(),
        ]
    };
    cx.update(|window, cx| {
        menu::open_at(
            gpui::point(gpui::px(10.), gpui::px(10.)),
            entries(&bench),
            window,
            cx,
        )
    });
    cx.simulate_keystrokes("down down enter");
    assert_eq!(
        bench.read_with(cx, |b, _| b.chosen),
        Some("two"),
        "the arrows skip what is off"
    );
    cx.update(|window, cx| {
        menu::open_at(
            gpui::point(gpui::px(10.), gpui::px(10.)),
            entries(&bench),
            window,
            cx,
        )
    });
    cx.simulate_keystrokes("escape enter");
    assert_eq!(
        bench.read_with(cx, |b, _| b.chosen),
        Some("two"),
        "Escape closes it without a choice"
    );
}

#[gpui::test]
fn a_menu_taller_than_the_window_stays_inside_it_and_follows_the_keys(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    let entries: Vec<menu::Entry> = (0..80)
        .map(|at| {
            let me = bench.downgrade();
            menu::Entry::item(format!("check entry {at}"), move |_, cx| {
                let _ = me.update(cx, |b, _| b.chosen = Some("chosen"));
            })
            .into()
        })
        .collect();
    cx.update(|window, cx| {
        menu::open_at(
            gpui::point(gpui::px(10.), gpui::px(10.)),
            entries,
            window,
            cx,
        );
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    let (menu, window) = cx.update(|window, cx| {
        (
            menu::shown(cx).expect("the menu is shown"),
            window.viewport_size(),
        )
    });
    menu.read_with(cx, |menu, _| {
        let list = menu.list();
        assert!(menu.scrolls(), "an 80-entry menu is taller than the window");
        assert!(
            list.bottom() <= window.height,
            "a menu taller than the window stays inside it"
        );
    });
    cx.simulate_keystrokes("up");
    let (at, _) = menu.read_with(cx, |menu, _| menu.seen().expect("the keys are on an entry"));
    assert!(at > 40, "the up key steps to the last entry");
    // Drawn twice: once to land the scroll the key asked for, once to lay the entries out
    // under it.
    for _ in 0..2 {
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    let (list, spot, down) = menu.read_with(cx, |menu, _| {
        (
            menu.list(),
            menu.seen().expect("the keys are on an entry"),
            menu.down(),
        )
    });
    // `bounds_for_item` speaks in the content's space: the scroll is taken off it.
    let (top, bottom) = (spot.1.top() - down, spot.1.bottom() - down);
    assert!(
        top >= list.top() && bottom <= list.bottom(),
        "and it scrolls to the entry the keys are on: chosen {} at {:?}..{:?} in {:?}",
        spot.0,
        top,
        bottom,
        list
    );
}

#[gpui::test]
fn a_menu_with_picks_narrows_them_to_what_one_types(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    let entries = |bench: &Entity<Bench>| -> Vec<menu::Entry> {
        vec![
            menu::Entry::Title("Group under a tag".into()),
            menu::Entry::Line,
        ]
        .into_iter()
        .chain(["pick-one", "pick-two", "pick-three"].map(|tag| {
            let me = bench.downgrade();
            menu::Entry::item(tag, move |_, cx| {
                let _ = me.update(cx, |b, _| b.chosen = Some(tag));
            })
            .pick()
            .into()
        }))
        .chain([menu::Entry::item("A new tag…", |_, _| {}).into()])
        .collect()
    };
    cx.update(|window, cx| {
        menu::open_at(
            gpui::point(gpui::px(10.), gpui::px(10.)),
            entries(&bench),
            window,
            cx,
        )
    });
    cx.run_until_parked();
    let menu = cx.update(|_, cx| menu::shown(cx).expect("the menu is shown"));
    menu.read_with(cx, |menu, cx| {
        let field = menu.search.as_ref().expect("a search field over the picks");
        assert!(field.read(cx).is_focused_now(), "the field holds the keys");
    });
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.simulate_input("two");
    menu.read_with(cx, |menu, cx| {
        assert_eq!(menu.query(cx), "two");
        assert!(
            Menu::left(&menu.entries[3], "two"),
            "pick-two holds the text"
        );
        assert!(!Menu::left(&menu.entries[2], "two"), "pick-one does not");
        assert!(Menu::left(&menu.entries[5], "two"), "a plain entry stays");
    });
    cx.simulate_keystrokes("enter");
    assert_eq!(
        bench.read_with(cx, |b, _| b.chosen),
        Some("pick-two"),
        "Enter takes the first pick the search left"
    );
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(!cx.update(|_, cx| menu::drawn(cx)), "the menu is closed");
}

#[gpui::test]
fn a_toast_goes_by_itself_after_a_while(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    cx.update(|_, cx| notices::say("Saved", notices::Tone::Ok, cx));
    let shown = |cx: &mut VisualTestContext| {
        let notices = bench.read_with(cx, |b, _| b.notices.clone().unwrap());
        notices.read_with(cx, |n, _| n.shown().len())
    };
    assert_eq!(shown(cx), 1);
    cx.executor().advance_clock(Duration::from_secs(6));
    cx.run_until_parked();
    assert_eq!(shown(cx), 0);
}

#[gpui::test]
fn a_toast_holding_the_keys_stays_until_they_leave(cx: &mut TestAppContext) {
    let (_root, bench, cx) = bench(cx);
    cx.update(|_, cx| notices::say("Saved", notices::Tone::Ok, cx));
    let notices = bench.read_with(cx, |b, _| b.notices.clone().unwrap());
    let shown = |cx: &mut VisualTestContext| notices.read_with(cx, |n, _| n.shown().len());
    cx.update(|window, cx| notices.update(cx, |n, cx| n.give_keys(window, cx)));
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(12));
    cx.run_until_parked();
    assert_eq!(shown(cx), 1, "kept up while its button has the keys");
    cx.update(|window, cx| {
        let focus = bench.read(cx).focus.clone().unwrap();
        window.focus(&focus, cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(6));
    cx.run_until_parked();
    assert_eq!(shown(cx), 0, "and goes once they leave");
}

struct Described {
    selection: super::rich::Selection,
    focus: gpui::FocusHandle,
}

impl Render for Described {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<super::theme::Theme>().clone();
        let blocks = rimstack_core::base::markup::blocks("First part.\n\nSecond part.");
        let link: super::rich::Link = std::rc::Rc::new(|_, _, _| {});
        div()
            .size_full()
            .track_focus(&self.focus)
            .on_action(|_: &focus::Next, window, cx| window.focus_next(cx))
            .child(self.selection.holding(
                div().children(super::rich::blocks(
                    "described",
                    &theme,
                    &blocks,
                    &link,
                    Some(&self.selection),
                )),
                &theme,
            ))
    }
}

#[gpui::test]
fn tab_reaches_a_description_and_ctrl_c_copies_all_of_it(cx: &mut TestAppContext) {
    let (_root, _bench, _) = bench(cx);
    let (view, cx) = cx.add_window_view(|_, cx| Described {
        selection: super::rich::Selection::new(cx),
        focus: cx.focus_handle(),
    });
    cx.update(|window, cx| {
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
    });
    cx.run_until_parked();
    press(cx, "tab");
    assert!(cx.update(|window, cx| window.focused(cx)).is_some());
    press(cx, "ctrl-c");
    let copied = cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(copied.as_deref(), Some("First part.\n\nSecond part."));
}

struct Asking {
    drawn: usize,
}

impl Render for Asking {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drawn += 1;
        let me = cx.entity_id();
        let first = self.drawn == 1;
        div().size_full().child(
            gpui::canvas(
                move |_, _, cx| {
                    if first {
                        super::redraw_after(me, cx);
                    }
                },
                |_, _, _, _| {},
            )
            .size_full(),
        )
    }
}

#[test]
fn a_view_that_asks_while_it_is_drawn_is_drawn_again_after() {
    let mut cx = gpui::HeadlessAppContext::with_asset_source(
        super::made_up::text_system(),
        Arc::new(super::assets::Assets),
    );
    let window = cx
        .open_window(gpui::size(gpui::px(200.), gpui::px(100.)), |_, cx| {
            cx.new(|_| Asking { drawn: 0 })
        })
        .unwrap();
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
    }
    let drawn = cx.update(|cx| window.read(cx).unwrap().drawn);
    assert_eq!(drawn, 2);
}

struct Hearer(Vec<crate::changes::Changes>);

#[gpui::test]
fn a_view_hears_only_what_it_reads(cx: &mut TestAppContext) {
    use crate::changes::Changes;
    let (_root, _bench, cx) = bench(cx);
    let hearer = cx.update(|_, cx| {
        cx.new(|cx| {
            super::hear(
                cx,
                Changes::LOGS | Changes::SAVES,
                |hearer: &mut Hearer, changes, _| hearer.0.push(changes),
            );
            Hearer(Vec::new())
        })
    });
    let publish = |changes, cx: &mut VisualTestContext| {
        cx.update(|_, cx| super::app(cx).publish(changes));
        cx.run_until_parked();
    };
    publish(Changes::FACTS, cx);
    publish(Changes::FACTS | Changes::LOGS, cx);
    assert_eq!(hearer.read_with(cx, |h, _| h.0.clone()), [Changes::LOGS]);
}

struct Hearing;

impl Render for Hearing {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[gpui::test]
fn a_view_released_stops_hearing_at_the_next_change_whatever_it_is(cx: &mut TestAppContext) {
    let (_root, _bench, cx) = bench(cx);
    let app = cx.update(|_, cx| super::app(cx));
    let before = app.listening();
    let hearing = cx.update(|_, cx| {
        cx.new(|cx| {
            super::hear(cx, crate::changes::Changes::NOTES, |_, _, _| {});
            Hearing
        })
    });
    assert_eq!(app.listening(), before + 1);
    drop(hearing);
    cx.update(|_, _| {});
    cx.run_until_parked();
    app.publish(crate::changes::Changes::RULES);
    assert_eq!(app.listening(), before);
}
