//! What the filter field offers for the word being typed: the terms, and for `author:`,
//! `tag:`, `colour:` and `patches:` the values the mods have.

use super::LoadOrder;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::{say, say_n};
use serde::Serialize;

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct FilterOffer {
    /// As it goes in the field, with the `!` typed before it.
    pub term: String,
    pub note: Sentence,
}

/// How many of each kind of mod there are, for the notes beside the terms.
#[derive(Clone, Copy, Default)]
pub struct Counts {
    pub errors: usize,
    pub warnings: usize,
    pub conflicts: usize,
    pub defclashes: usize,
    pub behind: usize,
}

const MOST: usize = 12;

fn terms(counts: Counts) -> Vec<(&'static str, Sentence)> {
    vec![
        (
            "is:error",
            say_n!(counts.errors, "{} mod", "{} mods", counts.errors),
        ),
        (
            "is:warning",
            say_n!(counts.warnings, "{} mod", "{} mods", counts.warnings),
        ),
        (
            "is:conflict",
            say_n!(
                counts.conflicts,
                "{} mod sharing textures",
                "{} mods sharing textures",
                counts.conflicts
            ),
        ),
        (
            "is:defclash",
            say_n!(
                counts.defclashes,
                "{} mod defining a def an active one also defines",
                "{} mods defining a def an active one also defines",
                counts.defclashes
            ),
        ),
        ("is:changed", say!("moved or activated since the last save")),
        ("is:new", say!("installed since the game was last started")),
        (
            "is:updated",
            match counts.behind {
                0 => say!("after checking for updates"),
                behind => say!("{} with an update", behind),
            },
        ),
        ("is:active", say!("in the active list only")),
        ("is:inactive", say!("in the inactive list only")),
        ("is:workshop", say!("from the Workshop")),
        ("is:local", say!("in the game's Mods folder")),
        ("is:git", say!("a git checkout")),
        ("is:pinned", say!("left where it is by Sort")),
        ("is:translation", say!("translates another mod")),
        ("is:notinsave", say!("active, and not in the newest save")),
        ("is:recent", say!("updated by Steam in the last week")),
        ("is:duplicate", say!("installed more than once")),
        (
            "is:incomplete",
            say!("missing a property, like a named Workshop item"),
        ),
        ("has:code", say!("ships compiled code")),
        ("patches:", say!("patches this method of the game")),
        ("def:", say!("defines or patches this def")),
        ("tag:", say!("has this tag")),
        ("colour:", say!("painted with this colour")),
        ("author:", say!("by this author")),
        ("needs:", say!("lists this DLC as a dependency")),
        ("is:unused", say!("a library no active mod needs")),
        ("size:>", say!("bigger than this many megabytes")),
        ("size:<", say!("smaller than this many megabytes")),
    ]
}

/// A value as a term takes it: spaces, which end a term, become `_`.
fn as_value(text: &str) -> String {
    text.trim().replace(' ', "_")
}

