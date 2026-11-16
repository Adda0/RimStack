use crate::ui::card::Fold;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, Chip, TextSized, icon, label, measure};
use crate::words::put;
use crate::workspace::load_order::{Details, Overview};
use gpui::{
    Context, Focusable, FontWeight, InteractiveElement, IntoElement, ObjectFit, ParentElement,
    Pixels, StatefulInteractiveElement, Styled, StyledImage, Window, div, prelude::FluentBuilder,
    px,
};
use rimstack_core::say;
use std::rc::Rc;

#[derive(Clone, Default, PartialEq)]
pub struct DetailsShown {
    pub demo: bool,
    pub active: usize,
    pub inactive: usize,
    pub game_version: String,
    pub overview: Option<Rc<Overview>>,
    pub details: Option<Rc<Details>>,
    pub warnings: usize,
    pub troubled: bool,
    pub dirty: bool,
    pub unsaved: String,
    pub conflicts: usize,
    pub selected_active: bool,
    pub panes: super::panes::Panes,
    pub problems_open: bool,
    pub picking: Option<super::notes::Picking>,
    /// The chosen mod's place in the load order, from 1; 0 off it.
    pub position: usize,
    pub notes_saved: bool,
    pub files: Option<Rc<FilesRead>>,
    pub unhide_problem: String,
    pub sort_moves: Option<Rc<Vec<crate::workspace::load_order::SortGroup>>>,
    /// The rules the sort on show could not follow.
    pub sort_dropped: Option<Rc<Vec<crate::workspace::load_order::SortDrop>>>,
    pub community_problem: String,
    pub fresh: usize,
    pub commits: Option<Rc<Vec<rimstack_core::sources::changes::Note>>>,
    pub chosen_id: Option<rimstack_core::mods::library::ModId>,
    pub comparison: Option<Rc<crate::workspace::textures::Comparison>>,
    /// The search for the broken mod runs: it is not offered again.
    pub hunting: bool,
    pub previewing: bool,
    pub selected: usize,
    pub selection: Option<Rc<crate::workspace::load_order::SelectionSummary>>,
}

pub type FilesRead = (
    crate::workspace::load_order::Textures,
    crate::workspace::load_order::Files,
    usize,
);

