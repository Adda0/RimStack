//! A list of what can be done, opened under the control that asks for it, closed
//! by a choice, Escape or a click beside it. Any view opens one through [`open`]; the shell draws
//! it over everything. Entries marked [`Item::pick`] bring a search field that narrows them as
//! one types.

use super::field::Field;
use super::text_field::{Edited, Entered, Escape, TextField};
use super::theme::{self, Theme};
use super::widgets::{TextSized, icon, measure};
use crate::words::put;
use gpui::{
    AnyElement, App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, FontWeight,
    Global, Hsla, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Point,
    Render, Role, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Subscription,
    Window, anchored, deferred, div, point, prelude::FluentBuilder, px,
};
use rimstack_core::say;
use std::rc::Rc;

type Run = Rc<dyn Fn(&mut Window, &mut App)>;

pub enum Entry {
    Item(Item),
    Line,
    Title(SharedString),
    Note(SharedString),
}

pub struct Item {
    text: SharedString,
    note: Option<SharedString>,
    tip: SharedString,
    icon: Option<&'static str>,
    colour: Option<Hsla>,
    off: bool,
    danger: bool,
    /// Narrowed by the menu's search field.
    pick: bool,
    run: Run,
}

impl Entry {
    pub fn item(
        text: impl Into<SharedString>,
        run: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Item {
        Item {
            text: text.into(),
            note: None,
            tip: SharedString::default(),
            icon: None,
            colour: None,
            off: false,
            danger: false,
            pick: false,
            run: Rc::new(run),
        }
    }
}

impl Item {
    pub fn icon(mut self, name: &'static str) -> Self {
        self.icon = Some(name);
        self
    }

    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn tip(mut self, tip: impl Into<SharedString>) -> Self {
        self.tip = tip.into();
        self
    }

    pub fn colour(mut self, colour: Hsla) -> Self {
        self.colour = Some(colour);
        self
    }

