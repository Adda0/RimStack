use crate::App;
use rimstack_core::base::locks::Shared;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::describe::Trouble;
use rimstack_core::mods::library::{Library, ModId};
pub use rimstack_core::mods::list::Side;
use rimstack_core::mods::list::{Arrange, Arrangement, Category, GroupedList, Mod, SortKey, flag};
use rimstack_core::mods::{demo, sort_reasons};
use rimstack_core::say;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

mod details;
mod edits;
mod offers;
mod rows;
mod rules;
mod sorting;

pub use offers::{Counts, FilterOffer};

pub use details::{Details, Files, Overview};
pub use rows::RowView;

pub struct Preview {
    before: Vec<Mod>,
    grouped_before: bool,
    /// 0 if it stays.
    moved_from: Vec<usize>,
    why: Vec<Option<sort_reasons::Why>>,
}

pub struct LoadOrder {
    active: GroupedList,
    inactive: GroupedList,
    troubles: Vec<Vec<Trouble>>,
    version_ignored: Vec<bool>,
    checked_order: Vec<Option<ModId>>,
    outdated: [Vec<Option<String>>; 2],
    checked_inactive: Vec<Option<ModId>>,
    twins: [Vec<bool>; 2],
    slowest: Option<u32>,
    preview: Option<Preview>,
    batching: bool,
    headings: bool,
    inactive_order: Arrangement,
    /// The reading its `ModId`s come from: after a rescan, `App::library` is a newer one.
    /// `None` for a made-up list or one that could not be read.
    reading: Option<std::sync::Arc<crate::workspace::Loaded>>,
    detected: rules::Detected,
    pub status: [Sentence; 2],
    /// Empty for none.
    pub problem: Sentence,
    /// A sentence each; the first is the news.
    pub sort_note: Vec<Sentence>,
}

fn side_index(side: Side) -> usize {
    match side {
        Side::Active => 0,
        Side::Inactive => 1,
    }
}

#[derive(Serialize)]
pub struct OrderChoice {
    key: &'static str,
    text: Sentence,
    chosen: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InactiveOrders {
    chosen: Sentence,
    /// The key after the chosen one, round again after the last.
    next: &'static str,
    next_text: Sentence,
    all: Vec<OrderChoice>,
    reversed: bool,
    /// Which way it runs now, and the icon that shows it.
    direction: Sentence,
    direction_icon: &'static str,
}

fn direction_text(how: Arrangement) -> Sentence {
    match (how.by, how.reversed) {
        (Arrange::Category | Arrange::Name, false) => say!("A to Z"),
        (Arrange::Category | Arrange::Name, true) => say!("Z to A"),
        (_, false) => say!("Newest first"),
        (_, true) => say!("Oldest first"),
    }
}

fn order_text(by: Arrange) -> Sentence {
    match by {
        Arrange::Category => say!("Category"),
        Arrange::Name => say!("Name"),
        Arrange::Updated => say!("Last updated"),
        Arrange::Downloaded => say!("Downloaded"),
        Arrange::Supports => say!("Game version"),
        Arrange::Size => say!("Folder size"),
    }
}

/// A column as the View card and the headings' menu offer it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnChoice {
    column: &'static str,
    text: Sentence,
    /// Before the text in a menu: a check while the column is shown.
    mark: &'static str,
    shown: bool,
    /// Load times come only from the Loading Progress mod; without them the column is not offered.
    off: bool,
}

pub fn column_choices(kept: &str, load_times: bool) -> Vec<ColumnChoice> {
    let chosen = rimstack_core::mods::columns::chosen(kept);
    let choice = |column: &'static str, text: Sentence| {
        let off = column == "load" && !load_times;
        let shown = chosen.contains(&column) && !off;
        let mark = if shown { "✓  " } else { "     " };
        ColumnChoice {
            column,
            text,
            mark,
            shown,
            off,
        }
    };
    let load = match load_times {
        true => say!("Load"),
        false => say!("Load, from the Loading Progress mod"),
    };
    vec![
        choice("author", say!("Author")),
        choice("version", say!("Version")),
        choice("game", say!("Game")),
        choice("load", load),
    ]
}

