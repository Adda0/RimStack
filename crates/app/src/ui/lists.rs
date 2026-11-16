use super::button::Btn;
use super::field::Field;
use super::menu::{self, Entry};
use super::notices;
use super::prompt::{self, Line, Question};
use super::text_field::{Entered, TextField};
use super::theme::{self, Theme};
use super::widgets::{TextSized, label};
use crate::changes::Changes;
use crate::words::put;
use crate::workspace::lists::{Comparison, HistoryLine, Snapshot};
use crate::workspace::logs::{clock, moment};
use gpui::{
    AnyElement, AppContext, Bounds, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use rimstack_core::base::error::Error;
use rimstack_core::kept::lists::Saved;
use rimstack_core::{say, say_n};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Lists,
    History,
    /// A saved list beside the load order: which mods each holds alone, and which moved.
    Compare,
}

/// The saved list a comparison card shows, and the groups read for it.
struct Comparing {
    name: String,
    /// None until the mods are read.
    groups: Option<Comparison>,
}

const READS: Changes = Changes::LISTS.or(Changes::LANGUAGE);

pub struct Lists {
    saved: Vec<Saved>,
    snapshots: Vec<Snapshot>,
    history: Vec<HistoryLine>,
    busy: bool,
    problem: String,
    clipboard_problem: String,
    name: Entity<TextField>,
    open: Option<(Which, Bounds<Pixels>)>,
    lock_report: Option<rimstack_core::mods::lock::Report>,
    comparing: Option<Comparing>,
    turn: usize,
    /// Closed, and drawn only while it fades out.
    fading: Option<(Which, Bounds<Pixels>)>,
    focus: FocusHandle,
    /// The card's first control, which the keys open on.
    keys: FocusHandle,
    before: Option<FocusHandle>,
    reading: crate::jobs::Lane,
}

impl Lists {
    pub fn new(demo: bool, cx: &mut Context<Self>) -> Self {
        let theme = cx.global::<Theme>().clone();
        let name = cx.new(|cx| TextField::new(theme.body(), cx));
        cx.subscribe(&name, |lists, _, _: &Entered, cx| lists.keep_typed(cx))
            .detach();
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        let mut lists = Self {
            saved: Vec::new(),
            snapshots: Vec::new(),
            history: Vec::new(),
            busy: false,
            problem: String::new(),
            clipboard_problem: String::new(),
            name,
            open: None,
            lock_report: None,
            comparing: None,
            turn: 0,
            fading: None,
            focus: cx.focus_handle(),
            keys: cx.focus_handle().tab_stop(false),
            before: None,
            reading: Default::default(),
        };
        if !demo {
            lists.hear(cx);
            lists.read(cx);
        }
        lists
    }

    fn hear(&self, cx: &mut Context<Self>) {
        super::hear(cx, READS, |lists: &mut Self, _, cx| lists.read(cx));
    }

    /// Every kept list's file is read and parsed: off the window's thread.
    fn read(&mut self, cx: &mut Context<Self>) {
        let app = super::app(cx);
        self.reading.run(
            "lists",
            cx,
            move |_| (app.saved_lists(), app.recent_snapshots(), app.history()),
            |lists, (saved, snapshots, history), cx| {
                (lists.saved, lists.snapshots, lists.history) = (saved, snapshots, history);
                cx.notify();
            },
        );
    }