    pub fn off(mut self, off: bool) -> Self {
        self.off = off;
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    /// One of the entries the search field narrows: a menu holding any shows the field, with
    /// the keys in it.
    pub fn pick(mut self) -> Self {
        self.pick = true;
        self
    }
}

impl From<Item> for Entry {
    fn from(item: Item) -> Self {
        Entry::Item(item)
    }
}

pub struct Menu {
    pub(crate) entries: Vec<Entry>,
    at: Point<Pixels>,
    corner: gpui::Anchor,
    /// Drawn as a column of buttons, so many captions' letters wide, or as wide as its widest
    /// button.
    buttons: Option<Option<usize>>,
    chosen: Option<usize>,
    focus: FocusHandle,
    before: Option<FocusHandle>,
    turn: usize,
    /// Closed, and drawn only while it fades out.
    fading: bool,
    /// Scrolls the entries when they are taller than the window.
    scroll: ScrollHandle,
    /// Whether a menu has been shown, so another can open where it stood.
    opened: bool,
    /// The search field of a menu with picks; typing hides the picks not holding the text.
    pub(crate) search: Option<Entity<TextField>>,
    heard: Vec<Subscription>,
}

struct Shown(Entity<Menu>);

impl Global for Shown {}

pub fn install(cx: &mut App) -> Entity<Menu> {
    let menu = cx.new(|cx| Menu {
        turn: 0,
        fading: false,
        entries: Vec::new(),
        at: Point::default(),
        corner: gpui::Anchor::TopLeft,
        buttons: None,
        chosen: None,
        focus: cx.focus_handle(),
        before: None,
        scroll: ScrollHandle::new(),
        opened: false,
        search: None,
        heard: Vec::new(),
    });
    cx.set_global(Shown(menu.clone()));
    menu
}

pub fn open(below: Bounds<Pixels>, entries: Vec<Entry>, window: &mut Window, cx: &mut App) {
    open_at(
        point(below.left(), below.bottom() + theme::GAP_TINY),
        entries,
        window,
        cx,
    );
}

/// Opens `entries` as a column of buttons under `below`, its right edge on the control's,
/// `letters` captions' letters wide or as wide as its widest button.
pub fn open_buttons_under_end(
    below: Bounds<Pixels>,
    letters: Option<usize>,
    entries: Vec<Entry>,
    window: &mut Window,
    cx: &mut App,
) {
    let at = point(below.right(), below.bottom() + theme::GAP_SMALL);
    show(
        at,
        gpui::Anchor::TopRight,
        Some(letters),
        entries,
        window,
        cx,
    );
}

pub fn open_above(over: Bounds<Pixels>, entries: Vec<Entry>, window: &mut Window, cx: &mut App) {
    let at = point(over.left(), over.top() - theme::GAP_TINY);
    show(at, gpui::Anchor::BottomLeft, None, entries, window, cx);
}

/// Opens `entries` with their top left corner at `at`, kept in the window.
pub fn open_at(at: Point<Pixels>, entries: Vec<Entry>, window: &mut Window, cx: &mut App) {
    show(at, gpui::Anchor::TopLeft, None, entries, window, cx);
}

fn show(
    at: Point<Pixels>,
    corner: gpui::Anchor,
    buttons: Option<Option<usize>>,
    entries: Vec<Entry>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(menu) = cx.try_global::<Shown>().map(|shown| shown.0.clone()) else {
        return;
    };
    let before = window.focused(cx);
    menu.update(cx, |menu, cx| {
        menu.entries = entries;
        menu.at = at;
        menu.corner = corner;
        menu.buttons = buttons;
        menu.chosen = None;
        menu.before = before;
        menu.turn = super::motion::opening();
        menu.fading = false;
        menu.scroll = ScrollHandle::new();
        menu.opened = true;
        menu.heard.clear();
        menu.search = None;
        let picks = menu.buttons.is_none()
            && (menu.entries.iter()).any(|e| matches!(e, Entry::Item(i) if i.pick));
        match picks {
            true => {
                let field = cx.new(|cx| TextField::new(cx.global::<Theme>().body(), cx));
                menu.heard
                    .push(cx.subscribe(&field, |menu, _, _: &Edited, cx| {
                        menu.chosen = None;
                        cx.notify();
                    }));
                menu.heard.push(cx.subscribe_in(
                    &field,
                    window,
                    |menu, _, _: &Entered, window, cx| menu.enter(window, cx),
                ));
                window.focus(&field.focus_handle(cx), cx);
                menu.search = Some(field);
            }
            false => window.focus(&menu.focus, cx),
        }
        cx.notify();
    });
}

impl Menu {
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match super::motion::fades_out(cx) {
            true => {
                self.fading = true;
                let turn = self.turn;
                super::motion::after_fade(cx, move |menu: &mut Self, cx| {
                    if menu.fading && menu.turn == turn {
                        menu.fading = false;
                        menu.entries.clear();
                        cx.notify();
                    }
                });
            }
            false => self.entries.clear(),
        }
        if let Some(before) = self.before.take() {
            window.focus(&before, cx);
        }
        cx.notify();
    }

    fn choose(&mut self, at: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.fading {
            return;
        }
        let Some(Entry::Item(item)) = self
            .entries
            .get(at)
            .filter(|e| matches!(e, Entry::Item(i) if !i.off))
        else {
            return;
        };
        let run = item.run.clone();
        self.close(window, cx);
        run(window, cx);
    }

    /// The text typed in the search field, lowered; empty without one.
    pub(crate) fn query(&self, cx: &App) -> String {
        (self.search.as_ref())
            .map(|field| field.read(cx).text().trim().to_lowercase())
            .unwrap_or_default()
    }

    /// Whether the search leaves the entry shown: all but the picks not holding the text.
    pub(crate) fn left(entry: &Entry, query: &str) -> bool {
        match entry {
            Entry::Item(item) if item.pick => {
                query.is_empty() || item.text.to_lowercase().contains(query)
            }
            _ => true,
        }
    }

    /// Enter takes the entry the keys are on, or else the first pick the search left.
    fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.query(cx);
        let reached = (self.chosen).filter(|&at| Self::left(&self.entries[at], &query));
        let first = || {
            self.entries.iter().position(|e| {
                matches!(e, Entry::Item(i) if i.pick && !i.off) && Self::left(e, &query)
            })
        };
        if let Some(at) = reached.or_else(first) {
            self.choose(at, window, cx);
        }
    }