/// Empty while load times are known.
pub fn column_note(load_times: bool) -> Sentence {
    match load_times {
        true => Sentence::default(),
        false => say!(
            "Load times come from the Loading Progress mod: turn on its export of load times, then start the game once."
        ),
    }
}

fn arranged(app: &App, library: &Library) -> rimstack_core::mods::library::Arranged {
    let mut now = library.arrange(app.session().current());
    let measured = app.startup();
    for m in now.active.iter_mut() {
        let took = measured.of(&rimstack_core::rimworld::id::split_steam(&m.id).0);
        m.load_micros = took.map(|ms| (ms * 1000.0) as u32);
    }
    let notes = app.notes().reading();
    for m in now.active.iter_mut().chain(now.inactive.iter_mut()) {
        let key = m.installed.and_then(|i| library.get(i)?.key());
        if let Some(note) = key.and_then(|k| notes.get(&k)) {
            m.tags = note.tags.clone();
            m.sub = note.group().to_string();
            m.colour = note.colour.clone();
            m.pinned = note.pinned;
            // Filed by hand: that stands over what RimStack worked out.
            if let Some(chosen) = Category::from_kept(&note.category) {
                m.category = chosen;
            }
        }
    }
    let steam = app.steam_record();
    // On both lists: a mod deactivated later keeps it.
    let all = now.active.iter_mut().chain(now.inactive.iter_mut());
    for m in all.filter(|m| m.workshop) {
        let folder = m.installed.and_then(|i| library.get(i)).map(|i| &i.folder);
        m.updated = folder.and_then(|f| steam.0.get(f)).copied();
        m.bytes = folder.and_then(|f| steam.1.get(f)).copied();
    }
    now
}

impl LoadOrder {
    fn empty() -> Self {
        Self {
            active: GroupedList::new(Vec::new()),
            inactive: GroupedList::new(Vec::new()),
            troubles: Vec::new(),
            version_ignored: Vec::new(),
            checked_order: Vec::new(),
            outdated: [Vec::new(), Vec::new()],
            checked_inactive: Vec::new(),
            twins: [Vec::new(), Vec::new()],
            slowest: None,
            preview: None,
            batching: false,
            headings: true,
            inactive_order: Arrangement::default(),
            reading: None,
            detected: rules::Detected::default(),
            status: [Sentence::default(), Sentence::default()],
            problem: Sentence::default(),
            sort_note: Vec::new(),
        }
    }

    pub fn demo(active: usize, inactive: usize) -> Self {
        let mut order = Self::empty();
        order.active = GroupedList::new(demo::generate(active));
        order.inactive = GroupedList::new(demo::generate(inactive));
        order.twins = [
            rimstack_core::mods::list::twins(order.active.mods()),
            rimstack_core::mods::list::twins(order.inactive.mods()),
        ];
        order
    }

    pub fn read(app: &App, headings: bool, inactive_order: impl Into<Arrangement>) -> Self {
        let inactive_order = inactive_order.into();
        let loaded = app.library();
        let (library, took) = match &*loaded {
            Ok(read) => read,
            Err(problem) => {
                let mut order = Self::empty();
                order.problem = problem.sentence().clone();
                return order;
            }
        };
        let mut order = Self::empty();
        order.reading = Some(loaded.clone());
        order.headings = headings;
        order.inactive_order = inactive_order;
        let now = arranged(app, library);
        let (mods, _) = rules::sorted_by_category(app, &mut order.detected, library, now.active);
        order.active = GroupedList::flat(mods);
        order.active.set_grouped(headings);
        order.inactive = GroupedList::new(now.inactive);
        order.inactive.arrange(inactive_order);
        order.status = [
            match library.missing {
                0 => say!("{} active", library.active.len()),
                n => say!("{} active, {} not installed", library.active.len(), n),
            },
            say!(
                "{} inactive, read in {} ms",
                library.inactive.len(),
                took.as_millis()
            ),
        ];
        order.settle(app);
        order
    }

