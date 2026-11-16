use super::made_up::made_up;
use super::shell::Shell;
use super::{Options, install};
use gpui::{AppContext, HeadlessAppContext, WindowHandle};
use std::sync::Arc;
use std::time::Duration;

fn opened() -> (tempfile::TempDir, HeadlessAppContext, WindowHandle<Shell>) {
    let (tmp, app) = made_up();
    let text = super::made_up::text_system();
    let mut cx = HeadlessAppContext::with_asset_source(text, Arc::new(super::assets::Assets));
    let options = Options::default();
    cx.update(|cx| install(Arc::new(app), &options, cx));
    let shell = cx
        .open_window(
            gpui::size(super::WINDOW.0, super::WINDOW.1),
            |window, cx| cx.new(|cx| Shell::new(&options, window, cx)),
        )
        .unwrap();
    cx.run_until_parked();
    (tmp, cx, shell)
}

fn frame(cx: &mut HeadlessAppContext, shell: WindowHandle<Shell>) -> usize {
    cx.advance_clock(Duration::from_millis(16));
    cx.run_until_parked();
    cx.update_window(shell.into(), |_, window, cx| {
        let asked = window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
        asked
    })
    .unwrap()
}

fn settled(cx: &mut HeadlessAppContext, shell: WindowHandle<Shell>) {
    for _ in 0..120 {
        if frame(cx, shell) == 0 {
            return;
        }
    }
    panic!("frames were still asked for");
}

fn key(cx: &mut HeadlessAppContext, shell: WindowHandle<Shell>, k: &'static str) {
    for each in k.split_whitespace() {
        cx.update_window(shell.into(), |_, window, cx| {
            window.dispatch_keystroke(gpui::Keystroke::parse(each).unwrap(), cx)
        })
        .unwrap();
        // Enter and Space act on the key coming up, as gpui does it.
        cx.update_window(shell.into(), |_, window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::KeyUp(gpui::KeyUpEvent {
                    keystroke: gpui::Keystroke::parse(each).unwrap(),
                }),
                cx,
            );
        })
        .unwrap();
    }
}

fn focused(cx: &mut HeadlessAppContext, shell: WindowHandle<Shell>) -> Option<gpui::FocusHandle> {
    cx.update_window(shell.into(), |_, window, cx| window.focused(cx))
        .unwrap()
}

fn ask(
    cx: &mut HeadlessAppContext,
    shell: WindowHandle<Shell>,
    ran: std::rc::Rc<std::cell::Cell<usize>>,
) {
    cx.update_window(shell.into(), |_, window, cx| {
        let question = super::prompt::Question {
            title: "Check".into(),
            lines: Vec::new(),
            label: "Run".into(),
            danger: false,
            run: Box::new(move |_, _| ran.set(ran.get() + 1)),
        };
        super::prompt::ask(question, window, cx);
    })
    .unwrap();
}

fn asking(cx: &mut HeadlessAppContext) -> bool {
    cx.update(|cx| super::prompt::shown(cx).is_some())
}

#[test]
fn a_confirmation_takes_the_keys_and_enter_runs_its_main_action() {
    let (_tmp, mut cx, shell) = opened();
    settled(&mut cx, shell);
    // Something under the card holds the keys, so a stray Enter would reach it.
    key(&mut cx, shell, "down");
    let ran = std::rc::Rc::new(std::cell::Cell::new(0));
    let chosen = selected(&mut cx, shell);
    ask(&mut cx, shell, ran.clone());
    frame(&mut cx, shell);
    let on_answer = focused(&mut cx, shell);
    assert!(on_answer.is_some(), "the question opens on its main button");
    key(&mut cx, shell, "enter");
    cx.run_until_parked();
    assert_eq!(ran.get(), 1, "Enter runs the main action");
    assert_eq!(
        selected(&mut cx, shell),
        chosen,
        "Enter did not move the mod under the card"
    );
    assert!(!asking(&mut cx), "the question is gone");
}