impl LoadOrder {
    /// `partial`: the word typed, without its `!`; `taken`: the terms already in the field.
    pub fn filter_offers(
        &self,
        partial: &str,
        bang: &str,
        taken: &[String],
        counts: Counts,
    ) -> Vec<FilterOffer> {
        let fresh = |term: &str| {
            !taken
                .iter()
                .any(|t| t.eq_ignore_ascii_case(&format!("{bang}{term}")))
        };
        let Some((key, typed)) = partial.split_once(':').filter(|(key, _)| {
            matches!(
                *key,
                "author" | "tag" | "colour" | "patches" | "def" | "needs"
            )
        }) else {
            return terms(counts)
                .into_iter()
                .filter(|(term, _)| term.starts_with(partial) && *term != partial && fresh(term))
                .map(|(term, note)| FilterOffer {
                    term: format!("{bang}{term}"),
                    note,
                })
                .collect();
        };
        let typed = typed.to_lowercase();
        let mods = self.active.mods().iter().chain(self.inactive.mods());
        let mut counted: Vec<(String, usize)> = Vec::new();
        for m in mods {
            let values: Vec<String> = match key {
                "author" => m.author.split(',').map(as_value).collect(),
                "tag" => m.tags.iter().map(|t| as_value(t)).collect(),
                "colour" => vec![as_value(&m.colour)],
                "needs" => m.needs_dlcs.iter().map(|d| as_value(d)).collect(),
                "def" => m.defs.iter().map(|d| as_value(d)).collect(),
                _ => m.patched.iter().map(|p| as_value(p)).collect(),
            };
            for value in values.into_iter().filter(|v| !v.is_empty()) {
                let lower = value.to_lowercase();
                if !lower.contains(&typed) || lower == typed {
                    continue;
                }
                match counted
                    .iter_mut()
                    .find(|(seen, _)| seen.eq_ignore_ascii_case(&value))
                {
                    Some((_, uses)) => *uses += 1,
                    None => counted.push((value, 1)),
                }
            }
        }
        // Those starting with what is typed before those holding it, then the most used.
        counted.sort_by_cached_key(|(value, uses)| {
            let lower = value.to_lowercase();
            (!lower.starts_with(&typed), std::cmp::Reverse(*uses), lower)
        });
        counted
            .into_iter()
            .map(|(value, uses)| (format!("{key}:{value}"), uses))
            .filter(|(term, _)| fresh(term))
            .take(MOST)
            .map(|(term, uses)| FilterOffer {
                term: format!("{bang}{term}"),
                note: say_n!(uses, "{} mod", "{} mods", uses),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstack_core::mods::list::{GroupedList, Mod};

    fn order() -> LoadOrder {
        let made = |name: &str, author: &str, tags: &[&str]| Mod {
            name: name.into(),
            author: author.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            ..rimstack_core::mods::demo::generate(1).remove(0)
        };
        let mut order = LoadOrder::empty();
        order.active = GroupedList::new(vec![
            made("a", "Oskar Potocki, Sarg", &["medieval"]),
            made("b", "Oskar Potocki", &["Medieval", "qol"]),
        ]);
        order.inactive = GroupedList::new(vec![made("c", "Mlie", &["mechs"])]);
        order
    }

    fn terms(offers: Vec<FilterOffer>) -> Vec<String> {
        offers.into_iter().map(|o| o.term).collect()
    }

    #[test]
    fn values_are_offered_from_every_mod_starting_ones_and_the_most_used_first() {
        let order = order();
        let none = Counts::default();
        assert_eq!(
            terms(order.filter_offers("author:", "", &[], none)),
            ["author:Oskar_Potocki", "author:Mlie", "author:Sarg"]
        );
        assert_eq!(
            terms(order.filter_offers("tag:me", "!", &[], none)),
            ["!tag:medieval", "!tag:mechs"]
        );
        let taken = ["!tag:medieval".to_string()];
        assert_eq!(
            terms(order.filter_offers("tag:me", "!", &taken, none)),
            ["!tag:mechs"]
        );
        assert_eq!(
            terms(order.filter_offers("author:arg", "", &[], none)),
            ["author:Sarg"]
        );
    }

    #[test]
    fn the_colours_in_use_are_offered_after_colour() {
        let mut order = order();
        let mut mods = order.active.mods().to_vec();
        mods[0].colour = "Rose".into();
        mods[1].colour = "Rose".into();
        order.active = GroupedList::new(mods);
        let offered = order.filter_offers("colour:", "", &[], Counts::default());
        assert_eq!(terms(offered), ["colour:Rose"]);
    }

    #[test]
    fn terms_are_offered_for_the_start_typed() {
        let offers = terms(order().filter_offers("is:wa", "", &[], Counts::default()));
        assert_eq!(offers, ["is:warning"]);
        let taken = ["is:warning".to_string()];
        assert!(
            order()
                .filter_offers("is:wa", "", &taken, Counts::default())
                .is_empty()
        );
    }

    #[test]
    fn def_values_are_offered_from_every_mod_most_used_first() {
        let mut order = order();
        let mut mods = order.active.mods().to_vec();
        mods[0].defs = ["Steel".to_string(), "Wood".to_string()].into();
        mods[1].defs = ["Steel".to_string()].into();
        order.active = GroupedList::new(mods);
        let mut off = order.inactive.mods().to_vec();
        off[0].defs = ["wood".to_string()].into();
        order.inactive = GroupedList::new(off);
        let offered = terms(order.filter_offers("def:", "", &[], Counts::default()));
        assert_eq!(offered, ["def:Steel", "def:Wood"]);
    }

    #[test]
    fn needs_values_are_the_dlcs_mods_depend_on_most_used_first() {
        let mods = vec![
            Mod {
                name: "a".into(),
                needs_dlcs: vec!["ludeon.rimworld.biotech".to_string()],
                ..rimstack_core::mods::demo::generate(1).remove(0)
            },
            Mod {
                name: "b".into(),
                needs_dlcs: vec![
                    "ludeon.rimworld.biotech".to_string(),
                    "ludeon.rimworld.royalty".to_string(),
                ],
                ..rimstack_core::mods::demo::generate(1).remove(0)
            },
        ];
        let mut order = LoadOrder::empty();
        order.active = GroupedList::new(mods);
        let offers = order.filter_offers("needs:", "", &[], Counts::default());
        assert_eq!(
            terms(offers),
            [
                "needs:ludeon.rimworld.biotech",
                "needs:ludeon.rimworld.royalty"
            ]
        );
    }
}