    pub fn is_installed(&self) -> bool {
        self.reading.is_some()
    }

    /// The order's own reading; a made-up list has none and is given the current one.
    fn reading(&self, app: &App) -> std::sync::Arc<crate::workspace::Loaded> {
        self.reading.clone().unwrap_or_else(|| app.library())
    }

    pub fn list(&self, side: Side) -> &GroupedList {
        match side {
            Side::Active => &self.active,
            Side::Inactive => &self.inactive,
        }
    }

    fn list_mut(&mut self, side: Side) -> &mut GroupedList {
        match side {
            Side::Active => &mut self.active,
            Side::Inactive => &mut self.inactive,
        }
    }

    pub fn troubles(&self) -> &[Vec<Trouble>] {
        &self.troubles
    }

    pub fn needed(&self) -> Vec<String> {
        rimstack_core::mods::describe::tally(&self.troubles).needed
    }

    /// The Workshop ids of the mods the active ones need and nothing installed brings.
    pub fn missing_workshop_ids(&self) -> Vec<String> {
        use rimstack_core::mods::describe::Fix;
        let mut ids: Vec<String> = (self.troubles.iter().flatten())
            .filter_map(|t| match &t.fix {
                Fix::Subscribe(id) => Some(id.clone()),
                _ => None,
            })
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    pub fn previewing(&self) -> bool {
        self.preview.is_some()
    }

    pub fn headings(&self) -> bool {
        self.headings
    }

    pub fn errors_and_warnings(&self) -> (usize, usize) {
        let with = |error: bool| {
            let has = |own: &&Vec<Trouble>| own.iter().any(|t| t.error == error);
            self.troubles.iter().filter(has).count()
        };
        (with(true), with(false))
    }

    /// `None` when what was read still holds.
    pub fn files_to_read(&self, app: &App) -> Option<Vec<ModId>> {
        if !self.is_installed() {
            return None;
        }
        let mut members: Vec<ModId> = self.checked_order.iter().flatten().copied().collect();
        members.sort_unstable();
        members.dedup();
        let known = app.conflicts().reading();
        let read = known.as_ref().is_some_and(|c| c.members == members);
        (!read).then_some(members)
    }

    pub fn settle(&mut self, app: &App) {
        self.twins = [
            rimstack_core::mods::list::twins(self.active.mods()),
            rimstack_core::mods::list::twins(self.inactive.mods()),
        ];
        self.slowest = self
            .active
            .mods()
            .iter()
            .filter_map(|m| m.load_micros)
            .max();
        if !self.is_installed() {
            return;
        }
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return;
        };
        let ids = |list: &GroupedList| -> Vec<Option<ModId>> {
            list.mods().iter().map(|m| m.installed).collect()
        };
        let (active, inactive) = (ids(&self.active), ids(&self.inactive));
        let changed = [
            active != self.checked_order,
            inactive != self.checked_inactive,
        ];
        if changed.contains(&true) {
            let trust = rules::Trust::read(app, library);
            if changed[0] {
                let mods = self.active.mods();
                (self.troubles, self.version_ignored) = rules::troubles(app, library, mods, &trust);
                self.outdated[0] = rules::outdated(library, mods, &trust);
                self.checked_order = active;
            }
            if changed[1] {
                self.outdated[1] = rules::outdated(library, self.inactive.mods(), &trust);
                self.checked_inactive = inactive;
            }
        }
        if self.preview.is_none() && !self.batching {
            let ids = self.active.mods().iter().map(|m| m.id.clone()).collect();
            app.session().record(ids);
        }
        for side in [Side::Active, Side::Inactive] {
            let flags = self.flags(app, library, side);
            self.list_mut(side).set_flags(flags);
        }
    }