    fn step(&mut self, by: isize, cx: &App) {
        let query = self.query(cx);
        let count = self.entries.len() as isize;
        let mut next = self
            .chosen
            .map_or(if by > 0 { -1 } else { count }, |at| at as isize);
        for _ in 0..count {
            next = (next + by).rem_euclid(count);
            let entry = &self.entries[next as usize];
            if matches!(entry, Entry::Item(item) if !item.off) && Self::left(entry, &query) {
                self.chosen = Some(next as usize);
                // Scrolled just enough to show the entry the keys landed on.
                self.scroll.scroll_to_item(next as usize);
                return;
            }
        }
    }

    /// How far a page of the page keys reaches, in entries: the window's height over an entry.
    fn page(&self, theme: &Theme, window: &Window) -> isize {
        let row = theme.tag() + theme::GAP_TINY + theme::GAP_SMALL;
        (window.viewport_size().height / row).max(1.) as isize
    }

    /// The next entry on whose words `letter` starts, or where the keys were.
    fn jump(&mut self, letter: char, cx: &App) {
        let was = self.chosen;
        for _ in 0..self.entries.len() {
            self.step(1, cx);
            if matches!(&self.entries[self.chosen.unwrap()], Entry::Item(item)
                if item.text.to_lowercase().starts_with(letter))
            {
                return;
            }
        }
        self.chosen = was;
    }

