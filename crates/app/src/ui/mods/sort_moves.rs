use super::chosen::DetailsView;
use crate::ui::card::Card;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::{Btn, TextSized, icon, label};
use crate::words::put;
use crate::workspace::load_order::{SortDrop, SortGroup};
use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, relative,
};
use rimstack_core::base::sentence::Sentence;
use rimstack_core::{say, say_n};
use std::rc::Rc;

impl DetailsView {
    pub(super) fn sort_moves(
        &self,
        theme: &Theme,
        groups: &[SortGroup],
        dropped: Option<Rc<Vec<SortDrop>>>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let c = &theme.colours;
        let moved = groups.iter().map(|g| g.moves.len()).sum::<usize>();
        let chosen = self.shown.chosen_id;
        let mut shown = div()
            .flex()
            .flex_col()
            .gap(theme::GAP_HAIR)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme::GAP)
                    .pb(theme::GAP_HAIR)
                    .child(
                        label(theme, theme.heading(), c.ink, put(&say!("Sort preview")))
                            .font_weight(theme.semibold),
                    )
                    .child(
                        Btn::new("copy-report", put(&say!("Copy the report")))
                            .icon("copy")
                            .tip(put(&say!(
                                "Copies what moves and why, and the rules the sort could not follow, as text for a bug report"
                            )))
                            .on_click(self.on_mods(|mods, cx| {
                                let report = mods.list.read(cx).order.sort_report();
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(report));
                            })),
                    )
            )
            .child(
                div()
                    .pb(theme::GAP_MEDIUM)
                    .sized_text(theme, theme.body())
                    .text_color(c.ink2)
                    .child(put(&say_n!(moved, "{} mod moves", "{} mods move"))),
            );
        for (at, group) in groups.iter().enumerate() {
            let moves = group.moves.iter().enumerate().map(|(nth, one)| {
                let (mod_id, up) = (one.mod_id, one.to < one.from);
                let why = put(&one.why);
                let row = div()
                    .id(("sort-move", at * 10_000 + nth))
                    .role(gpui::Role::Button)
                    .aria_label(format!("{}, {}", one.name, why))
                    .flex()
                    .items_center()
                    .gap(theme::GAP)
                    .min_h(theme.sized(40.))
                    .py(theme::GAP_MEDIUM / 2.)
                    .px(theme::GAP_SMALL)
                    .rounded(theme::CORNER)
                    .cursor_pointer()
                    .when(mod_id.is_some() && mod_id == chosen, |d| d.bg(c.seg))
                    .when(mod_id.is_none() || mod_id != chosen, |d| {
                        d.hover(|s| s.bg(c.chip))
                    })
                    .child(icon(
                        if up { "arrow-up" } else { "arrow-down" },
                        theme.sized(13.),
                        c.moved,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .sized_text(theme, theme.body())
                                    .text_color(c.ink)
                                    .child(one.name.clone()),
                            )
                            .child(
                                div()
                                    .line_clamp(3)
                                    .sized_text(theme, theme.caption())
                                    .text_color(c.ink2)
                                    .child(why.clone()),
                            ),
                    )
                    .child(label(
                        theme,
                        theme.small(),
                        c.ink2,
                        format!("{} → {}", one.from, one.to),
                    ))
                    .on_click(self.on_mods(move |mods, cx| {
                        if let Some(id) = mod_id {
                            mods.show_mod(id, cx)
                        }
                    }));
                crate::ui::tip::tipped(row, why.into()).into_any_element()
            });
            shown = shown.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme::GAP_HAIR)
                    .pb(theme::GAP_MEDIUM)
                    .child(
                        div()
                            .flex()
                            .gap(theme::GAP_SMALL)
                            .pb(theme::GAP_HAIR)
                            .child(
                                label(
                                    theme,
                                    theme.small(),
                                    c.ink2,
                                    put(&Sentence::said(group.name, Vec::new())),
                                )
                                .font_weight(theme.semibold),
                            )
                            .child(label(
                                theme,
                                theme.small(),
                                c.ink3,
                                group.moves.len().to_string(),
                            )),
                    )
                    .children(moves),
            );
        }
        if let Some(dropped) = dropped.filter(|d| !d.is_empty()) {
            let lines: Vec<_> = dropped
                .iter()
                .enumerate()
                .map(|(at, d)| {
                    let (rule, kind, why) = (put(&d.rule), put(&d.kind), put(&d.why));
                    let text = format!("{rule} ({kind}, {why})");
                    let whole = format!("{text} [{} → {}]", d.first_id, d.then_id);
                    let field = self.copyable(
                        ("sort-dropped", at),
                        &whole,
                        theme.caption(),
                        FontWeight::NORMAL,
                        c.ink2,
                        cx,
                    );
                    div().pb(theme::GAP_HAIR).child(field).into_any_element()
                })
                .collect();
            let (folded, fold) = self.fold("sort-dropped", false, cx);
            let title = put(&say_n!(
                dropped.len(),
                "{} rule could not be followed",
                "{} rules could not be followed"
            ));
            shown = shown.pt(theme::GAP_SMALL).child(
                Card::new("sort-dropped", title)
                    .body(lines)
                    .folds(folded, fold)
                    .render(theme, None),
            );
        }
        let _ = cx;
        shown
    }

    pub(super) fn slowest(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let overview = self.shown.overview.as_ref()?;
        if overview.slowest.is_empty() {
            return None;
        }
        let c = &theme.colours;
        let rows = overview.slowest.iter().enumerate().map(|(at, slow)| {
            let mod_id = slow.mod_id;
            div()
                .id(("slowest", at))
                .relative()
                .flex()
                .items_center()
                .h(theme.control())
                .px(theme::GAP_SMALL)
                .gap(theme::GAP_SMALL)
                .cursor_pointer()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left_0()
                        .w(relative(slow.part as f32))
                        .rounded(theme::CORNER)
                        .bg(crate::ui::widgets::with_alpha(c.warn, 0.14)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .sized_text(theme, theme.body())
                        .text_color(c.ink)
                        .child(slow.name.clone()),
                )
                .child(label(theme, theme.small(), c.ink2, slow.took.clone()))
                .on_click(self.on_mods(move |mods, cx| {
                    if let Some(id) = mod_id {
                        mods.show_mod(id, cx)
                    }
                }))
                .into_any_element()
        });
        let (folded, fold) = self.fold("overview-slowest", false, cx);
        let card =
            Card::new("overview-slowest", put(&say!("Slowest to load"))).body(rows.collect());
        Some(card.folds(folded, fold).render(theme, None))
    }
}