    pub fn recheck(&mut self, app: &App) {
        self.checked_order = Vec::new();
        self.checked_inactive = Vec::new();
        self.settle(app);
    }

    pub fn sync(&mut self, app: &App) {
        let loaded = app.library();
        let Ok((library, _)) = &*loaded else {
            return;
        };
        if !self.is_installed() {
            return;
        }
        self.preview = None;
        self.sort_note.clear();
        let now = arranged(app, library);
        let (mods, _) = rules::sorted_by_category(app, &mut self.detected, library, now.active);
        self.active.set_grouped(self.headings);
        self.active.set_mods(mods);
        self.inactive.set_mods(now.inactive);
        self.inactive.arrange(self.inactive_order);
        self.reading = Some(loaded.clone());
        self.settle(app);
    }

    pub fn refile(&mut self, app: &App) {
        let loaded = self.reading(app);
        let (Ok((library, _)), None) = (&*loaded, &self.preview) else {
            return;
        };
        if !self.is_installed() {
            return;
        }
        let mods = self.active.mods().to_vec();
        let (mods, _) = rules::sorted_by_category(app, &mut self.detected, library, mods);
        self.active.set_mods(mods);
        self.settle(app);
    }

    pub fn set_headings(&mut self, app: &App, headings: bool) {
        self.headings = headings;
        if self.active.is_grouped() != headings {
            self.active.set_grouped(headings);
            self.settle(app);
        }
    }

    pub fn set_inactive_order(&mut self, app: &App, how: Arrangement) {
        self.inactive_order = how;
        self.inactive.arrange(how);
        self.settle(app);
    }

    pub fn inactive_orders(&self) -> InactiveOrders {
        let how = self.inactive_order;
        let by = how.by;
        let all = Arrange::ALL.map(|each| OrderChoice {
            key: each.key(),
            text: order_text(each),
            chosen: each == by,
        });
        InactiveOrders {
            chosen: order_text(by),
            next: by.next().key(),
            next_text: order_text(by.next()),
            all: all.into(),
            reversed: how.reversed,
            direction: direction_text(how),
            direction_icon: match how.reversed {
                false => "arrow-down",
                true => "arrow-up",
            },
        }
    }

    pub fn set_filter(&mut self, text: &str) {
        self.active.set_filter_on(Side::Active, text);
        self.inactive.set_filter_on(Side::Inactive, text);
    }

    /// "name", "author", "version", "game" or "load"; anything else is load order.
    pub fn show_by(&mut self, column: &str, descending: bool) {
        let key = match column {
            "name" => Some(SortKey::Name),
            "author" => Some(SortKey::Author),
            "version" => Some(SortKey::Version),
            "game" => Some(SortKey::Game),
            "load" => Some(SortKey::Load),
            _ => None,
        };
        self.active.show_by(key.map(|key| (key, descending)));
    }