pub enum Asked {
    Section(crate::ui::section::Section),
    Filter(&'static str),
    Tab(usize),
}

/// The tag offers around the tags field are closed.
pub struct OffersShut;

impl gpui::EventEmitter<Asked> for DetailsView {}
impl gpui::EventEmitter<OffersShut> for crate::ui::text_field::TextField {}

/// The details pane, a view of its own: choosing a mod or a tab draws nothing else.
pub struct DetailsView {
    /// The chosen mod's preview, let go once another mod is chosen.
    pictures: crate::ui::pictures::Pictures,
    /// What the pane showed last (a comparison, the problems, Sort's moves, a selection, a mod or
    /// the list), and the turn of its last change, which fades the pane in.
    view_shown: Option<u8>,
    view_turn: usize,
    pub(super) description: crate::ui::rich::Selection,
    pub shown: DetailsShown,
    pub tab: usize,
    pub folded: std::collections::HashSet<&'static str>,
    pub summary_open: bool,
    pub mods: gpui::WeakEntity<super::Mods>,
    pub(crate) tags: gpui::Entity<crate::ui::text_field::TextField>,
    pub(super) note: gpui::Entity<crate::ui::text_field::TextField>,
    pub(super) small_picture: bool,
    /// The pane's own, which F6 gives the keys to.
    pub(super) focus: gpui::FocusHandle,
    /// The tag offer picked with the arrow keys, and the offers closed with Escape, while typing.
    pub(super) tag_choose: Option<usize>,
    pub(super) tag_shut: bool,
    paging: crate::ui::page::Paging,
    copyable: std::cell::RefCell<
        std::collections::HashMap<gpui::ElementId, gpui::Entity<crate::ui::text_field::TextField>>,
    >,
}

impl DetailsView {
    pub fn new(mods: gpui::WeakEntity<super::Mods>, cx: &mut Context<Self>) -> Self {
        use crate::ui::text_field::{Edited, Entered, TextField};
        use gpui::AppContext;
        let size = cx.global::<Theme>().body();
        // Escape in the tags field closes the tag offers around it; a later press keeps its
        // usual meaning.
        let tags = cx.new(|cx| {
            TextField::new(size, cx).kept_on_escape().on_escape(|cx| {
                cx.emit(OffersShut);
            })
        });
        let note = cx.new(|cx| TextField::new(size, cx).several_lines(2));
        cx.subscribe(&note, |view, _, _: &Edited, cx| view.draft_note(cx))
            .detach();
        cx.subscribe(&note, |view, _, _: &Entered, cx| view.keep_drafts(cx))
            .detach();
        cx.subscribe(&tags, |view, field, _: &Edited, cx| {
            if field.read(cx).is_focused_now() {
                // The offers are read as the field is drawn, so a change of what is typed draws
                // them; typing opens them again and takes the arrow-key choice off.
                view.tag_choose = None;
                view.tag_shut = false;
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&tags, |view, _, _: &OffersShut, cx| {
            view.tag_choose = None;
            view.tag_shut = true;
            cx.notify();
        })
        .detach();
        cx.subscribe(&tags, |view, _, _: &Entered, cx| {
            view.add_tags_typed(cx);
        })
        .detach();
        Self {
            description: crate::ui::rich::Selection::new(cx),
            shown: Default::default(),
            tab: 0,
            folded: Default::default(),
            summary_open: false,
            mods,
            tags,
            note,
            small_picture: false,
            focus: cx.focus_handle(),
            tag_choose: None,
            tag_shut: false,
            pictures: Default::default(),
            copyable: Default::default(),
            paging: crate::ui::page::Paging::new(cx.global::<Theme>().appearance.reduce_motion),
            view_shown: None,
            view_turn: 0,
        }
    }

    /// The note typed, kept once typing stops.
    fn draft_note(&mut self, cx: &mut Context<Self>) {
        let typing = self.note.read(cx).is_focused_now();
        if !typing {
            return;
        }
        let note = self.note.read(cx).text().to_string();
        let _ = self.mods.update(cx, |mods, cx| mods.draft_note(note, cx));
    }

    /// The tags typed in the add-tags field go onto the mod shown, the offer picked with the
    /// arrow keys in their place, and the field is cleared.
    pub(super) fn add_tags_typed(&mut self, cx: &mut Context<Self>) {
        let typed = self.tags.read(cx).text().to_string();
        let text = self.chosen_tag(&typed, cx).unwrap_or(typed);
        self.tag_choose = None;
        let field = self.tags.clone();
        let _ = self
            .mods
            .update(cx, move |mods, cx| mods.add_tags(text, cx));
        field.update(cx, |field, cx| field.set_text_quietly("", cx));
    }

    /// The whole text that adds the tag offer picked with the arrow keys, if it is still offered.
    fn chosen_tag(&self, typed: &str, cx: &mut Context<Self>) -> Option<String> {
        if self.tag_shut {
            return None;
        }
        self.offered(typed, cx)
            .into_iter()
            .nth(self.tag_choose?)
            .map(|offer| offer.text)
    }

    /// The tags the mod shown has, as the offers take them into account.
    fn had_tags(&self, cx: &mut Context<Self>) -> String {
        self.mods
            .upgrade()
            .and_then(|mods| {
                mods.read(cx)
                    .details
                    .as_ref()
                    .map(|details| details.tags.clone())
            })
            .unwrap_or_default()
    }

    fn offered(
        &self,
        typed: &str,
        cx: &mut Context<Self>,
    ) -> Vec<crate::workspace::notes::TagOffer> {
        let had = self.had_tags(cx);
        crate::ui::app(cx).tag_offers(&had, typed)
    }

    /// The arrow keys take the offer list up and down while it is open.
    pub(super) fn tags_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let (offer_count, typing) = {
            let typed = self.tags.read(cx).text().to_string();
            (
                self.chosen_count(&typed, cx),
                self.tags.read(cx).is_focused(window),
            )
        };
        if offer_count == 0 || self.tag_shut || !typing {
            return;
        }
        let chosen = self.tag_choose.unwrap_or(1);
        self.tag_choose = Some(match event.keystroke.key.as_str() {
            "down" => (chosen + 1).min(offer_count - 1),
            "up" => chosen.saturating_sub(1),
            _ => return,
        });
        cx.notify();
    }

    /// How many tag offers the field holds for what is typed.
    fn chosen_count(&self, typed: &str, cx: &mut Context<Self>) -> usize {
        self.offered(typed, cx).len()
    }

    /// The offer picked goes onto the mod shown, and the field is cleared and refocused.
    pub(super) fn add_offered_tags(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let field = self.tags.clone();
        let text = text.to_string();
        let _ = self.mods.update(cx, |mods, cx| mods.add_tags(text, cx));
        field.update(cx, |field, cx| field.set_text_quietly("", cx));
        let focus = Focusable::focus_handle(self.tags.read(cx), cx);
        window.focus(&focus, cx);
    }

    fn keep_drafts(&mut self, cx: &mut Context<Self>) {
        let _ = self.mods.update(cx, |mods, cx| mods.keep_drafts(cx));
    }

    /// The fields hold what the mod shown has, unless they are being typed in.
    fn sync_fields(&mut self, cx: &mut Context<Self>) {
        let Some(details) = self.shown.details.clone() else {
            return;
        };
        let typing = self.note.read(cx).is_focused_now();
        if !typing && self.note.read(cx).text() != details.note {
            let text = details.note.clone();
            self.note
                .update(cx, |field, cx| field.set_text_quietly(&text, cx));
        }
    }

    pub(super) fn fold(
        &self,
        key: &'static str,
        starts_folded: bool,
        cx: &mut Context<Self>,
    ) -> (bool, Fold) {
        let folded = self.folded.contains(key) != starts_folded;
        let fold = cx.listener(move |view: &mut Self, _, _, cx| {
            if !view.folded.remove(key) {
                view.folded.insert(key);
            }
            cx.notify();
        });
        (folded, Box::new(fold))
    }

    pub(super) fn copyable(
        &self,
        key: impl Into<gpui::ElementId>,
        text: &str,
        size: Pixels,
        weight: FontWeight,
        colour: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<crate::ui::text_field::TextField> {
        use gpui::AppContext;
        let key = key.into();
        let made = self.copyable.borrow().get(&key).cloned();
        let field = made.unwrap_or_else(|| {
            let field = cx.new(|cx| {
                crate::ui::text_field::TextField::new(size, cx)
                    .read_only()
                    .weight(weight)
                    .colour(colour)
            });
            self.copyable.borrow_mut().insert(key, field.clone());
            field
        });
        if field.read(cx).text() != text {
            field.update(cx, |field, cx| field.set_text_quietly(text, cx));
        }
        field
    }

    /// The About tab with its tags and note open, and the keys in the tags or the note, as T and
    /// N on the list ask.
    pub(super) fn edit_notes(&mut self, tags: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = 0;
        self.folded.remove("about-notes");
        let field = if tags { &self.tags } else { &self.note };
        window.focus(&gpui::Focusable::focus_handle(field.read(cx), cx), cx);
        cx.emit(Asked::Tab(0));
        cx.notify();
    }

    /// The tab before or after the shown one, round the ends, when a mod's details are shown.
    pub(super) fn step_tab(&mut self, by: isize, cx: &mut Context<Self>) {
        if self.shown.details.is_none() {
            return;
        }
        let count = tabs(0).len() as isize;
        self.tab = (self.tab as isize + by).rem_euclid(count) as usize;
        cx.emit(Asked::Tab(self.tab));
        cx.notify();
    }

    pub fn show(&mut self, shown: DetailsShown, cx: &mut Context<Self>) {
        let other = self.shown.details.as_ref().map(|d| d.path.clone())
            != shown.details.as_ref().map(|d| d.path.clone());
        if other {
            self.small_picture = false;
        }
        self.shown = shown;
        self.sync_fields(cx);
        cx.notify();
    }
}

#[cfg(test)]
thread_local! {
    pub(super) static RENDERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl gpui::Render for DetailsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(test)]
        RENDERS.with(|renders| renders.set(renders.get() + 1));
        let theme = cx.global::<Theme>().clone();
        let (_, _, width) =
            super::pane_widths(&theme, window.viewport_size().width, self.shown.panes);
        let view = self.view_of();
        if self.view_shown.is_some_and(|shown| shown != view) {
            self.view_turn = crate::ui::motion::opening();
        }
        self.view_shown = Some(view);
        let pane = self.details_pane(&theme, width, window, cx);
        let pane = match self.view_turn {
            0 => pane.into_any_element(),
            turn => crate::ui::motion::appear(pane, ("details-appear", turn)).into_any_element(),
        };
        let focused = self.focus.is_focused(window);
        div()
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .bg(theme.colours.panel)
            .child(crate::ui::page::page(
                "details",
                &self.paging,
                &theme,
                cx.entity_id(),
                pane,
            ))
            .when(focused, |d| {
                d.child(
                    div()
                        .absolute()
                        .inset(theme::GAP_LINE)
                        .rounded(theme::CORNER)
                        .border_2()
                        .border_color(theme.colours.focus),
                )
            })
    }
}

fn tabs(needing: usize) -> [String; 4] {
    [
        put(&say!("About")),
        put(&say!("Load order")),
        put(&say!("Files")),
        put(&say!("Needed by ({})", needing)),
    ]
}

pub(super) fn defined(theme: &Theme, term: String) -> gpui::Stateful<gpui::Div> {
    let meaning = crate::ui::card::meaning(&term).unwrap_or_default();
    let has = !meaning.is_empty();
    let shown = div()
        .id(gpui::SharedString::from(format!("defined-{term}")))
        .flex()
        .flex_none()
        .items_center()
        .gap(theme::GAP_SMALL)
        .aria_label(term.clone())
        .aria_description(meaning.clone())
        .child(
            label(theme, theme.body(), theme.colours.ink2, term).when(has, |d| {
                d.border_b_1()
                    .border_dashed()
                    .border_color(theme.colours.ink3)
            }),
        )
        .when(has, |d| {
            d.child(icon("info", theme.sized(12.), theme.colours.ink3))
        });
    crate::ui::tip::tipped(shown, meaning.into())
}

impl DetailsView {
    fn view_of(&self) -> u8 {
        let shown = &self.shown;
        match () {
            _ if shown.comparison.is_some() => 0,
            _ if shown.problems_open && !shown.previewing => 1,
            _ if shown.sort_moves.is_some() && shown.selection.is_none() => 2,
            _ if shown.selection.is_some() => 3,
            _ if shown.details.is_some() => 10 + self.tab.min(9) as u8,
            _ => 5,
        }
    }

