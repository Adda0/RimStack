use super::{LoadOrder, Preview, rules};
use crate::App;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::describe;
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::{Category, Mod, Row};
use rimstack_core::mods::rules::{Kind, Rule};
use rimstack_core::mods::sort::Sorted;
use rimstack_core::mods::sort_reasons::{self, Why};
use rimstack_core::{say, say_n};
use serde::Serialize;

pub struct SortCount {
    mods: Vec<Mod>,
    detected: rules::Detected,
}

impl SortCount {
    pub fn moves(mut self, app: &App) -> usize {
        let loaded = app.library();
        let Ok((library, _)) = &*loaded else {
            return 0;
        };
        let (_, sorted) = rules::sorted_by_category(app, &mut self.detected, library, self.mods);
        sorted.moved().iter().filter(|moved| **moved).count()
    }
}

#[derive(Serialize)]
pub struct SortGroup {
    pub name: &'static str,
    pub moves: Vec<SortMove>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SortMove {
    pub name: String,
    pub from: usize,
    pub to: usize,
    /// -1 for a mod that is not installed.
    pub mod_id: i32,
    pub why: Sentence,
}

/// A rule the sort could not follow.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SortDrop {
    pub first: String,
    pub then: String,
    /// Package ids as the list names them.
    pub first_id: String,
    pub then_id: String,
    pub kind: Sentence,
    pub why: Sentence,
    /// The rule in words, naming both mods.
    pub rule: Sentence,
}

fn kind_name(kind: Kind) -> Sentence {
    match kind {
        Kind::Yours => say!("your rule"),
        Kind::Requires => say!("required"),
        Kind::LoadAfter => say!("load after"),
        Kind::LoadBefore => say!("load before"),
        Kind::ForceLoadAfter => say!("force load after"),
        Kind::ForceLoadBefore => say!("force load before"),
        Kind::CommunityAfter | Kind::CommunityBefore => say!("community rule"),
        Kind::CommunityBottom => say!("community: at the bottom"),
        Kind::Hint => say!("patch hint"),
        Kind::Inferred => say!("detected from code"),
    }
}

fn dropped(mods: &[Mod], sorted: &Sorted) -> Vec<SortDrop> {
    let drop = |r: &Rule| SortDrop {
        first: mods[r.first].name.clone(),
        then: mods[r.then].name.clone(),
        first_id: mods[r.first].id.clone(),
        then_id: mods[r.then].id.clone(),
        kind: kind_name(r.kind),
        why: match sorted.off_group.contains(r) {
            true => say!("would leave its category"),
            false => say!("in a loop"),
        },
        rule: describe::rule(r, mods),
    };
    sorted.dropped.iter().map(drop).collect()
}

impl LoadOrder {
    pub fn preview_sort(&mut self, app: &App) {
        if self.preview.is_some() || !self.installed {
            return;
        }
        let loaded = app.library();
        let Ok((library, _)) = &*loaded else {
            return;
        };
        let started = std::time::Instant::now();
        let before = self.active.mods().to_vec();
        let (mods, sorted) =
            rules::sorted_by_category(app, &mut self.detected, library, before.clone());
        let moved = sorted.moved();
        rimstack_core::note!("sort_us={}", started.elapsed().as_micros());
        let moves = moved.iter().filter(|m| **m).count();
        self.sort_note = note(&mods, &sorted, moves);
        if moves == 0 {
            return;
        }
        self.sort_dropped = dropped(&mods, &sorted);
        let found = rules::all_rules(app, &mut self.detected, library, &mods);
        let names: Vec<&str> = mods.iter().map(|m| m.name.as_str()).collect();
        let categories: Vec<Category> = mods.iter().map(|m| m.category).collect();
        let why = sorted
            .order
            .iter()
            .zip(&moved)
            .map(|(&old, moved)| {
                moved.then(|| sort_reasons::moved(old, &found, &names, &categories))
            })
            .collect();
        let moved_from = sorted
            .order
            .iter()
            .zip(moved)
            .map(|(&old, moved)| if moved { old + 1 } else { 0 })
            .collect();
        let after: Vec<Mod> = sorted.order.iter().map(|&i| mods[i].clone()).collect();
        self.preview = Some(Preview {
            before,
            grouped_before: self.active.is_grouped(),
            moved_from,
            why,
        });
        self.active.set_grouped(self.headings);
        self.active.set_mods(after);
        self.settle(app);
    }