    fn flags(&self, app: &App, library: &Library, side: Side) -> Vec<u16> {
        let in_conflict = self.in_conflict(app);
        let in_def_collision = if side == Side::Active {
            self.in_def_collision(app)
        } else {
            HashSet::new()
        };
        let behind = app.behind().reading();
        let mut copies: HashMap<String, usize> = HashMap::new();
        for key in library.installed.iter().filter_map(|m| m.key()) {
            *copies.entry(key).or_default() += 1;
        }
        let week_ago = super::now_secs().saturating_sub(7 * 24 * 3600);
        let newest_save = app.newest_save();
        let in_save = newest_save
            .as_ref()
            .as_ref()
            .filter(|_| side == Side::Active);
        let steam_record = app.steam_record();
        let seen = app.seen().reading();
        let session = app.session();
        let changes = session.changes();
        let changed: HashSet<&str> = match side {
            Side::Active => changes
                .moved
                .iter()
                .chain(&changes.activated)
                .copied()
                .collect(),
            Side::Inactive => HashSet::new(),
        };
        // Compute which mods are required by the active list
        let mut required_ids: HashSet<String> = HashSet::new();
        for active_mod in self.active.mods() {
            if let Some(id) = active_mod.installed
                && let Some(installed) = library.get(id)
                && let Ok(about) = &installed.about
            {
                for dep in about.dependencies.for_version(&library.game_version) {
                    let dep_key = rimstack_core::rimworld::id::fold(
                        &rimstack_core::rimworld::id::split_steam(dep).0,
                    );
                    required_ids.insert(dep_key);
                }
            }
        }
        let troubles: &[Vec<Trouble>] = match side {
            Side::Active => &self.troubles,
            Side::Inactive => &[],
        };
        let ignored = rules::ignored(app);
        let mods = self.list(side).mods().iter().enumerate();
        mods.map(|(at, m)| {
            let own = troubles.get(at).map(Vec::as_slice).unwrap_or_default();
            let installed = m.installed.and_then(|i| library.get(i));
            let folder = installed.map(|i| &i.folder);
            let key = rimstack_core::rimworld::id::split_steam(&m.id).0;
            let marks = [
                (own.iter().any(|t| t.error), flag::ERROR),
                (own.iter().any(|t| !t.error), flag::WARNING),
                (
                    rimstack_core::mods::describe::incomplete(installed, &key, &ignored),
                    flag::INCOMPLETE,
                ),
                (
                    m.installed.is_some_and(|i| in_conflict.contains(&i)),
                    flag::CONFLICT,
                ),
                (
                    m.installed.is_some_and(|i| in_def_collision.contains(&i)),
                    flag::DEFCLASH,
                ),
                (
                    folder.is_some_and(|f| behind.as_ref().is_some_and(|b| b.contains(f))),
                    flag::UPDATED,
                ),
                (
                    in_save.is_some_and(|ids| !ids.contains(&key)),
                    flag::NOT_IN_SAVE,
                ),
                (
                    folder.is_some_and(|f| steam_record.0.get(f).is_some_and(|at| *at >= week_ago)),
                    flag::RECENT,
                ),
                (copies.get(&key).is_some_and(|n| *n > 1), flag::DUPLICATE),
                (
                    installed.is_some_and(|i| seen.is_new(&i.path.to_string_lossy())),
                    flag::NEW,
                ),
                (changed.contains(m.id.as_str()), flag::CHANGED),
                (
                    m.category == rimstack_core::mods::list::Category::Libraries
                        && !required_ids.contains(&key),
                    flag::UNUSED,
                ),
            ];
            marks
                .iter()
                .filter(|(on, _)| *on)
                .fold(0, |flags, (_, mark)| flags | mark)
        })
        .collect()
    }

    pub fn in_conflict(&self, app: &App) -> HashSet<ModId> {
        let known = app.conflicts().reading();
        let Some(known) = known.as_ref() else {
            return HashSet::new();
        };
        (0..known.members.len())
            .filter(|&at| !known.overlaps.with(at).is_empty())
            .map(|at| known.members[at])
            .collect()
    }

    /// Sharing at least one def with another active mod, whatever the load order.
    pub fn in_def_collision(&self, app: &App) -> HashSet<ModId> {
        let known = app.conflicts().reading();
        let Some(known) = known.as_ref() else {
            return HashSet::new();
        };
        rimstack_core::mods::defs::in_collision(&known.def_collisions)
            .into_iter()
            .map(|at| known.members[at])
            .collect()
    }

    pub fn fresh(&self, app: &App, side: Side) -> Vec<ModId> {
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return Vec::new();
        };
        let seen = app.seen().reading();
        let mods = self.list(side).mods().iter();
        mods.filter_map(|m| m.installed)
            .filter(|&i| {
                library
                    .get(i)
                    .is_some_and(|m| seen.is_new(&m.path.to_string_lossy()))
            })
            .collect()
    }
}