    pub(super) fn chosen_pane(
        &self,
        theme: &Theme,
        details: &Details,
        width: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let c = &theme.colours;
        let inner = width - theme::GAP_CARD * 2. - theme.sized(9.) - theme::GAP_HAIR * 2.;
        let preview = details.preview.clone();
        let pictures = self.pictures.of(&details.path, cx);
        let tile = |cx: &mut Context<Self>| -> gpui::AnyElement {
            if let Some(path) = preview.clone() {
                let tip = put(&say!("Click for the picture at full size"));
                let shown = div()
                    .id("small-picture")
                    .flex_none()
                    .size(theme.sized(64.))
                    .rounded(theme::CORNER_LARGE)
                    .bg(c.well)
                    .overflow_hidden()
                    .cursor_pointer()
                    .child(
                        pictures
                            .img(std::path::PathBuf::from(path))
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    )
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.small_picture = false;
                        cx.notify();
                    }));
                return crate::ui::tip::tipped(shown, tip.into()).into_any_element();
            }
            let first: String = details
                .name
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect())
                .unwrap_or("?".into());
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(theme.sized(64.))
                .rounded(theme::CORNER_LARGE)
                .bg(c.well)
                .child(label(theme, theme.display(), c.brand, first).font_weight(theme.semibold))
                .into_any_element()
        };
        let large = details.preview.as_ref().filter(|_| !self.small_picture);
        let made_for_warns = details.made_for.is_some() && !details.version_ok && !details.official;
        let chips = div()
            .flex()
            .flex_wrap()
            .pt(theme::GAP_THIN)
            .gap(theme::GAP_TINY)
            .when(!details.source.is_empty(), |d| {
                let said = put(&rimstack_core::base::sentence::Sentence::said(
                    details.source,
                    Vec::new(),
                ));
                d.child(
                    Chip::new(said)
                        .icon(if details.workshop {
                            "cloud"
                        } else {
                            "hard-drive"
                        })
                        .max_width(inner),
                )
            })
            .when(!details.repository.is_empty() && !details.workshop, |d| {
                d.child(Chip::new(put(&say!("Git"))).icon("git-branch"))
            })
            .when(!details.version.is_empty(), |d| {
                d.child(Chip::new(details.version.clone()).max_width(inner))
            })
            .when(!details.game_versions.is_empty(), |d| {
                let chip = Chip::new(put(&say!("RimWorld {}", details.game_versions.clone())))
                    .max_width(inner)
                    .tip(match details.version_ok {
                        true => put(&say!("Marked as working on this game version")),
                        false => String::new(),
                    });
                d.child(if made_for_warns {
                    chip.tone(c.warn)
                } else {
                    chip
                })
            })
            .when_some(details.megabytes, |d, mb| {
                let shown = if mb < 10. {
                    format!("{mb:.1}")
                } else {
                    format!("{}", mb.round())
                };
                d.child(Chip::new(put(&say!("{} MB", shown))))
            })
            .when_some(details.updated, |d, at| {
                let ago = rimstack_core::base::elapsed::ago(
                    crate::workspace::now_secs().saturating_sub(at),
                );
                d.child(Chip::new(put(&say!("updated {}", ago.inside()))).max_width(inner))
            });
        let active = self.shown.selected_active;
        let toggle = if active {
            put(&say!("Deactivate"))
        } else {
            put(&say!("Activate"))
        };
        let (open, page) = (put(&say!("Open folder")), put(&say!("Workshop page")));
        // The Workshop button's room counts even when it is not shown.
        let full = |text: &str| {
            measure(window, theme, text, theme.small(), FontWeight::NORMAL)
                + theme.icon()
                + theme::GAP_SMALL
                + theme.sized(20.)
        };
        let narrow = inner < full(&toggle) + full(&open) + full(&page) + theme::GAP_SMALL * 2.;
        let tab = self.tab;
        let names_of_tabs = tabs(details.needed_by.len());
        let all_words = names_of_tabs.iter().fold(px(0.), |all, name| {
            all + measure(window, theme, name, theme.small(), theme.semibold)
        });
        let share = (inner - px(4.) - all_words) / names_of_tabs.len() as f32;
        let choice = div()
            .id("details-tabs")
            .role(gpui::Role::TabList)
            .aria_label(put(&say!("Details")))
            .p(px(2.))
            .rounded(theme::CORNER)
            .bg(c.seg)
            .w_full()
            .child(
                div()
                    .flex()
                    .children(names_of_tabs.into_iter().enumerate().map(|(at, name)| {
                        let on = at == tab;
                        let weight = if on {
                            theme.semibold
                        } else {
                            FontWeight::NORMAL
                        };
                        let wide =
                            measure(window, theme, &name, theme.small(), theme.semibold) + share;
                        div()
                            .id(("tab", at))
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(wide)
                            .h(theme.segment())
                            .rounded(theme::CORNER - px(1.))
                            .when(on, |d| d.bg(c.ctl_hi))
                            .child(
                                label(theme, theme.small(), if on { c.ink } else { c.ink2 }, name)
                                    .font_weight(weight),
                            )
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.tab = at;
                                cx.emit(Asked::Tab(at));
                                cx.notify();
                            }))
                    })),
            );
        let header = div()
            .flex()
            .flex_col()
            .when_some(large, |d, path| {
                let tall = (inner * (9. / 16.)).min(theme.sized(220.));
                d.child(
                    div()
                        .id("large-picture")
                        .w_full()
                        .h(tall)
                        .rounded(theme::CORNER_LARGE)
                        .bg(c.well)
                        .overflow_hidden()
                        .cursor_pointer()
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.small_picture = true;
                            cx.notify();
                        }))
                        .child(
                            pictures
                                .img(std::path::PathBuf::from(path))
                                .w(inner)
                                .h(tall)
                                // The picture's own ratio would otherwise set the height.
                                .aspect_ratio(inner / tall)
                                .object_fit(ObjectFit::Contain),
                        ),
                )
                .child(div().h(theme::GAP_MEDIUM))
            })
            .child(
                div()
                    .flex()
                    .gap(theme::GAP_LARGE)
                    .when(large.is_none(), |d| d.child(tile(cx)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(theme::GAP_THIN)
                            .child(
                                div()
                                    .w_full()
                                    .child(self.copyable(
                                        "name",
                                        &details.name,
                                        theme.heading(),
                                        FontWeight::BOLD,
                                        c.ink,
                                        cx,
                                    ))
                                    .font_weight(FontWeight::BOLD),
                            )
                            .child(
                                div()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .sized_text(theme, theme.body())
                                    .text_color(c.ink2)
                                    .child(details.author.clone()),
                            )
                            .child(chips),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .pt(theme::GAP_LARGE)
                    .gap(theme::GAP_SMALL)
                    .when(!self.shown.previewing, |d| {
                        d.child(
                            Btn::new("toggle", toggle)
                                .icon(if active { "minus" } else { "plus" })
                                .primary_if(!active)
                                .on_click(self.on_mods(|mods, cx| mods.toggle_selected(cx))),
                        )
                    })
                    .child({
                        let path = details.path.clone();
                        Btn::new(
                            "open-folder",
                            if narrow { String::new() } else { open.clone() },
                        )
                        .icon("folder-open")
                        .tip(open)
                        .on_click(move |_, _, cx| cx.open_with_system(std::path::Path::new(&path)))
                    })
                    .when(details.workshop, |d| {
                        let folder = details.folder.clone();
                        d.child(
                            Btn::new(
                                "workshop-page",
                                if narrow { String::new() } else { page.clone() },
                            )
                            .icon("external-link")
                            .tip(page)
                            .on_click(move |_, _, cx| {
                                cx.open_url(&crate::ui::workshop_page(&folder))
                            }),
                        )
                    })
                    .when(!details.workshop && !details.repository.is_empty(), |d| {
                        let repository = details.repository.clone();
                        d.child(
                            Btn::new("repository", put(&say!("Repository page")))
                                .icon("git-branch")
                                .tip(repository.clone())
                                .on_click(move |_, _, cx| cx.open_url(&repository)),
                        )
                    }),
            )
            .child(div().h(theme::GAP_LARGE))
            .child(choice);
        let body = match tab {
            0 => div()
                .flex()
                .flex_col()
                .pt(theme::GAP_LARGE)
                .gap(theme::GAP_CARD)
                .children(self.about(theme, details, inner, window, cx)),
            1 => div()
                .flex()
                .flex_col()
                .children(self.placed(theme, details, inner, cx)),
            2 => div()
                .flex()
                .flex_col()
                .children(self.files(theme, details, inner, window, cx)),
            _ => div()
                .flex()
                .flex_col()
                .children(self.needed_by(theme, details)),
        };
        div().flex().flex_col().w_full().child(header).child(body)
    }
}