    pub fn keep_sort(&mut self, app: &App) {
        if self.preview.is_some() {
            self.end_preview();
            self.settle(app);
        }
    }

    pub fn drop_sort(&mut self, app: &App) {
        if let Some(preview) = self.preview.take() {
            self.active.set_grouped(preview.grouped_before);
            self.active.set_mods(preview.before);
            self.sort_note.clear();
            self.sort_dropped.clear();
            self.settle(app);
        }
    }

    pub(super) fn end_preview(&mut self) {
        self.preview = None;
        self.sort_note.clear();
        self.sort_dropped.clear();
    }

    /// Off the window's thread. `None` while a sort is on show or the list is made up.
    pub fn sort_count(&self) -> Option<SortCount> {
        (self.preview.is_none() && self.installed).then(|| SortCount {
            mods: self.active.mods().to_vec(),
            detected: self.detected.clone(),
        })
    }

    fn previewed_moves(&self) -> Vec<(usize, &Mod, usize, &Why)> {
        let Some(preview) = &self.preview else {
            return Vec::new();
        };
        let moved = self
            .active
            .mods()
            .iter()
            .zip(&preview.moved_from)
            .enumerate();
        moved
            .filter_map(|(now, (m, &from))| {
                let why = preview.why.get(now)?.as_ref()?;
                (from > 0).then_some((now, m, from, why))
            })
            .collect()
    }

    pub fn sort_move_count(&self) -> usize {
        self.previewed_moves().len()
    }

    /// Largest kind first.
    pub fn sort_moves(&self) -> Vec<SortGroup> {
        let reasoned = self.previewed_moves().into_iter();
        let mut groups = rimstack_core::base::grouped::by_key(reasoned, |(_, _, _, why)| why.group);
        groups.sort_by_key(|(_, moves)| std::cmp::Reverse(moves.len()));
        groups
            .into_iter()
            .map(|(name, moves)| SortGroup {
                name,
                moves: moves
                    .into_iter()
                    .map(|(now, m, from, why)| SortMove {
                        name: m.name.clone(),
                        from,
                        to: now + 1,
                        mod_id: m.installed.map_or(-1, ModId::to_qml),
                        why: why.text.clone(),
                    })
                    .collect(),
            })
            .collect()
    }

    pub fn sort_dropped(&self) -> &[SortDrop] {
        &self.sort_dropped
    }

    /// The sort on show as text to paste into a report: what moves and why, then the rules it
    /// could not follow.
    pub fn sort_report(&self) -> String {
        let put = crate::words::put;
        let mut lines: Vec<String> = self.sort_note.iter().map(put).collect();
        for group in self.sort_moves() {
            lines.push(String::new());
            lines.push(put(&Sentence::named(group.name)));
            for m in group.moves {
                let why = put(&m.why);
                lines.push(format!("- {}: {} → {}, {why}", m.name, m.from, m.to));
            }
        }
        if !self.sort_dropped.is_empty() {
            lines.push(String::new());
            lines.push(put(&dropped_title(self.sort_dropped.len())));
            for d in &self.sort_dropped {
                let (rule, kind, why) = (put(&d.rule), put(&d.kind), put(&d.why));
                let ids = format!("[{} → {}]", d.first_id, d.then_id);
                lines.push(format!("- {rule} ({kind}, {why}) {ids}"));
            }
        }
        lines.join("\n")
    }

    pub(super) fn why_moved(&self, index: usize) -> Option<&Sentence> {
        let why = self.preview.as_ref()?.why.get(index)?.as_ref();
        why.map(|w| &w.text)
    }