impl App {
    /// Off the window's thread.
    pub fn read_active_files(&self, members: Vec<ModId>) {
        use rayon::prelude::*;
        use rimstack_core::mods::{conflicts, infer};
        let generation = self.conflicts_generation();
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return;
        };
        let active: HashSet<String> = members
            .iter()
            .filter_map(|&i| library.get(i)?.key())
            .collect();
        let read: Vec<(Vec<String>, infer::Scanned)> = members
            .par_iter()
            .map(|&i| {
                let Some(root) = library.get(i).map(|m| &m.path) else {
                    return Default::default();
                };
                let loaded = rimstack_core::rimworld::loadfolders::folders(
                    root,
                    &library.game_version,
                    &|m| active.contains(m),
                );
                (conflicts::provided(&loaded), infer::scan(&loaded))
            })
            .collect();
        let (provided, scanned): (Vec<_>, Vec<_>) = read.into_iter().unzip();
        let defines: Vec<_> = (scanned.iter()).map(|s| &s.defines).collect();
        let found = super::Conflicts {
            overlaps: conflicts::overlaps(&provided),
            def_collisions: rimstack_core::mods::defs::collisions(&defines),
            scanned: members.iter().copied().zip(scanned).collect(),
            members,
        };
        self.keep_files_read(generation, found);
    }

    /// Dropped when `textures_changed` ran since `generation`.
    fn keep_files_read(&self, generation: u64, found: super::Conflicts) {
        let mut slot = self.conflicts().writing();
        if self.conflicts_generation() != generation {
            return;
        }
        *slot = Some(found);
        drop(slot);
        self.publish(crate::changes::Changes::FILES_READ);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundMod {
    pub name: String,
    pub mod_id: i32,
    pub active: bool,
}

impl LoadOrder {
    pub fn found(&self, app: &App, text: &str, most: usize) -> Vec<FoundMod> {
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return Vec::new();
        };
        let active = self.list(Side::Active);
        library
            .find(text, most)
            .into_iter()
            .filter_map(|id| {
                Some(FoundMod {
                    name: library.get(id)?.display_name().to_string(),
                    mod_id: id.to_qml(),
                    active: active.position_of(id).is_some(),
                })
            })
            .collect()
    }

    pub fn same_mods_in(&self, newer: &LoadOrder) -> HashMap<ModId, ModId> {
        let (Some(old), Some(new)) = (self.held(), newer.held()) else {
            return HashMap::new();
        };
        let by_folder: HashMap<&std::path::Path, ModId> =
            new.ids().map(|(id, m)| (m.path.as_path(), id)).collect();
        let found = |(id, m): (ModId, &rimstack_core::mods::library::Installed)| {
            Some((id, *by_folder.get(m.path.as_path())?))
        };
        old.ids().filter_map(found).collect()
    }

    fn held(&self) -> Option<&Library> {
        self.reading.as_deref()?.as_ref().ok().map(|(l, _)| l)
    }

    /// `selected` of the earlier reading, in this one: what is gone is left out.
    pub fn carried(&self, same: &HashMap<ModId, ModId>, selected: &[ModId]) -> Carried {
        let kept: Vec<ModId> = selected
            .iter()
            .filter_map(|id| same.get(id).copied())
            .collect();
        let Some((&first, others)) = kept.split_first() else {
            return Carried {
                first: -1,
                active: false,
                also: BTreeMap::new(),
            };
        };
        Carried {
            first: first.to_qml(),
            active: self.active.position_of(first).is_some(),
            also: others
                .iter()
                .map(|id| (id.to_qml().to_string(), true))
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Carried {
    /// -1 for none.
    pub first: i32,
    pub active: bool,
    /// The others, as the window keeps them: id to `true`.
    pub also: BTreeMap<String, bool>,
}

#[cfg(test)]
mod tests;