    fn entry(
        &self,
        at: usize,
        query: &str,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        if !Self::left(&self.entries[at], query) {
            // Kept as an empty child, so the scroll handle still finds each entry by its place.
            return div().into_any_element();
        }
        let item = match &self.entries[at] {
            Entry::Line => {
                return div()
                    .h(theme::GAP + theme::GAP_LINE)
                    .pt(theme::GAP_TINY)
                    .px(theme::GAP_TINY)
                    .child(div().h(px(1.)).bg(c.line))
                    .into_any_element();
            }
            Entry::Title(title) if self.buttons.is_some() => {
                return div()
                    .pt(theme::GAP_TINY)
                    .sized_text(theme, theme.small())
                    .font_weight(theme.semibold)
                    .text_color(c.ink2)
                    .child(title.clone())
                    .into_any_element();
            }
            Entry::Title(title) => {
                return div()
                    .flex()
                    .items_center()
                    .h(theme.tag() + theme::GAP_TINY)
                    .px(theme::GAP_MEDIUM)
                    .child(
                        div()
                            .sized_text(theme, theme.caption())
                            .font_weight(theme.medium)
                            .text_color(c.ink3)
                            .child(title.clone()),
                    )
                    .into_any_element();
            }
            Entry::Note(note) => {
                return div()
                    .w_full()
                    .when(self.buttons.is_none(), |d| {
                        let room = measure(window, theme, "n", theme.caption(), FontWeight::NORMAL);
                        d.px(theme::GAP_MEDIUM)
                            .py(theme::GAP_TINY)
                            .max_w(room * 40. + theme::GAP_MEDIUM * 2.)
                    })
                    .spaced_text(theme, theme.caption(), 1.25)
                    .text_color(c.ink3)
                    .child(note.clone())
                    .into_any_element();
            }
            Entry::Item(item) => item,
        };
        let reached = self.chosen == Some(at);
        if self.buttons.is_some() {
            return self.button(at, item, reached, theme, cx);
        }
        let ink = if item.danger { c.err } else { c.ink };
        let words_x = theme::GAP * 2. + theme.icon();
        let note_room = measure(window, theme, "n", theme.small(), FontWeight::NORMAL) * 44.;
        div()
            .id(("menu-entry", at))
            .role(Role::MenuItem)
            .aria_label(item.text.clone())
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .pl(words_x)
            .pr(theme::GAP_WIDE)
            .rounded(theme::CORNER)
            .when(item.off, |d| d.opacity(0.4))
            .when(!item.off, |d| {
                let (chosen, on) = (c.chosen, c.on_selected);
                d.when(reached, |d| d.bg(chosen).text_color(on))
                    .hover(move |s| s.bg(chosen).text_color(on))
            })
            .text_color(ink)
            .child(
                div()
                    .absolute()
                    .left(theme::GAP)
                    .top_0()
                    .h(theme.strip())
                    .flex()
                    .items_center()
                    .when_some(item.icon, |d, name| d.child(icon(name, theme.icon(), ink)))
                    .when_some(item.colour, |d, colour| {
                        d.pl(theme::GAP_MEDIUM - theme::GAP)
                            .child(div().size(theme::GAP_MEDIUM).rounded_full().bg(colour))
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(theme.strip())
                    .whitespace_nowrap()
                    .sized_text(theme, theme.body())
                    .child(item.text.clone()),
            )
            .when_some(item.note.clone(), |d, note| {
                d.child(
                    div()
                        .max_w(note_room)
                        .pb(theme::GAP)
                        .opacity(0.75)
                        .spaced_text(theme, theme.small(), 1.25)
                        .child(note),
                )
            })
            .when(!item.off, |d| {
                d.on_click(cx.listener(move |menu, _, window, cx| menu.choose(at, window, cx)))
            })
            .into_any_element()
    }
}

impl Menu {
    fn button(
        &self,
        at: usize,
        item: &Item,
        reached: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let c = &theme.colours;
        let shown = div()
            .id(("menu-button", at))
            .role(Role::MenuItem)
            .aria_label(item.text.clone())
            .flex()
            .items_center()
            .justify_center()
            .gap(theme::GAP_SMALL)
            .w_full()
            .h(theme.control())
            .px(theme::GAP_LARGE)
            .rounded(theme::CORNER)
            .bg(if reached { c.ctl_hi } else { c.ctl })
            .hover(|s| s.bg(c.ctl_hi))
            .when(reached, |d| d.border_1().border_color(c.focus))
            .cursor_pointer()
            .when(item.off, |d| d.opacity(0.4))
            .when_some(item.icon.filter(|name| !name.is_empty()), |d, name| {
                d.child(icon(name, theme.icon(), c.ink))
            })
            .child(
                div()
                    .whitespace_nowrap()
                    .sized_text(theme, theme.small())
                    .text_color(c.ink)
                    .child(item.text.clone()),
            )
            .when(!item.off, |d| {
                d.on_click(cx.listener(move |menu, _, window, cx| menu.choose(at, window, cx)))
            });
        super::tip::tipped(shown, item.tip.clone()).into_any_element()
    }
}

impl Focusable for Menu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Menu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.entries.is_empty() {
            return div().id("menu");
        }
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let query = self.query(cx);
        let searched = self.search.clone().map(|field| {
            div()
                .p(theme::GAP_TINY)
                .pb_0()
                .min_w(theme.sized(220.))
                .child(
                    Field {
                        id: "menu-search",
                        text: &field,
                        icon: "search",
                        placeholder: put(&say!("Search")),
                        found: None,
                        keys: None,
                    }
                    .render(&theme, window, cx),
                )
        });
        // A menu taller than the window stays inside it and scrolls, below its search field and
        // within its border.
        let above = match searched {
            Some(_) => theme.field() + theme::GAP_TINY,
            None => px(0.),
        };
        let room = window.viewport_size().height - theme::GAP_TINY * 2. - above - px(2.);
        let entries: Vec<AnyElement> = (0..self.entries.len())
            .map(|at| self.entry(at, &query, &theme, window, cx))
            .collect();
        let letter = measure(
            window,
            &theme,
            "abcdefghijklmnopqrstuvwxyz",
            theme.caption(),
            FontWeight::NORMAL,
        ) / 26.;
        let list = div()
            .id("menu-list")
            .role(Role::Menu)
            .flex()
            .flex_col()
            .when_some(self.buttons, |d, letters| {
                d.when_some(letters, |d, letters| {
                    d.w(letter * letters as f32 + theme::GAP_MEDIUM * 2.)
                })
                .p(theme::GAP_MEDIUM)
                .gap(theme::GAP_SMALL)
            })
            .when(self.buttons.is_none(), |d| d.p(theme::GAP_TINY))
            .max_h(room)
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .children(entries);
        let list = div()
            .flex()
            .flex_col()
            .rounded(theme::CORNER_LARGE)
            .bg(c.bar)
            .border_1()
            .border_color(c.line)
            .when(!self.fading, |d| d.occlude())
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .children(searched)
            .child(list);
        if self.fading {
            return div().id("menu").absolute().inset_0().child(deferred(
                anchored()
                    .anchor(self.corner)
                    .position(self.at)
                    .snap_to_window_with_margin(theme::GAP_TINY)
                    .child(super::motion::pop_out(
                        list.child(super::motion::shield()),
                        self.turn,
                    )),
            ));
        }
        div()
            .id("menu")
            .absolute()
            .inset_0()
            .map(|d| super::focus::trap(d, &self.focus))
            .capture_action(cx.listener(|menu, _: &Escape, window, cx| {
                cx.stop_propagation();
                menu.close(window, cx);
            }))
            .on_key_down(cx.listener(|menu, event: &KeyDownEvent, window, cx| {
                // Escape, Enter, Space and letters in the search field go to the field.
                let typing = (menu.search.as_ref()).is_some_and(|f| f.read(cx).is_focused(window));
                let key = event.keystroke.key.as_str();
                let page = menu.page(&cx.global::<Theme>().clone(), window);
                // The ends, a page, or the next entry starting with the letter typed.
                let letter = match (key.len(), event.keystroke.key_char.clone()) {
                    // A single-character key: a letter (or a digit, which finds nothing).
                    (1, Some(typed)) if !typing => typed.to_lowercase().chars().next(),
                    _ => None,
                };
                match key {
                    "escape" if !typing => menu.close(window, cx),
                    "down" => {
                        menu.step(1, cx);
                        cx.notify();
                    }
                    "up" => {
                        menu.step(-1, cx);
                        cx.notify();
                    }
                    "enter" | "space" if !typing => {
                        if let Some(at) = menu.chosen {
                            menu.choose(at, window, cx);
                        }
                    }
                    "home" if !typing => {
                        menu.chosen = None;
                        menu.step(1, cx);
                        cx.notify();
                    }
                    "end" if !typing => {
                        menu.chosen = None;
                        menu.step(-1, cx);
                        cx.notify();
                    }
                    "pageup" => {
                        menu.step(-page, cx);
                        cx.notify();
                    }
                    "pagedown" => {
                        menu.step(page, cx);
                        cx.notify();
                    }
                    _ => {
                        if let Some(letter) = letter {
                            menu.jump(letter, cx);
                            cx.notify();
                        }
                    }
                }
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|menu, _, window, cx| menu.close(window, cx)),
            )
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(|menu, _, window, cx| menu.close(window, cx)),
            )
            .child(deferred(
                anchored()
                    .anchor(self.corner)
                    .position(self.at)
                    .snap_to_window_with_margin(theme::GAP_TINY)
                    .child(super::motion::pop_in(list, self.turn)),
            ))
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn chosen(cx: &App) -> Option<usize> {
    cx.try_global::<Shown>()
        .and_then(|shown| shown.0.read(cx).chosen)
}

/// Where the menu last shown was opened, for a menu that opens another where it stands.
pub fn shown_at(cx: &App) -> Option<Point<Pixels>> {
    let menu = cx.try_global::<Shown>().map(|shown| shown.0.clone())?;
    let menu = menu.read(cx);
    menu.opened.then_some(menu.at)
}

#[cfg(all(test, target_os = "linux"))]
impl Menu {
    /// The bounds of the entries' list, for the checks.
    pub(crate) fn list(&self) -> Bounds<Pixels> {
        self.scroll.bounds()
    }

    /// Whether the entries are taller than the room they were given.
    pub(crate) fn scrolls(&self) -> bool {
        self.scroll.max_offset().y > px(0.)
    }

    /// How far the entries are scrolled down, negative as a scroll offset is.
    pub(crate) fn down(&self) -> Pixels {
        -self.scroll.offset().y
    }

    /// The entry the keys are on, and its bounds, for the checks.
    pub(crate) fn seen(&self) -> Option<(usize, Bounds<Pixels>)> {
        let at = self.chosen?;
        Some((at, self.scroll.bounds_for_item(at)?))
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn drawn(cx: &App) -> bool {
    cx.try_global::<Shown>()
        .is_some_and(|shown| !shown.0.read(cx).entries.is_empty())
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn shown(cx: &App) -> Option<Entity<Menu>> {
    cx.try_global::<Shown>().map(|shown| shown.0.clone())
}

/// The captions of the entries drawn, the test's window into what a menu offers.
#[cfg(all(test, target_os = "linux"))]
pub(crate) fn texts(cx: &App) -> Vec<String> {
    cx.try_global::<Shown>()
        .map(|shown| {
            shown
                .0
                .read(cx)
                .entries
                .iter()
                .filter_map(|entry| match entry {
                    Entry::Item(item) => Some(item.text.to_string()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn redraw(cx: &mut App) {
    if let Some(menu) = cx.try_global::<Shown>().map(|shown| shown.0.clone()) {
        menu.update(cx, |_, cx| cx.notify());
    }
}