    pub(super) fn moved_from(&self, index: usize) -> usize {
        let preview = self.preview.as_ref();
        preview
            .and_then(|p| p.moved_from.get(index).copied())
            .unwrap_or(0)
    }

    pub fn first_moved(&self) -> Option<usize> {
        self.active
            .rows()
            .iter()
            .position(|row| matches!(row, Row::Mod { index } if self.moved_from(*index) > 0))
    }
}

fn note(mods: &[Mod], sorted: &Sorted, moves: usize) -> Vec<Sentence> {
    let mut note = vec![match moves {
        0 => say!("Already in order."),
        n => say_n!(
            n,
            "Sorting would move {} mod.",
            "Sorting would move {} mods."
        ),
    }];
    let apart = rimstack_core::mods::list::split_groups(sorted.order.iter().map(|&i| &mods[i]));
    if !apart.is_empty() {
        note.push(say_n!(
            apart.len(),
            "A rule keeps {} group in more than one place: {}.",
            "A rule keeps {} groups in more than one place: {}.",
            apart.join(", ")
        ));
    }
    let each: Vec<Sentence> = sorted
        .dropped
        .iter()
        .filter(|r| !matches!(r.kind, Kind::CommunityBottom | Kind::Inferred | Kind::Hint))
        .map(|r| describe::rule(r, mods))
        .collect();
    if !each.is_empty() {
        note.push(say_n!(
            each.len(),
            "Ignored {} rule that contradicts another: {}.",
            "Ignored {} rules that contradict another: {}.",
            Sentence::joined("; ", each).inside()
        ));
    }
    note
}

fn dropped_title(n: usize) -> Sentence {
    say_n!(
        n,
        "{} rule could not be followed",
        "{} rules could not be followed"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstack_core::mods::sort;

    fn mods(names: &[&str]) -> Vec<Mod> {
        let mut mods = rimstack_core::mods::demo::generate(names.len());
        for (m, name) in mods.iter_mut().zip(names) {
            m.name = name.to_string();
            m.id = format!("ann.{}", name.to_lowercase());
        }
        mods
    }

    fn said(drop: &SortDrop) -> [String; 7] {
        [
            drop.first.clone(),
            drop.then.clone(),
            drop.first_id.clone(),
            drop.then_id.clone(),
            drop.kind.to_string(),
            drop.why.to_string(),
            drop.rule.to_string(),
        ]
    }

    #[test]
    fn a_loop_of_rules_gives_the_rule_it_dropped_with_its_kind_and_sentence() {
        let mods = mods(&["Roads", "Hats"]);
        let required = Rule {
            first: 0,
            then: 1,
            kind: Kind::Requires,
        };
        let yours = Rule {
            first: 1,
            then: 0,
            kind: Kind::Yours,
        };
        let found = dropped(&mods, &sort::sort(2, &[required, yours]));
        let found: Vec<[String; 7]> = found.iter().map(said).collect();
        assert_eq!(
            found,
            [[
                "Roads",
                "Hats",
                "ann.roads",
                "ann.hats",
                "required",
                "in a loop",
                "Hats requires Roads"
            ]
            .map(String::from)]
        );
        assert_eq!(
            dropped_title(found.len()).to_string(),
            "1 rule could not be followed"
        );
    }

    #[test]
    fn a_patch_hint_that_would_take_a_mod_out_of_its_category_says_so() {
        let mods = mods(&["Lib", "Other", "Patch", "Last"]);
        let hint = Rule {
            first: 2,
            then: 0,
            kind: Kind::Hint,
        };
        let found = dropped(&mods, &sort::sort_grouped(&[0, 0, 3, 1], 9, &[hint]));
        let found: Vec<[String; 7]> = found.iter().map(said).collect();
        assert_eq!(
            found,
            [[
                "Patch",
                "Lib",
                "ann.patch",
                "ann.lib",
                "patch hint",
                "would leave its category",
                "their files put Patch before Lib"
            ]
            .map(String::from)]
        );
    }
}
