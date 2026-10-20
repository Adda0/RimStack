use crate::mods::community::Community;
use crate::mods::infer::Inferred;
use crate::mods::library::Library;
use crate::mods::list::{self, Category, Mod};
use crate::mods::rules::{self, Kind, Rule};
use crate::mods::sort;
use crate::mods::userrules::UserRules;

#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub yours: &'a UserRules,
    /// Empty unless the community rules are downloaded and switched on.
    pub community: Option<&'a Community>,
    pub from_files: &'a [Inferred],
    /// Compiled code's order always counts: the game needs that order.
    pub hints: bool,
}

pub fn all_rules(library: &Library, mods: &[Mod], sources: Sources) -> Vec<Rule> {
    let abouts = library.abouts(mods);
    let mut found = rules::among(&abouts, &library.game_version);
    found.extend(sources.yours.rules(&abouts));
    let from_files = sources
        .from_files
        .iter()
        .filter(|d| d.code || sources.hints);
    found.extend(from_files.map(|d| Rule {
        first: d.first,
        then: d.then,
        kind: if d.code { Kind::Inferred } else { Kind::Hint },
    }));
    if let Some(community) = sources.community {
        found.extend(community.rules(&abouts));
    }
    found
}

pub fn place(rules: &[Rule], at: usize, count: usize) -> usize {
    let among_others = |m: usize| if m > at { m - 1 } else { m };
    let must_follow: Vec<usize> = rules
        .iter()
        .filter(|r| r.then == at)
        .map(|r| among_others(r.first))
        .collect();
    let must_precede: Vec<usize> = rules
        .iter()
        .filter(|r| r.first == at)
        .map(|r| among_others(r.then))
        .collect();
    sort::place(at, count.saturating_sub(1), &must_follow, &must_precede)
}

/// Sorting the list it gives again moves nothing.
pub fn sorted_by_category(
    library: &Library,
    mods: Vec<Mod>,
    sources: Sources,
) -> (Vec<Mod>, sort::Sorted) {
    // A pass can leave groups its mods' categories and tags don't say (a mod pulled up into
    // another's group) and pinned mods back in their places; the next Sort would read the list
    // anew and move mods again. So the result is sorted again the way that Sort would, until a
    // pass moves nothing, an order comes back, or PASSES ran.
    const PASSES: usize = 8;
    let (mut mods, mut sorted) = sorted_once(library, mods, sources);
    let mut seen = vec![sorted.order.clone()];
    for _ in 1..PASSES {
        let count = mods.len();
        let mut now_at = vec![0; count];
        for (position, &old) in sorted.order.iter().enumerate() {
            now_at[old] = position;
        }
        let from_files: Vec<Inferred> = (sources.from_files.iter())
            .filter(|d| d.first < count && d.then < count)
            .map(|d| Inferred {
                first: now_at[d.first],
                then: now_at[d.then],
                ..d.clone()
            })
            .collect();
        let in_order: Vec<Mod> = sorted.order.iter().map(|&m| mods[m].clone()).collect();
        let sources = Sources {
            from_files: &from_files,
            ..sources
        };
        let (again_mods, again) = sorted_once(library, in_order, sources);
        let held = again.order.iter().enumerate().all(|(at, &m)| at == m);
        for (at, m) in again_mods.into_iter().enumerate() {
            mods[sorted.order[at]] = m;
        }
        let mut groups = vec![0; count];
        for (at, &group) in again.groups.iter().enumerate() {
            groups[sorted.order[at]] = group;
        }
        let dropped = (again.dropped.iter())
            .map(|r| Rule {
                first: sorted.order[r.first],
                then: sorted.order[r.then],
                kind: r.kind,
            })
            .collect();
        sorted = sort::Sorted {
            order: again.order.iter().map(|&at| sorted.order[at]).collect(),
            dropped,
            groups,
        };
        if held || seen.contains(&sorted.order) {
            break;
        }
        seen.push(sorted.order.clone());
    }
    (mods, sorted)
}

