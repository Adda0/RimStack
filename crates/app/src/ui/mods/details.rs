use super::chosen::DetailsView;
use crate::ui::card::Card;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, Chip, TextSized, label};
use crate::words::{count, put};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder,
};
use rimstack_core::{say, say_n};

type Pressed = Box<dyn Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App)>;

fn line(
    theme: &Theme,
    id: &'static str,
    text: String,
    action: Option<(String, bool, Pressed)>,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(theme::GAP)
        .child(
            div()
                .flex_initial()
                .min_w_0()
                .sized_text(theme, theme.body())
                .text_color(theme.colours.ink)
                .child(text),
        )
        .when_some(action, |d, (said, primary, pressed)| {
            d.child(Btn::new(id, said).primary_if(primary).on_click(pressed))
        })
        .child(div().flex_1())
        .into_any_element()
}

impl DetailsView {
    pub(super) fn on_mods(
        &self,
        run: impl Fn(&mut super::Mods, &mut Context<super::Mods>) + 'static,
    ) -> Pressed {
        let mods = self.mods.clone();
        Box::new(move |_, _, cx| {
            let _ = mods.update(cx, |mods, cx| run(mods, cx));
        })
    }

    pub(super) fn details_pane(
        &self,
        theme: &Theme,
        width: gpui::Pixels,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let c = &theme.colours;
        let bar = crate::ui::page::room(theme);
        let scrolled = div()
            .flex()
            .flex_col()
            .p(theme::GAP_CARD)
            .pr(theme::GAP_CARD + bar);
        if let Some(comparison) = self.shown.comparison.clone() {
            return scrolled.child(self.compare_summary(theme, &comparison));
        }
        if self.shown.problems_open && !self.shown.previewing {
            let inner = width - bar - theme::GAP_CARD * 2.;
            if let Some(panel) = self.problems_pane(theme, inner, window) {
                return scrolled.child(panel);
            }
        }
        if let Some(groups) = self
            .shown
            .sort_moves
            .clone()
            .filter(|_| self.shown.selection.is_none())
        {
            let dropped = self.shown.sort_dropped.clone();
            return scrolled.child(self.sort_moves(theme, &groups, dropped, cx));
        }
        if let Some(summary) = self.shown.selection.clone() {
            return scrolled.child(self.selection_pane(
                theme,
                &summary,
                width - bar - theme::GAP_CARD * 2.,
                window,
                cx,
            ));
        }
        if let Some(details) = &self.shown.details {
            return scrolled.child(self.chosen_pane(theme, details, width, window, cx));
        }
        let title = match self.shown.demo {
            true => put(&say!("Demo data")),
            false => put(&say!("Load order")),
        };
        let head = div()
            .flex()
            .flex_col()
            .child(label(theme, theme.heading(), c.ink, title).font_weight(FontWeight::BOLD))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .pt(theme::GAP_SMALL)
                    .gap(theme::GAP_TINY)
                    .when(!self.shown.demo, |d| {
                        d.child(Chip::new(put(&say!(
                            "RimWorld {}",
                            self.shown.game_version.clone()
                        ))))
                    })
                    .child(Chip::new(put(&say!("{} active", count(self.shown.active)))))
                    .child(Chip::new(put(&say!(
                        "{} inactive",
                        count(self.shown.inactive)
                    ))))
                    .when(self.shown.fresh > 0, |d| {
                        d.child(
                            Chip::new(put(&say!("{} new", count(self.shown.fresh))))
                                .tone(c.ok)
                                .tip(put(&say!(
                                    "Activate the mods installed since the game was last started from RimStack"
                                )))
                                .on_click("summary-fresh", self.on_mods(|mods, cx| mods.activate_fresh(cx))),
                        )
                    }),
            );
        scrolled
            .gap(theme::GAP_CARD)
            .child(head)
            .when(!self.shown.demo, |d| d.children(self.verdict(theme, cx)))
            .children(self.review(theme, cx))
            .children(self.slowest(theme, cx))
            .when(!self.shown.demo, |d| {
                d.children(self.kept(theme, window, cx))
                    .children(self.on_disk(theme, window, cx))
            })
    }

    fn verdict(&self, theme: &Theme, cx: &gpui::App) -> Option<gpui::Stateful<gpui::Div>> {
        let c = &theme.colours;
        let overview = self.shown.overview.as_ref()?;
        let warnings = self.shown.warnings;
        let dirty = self.shown.dirty;
        let errors = overview.error_count;
        let playable = errors == 0 && !dirty;
        let title = match (playable, warnings) {
            (false, _) => put(&say!("Before you play")),
            (true, 0) => put(&say!("Ready to play")),
            (true, n) => put(&say_n!(n, "Playable, {} warning", "Playable, {} warnings")),
        };
        let tone = if errors > 0 {
            c.err
        } else if dirty {
            c.warn
        } else if warnings > 0 {
            c.ink
        } else {
            c.ok
        };
        let mut body: Vec<AnyElement> = overview
            .first_errors
            .iter()
            .enumerate()
            .map(|(at, said)| {
                let act = super::edit::acting(self.mods.clone(), said);
                crate::ui::problem::problem(
                    theme,
                    at,
                    said,
                    None,
                    (!self.shown.previewing).then_some(act),
                )
            })
            .collect();
        let troubled = self.shown.troubled;
        if troubled {
            let text = if overview.more_errors > 0 {
                put(&say_n!(
                    overview.more_errors,
                    "{} more problem",
                    "{} more problems"
                ))
            } else if warnings == 0 {
                String::new()
            } else if overview.version_only == warnings {
                put(&say!("All for the game version"))
            } else {
                put(&say_n!(
                    warnings,
                    "{} mod with a warning",
                    "{} mods with a warning"
                ))
            };
            body.push(line(
                theme,
                "all-problems",
                text,
                Some((
                    put(&say!("All problems")),
                    false,
                    self.on_mods(super::Mods::open_problems),
                )),
            ));
        }
        let outside = crate::ui::outside::outside(cx).read(cx);
        if let Some(start) = outside.last_start.clone() {
            let logged = start.start.errors.unwrap_or(0) > 0;
            body.push(
                div()
                    .flex()
                    .items_center()
                    .gap(theme::GAP)
                    .child(
                        div()
                            .flex_initial()
                            .min_w_0()
                            .sized_text(theme, theme.body())
                            .text_color(c.ink2)
                            .child(put(&start.line)),
                    )
                    .flex_initial()
                    .when(logged, |d| {
                        d.child(Btn::new("open-log", put(&say!("Open log"))).on_click(
                            self.on_mods(|_, cx| {
                                crate::ui::navigate::go(
                                    crate::ui::navigate::Go::Section(
                                        crate::ui::section::Section::Logs,
                                    ),
                                    cx,
                                )
                            }),
                        ))
                    })
                    .child(div().flex_1())
                    .into_any_element(),
            );
        }
        if dirty {
            let said = put(&say!("Not saved: {}", self.shown.unsaved.clone()));
            body.push(line(
                theme,
                "save-list",
                said,
                Some((
                    put(&say!("Save")),
                    true,
                    self.on_mods(|mods, cx| mods.save(None, cx)),
                )),
            ));
        }
        if errors == 0 && !dirty && !troubled {
            body.push(line(
                theme,
                "saved",
                put(&say!("No problems, and the list is saved.")),
                None,
            ));
        }
        let note = if errors > 0 {
            count(errors)
        } else {
            String::new()
        };
        Some(
            Card::new("verdict", title)
                .tone(tone)
                .note(note)
                .spacing(theme::GAP)
                .body(body)
                .render(theme, None),
        )
    }

    fn review(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<gpui::Stateful<gpui::Div>> {
        use super::chosen::Asked;
        let summary = crate::ui::outside::outside(cx).read(cx).summary.clone();
        let replacements = summary.as_ref().map_or(0, |s| s.replacements);
        let line = |id: &'static str, said: String, asked: fn() -> Asked| {
            let show = Btn::new(id, put(&say!("Show")))
                .on_click(cx.listener(move |_, _, _, cx| cx.emit(asked())));
            div()
                .flex()
                .items_center()
                .gap(theme::GAP)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .sized_text(theme, theme.body())
                        .text_color(theme.colours.ink)
                        .child(said),
                )
                .child(show)
                .into_any_element()
        };
        let mut body = Vec::new();
        if self.shown.conflicts > 0 {
            let said = put(&say_n!(
                self.shown.conflicts,
                "{} mod shares textures with another",
                "{} mods share textures with another"
            ));
            body.push(line("review-conflicts", said, || {
                Asked::Filter("is:conflict")
            }));
        }
        let counts = crate::ui::counts::read(cx);
        if counts.changed_since > 0 {
            let said = put(&say_n!(
                counts.changed_since,
                "{} mod changed since the game was last started",
                "{} mods changed since the game was last started"
            ));
            body.push(line("review-changed", said, || {
                Asked::Section(crate::ui::section::Section::Downloads)
            }));
        }
        if counts.behind > 0 {
            let said = put(&say_n!(
                counts.behind,
                "{} update available",
                "{} updates available"
            ));
            body.push(line("review-updates", said, || {
                Asked::Section(crate::ui::section::Section::Downloads)
            }));
        }
        if replacements > 0 {
            let said = put(&say_n!(
                replacements,
                "{} mod has a replacement",
                "{} mods have a replacement"
            ));
            body.push(line("review-replaced", said, || {
                Asked::Section(crate::ui::section::Section::Downloads)
            }));
        }
        if body.is_empty() {
            return None;
        }
        let (folded, fold) = self.fold("overview-review", false, cx);
        let card = Card::new("overview-review", put(&say!("To review")))
            .spacing(theme::GAP)
            .body(body);
        Some(card.folds(folded, fold).render(theme, None))
    }

    fn kept(
        &self,
        theme: &Theme,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let summary = crate::ui::outside::outside(cx).read(cx).summary.clone()?;
        let summary = &summary;
        let (folded, fold) = self.fold("overview-inventory", true, cx);
        // Its note says what it holds while it is folded.
        let note = if folded {
            put(&summary.kept_note)
        } else {
            String::new()
        };
        let labels: Vec<String> = summary.kept.iter().map(|kept| put(&kept.label)).collect();
        let named = labels
            .iter()
            .map(|said| {
                crate::ui::widgets::measure(window, theme, said, theme.body(), FontWeight::NORMAL)
            })
            .fold(gpui::px(0.), gpui::Pixels::max)
            + theme::GAP;
        let lines = summary.kept.iter().zip(labels).map(|(kept, said)| {
            div()
                .flex()
                .child(
                    label(theme, theme.body(), theme.colours.ink2, said)
                        .flex_none()
                        .w(named),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.body())
                        .text_color(theme.colours.ink)
                        .child(put(&kept.value)),
                )
                .into_any_element()
        });
        let mut lines: Vec<AnyElement> = lines.collect();
        if !self.shown.community_problem.is_empty() {
            lines.push(
                div()
                    .w_full()
                    .sized_text(theme, theme.body())
                    .text_color(theme.colours.err)
                    .child(put(&say!(
                        "Community rules not updated: {}",
                        self.shown.community_problem.clone()
                    )))
                    .into_any_element(),
            );
        }
        let card = Card::new("overview-inventory", put(&say!("Kept")))
            .note(note)
            .body(lines);
        Some(card.folds(folded, fold).render(theme, None))
    }

    /// The space the lists take; a line shows the mods it counts, biggest first.
    fn on_disk(
        &self,
        theme: &Theme,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let usage = self.shown.overview.as_ref()?.usage;
        if usage.active + usage.inactive == 0 {
            return None;
        }
        let text = |bytes: u64| {
            put(&rimstack_core::base::sizes::together(bytes as f64 / 1_048_576.0, 0.0).0)
        };
        let (folded, fold) = self.fold("overview-disk", true, cx);
        let note = match folded {
            true => [
                (usage.active, say!("{} active", text(usage.active))),
                (usage.inactive, say!("{} inactive", text(usage.inactive))),
                (usage.unplayed, say!("{} not played", text(usage.unplayed))),
            ]
            .into_iter()
            .filter(|(bytes, _)| *bytes > 0)
            .map(|(_, said)| put(&said))
            .collect::<Vec<_>>()
            .join(", "),
            false => String::new(),
        };
        // "" for the active list, ordered by size; else the filter that shows the inactive ones.
        let lines = [
            (say!("Active"), usage.active, None),
            (say!("Inactive"), usage.inactive, Some("is:inactive")),
            (say!("Workshop"), usage.workshop, Some("is:workshop")),
            (say!("local"), usage.local, Some("is:local")),
            (say!("Git"), usage.git, Some("is:git")),
            (
                say!("Not played"),
                usage.unplayed,
                Some("is:inactive is:unplayed"),
            ),
        ];
        let lines: Vec<_> = lines
            .into_iter()
            .filter(|(_, bytes, _)| *bytes > 0)
            .collect();
        let labels: Vec<String> = lines.iter().map(|(said, _, _)| put(said)).collect();
        let named = labels
            .iter()
            .map(|said| {
                crate::ui::widgets::measure(window, theme, said, theme.body(), FontWeight::NORMAL)
            })
            .fold(gpui::px(0.), gpui::Pixels::max)
            + theme::GAP;
        let body =
            lines
                .into_iter()
                .zip(labels)
                .enumerate()
                .map(|(at, ((_, bytes, filter), said))| {
                    let show = self.on_mods(move |mods, cx| match filter {
                        None => mods.order_active("size down", cx),
                        Some(filter) => {
                            mods.filter_by(filter, cx);
                            mods.order_inactive("size down", cx);
                        }
                    });
                    let line = div()
                        .id(("disk-line", at))
                        .flex()
                        .cursor_pointer()
                        .child(
                            label(theme, theme.body(), theme.colours.ink2, said)
                                .flex_none()
                                .w(named),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .sized_text(theme, theme.body())
                                .text_color(theme.colours.ink)
                                .child(text(bytes)),
                        )
                        .on_click(show);
                    crate::ui::tip::tipped(line, put(&say!("Show these, biggest first")).into())
                        .into_any_element()
                });
        let card = Card::new("overview-disk", put(&say!("On disk")))
            .note(note)
            .body(body.collect());
        Some(card.folds(folded, fold).render(theme, None))
    }
}
