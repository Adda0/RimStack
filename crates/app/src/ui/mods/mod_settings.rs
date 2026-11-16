//! A mod's own settings files of the game's Config folder, and resetting them.

use super::Mods;
use super::chosen::DetailsView;
use crate::ui::button::Btn;
use crate::ui::card::Card;
use crate::ui::notices;
use crate::ui::prompt::{self, Line, Question};
use crate::ui::theme::{self, Theme};
use crate::ui::widgets::TextSized;
use crate::words::put;
use crate::workspace::load_order::Details;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use rimstack_core::mods::library::ModId;
use rimstack_core::say;

impl DetailsView {
    pub(super) fn mod_settings(&self, theme: &Theme, details: &Details) -> Option<AnyElement> {
        if details.settings_files.is_empty() {
            return None;
        }
        let c = &theme.colours;
        let mut body = vec![
            div()
                .spaced_text(theme, theme.body(), 1.3)
                .text_color(c.ink)
                .child(put(&say!(
                    "Its Mod_*.xml files of the game's Config folder:"
                )))
                .into_any_element(),
        ];
        body.extend(details.settings_files.iter().map(|file| {
            div()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .sized_text(theme, theme.small())
                .text_color(c.ink3)
                .child(file.clone())
                .into_any_element()
        }));
        let (mods, chosen, name) = (
            self.mods.clone(),
            self.shown.chosen_id,
            details.name.clone(),
        );
        body.push(
            div()
                .flex()
                .child(
                    Btn::new("reset-settings", put(&say!("Reset its settings…")))
                        .danger()
                        .icon("trash-2")
                        .tip(put(&say!(
                            "Its settings files are set aside, so the mod starts from its defaults."
                        )))
                        .on_click(move |_, window, cx| {
                            let Some(id) = chosen else {
                                return;
                            };
                            let name = name.clone();
                            let _ = mods.update(cx, |mods, cx| {
                                mods.ask_reset_settings(id, name, window, cx)
                            });
                        }),
                )
                .into_any_element(),
        );
        Some(
            Card::new("about-mod-settings", put(&say!("Mod settings")))
                .spacing(theme::GAP)
                .body(body)
                .render(theme, None)
                .into_any_element(),
        )
    }
}

impl Mods {
    fn ask_reset_settings(
        &mut self,
        id: ModId,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let report = crate::ui::app(cx).settings_report();
        let lines = vec![
            Line::Text(put(&say!("Its Mod_*.xml are moved out of"))),
            Line::Path(report.config_folder.clone().unwrap_or_default()),
            Line::Text(put(&say!(
                "into a new folder, named mod-settings and the time, in"
            ))),
            Line::Path(report.data_folder.clone()),
            Line::Text(put(&say!("Moving the files back restores them."))),
        ];
        prompt::ask(
            Question {
                title: put(&say!("Reset the settings of {}", name)),
                lines,
                label: put(&say!("Set aside")),
                danger: true,
                run: Box::new(move |_, cx| {
                    let app = crate::ui::app(cx);
                    let (said, tone) = match app.reset_mod_settings(id) {
                        Ok(said) => (put(&said), notices::Tone::Ok),
                        Err(problem) => (crate::words::said_of(&problem), notices::Tone::Err),
                    };
                    notices::say(said, tone, cx);
                }),
                second: None,
            },
            window,
            cx,
        );
    }
}