fn sorted_once(
    library: &Library,
    mut mods: Vec<Mod>,
    sources: Sources,
) -> (Vec<Mod>, sort::Sorted) {
    let found = all_rules(library, &mods, sources);
    if let Some(community) = sources.community {
        let abouts = library.abouts(&mods);
        let top = abouts
            .iter()
            .map(|about| about.is_some_and(|a| community.loads_at_top(&a.package_id)));
        let top: Vec<bool> = top.collect();
        for (m, top) in mods.iter_mut().zip(top) {
            if top && m.category > Category::Libraries {
                m.category = Category::Libraries;
            }
        }
    }
    let mut subs: Vec<&str> = Vec::new();
    for m in mods
        .iter()
        .filter(|m| !m.sub.is_empty() && m.category != Category::Last)
    {
        if !subs.contains(&m.sub.as_str()) {
            subs.push(&m.sub);
        }
    }
    let per_category = subs.len() + 1;
    let rank = |m: &Mod| subs.iter().position(|s| *s == m.sub).map_or(0, |at| at + 1);
    let own: Vec<usize> = mods
        .iter()
        .map(|m| m.category as usize * per_category + rank(m))
        .collect();
    let bottom = Category::Last as usize * per_category;
    let mut sorted = sort::sort_grouped(&own, bottom, &found);
    for group in &mut sorted.groups {
        *group /= per_category;
    }
    let pinned: Vec<bool> = mods.iter().map(|m| m.pinned).collect();
    sorted.order = sort::keep_pinned(&sorted.order, &pinned);
    for (m, &group) in mods.iter_mut().zip(&sorted.groups) {
        m.category = Category::ALL[group];
    }
    (mods, sorted)
}