#[test]
fn tab_stays_inside_a_confirmation_and_esc_cancels_it() {
    let (_tmp, mut cx, shell) = opened();
    settled(&mut cx, shell);
    let ran = std::rc::Rc::new(std::cell::Cell::new(0));
    ask(&mut cx, shell, ran.clone());
    frame(&mut cx, shell);
    key(&mut cx, shell, "tab");
    let after_tab = focused(&mut cx, shell);
    assert!(after_tab.is_some(), "Tab stays inside the question");
    key(&mut cx, shell, "tab tab");
    let round = focused(&mut cx, shell);
    assert!(round.is_some(), "Tab rounds the question's buttons");
    assert!(asking(&mut cx), "still open until Esc");
    key(&mut cx, shell, "escape");
    cx.run_until_parked();
    assert!(!asking(&mut cx), "Esc cancels it");
    assert_eq!(ran.get(), 0, "nothing ran");
}

#[test]
fn the_keys_go_back_where_they_were_once_a_question_is_answered() {
    let (_tmp, mut cx, shell) = opened();
    settled(&mut cx, shell);
    // The lists hold the keys; the question gives them back when it closes.
    let lists = focus_lists(&mut cx, shell);
    let ran = std::rc::Rc::new(std::cell::Cell::new(0));
    ask(&mut cx, shell, ran.clone());
    frame(&mut cx, shell);
    assert!(asking(&mut cx));
    key(&mut cx, shell, "escape");
    cx.run_until_parked();
    let back = focused(&mut cx, shell);
    assert_eq!(back, lists, "the keys go back to the list");
}

#[test]
fn the_before_you_play_card_takes_the_keys_and_tab_rounds_it() {
    let (_tmp, mut cx, shell) = opened();
    settled(&mut cx, shell);
    shell
        .update(&mut cx, |shell, window, cx| shell.press_play(window, cx))
        .unwrap();
    frame(&mut cx, shell);
    let open = cx.update(|cx| shell.read(cx).unwrap().before_play);
    assert!(open, "the made-up load order is in the way");
    let first = focused(&mut cx, shell);
    assert!(first.is_some(), "the card opens on its first button");
    for _ in 0..5 {
        key(&mut cx, shell, "tab");
        let now = focused(&mut cx, shell);
        assert!(now.is_some(), "Tab stays inside the card");
    }
    key(&mut cx, shell, "escape");
    cx.run_until_parked();
    assert!(!cx.update(|cx| shell.read(cx).unwrap().before_play));
}

#[test]
fn the_saved_lists_card_takes_the_keys_and_tab_stays_inside() {
    let (_tmp, mut cx, shell) = opened();
    settled(&mut cx, shell);
    let lists = cx.update(|cx| shell.read(cx).unwrap().lists.clone());
    cx.update_window(shell.into(), |_, window, cx| {
        lists.update(cx, |lists, cx| {
            lists.toggle(
                super::lists::Which::Lists,
                gpui::Bounds::default(),
                window,
                cx,
            )
        })
    })
    .unwrap();
    frame(&mut cx, shell);
    let first = focused(&mut cx, shell);
    let keys = cx.update(|cx| lists.read(cx).keys_handle());
    assert_eq!(first, Some(keys), "the card opens on its first control");
    for _ in 0..4 {
        key(&mut cx, shell, "tab");
        let now = focused(&mut cx, shell);
        assert!(now.is_some(), "Tab stays inside the card");
    }
    key(&mut cx, shell, "escape");
    let gone = (0..30).take_while(|_| frame(&mut cx, shell) > 0).count();
    assert!(gone < 30, "the card's leaving was drawn");
    assert!(!cx.update(|cx| lists.read(cx).drawn()));
}

fn selected(
    cx: &mut HeadlessAppContext,
    shell: WindowHandle<Shell>,
) -> Option<rimstack_core::mods::library::ModId> {
    cx.update(|cx| {
        shell
            .read(cx)
            .unwrap()
            .sections
            .mods
            .read(cx)
            .selected_id(cx)
    })
}

fn focus_lists(
    cx: &mut HeadlessAppContext,
    shell: WindowHandle<Shell>,
) -> Option<gpui::FocusHandle> {
    let lists = cx.update(|cx| shell.read(cx).unwrap().lists.clone());
    cx.update_window(shell.into(), |_, window, cx| {
        lists.update(cx, |lists, cx| {
            lists.toggle(
                super::lists::Which::Lists,
                gpui::Bounds::default(),
                window,
                cx,
            )
        })
    })
    .unwrap();
    frame(cx, shell);
    focused(cx, shell)
}