    pub fn toggle(
        &mut self,
        which: Which,
        below: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.open.is_some_and(|(open, _)| open == which) {
            return self.close(window, cx);
        }
        self.problem.clear();
        self.clipboard_problem.clear();
        self.open = Some((which, below));
        self.fading = None;
        self.turn = super::motion::opening();
        self.before = window.focused(cx);
        // The card's first control; a History with nothing to restore keeps the card itself.
        let reverts = self
            .history
            .iter()
            .any(|line| !line.restore.is_empty() || !line.bring_back.is_empty());
        let first = match (which, reverts) {
            (Which::Lists, _) | (Which::History, true) | (Which::Compare, _) => self.keys.clone(),
            (Which::History, false) => self.focus.clone(),
        };
        window.focus(&first, cx);
        cx.notify();
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn keys_handle(&self) -> FocusHandle {
        self.keys.clone()
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn drawn(&self) -> bool {
        self.open.is_some() || self.fading.is_some()
    }

    pub fn is_open(&self, which: Which) -> bool {
        self.open.is_some_and(|(open, _)| open == which)
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let closed = self.open.take();
        if super::motion::fades_out(cx) {
            self.fading = closed;
            let turn = self.turn;
            super::motion::after_fade(cx, move |lists: &mut Self, cx| {
                if lists.turn == turn && lists.fading.take().is_some() {
                    cx.notify();
                }
            });
        }
        if let Some(before) = self.before.take() {
            window.focus(&before, cx);
        }
        cx.notify();
    }

    fn list_name(&self, cx: &gpui::App) -> String {
        super::around::around(cx).read(cx).list_name.clone()
    }

    /// The load order now goes by `name`; "" when it has none.
    fn go_by(&self, name: &str, cx: &mut Context<Self>) {
        match super::app(cx).keep_pref("Mods", "listName", name) {
            Ok(()) => super::around::change(cx, |around| around.list_name = name.to_string()),
            Err(problem) => notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx),
        }
        cx.notify();
    }

    fn use_ids(
        &mut self,
        ids: Vec<String>,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        super::navigate::go(super::navigate::Go::TakeList(ids), cx);
        self.go_by(name, cx);
        self.close(window, cx);
    }

    /// The saved list `name` beside the load order as it is, the card opening where the lists did.
    fn compare(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let below = self.open.map_or_else(Bounds::default, |(_, below)| below);
        self.comparing = Some(Comparing {
            name: name.to_string(),
            groups: None,
        });
        self.toggle(Which::Compare, below, window, cx);
        self.compare_again(cx);
    }

    /// The comparison card's groups, read anew: off the window's thread.
    fn compare_again(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self
            .comparing
            .as_ref()
            .map(|comparing| comparing.name.clone())
        else {
            return;
        };
        let app = super::app(cx);
        let asked = name.clone();
        crate::jobs::run(
            "list compared",
            cx,
            move || app.compare_saved(&asked),
            move |lists, groups, cx| {
                let Some(comparing) = &mut lists.comparing else {
                    return;
                };
                if comparing.name != name {
                    return;
                }
                comparing.groups = Some(groups);
                cx.notify();
            },
        );
    }

    /// The saved list's mods the load order lacks join its end, in its order; nothing else moves.
    fn merge_missing(&mut self, cx: &mut Context<Self>) {
        let Some(groups) = self
            .comparing
            .as_ref()
            .and_then(|shown| shown.groups.as_ref())
        else {
            return;
        };
        if groups.addable.is_empty() {
            return;
        }
        super::navigate::go(super::navigate::Go::Activate(groups.addable.clone()), cx);
        self.compare_again(cx);
    }

    /// Asks where the list goes, offering `suggest` in the user's own folder, and writes it.
    fn export(&mut self, suggest: &str, window: &mut Window, cx: &mut Context<Self>) {
        let home = super::app(cx).home_dir().to_path_buf();
        let chosen = cx.prompt_for_new_path(&home, Some(suggest));
        self.close(window, cx);
        cx.spawn_in(window, async move |lists, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };
            let _ = lists.update(cx, |lists, cx| lists.write_list(path, cx));
        })
        .detach();
    }

    fn write_list(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let (app, active) = (super::app(cx), (*super::order::read(cx).active).clone());
        crate::jobs::run(
            "list export",
            cx,
            move || app.export_list(&active, &path),
            |_, done, cx| match done {
                Ok((at, count, what)) => {
                    let counted = say_n!(count, "One active mod", "{} active mods");
                    let note = say!(
                        "{} as {} in {}",
                        counted.inside(),
                        what.inside(),
                        at.display()
                    );
                    notices::say(put(&note), notices::Tone::Ok, cx);
                }
                Err(problem) => {
                    notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx)
                }
            },
        );
    }

    /// Asks where the lockfile goes, then writes the load order and its versions there.
    fn lock_write(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let home = super::app(cx).home_dir().to_path_buf();
        let chosen = cx.prompt_for_new_path(&home, Some("loadorder.lock.json"));
        self.close(window, cx);
        cx.spawn_in(window, async move |lists, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return;
            };
            let _ = lists.update(cx, |lists, cx| lists.write_lock(path, cx));
        })
        .detach();
    }

    fn write_lock(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let app = super::app(cx);
        crate::jobs::run(
            "lockfile",
            cx,
            move || app.write_lockfile(&path),
            |_, done, cx| match done {
                Ok(()) => notices::say(
                    put(&say!("The load order written as a lockfile")),
                    notices::Tone::Ok,
                    cx,
                ),
                Err(problem) => {
                    notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx)
                }
            },
        );
    }

    fn lock_read(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(put(&say!("Choose the lockfile…")).into()),
        });
        self.close(window, cx);
        cx.spawn_in(window, async move |lists, cx| {
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = lists.update(cx, |lists, cx| lists.check_lock(path, cx));
        })
        .detach();
    }

    fn check_lock(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let app = super::app(cx);
        crate::jobs::run(
            "lockfile check",
            cx,
            move || app.check_lockfile(&path),
            |lists, checked, cx| match checked {
                Ok(report) => lists.report_lock(report, cx),
                Err(problem) => {
                    lists.problem = crate::words::said_of(&problem);
                    cx.notify();
                }
            },
        );
    }

    fn report_lock(&mut self, report: rimstack_core::mods::lock::Report, cx: &mut Context<Self>) {
        self.lock_report = Some(report);
        cx.notify();
    }

    /// Asks for a RimSort, RimPy or text list file and makes it the load order.
    fn use_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(put(&say!("Choose the list file…")).into()),
        });
        self.close(window, cx);
        cx.spawn_in(window, async move |lists, cx| {
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = lists.update(cx, |lists, cx| lists.read_file(path, cx));
        })
        .detach();
    }

    /// Kept off the window's thread, whatever kind of list file it is.
    fn read_file(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        crate::jobs::run(
            "list file",
            cx,
            move || rimstack_core::mods::share_file::file(&path),
            |lists, read, cx| match read {
                Ok(ids) if ids.is_empty() => {
                    lists.problem = put(&say!("No mod found in that file"));
                    cx.notify();
                }
                Ok(ids) => {
                    super::navigate::go(super::navigate::Go::TakeList(ids), cx);
                    lists.go_by("", cx);
                    lists.open = None;
                }
                Err(problem) => {
                    lists.problem = crate::words::said_of(&problem);
                    cx.notify();
                }
            },
        );
    }

    /// Kept off the window's thread; `typed`: the name field is emptied once it is.
    fn keep(&mut self, name: &str, typed: bool, cx: &mut Context<Self>) {
        let (app, name) = (super::app(cx), name.trim().to_string());
        let asked = name.clone();
        crate::jobs::run(
            "list kept",
            cx,
            move || app.keep_list(&asked, None),
            move |lists, kept, cx| {
                lists.problem = crate::words::why_not(&kept);
                if kept.is_ok() {
                    lists.go_by(&name, cx);
                    if typed {
                        lists.name.update(cx, |field, cx| field.set_text("", cx));
                    }
                }
                cx.notify();
            },
        );
    }

    fn keep_typed(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).text().trim().to_string();
        if !name.is_empty() {
            self.keep(&name, true, cx);
        }
    }

    fn start_fresh(&mut self, with_expansions: bool, window: &mut Window, cx: &mut Context<Self>) {
        let ids = super::app(cx).fresh_list(with_expansions);
        self.use_ids(ids, "", window, cx);
    }

    pub(crate) fn use_copied(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        self.clipboard_problem.clear();
        self.busy = true;
        let app = super::app(cx);
        crate::jobs::run(
            "copied list",
            cx,
            move || app.ids_in_copied(&text),
            |lists, read: Result<Vec<String>, Error>, cx| {
                lists.busy = false;
                match read {
                    Ok(ids) if ids.is_empty() => {
                        lists.clipboard_problem = put(&say!("No mod found on the clipboard"))
                    }
                    Ok(ids) => {
                        super::navigate::go(super::navigate::Go::TakeList(ids), cx);
                        lists.go_by("", cx);
                        lists.open = None;
                    }
                    Err(problem) => lists.clipboard_problem = crate::words::said_of(&problem),
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    fn share(&mut self, cx: &mut Context<Self>) {
        let listing = super::app(cx).listing(&super::order::read(cx).active);
        if self.busy {
            return;
        }
        if listing.is_empty() {
            self.clipboard_problem = put(&say!("There is no load order to share"));
            return cx.notify();
        }
        self.clipboard_problem.clear();
        self.busy = true;
        let app = super::app(cx);
        crate::jobs::run(
            "rentry publish",
            cx,
            move || app.publish_list(&listing),
            |lists, published, cx| {
                lists.busy = false;
                match published {
                    Ok(page) => {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(page.url.clone()));
                        let note = say!(
                            "The load order is at {}, and the link is copied. Its edit code, which RimStack does not keep: {}",
                            page.url,
                            page.edit_code
                        );
                        notices::say(put(&note), notices::Tone::Ok, cx);
                    }
                    Err(problem) => lists.clipboard_problem = crate::words::said_of(&problem),
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    pub(crate) fn write_a_lockfile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.lock_write(window, cx);
    }

    pub(crate) fn check_a_lockfile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.lock_read(window, cx);
    }

    fn ask_share(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let me = cx.entity().downgrade();
        prompt::ask(
            Question {
                title: put(&say!("Share on Rentry.co")),
                lines: vec![Line::Text(put(&say!(
                    "The load order is published at a public link on Rentry.co that anyone who has it can open. RimStack does not keep the code that edits the page; the message that follows shows it."
                )))],
                label: put(&say!("Publish")),
                danger: false,
                run: Box::new(move |_, cx| {
                    let _ = me.update(cx, |lists, cx| lists.share(cx));
                }),
            },
            window,
            cx,
        );
    }

    fn revert(&mut self, line: &HistoryLine, window: &mut Window, cx: &mut Context<Self>) {
        let app = super::app(cx);
        if !line.restore.is_empty() {
            if let Some(ids) = app.snapshot_named(&line.restore) {
                super::navigate::go(super::navigate::Go::TakeList(ids), cx);
            }
        } else if !line.bring_back.is_empty() {
            let done = app.unhide_folder(std::path::Path::new(&line.bring_back));
            match done {
                Ok(()) => notices::say(put(&say!("Textures brought back")), notices::Tone::Ok, cx),
                Err(problem) => {
                    notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx)
                }
            }
        }
        self.close(window, cx);
    }

    fn title(theme: &Theme, text: String) -> gpui::Div {
        label(theme, theme.title(), theme.colours.ink, text).font_weight(theme.semibold)
    }

    fn lists_popup(
        &self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let c = &theme.colours;
        let current = self.list_name(cx);
        let row = |d: gpui::Div| {
            d.flex()
                .items_center()
                .gap(theme::GAP_SMALL)
                .h(theme.sized(28.))
        };
        let mut shown = vec![
            div()
                .flex()
                .items_center()
                .gap(theme::GAP_SMALL)
                .child(div().flex_1().min_w_0().child(Self::title(theme, put(&say!("Saved lists")))))
                .child(
                    Btn::new("new-list", put(&say!("New")))
                        .keys(self.keys.clone())
                        .icon("plus")
                        .tip(put(&say!("Start a load order from the game alone or with its expansions; one Undo brings the old one back")))
                        .pops({
                            let me = cx.entity().downgrade();
                            move |below, window, cx| {
                                let (only, both) = (me.clone(), me.clone());
                                let entries: Vec<Entry> = vec![
                                    Entry::item(put(&say!("Only the game")), move |window, cx| {
                                        let _ = only.update(cx, |lists, cx| lists.start_fresh(false, window, cx));
                                    })
                                    .into(),
                                    Entry::item(put(&say!("The game and its expansions")), move |window, cx| {
                                        let _ = both.update(cx, |lists, cx| lists.start_fresh(true, window, cx));
                                    })
                                    .into(),
                                ];
                                menu::open(below, entries, window, cx);
                            }
                        }),
                )
                .into_any_element(),
        ];
        if self.saved.is_empty() {
            shown.push(label(theme, theme.body(), c.ink2, put(&say!("None"))).into_any_element());
        }
        for (at, saved) in self.saved.iter().enumerate() {
            let name = saved.name.clone();
            let (kept, used, deleted, compared) =
                (name.clone(), name.clone(), name.clone(), name.clone());
            let update = Btn::new(
                ("update-list", at),
                if current == name {
                    put(&say!("Update"))
                } else {
                    String::new()
                },
            )
            .icon("save")
            .tip(put(&say!(
                "Saves the current load order as {}, replacing what it held",
                name.clone()
            )))
            .on_click(cx.listener(move |lists, _, _, cx| {
                lists.keep(&kept, false, cx);
            }));
            shown.push(
                row(div())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .sized_text(theme, theme.body())
                            .text_color(c.ink)
                            .child(name.clone()),
                    )
                    .child(label(
                        theme,
                        theme.small(),
                        c.ink3,
                        put(&say_n!(saved.mods, "{} mod", "{} mods")),
                    ))
                    .child(update)
                    .child(
                        Btn::new(("use-list", at), put(&say!("Use"))).on_click(cx.listener(
                            move |lists, _, window, cx| {
                                if let Some(ids) = super::app(cx).list_named(&used) {
                                    lists.use_ids(ids, &used, window, cx);
                                } else {
                                    lists.close(window, cx);
                                }
                            },
                        )),
                    )
                    .child(
                        Btn::new(("compare-list", at), put(&say!("Compare")))
                            .tip(put(&say!(
                                "Shows the mods {} and the load order hold apart or in a different order, and merges the missing ones",
                                compared.clone()
                            )))
                            .on_click(cx.listener(move |lists, _, window, cx| {
                                lists.compare(&compared, window, cx);
                            })),
                    )
                    .child(
                        Btn::new(("delete-list", at), "")
                            .icon("trash-2")
                            .tip(put(&say!("Delete {}", name)))
                            .on_click(move |_, _, cx| super::app(cx).delete_list(&deleted)),
                    )
                    .into_any_element(),
            );
        }
        shown.push(div().h(px(1.)).bg(c.line).into_any_element());
        let typed = !self.name.read(cx).text().trim().is_empty();
        shown.push(
            div()
                .flex()
                .items_center()
                .gap(theme::GAP_SMALL)
                .child(
                    div().flex_1().min_w_0().child(
                        Field {
                            id: "list-name",
                            text: &self.name,
                            icon: "",
                            placeholder: put(&say!("Name for the current list")),
                            found: None,
                            keys: None,
                        }
                        .render(theme, window, cx),
                    ),
                )
                .child(
                    Btn::new("keep-list", put(&say!("Keep")))
                        .icon("save")
                        .enabled(typed)
                        .on_click(cx.listener(|lists, _, _, cx| lists.keep_typed(cx))),
                )
                .into_any_element(),
        );
        let wrapped = |text: String, colour| {
            div()
                .sized_text(theme, theme.body())
                .text_color(colour)
                .child(text)
                .into_any_element()
        };
        if !self.problem.is_empty() {
            shown.push(wrapped(self.problem.clone(), c.err));
        }
        if !self.snapshots.is_empty() {
            shown.push(
                Self::title(theme, put(&say!("Earlier")))
                    .pt(theme::GAP_SMALL)
                    .into_any_element(),
            );
        }
        for (at, snapshot) in self.snapshots.iter().enumerate() {
            let name = snapshot.name.clone();
            shown.push(
                row(div())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .sized_text(theme, theme.body())
                            .text_color(c.ink2)
                            .child(put(&snapshot.title)),
                    )
                    .child(
                        Btn::new(("use-earlier", at), put(&say!("Use"))).on_click(cx.listener(
                            move |lists, _, window, cx| {
                                if let Some(ids) = super::app(cx).snapshot_named(&name) {
                                    lists.use_ids(ids, "", window, cx);
                                } else {
                                    lists.close(window, cx);
                                }
                            },
                        )),
                    )
                    .into_any_element(),
            );
        }
        shown.push(
            Self::title(theme, put(&say!("Clipboard and Rentry.co")))
                .pt(theme::GAP_SMALL)
                .into_any_element(),
        );
        shown.push(
            div()
                .flex()
                .flex_wrap()
                .gap(theme::GAP_SMALL)
                .child(
                    Btn::new("copy-order", put(&say!("Copy the load order")))
                        .icon("copy")
                        .tip(put(&say!("The active mods as text, one a line")))
                        .on_click(cx.listener(|lists, _, window, cx| {
                            let listing =
                                super::app(cx).listing(&super::order::read(cx).active);
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(listing));
                            lists.close(window, cx);
                        })),
                )
                .child(
                    Btn::new("copy-share-code", put(&say!("Copy the share code")))
                        .icon("copy")
                        .tip(put(&say!(
                            "The load order as a short code; this clipboard read into a friend's RimStack gives the same list, with the Workshop pages of its mods"
                        )))
                        .on_click(cx.listener(|lists, _, window, cx| {
                            let code =
                                super::app(cx).share_code(&super::order::read(cx).active);
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(code));
                            lists.close(window, cx);
                        })),
                )
                .child(
                    Btn::new("use-copied", put(&say!("Use the copied list")))
                        .enabled(!self.busy)
                        .tip(put(&say!("A list of mods copied as text, links or a share code, or a link to a Rentry.co page, becomes the load order. Save writes it.")))
                        .on_click(cx.listener(|lists, _, _, cx| lists.use_copied(cx))),
                )
                .child(
                    Btn::new("share-rentry", put(&say!("Share on Rentry.co")))
                        .icon("share-2")
                        .enabled(!self.busy)
                        .tip(put(&say!("Publishes the load order as a page at Rentry.co and copies its link")))
                        .on_click(cx.listener(|lists, _, window, cx| lists.ask_share(window, cx))),
                )
                .into_any_element(),
        );
        if self.busy {
            shown.push(wrapped(put(&say!("Waiting for Rentry.co…")), c.ink2));
        }
        if !self.clipboard_problem.is_empty() {
            shown.push(wrapped(self.clipboard_problem.clone(), c.err));
        }
        if !self.problem.is_empty() {
            shown.push(wrapped(self.problem.clone(), c.err));
        }
        shown.push(
            Self::title(theme, put(&say!("Files")))
                .pt(theme::GAP_SMALL)
                .into_any_element(),
        );
        shown.push(
            div()
                .flex()
                .flex_wrap()
                .gap(theme::GAP_SMALL)
                .child(
                    Btn::new("export-csv", put(&say!("Export as CSV")))
                        .icon("download")
                        .tip(put(&say!("Writes the active mods as a CSV file, the columns of RimSort's export")))
                        .on_click(cx.listener(|lists, _, window, cx| {
                            lists.export("modlist.csv", window, cx)
                        })),
                )
                .child(
                    Btn::new("export-modsconfig", put(&say!("Export as ModsConfig")))
                        .tip(put(&say!("Writes the load order as the game's ModsConfig.xml")))
                        .on_click(cx.listener(|lists, _, window, cx| {
                            lists.export("ModsConfig.xml", window, cx)
                        })),
                )
                .child(
                    Btn::new("export-rml", put(&say!("Export as a RimSort list")))
                        .tip(put(&say!("Writes the load order as a saved mod list (.rml), which RimSort and the game read")))
                        .on_click(cx.listener(|lists, _, window, cx| {
                            lists.export("modlist.rml", window, cx)
                        })),
                )
                .child(
                    Btn::new("use-file", put(&say!("Use a list from a file")))
                        .tip(put(&say!("A RimSort or RimPy mod list, a save's list, a share code or a text list becomes the load order. Save writes it.")))
                        .on_click(cx.listener(|lists, _, window, cx| lists.use_file(window, cx))),
                )
                .into_any_element(),
        );
        shown.push(
            Self::title(theme, put(&say!("For a friend")))
                .pt(theme::GAP_SMALL)
                .into_any_element(),
        );
        shown.push(
            div()
                .flex()
                .flex_wrap()
                .gap(theme::GAP_SMALL)
                .child(
                    Btn::new("copy-workshop-links", put(&say!("Copy the Workshop links")))
                        .icon("copy")
                        .tip(put(&say!(
                            "A link a line, one for each active Workshop mod; a friend's RimStack reads them into the downloads"
                        )))
                        .on_click(cx.listener(|lists, _, window, cx| {
                            let links =
                                super::app(cx).workshop_links(&super::order::read(cx).active);
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(links));
                            lists.close(window, cx);
                        })),
                )
                .child(
                    Btn::new("lock-write", put(&say!("Write a lockfile…")))
                        .icon("send")
                        .tip(put(&say!("The load order, with each mod's source and when the Workshop last published its version, as a small file to give a friend")))
                        .on_click(cx.listener(|lists, _, window, cx| lists.lock_write(window, cx))),
                )
                .child(
                    Btn::new("lock-read", put(&say!("Check a lockfile…")))
                        .tip(put(&say!("Tells which of a friend's mods are missing, extra, in another order or another version here")))
                        .on_click(cx.listener(|lists, _, window, cx| lists.lock_read(window, cx))),
                )
                .into_any_element(),
        );

        if let Some(report) = &self.lock_report {
            if report.same {
                shown.push(
                    Self::title(theme, put(&say!("Lockfile of the load order")))
                        .pt(theme::GAP_SMALL)
                        .into_any_element(),
                );
                shown.push(
                    label(
                        theme,
                        theme.body(),
                        c.ink,
                        put(&say!(
                            "The load order is the lockfile's, every mod the same."
                        )),
                    )
                    .into_any_element(),
                );
            } else {
                for sentence in &report.said {
                    shown.push(
                        label(theme, theme.body(), c.ink2, sentence.to_string()).into_any_element(),
                    );
                }
            }
            if report.present && !report.ids.is_empty() {
                let ids = report.ids.clone();
                shown.push(
                    Btn::new("apply-lock-file", put(&say!("Use this order")))
                        .tip(put(&say!(
                            "Makes the lockfile's order the load order. Save writes it."
                        )))
                        .on_click(cx.listener(move |lists, _, window, cx| {
                            super::navigate::go(super::navigate::Go::TakeList(ids.clone()), cx);
                            lists.close(window, cx);
                        }))
                        .into_any_element(),
                );
            }
        }

        shown
    }

    /// The saved list beside the load order: the mods each holds alone, the ones placed
    /// differently, and the saved list's mods the load order can take.
    fn compare_popup(&self, theme: &Theme, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let c = &theme.colours;
        let Some(comparing) = &self.comparing else {
            return Vec::new();
        };
        let name = comparing.name.clone();
        let mut shown = vec![
            div()
                .flex()
                .items_center()
                .gap(theme::GAP_SMALL)
                .child(div().flex_1().min_w_0().child(Self::title(
                    theme,
                    put(&say!("Compare {} with the load order", name.clone())),
                )))
                .child(
                    Btn::new("compare-close", put(&say!("Close")))
                        .keys(self.keys.clone())
                        .on_click(cx.listener(|lists, _, window, cx| lists.close(window, cx))),
                )
                .into_any_element(),
        ];
        let Some(groups) = &comparing.groups else {
            return shown;
        };
        if groups.only_saved.is_empty() && groups.only_current.is_empty() && groups.moved.is_empty()
        {
            shown.push(
                label(
                    theme,
                    theme.body(),
                    c.ink2,
                    put(&say!(
                        "The saved list and the load order hold the same mods in the same order"
                    )),
                )
                .into_any_element(),
            );
        }
        for (heading, mods) in [
            (say!("Only in the saved list"), &groups.only_saved),
            (say!("Only in the load order"), &groups.only_current),
            (say!("Placed differently"), &groups.moved),
        ] {
            if mods.is_empty() {
                continue;
            }
            shown.push(super::card::group_title(theme, put(&heading)));
            for held in mods {
                shown.push(
                    label(theme, theme.body(), c.ink, held.name.clone())
                        .overflow_hidden()
                        .text_ellipsis()
                        .into_any_element(),
                );
            }
        }
        let count = groups.addable.len();
        shown.push(
            Btn::new(
                "compare-merge",
                put(&say_n!(
                    count,
                    "Activate the {} missing mod",
                    "Activate the {} missing mods"
                )),
            )
            .enabled(count > 0)
            .tip(put(&say!(
                "Activates the mods {} holds that the load order lacks; nothing else changes, and Save still writes it",
                name
            )))
            .on_click(cx.listener(|lists, _, _, cx| lists.merge_missing(cx)))
            .into_any_element(),
        );
        shown
    }

    fn history_popup(&self, theme: &Theme, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let c = &theme.colours;
        let mut first_revert = false;
        let mut shown =
            vec![Self::title(theme, put(&say!("What RimStack changed"))).into_any_element()];
        if self.history.is_empty() {
            shown.push(
                label(theme, theme.body(), c.ink2, put(&say!("Nothing yet"))).into_any_element(),
            );
        }
        for (at, line) in self.history.iter().enumerate() {
            let said = put(&line.said);
            let restores = !line.restore.is_empty();
            let reverts = restores || !line.bring_back.is_empty();
            let opening = reverts && !first_revert;
            first_revert |= reverts;
            let held = line.clone();
            shown.push(
                div()
                    .flex()
                    .items_start()
                    .gap(theme::GAP)
                    .child(label(theme, theme.small(), c.ink3, moment(line.time)).w(theme.sized(112.)))
                    .child(div().flex_1().min_w_0().sized_text(theme, theme.body()).text_color(c.ink).child(said.clone()))
                    .when(reverts, |d| {
                        d.child(
                            Btn::new(("revert", at), match restores {
                                true => put(&say!("Restore")),
                                false => put(&say!("Bring back")),
                            })
                            .spoken(put(&say!("Restore the list from before: {}", said)))
                            .tip(match restores {
                                true => put(&say!("The load order as it was just before. Nothing is written before Save.")),
                                false => put(&say!("Renames the files back inside the mod")),
                            })
                            .when(opening, |d| d.keys(self.keys.clone()))
                            .on_click(cx.listener(move |lists, _, window, cx| lists.revert(&held, window, cx))),
                        )
                    })
                    .into_any_element(),
            );
        }
        let earlier = notices::earlier(cx);
        if !earlier.is_empty() {
            shown.push(
                Self::title(theme, put(&say!("Since RimStack started")))
                    .pt(theme::GAP)
                    .into_any_element(),
            );
        }
        for (text, err, time) in earlier {
            shown.push(
                div()
                    .flex()
                    .items_start()
                    .gap(theme::GAP)
                    .child(label(theme, theme.small(), c.ink3, clock(time)).w(theme.sized(92.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .sized_text(theme, theme.body())
                            .text_color(if err { c.err } else { c.ink2 })
                            .child(text),
                    )
                    .into_any_element(),
            );
        }
        shown
    }
}

impl Render for Lists {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((which, below)) = self.open.or(self.fading) else {
            return div().id("lists-closed");
        };
        let fading = self.open.is_none();
        let theme = cx.global::<Theme>().clone();
        let c = theme.colours;
        let viewport = window.viewport_size();
        let content = match which {
            Which::Lists => self.lists_popup(&theme, window, cx),
            Which::History => self.history_popup(&theme, cx),
            Which::Compare => self.compare_popup(&theme, cx),
        };
        let column = div()
            .flex()
            .flex_col()
            .gap(theme::GAP_SMALL)
            .children(content);
        let popup = div()
            .id("lists-popup")
            .absolute()
            .p(theme::GAP_LARGE)
            .rounded(theme::CORNER_LARGE)
            .bg(c.bar)
            .border_1()
            .border_color(c.line)
            .when(!fading, |d| d.occlude())
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());
        let popup = match which {
            Which::Lists => popup
                .left(below.left())
                .top(below.bottom() + px(6.))
                .w(theme.sized(380.))
                .child(column),
            Which::History => popup
                .right(viewport.width - below.right())
                .bottom(viewport.height - below.top() + px(8.))
                .w(theme.sized(440.))
                .child(
                    div()
                        .id("history-lines")
                        .max_h(
                            theme.sized(520.).min(below.top() - theme::GAP_LARGE)
                                - theme::GAP_LARGE * 2.,
                        )
                        .overflow_y_scroll()
                        .child(column),
                ),
            Which::Compare => popup
                .left(below.left())
                .top(below.bottom() + px(6.))
                .w(theme.sized(440.))
                .child(
                    div()
                        .id("compare-lines")
                        .max_h(
                            theme
                                .sized(520.)
                                .min(viewport.height - below.bottom() - theme::GAP_LARGE * 2.),
                        )
                        .overflow_y_scroll()
                        .child(column),
                ),
        };
        if fading {
            return div()
                .id("lists-scrim")
                .absolute()
                .inset_0()
                .child(super::motion::pop_out(
                    popup.child(super::motion::shield()),
                    self.turn,
                ));
        }
        super::focus::trap(div().id("lists-scrim").absolute().inset_0(), &self.focus)
            .on_key_down(cx.listener(|lists, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    lists.close(window, cx);
                }
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|lists, _, window, cx| lists.close(window, cx)),
            )
            .child(super::motion::pop_in(popup, self.turn))
    }
}