pub fn tidy(mods: &[Mod]) -> bool {
    list::runs(mods) <= 2 * Category::ALL.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::library::{build, scan};
    use crate::rimworld::paths::{ModRoot, Source};
    use std::fs;

    fn four_mods() -> (tempfile::TempDir, Library) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Mods");
        for (id, folder) in [
            ("a", "Defs"),
            ("b", "Defs"),
            ("c", "Textures"),
            ("d", "Textures"),
        ] {
            fs::create_dir_all(root.join(id).join("About")).unwrap();
            fs::create_dir_all(root.join(id).join(folder)).unwrap();
            let about = format!(
                "<ModMetaData><packageId>t.{id}</packageId><name>{id}</name></ModMetaData>"
            );
            fs::write(root.join(id).join("About/About.xml"), about).unwrap();
        }
        let roots = [ModRoot {
            path: root,
            source: Source::Local,
        }];
        let active: Vec<String> = ["t.a", "t.b", "t.c", "t.d"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        (tmp, build(scan(&roots), &active, "1.6"))
    }

    fn yours(pairs: &[(&str, &str)]) -> UserRules {
        let mut rules = UserRules::default();
        for (first, then) in pairs {
            rules.add(first, then);
        }
        rules
    }

    fn inferred(first: usize, then: usize, code: bool) -> Inferred {
        Inferred {
            first,
            then,
            why: Default::default(),
            code,
        }
    }

    #[test]
    fn rules_come_from_you_and_from_files_and_hints_only_when_asked() {
        let (_tmp, library) = four_mods();
        let own = yours(&[("t.b", "t.a")]);
        let from_files = [inferred(2, 3, true), inferred(0, 3, false)];
        let mut sources = Sources {
            yours: &own,
            community: None,
            from_files: &from_files,
            hints: false,
        };
        let pairs = |sources| -> Vec<(usize, usize, Kind)> {
            let found = all_rules(&library, &library.active, sources);
            found
                .into_iter()
                .map(|r| (r.first, r.then, r.kind))
                .collect()
        };
        assert_eq!(
            pairs(sources),
            [(1, 0, Kind::Yours), (2, 3, Kind::Inferred)]
        );
        sources.hints = true;
        assert_eq!(pairs(sources).len(), 3);
    }

    #[test]
    fn a_mod_is_placed_after_what_it_must_follow() {
        let rules = [Rule {
            first: 2,
            then: 0,
            kind: Kind::Yours,
        }];
        assert_eq!(place(&rules, 0, 4), 2);
        assert_eq!(place(&[], 1, 4), 1);
    }

    #[test]
    fn sorting_by_category_follows_the_rules_and_keeps_the_old_order_of_the_rest() {
        let (_tmp, library) = four_mods();
        let own = yours(&[("t.b", "t.a")]);
        let sources = Sources {
            yours: &own,
            community: None,
            from_files: &[],
            hints: false,
        };
        let (mods, sorted) = sorted_by_category(&library, library.active.clone(), sources);
        let names: Vec<&str> = sorted
            .order
            .iter()
            .map(|&i| mods[i].name.as_str())
            .collect();
        assert_eq!(names, ["b", "a", "c", "d"]);
        assert!(tidy(&mods));
    }

    #[test]
    fn sorting_keeps_a_sub_category_of_yours_together_inside_its_category() {
        let (_tmp, library) = four_mods();
        let none = UserRules::default();
        let sources = Sources {
            yours: &none,
            community: None,
            from_files: &[],
            hints: false,
        };
        let mut mods = library.active.clone();
        mods[2].category = Category::Content;
        mods[0].sub = "Genes".into();
        mods[2].sub = "Genes".into();
        let (mods, sorted) = sorted_by_category(&library, mods, sources);
        let names: Vec<&str> = sorted
            .order
            .iter()
            .map(|&i| mods[i].name.as_str())
            .collect();
        assert_eq!(names, ["b", "a", "c", "d"]);
        assert_eq!(mods[2].category, Category::Content);
        assert_eq!(sorted.groups[3], Category::Textures as usize);
    }

    #[test]
    fn sort_leaves_a_pinned_mod_where_it_is_and_sorts_the_rest_around_it() {
        let (_tmp, library) = four_mods();
        let own = yours(&[("t.d", "t.a"), ("t.c", "t.b")]);
        let sources = Sources {
            yours: &own,
            community: None,
            from_files: &[],
            hints: false,
        };
        let names = |mods: &[Mod], order: &[usize]| -> Vec<String> {
            order.iter().map(|&i| mods[i].name.clone()).collect()
        };
        let mods = library.active.clone();
        let (mods, sorted) = sorted_by_category(&library, mods, sources);
        assert_eq!(names(&mods, &sorted.order), ["c", "b", "d", "a"]);
        let mut mods = library.active.clone();
        mods[0].pinned = true;
        let (mods, sorted) = sorted_by_category(&library, mods, sources);
        assert_eq!(names(&mods, &sorted.order), ["a", "c", "b", "d"]);
    }

    fn many_mods(count: usize) -> (tempfile::TempDir, Library) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Mods");
        let ids: Vec<String> = (0..count).map(|n| format!("t.m{n}")).collect();
        for id in &ids {
            fs::create_dir_all(root.join(id).join("About")).unwrap();
            let about =
                format!("<ModMetaData><packageId>{id}</packageId><name>{id}</name></ModMetaData>");
            fs::write(root.join(id).join("About/About.xml"), about).unwrap();
        }
        let roots = [ModRoot {
            path: root,
            source: Source::Local,
        }];
        (tmp, build(scan(&roots), &ids, "1.6"))
    }

    struct Random(u64);
    impl Random {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % n
        }
    }

    /// Sort as the app does it twice: the list one Sort gave, with the categories it wrote into
    /// the mods, their tags, pinned mods and the rules read again for the new places, is sorted.
    #[test]
    fn sorting_the_list_a_sort_gave_moves_nothing() {
        let count = 24;
        let (_tmp, library) = many_mods(count);
        let none = UserRules::default();
        let mut random = Random(29);
        for round in 0..400 {
            let mut mods = library.active.clone();
            for m in &mut mods {
                m.category = Category::ALL[random.below(Category::ALL.len())];
                m.sub = ["", "", "x", "y", "z"][random.below(5)].into();
                m.pinned = random.below(10) == 0;
            }
            let how_many = random.below(40);
            let pairs: Vec<(usize, usize, bool)> = (0..how_many)
                .map(|_| {
                    (
                        random.below(count),
                        random.below(count),
                        random.below(2) == 0,
                    )
                })
                .collect();
            let click = |mods: Vec<Mod>| -> (Vec<Mod>, usize) {
                let at = |n: usize| {
                    let id = library.active[n].installed;
                    mods.iter().position(|m| m.installed == id).unwrap()
                };
                let from_files: Vec<Inferred> = (pairs.iter())
                    .map(|&(first, then, code)| inferred(at(first), at(then), code))
                    .collect();
                let sources = Sources {
                    yours: &none,
                    community: None,
                    from_files: &from_files,
                    hints: true,
                };
                let (mods, sorted) = sorted_by_category(&library, mods, sources);
                let moves = sorted.moved().iter().filter(|m| **m).count();
                (
                    sorted.order.iter().map(|&i| mods[i].clone()).collect(),
                    moves,
                )
            };
            let (once, _) = click(mods);
            let (_, moves) = click(once);
            assert_eq!(moves, 0, "round {round}");
        }
    }
}
