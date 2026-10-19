use crate::base::versions::numbers;
use crate::mods::library::ModId;
use std::collections::HashSet;

/// Earlier variants load earlier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Loaders,
    Game,
    Libraries,
    Content,
    Patches,
    Textures,
    Interface,
    Tweaks,
    Last,
}

impl Category {
    pub const ALL: [Category; 9] = [
        Category::Loaders,
        Category::Game,
        Category::Libraries,
        Category::Content,
        Category::Patches,
        Category::Textures,
        Category::Interface,
        Category::Tweaks,
        Category::Last,
    ];

    pub fn from_label(label: &str) -> Option<Self> {
        // Notes written before the tweaks joined still say "Performance".
        let label = if label == "Performance" {
            Category::Tweaks.label()
        } else {
            label
        };
        Self::ALL.into_iter().find(|c| c.label() == label)
    }

    /// Kept in the notes by key: never reworded, as its label may be.
    pub fn key(self) -> &'static str {
        match self {
            Category::Loaders => "loaders",
            Category::Game => "game",
            Category::Libraries => "libraries",
            Category::Content => "content",
            Category::Patches => "patches",
            Category::Textures => "textures",
            Category::Interface => "interface",
            Category::Tweaks => "tweaks",
            Category::Last => "last",
        }
    }

    /// Notes kept before there were keys hold a label instead.
    pub fn from_kept(kept: &str) -> Option<Self> {
        let by_key = Self::ALL.into_iter().find(|c| c.key() == kept);
        by_key.or_else(|| Self::from_label(kept))
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Loaders => "Loaders",
            Category::Game => "Game and DLC",
            Category::Libraries => "Libraries",
            Category::Content => "Content",
            Category::Patches => "Patches",
            Category::Textures => "Textures",
            Category::Interface => "Interface",
            Category::Tweaks => "Performance & tweaks",
            Category::Last => "Loads last",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mod {
    /// Empty for demo data and for a folder that cannot be read as a mod.
    pub id: String,
    pub name: String,
    pub author: String,
    /// The mod's own version; empty when its About.xml names none.
    pub version: String,
    /// The newest game version it names as supported, as written; empty for none.
    pub game: String,
    pub category: Category,
    /// `None` for a mod that is not installed and for demo data.
    pub installed: Option<ModId>,
    pub tags: Vec<String>,
    pub local: bool,
    pub workshop: bool,
    pub git: bool,
    pub code: bool,
    /// Lower case `type.method`.
    pub patched: std::sync::Arc<[String]>,
    /// Its first tag; empty without tags.
    pub sub: String,
    /// Microseconds it took at the game's last start, where measured.
    pub load_micros: Option<u32>,
    /// Seconds since 1970 the Workshop last published it; `None` off the Workshop.
    pub updated: Option<u64>,
    /// Seconds since 1970 its folder was made, which is when it was subscribed to or copied in.
    pub downloaded: Option<u64>,
    /// The newest game version it names as supported, by numbers, at most the installed game's;
    /// empty for none.
    pub supports: Vec<u64>,
    /// The swatch its row is painted with; empty for none.
    pub colour: String,
    /// Sort leaves it where it is.
    pub pinned: bool,
    /// Folded package ids of official DLCs this mod lists as dependencies.
    pub needs_dlcs: Vec<String>,
    /// Bytes its folder takes on disk; `None` for a mod that is not installed.
    pub bytes: Option<u64>,
    /// Its folders hold only translations of keyed strings, nothing else.
    pub translation: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Author,
    Version,
    /// By the numbers of `Mod::game`, so 1.10 comes after 1.9.
    Game,
    Load,
}

/// How the inactive list is ordered; all but `Category` drop the headings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Arrange {
    #[default]
    Category,
    Name,
    /// Last published on the Workshop, newest first.
    Updated,
    /// Newest first.
    Downloaded,
    /// By the newest game version supported, newest first.
    Supports,
    /// Biggest folder first.
    Size,
}

impl Arrange {
    pub const ALL: [Arrange; 6] = [
        Self::Category,
        Self::Name,
        Self::Updated,
        Self::Downloaded,
        Self::Supports,
        Self::Size,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Category => "category",
            Self::Name => "name",
            Self::Updated => "updated",
            Self::Downloaded => "downloaded",
            Self::Supports => "supports",
            Self::Size => "size",
        }
    }

    /// By `key`; anything else is by category.
    pub fn from_name(name: &str) -> Self {
        let found = Self::ALL.into_iter().find(|by| by.key() == name);
        found.unwrap_or_default()
    }

    pub fn next(self) -> Self {
        let at = Self::ALL.iter().position(|&by| by == self).unwrap_or(0);
        Self::ALL[(at + 1) % Self::ALL.len()]
    }
}

/// An order and which way it runs: reversed, the oldest, the lowest version, the smallest or Z
/// comes first, and those without a time, version or size still go last.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Arrangement {
    pub by: Arrange,
    pub reversed: bool,
}

impl From<Arrange> for Arrangement {
    fn from(by: Arrange) -> Self {
        Self {
            by,
            reversed: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Header {
        category: Category,
        count: usize,
        collapsed: bool,
    },
    Sub {
        first: usize,
        count: usize,
        collapsed: bool,
    },
    Mod {
        index: usize,
    },
}

/// After `Inserted` or `Removed`, the header at `first - 1` also changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Reset,
    Inserted { first: usize, count: usize },
    Removed { first: usize, count: usize },
}

pub fn runs(mods: &[Mod]) -> usize {
    let changes = mods.windows(2).filter(|w| w[0].category != w[1].category);
    changes.count() + usize::from(!mods.is_empty())
}

pub struct GroupedList {
    mods: Vec<Mod>,
    /// Lower-cased "name author" per mod; filtering allocates nothing.
    haystacks: Vec<String>,
    collapsed: HashSet<Category>,
    collapsed_subs: HashSet<(Category, String)>,
    filter: Filter,
    /// Which list this is, as the filter last said; `None` lets `is:active` and `is:inactive` pass.
    side: Option<Side>,
    /// Empty until `set_flags`, and again after the order changes.
    flags: Vec<u16>,
    grouped: bool,
    shown_by: Option<(SortKey, bool)>,
    rows: Vec<Row>,
}

fn haystack(m: &Mod) -> String {
    format!("{} {}", m.name, m.author).to_lowercase()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Test {
    Word(String),
    Author(String),
    Tag(String),
    Colour(String),
    Local,
    Workshop,
    Git,
    Code,
    Pinned,
    Needs(String),
    Translation,
    Patches(String),
    /// Megabytes given as bytes.
    Bigger(u64),
    Smaller(u64),
    Flag(u16),
}

impl Test {
    fn passes(&self, m: &Mod, haystack: &str, flags: u16) -> bool {
        match self {
            Test::Word(w) => haystack.contains(w.as_str()),
            // A space, which would end the term, is typed as `_`.
            Test::Author(a) => m
                .author
                .to_lowercase()
                .replace(' ', "_")
                .contains(a.as_str()),
            Test::Tag(t) => m
                .tags
                .iter()
                .any(|own| own.to_lowercase().replace(' ', "_").starts_with(t.as_str())),
            Test::Colour(c) => {
                m.colour.to_lowercase().starts_with(c.as_str()) && !m.colour.is_empty()
            }
            Test::Local => m.local,
            Test::Workshop => m.workshop,
            Test::Git => m.git,
            Test::Code => m.code,
            Test::Pinned => m.pinned,
            Test::Patches(p) => m.patched.iter().any(|at| at.contains(p.as_str())),
            Test::Needs(dlc) => m.needs_dlcs.iter().any(|own| own.contains(dlc.as_str())),
            Test::Bigger(bytes) => m.bytes.is_some_and(|own| own > *bytes),
            Test::Smaller(bytes) => m.bytes.is_some_and(|own| own < *bytes),
            Test::Translation => m.translation,
            Test::Flag(f) => flags & f != 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Filter {
    /// Each with whether it must pass: `!` before a term asks that it fail.
    tests: Vec<(Test, bool)>,
    only: Option<Side>,
    both: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Active,
    Inactive,
}

impl Side {
    pub fn of(active: bool) -> Self {
        if active { Side::Active } else { Side::Inactive }
    }

    pub fn other(self) -> Self {
        match self {
            Side::Active => Side::Inactive,
            Side::Inactive => Side::Active,
        }
    }
}

pub mod flag {
    pub const ERROR: u16 = 1;
    pub const WARNING: u16 = 2;
    pub const CONFLICT: u16 = 4;
    pub const UPDATED: u16 = 8;
    pub const NOT_IN_SAVE: u16 = 16;
    pub const RECENT: u16 = 32;
    pub const DUPLICATE: u16 = 64;
    pub const NEW: u16 = 128;
    pub const CHANGED: u16 = 256;
    pub const UNUSED: u16 = 512;
    /// Missing a property RimSort looks for: no readable About.xml, or no named Workshop item.
    pub const INCOMPLETE: u16 = 1024;
    pub const DEFCLASH: u16 = 2048;
}

impl Filter {
    fn parse(text: &str) -> Self {
        let mut filter = Self::default();
        for term in text.to_lowercase().split_whitespace() {
            let (wanted, term) = match term.strip_prefix('!') {
                Some("") => continue,
                Some(rest) => (false, rest),
                None => (true, term),
            };
            let test = match term.split_once(':') {
                Some(("author", name)) if !name.is_empty() => Test::Author(name.to_string()),
                Some(("tag", label)) if !label.is_empty() => Test::Tag(label.to_string()),
                Some(("colour", name)) if !name.is_empty() => Test::Colour(name.to_string()),
                Some(("is", "local")) => Test::Local,
                Some(("is", "workshop")) => Test::Workshop,
                Some(("is", "git")) => Test::Git,
                Some(("is", "pinned")) => Test::Pinned,
                Some(("is", side @ ("active" | "inactive"))) => {
                    let side = Side::of(side == "active");
                    filter.ask(if wanted { side } else { side.other() });
                    continue;
                }
                Some(("has", "code")) => Test::Code,
                Some(("patches", method)) if !method.is_empty() => {
                    Test::Patches(method.to_string())
                }
                Some(("is", "error")) => Test::Flag(flag::ERROR),
                Some(("is", "warning")) => Test::Flag(flag::WARNING),
                Some(("is", "conflict")) => Test::Flag(flag::CONFLICT),
                Some(("is", "updated")) => Test::Flag(flag::UPDATED),
                Some(("needs", name)) if !name.is_empty() => Test::Needs(name.to_string()),
                Some(("is", "notinsave")) => Test::Flag(flag::NOT_IN_SAVE),
                Some(("is", "recent")) => Test::Flag(flag::RECENT),
                Some(("is", "duplicate")) => Test::Flag(flag::DUPLICATE),
                Some(("is", "incomplete")) => Test::Flag(flag::INCOMPLETE),
                Some(("is", "defclash")) => Test::Flag(flag::DEFCLASH),
                Some(("is", "new")) => Test::Flag(flag::NEW),
                Some(("is", "changed")) => Test::Flag(flag::CHANGED),
                Some(("is", "unused")) => Test::Flag(flag::UNUSED),
                Some(("size", bound)) => {
                    size_test(bound).unwrap_or_else(|| Test::Word(term.to_string()))
                }
                Some(("is", "translation")) => Test::Translation,
                _ => Test::Word(term.to_string()),
            };
            filter.tests.push((test, wanted));
        }
        filter
    }

    fn ask(&mut self, side: Side) {
        self.both |= self.only.is_some_and(|asked| asked != side);
        self.only = Some(side);
    }

    fn shuts(&self, side: Option<Side>) -> bool {
        self.both || side.zip(self.only).is_some_and(|(side, only)| side != only)
    }

    fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn reads_flags(&self) -> bool {
        self.tests
            .iter()
            .any(|(test, _)| matches!(test, Test::Flag(_)))
    }

    fn admits(&self, m: &Mod, haystack: &str, flags: u16) -> bool {
        self.tests
            .iter()
            .all(|(test, wanted)| test.passes(m, haystack, flags) == *wanted)
    }
}

/// `>N` or `<N` megabytes, as `size:` takes it.
fn size_test(bound: &str) -> Option<Test> {
    let (over, number) = match bound.split_at_checked(1)? {
        (">", number) => (true, number),
        ("<", number) => (false, number),
        _ => return None,
    };
    let megabytes: f64 = number.parse().ok().filter(|mb: &f64| *mb >= 0.0)?;
    let bytes = (megabytes * 1_048_576.0) as u64;
    Some(match over {
        true => Test::Bigger(bytes),
        false => Test::Smaller(bytes),
    })
}

impl GroupedList {
    /// `mods` must already be in load order.
    pub fn new(mods: Vec<Mod>) -> Self {
        let haystacks = mods.iter().map(haystack).collect();
        let mut list = Self {
            mods,
            haystacks,
            collapsed: HashSet::new(),
            collapsed_subs: HashSet::new(),
            filter: Filter::default(),
            side: None,
            flags: Vec::new(),
            grouped: true,
            shown_by: None,
            rows: Vec::new(),
        };
        list.rebuild();
        list
    }

    pub fn flat(mods: Vec<Mod>) -> Self {
        let mut list = Self::new(mods);
        list.grouped = false;
        list.rebuild();
        list
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn mods(&self) -> &[Mod] {
        &self.mods
    }

    /// Changing the order forgets it.
    pub fn set_flags(&mut self, flags: Vec<u16>) {
        self.flags = flags;
        if self.filter.reads_flags() {
            self.rebuild();
        }
    }

    /// Unknown times and versions go last; ties are by name.
    pub fn arrange(&mut self, how: impl Into<Arrangement>) {
        use std::cmp::Reverse;
        let Arrangement { by, reversed } = how.into();
        let mut mods = std::mem::take(&mut self.mods);
        let name = |m: &Mod| m.name.to_lowercase();
        let newest = |time: Option<u64>| (time.is_none(), Reverse(time));
        match by {
            Arrange::Category => mods.sort_by_cached_key(|m| (m.category, name(m))),
            Arrange::Name => mods.sort_by_cached_key(name),
            Arrange::Updated => mods.sort_by_cached_key(|m| (newest(m.updated), name(m))),
            Arrange::Downloaded => mods.sort_by_cached_key(|m| (newest(m.downloaded), name(m))),
            Arrange::Supports => mods.sort_by_cached_key(|m| {
                (m.supports.is_empty(), Reverse(m.supports.clone()), name(m))
            }),
            Arrange::Size => mods.sort_by_cached_key(|m| (newest(m.bytes), name(m))),
        }
        if reversed {
            let unknown = |m: &Mod| match by {
                Arrange::Updated => m.updated.is_none(),
                Arrange::Downloaded => m.downloaded.is_none(),
                Arrange::Supports => m.supports.is_empty(),
                Arrange::Size => m.bytes.is_none(),
                Arrange::Category | Arrange::Name => false,
            };
            let known = mods.iter().position(unknown).unwrap_or(mods.len());
            mods[..known].reverse();
        }
        self.grouped = by == Arrange::Category;
        self.set_mods(mods);
    }

    pub fn set_tags(&mut self, installed: ModId, tags: &[String]) -> bool {
        let Some(m) = self
            .mods
            .iter_mut()
            .find(|m| m.installed == Some(installed))
        else {
            return false;
        };
        m.tags = tags.to_vec();
        m.sub = tags.first().cloned().unwrap_or_default();
        self.rebuild();
        true
    }

    pub fn toggle_sub(&mut self, first: usize) {
        let Some(m) = self.mods.get(first).filter(|m| !m.sub.is_empty()) else {
            return;
        };
        let key = (m.category, m.sub.clone());
        if !self.collapsed_subs.remove(&key) {
            self.collapsed_subs.insert(key);
        }
        self.rebuild();
    }

    pub fn in_sub_of(&self, first: usize) -> Vec<usize> {
        let Some(of) = self.mods.get(first).filter(|m| !m.sub.is_empty()) else {
            return Vec::new();
        };
        let same = |m: &Mod| m.category == of.category && m.sub == of.sub;
        let each = self.mods.iter().enumerate();
        each.filter(|(_, m)| same(m)).map(|(at, _)| at).collect()
    }

    pub fn subs(&self, category: Category) -> Vec<String> {
        let in_it = self.mods.iter().filter(|m| m.category == category);
        let names: std::collections::BTreeSet<&str> = in_it.map(|m| m.sub.as_str()).collect();
        names
            .into_iter()
            .filter(|n| !n.is_empty())
            .map(String::from)
            .collect()
    }

    /// Positions in `Row::Mod` stay load-order positions.
    pub fn show_by(&mut self, order: Option<(SortKey, bool)>) {
        self.shown_by = order;
        self.rebuild();
    }

    pub fn is_grouped(&self) -> bool {
        self.grouped
    }

    pub fn set_grouped(&mut self, grouped: bool) {
        self.grouped = grouped;
        self.rebuild();
    }

    pub fn set_mods(&mut self, mods: Vec<Mod>) {
        self.haystacks = mods.iter().map(haystack).collect();
        self.mods = mods;
        self.flags.clear();
        self.rebuild();
    }

    pub fn matching(&self) -> usize {
        (0..self.mods.len()).filter(|&i| self.matches(i)).count()
    }

    /// Where a mod dropped below the last row goes: after the last mod the filter shows,
    /// so a drop under a filtered list lands beside what it shows, not at the very end.
    pub fn end_place(&self) -> usize {
        let last_shown = (0..self.mods.len()).rev().find(|&i| self.matches(i));
        last_shown.map_or(self.mods.len(), |i| i + 1)
    }

    pub fn set_filter(&mut self, text: &str) {
        self.filter = Filter::parse(text);
        self.side = None;
        self.rebuild();
    }

    pub fn set_filter_on(&mut self, side: Side, text: &str) {
        self.filter = Filter::parse(text);
        self.side = Some(side);
        self.rebuild();
    }

    /// Qt must be told which rows go or come before the change.
    pub fn toggling(&self, category: Category) -> Change {
        if self.mods.iter().any(|m| !m.sub.is_empty()) {
            return Change::Reset;
        }
        let mut headers = self.rows.iter().enumerate().filter_map(|(row, r)| match r {
            Row::Header {
                category: c, count, ..
            } if *c == category => Some((row, *count)),
            _ => None,
        });
        match (headers.next(), headers.next()) {
            (Some((row, count)), None) if count > 0 => {
                let first = row + 1;
                match self.collapsed.contains(&category) {
                    true => Change::Inserted { first, count },
                    false => Change::Removed { first, count },
                }
            }
            _ => Change::Reset,
        }
    }

    pub fn toggle(&mut self, category: Category) -> Change {
        let change = self.toggling(category);
        if !self.collapsed.remove(&category) {
            self.collapsed.insert(category);
        }
        self.rebuild();
        change
    }

    pub fn move_mod(&mut self, from: usize, to: usize) {
        if from >= self.mods.len() || to >= self.mods.len() || from == to {
            return;
        }
        let m = self.mods.remove(from);
        let h = self.haystacks.remove(from);
        self.mods.insert(to, m);
        self.haystacks.insert(to, h);
        self.flags.clear();
        self.rebuild();
    }

    pub fn insert_in_category(&mut self, m: Mod) -> usize {
        let at = if self.grouped {
            self.mods
                .iter()
                .rposition(|x| x.category <= m.category)
                .map_or(0, |p| p + 1)
        } else {
            self.mods.len()
        };
        self.haystacks.insert(at, haystack(&m));
        self.mods.insert(at, m);
        self.flags.clear();
        self.rebuild();
        at
    }

    pub fn remove(&mut self, index: usize) -> Option<Mod> {
        if index >= self.mods.len() {
            return None;
        }
        self.haystacks.remove(index);
        let m = self.mods.remove(index);
        self.flags.clear();
        self.rebuild();
        Some(m)
    }

    fn matches(&self, index: usize) -> bool {
        let flags = self.flags.get(index).copied().unwrap_or(0);
        if self.filter.shuts(self.side) {
            return false;
        }
        self.filter.is_empty()
            || self
                .filter
                .admits(&self.mods[index], &self.haystacks[index], flags)
    }

    fn rebuild(&mut self) {
        if let Some((key, descending)) = self.shown_by {
            let mut shown: Vec<usize> = (0..self.mods.len()).filter(|&i| self.matches(i)).collect();
            let text = |i: usize| match key {
                SortKey::Author => self.mods[i].author.to_lowercase(),
                SortKey::Version => self.mods[i].version.to_lowercase(),
                _ => self.mods[i].name.to_lowercase(),
            };
            match key {
                SortKey::Game => {
                    shown.sort_by_cached_key(|&i| {
                        let game = &self.mods[i].game;
                        let parts = numbers(game);
                        (game.is_empty(), parts)
                    });
                }
                SortKey::Load => {
                    shown.sort_by_key(|&i| self.mods[i].load_micros);
                }
                _ => shown.sort_by_cached_key(|&i| text(i)),
            }
            if descending {
                shown.reverse();
            }
            self.rows = shown.into_iter().map(|index| Row::Mod { index }).collect();
            return;
        }
        if !self.grouped {
            self.rows = (0..self.mods.len())
                .filter(|&i| self.matches(i))
                .map(|index| Row::Mod { index })
                .collect();
            return;
        }
        let mut rows = Vec::with_capacity(self.mods.len() + Category::ALL.len());
        let mut start = 0;
        while start < self.mods.len() {
            let category = self.mods[start].category;
            let mut end = start;
            while end < self.mods.len() && self.mods[end].category == category {
                end += 1;
            }
            let visible: Vec<usize> = (start..end).filter(|&i| self.matches(i)).collect();
            if !visible.is_empty() {
                let collapsed = self.collapsed.contains(&category);
                rows.push(Row::Header {
                    category,
                    count: visible.len(),
                    collapsed,
                });
                if !collapsed {
                    self.push_with_subs(&mut rows, &visible);
                }
            }
            start = end;
        }
        self.rows = rows;
    }

    fn push_with_subs(&self, rows: &mut Vec<Row>, visible: &[usize]) {
        let mut at = 0;
        while at < visible.len() {
            let sub = &self.mods[visible[at]].sub;
            let same = visible[at..]
                .iter()
                .take_while(|&&i| self.mods[i].sub == *sub);
            let count = same.count();
            let key = (self.mods[visible[at]].category, sub.clone());
            let collapsed = !sub.is_empty() && self.collapsed_subs.contains(&key);
            if !sub.is_empty() {
                rows.push(Row::Sub {
                    first: visible[at],
                    count,
                    collapsed,
                });
            }
            if !collapsed {
                let stretch = &visible[at..at + count];
                rows.extend(stretch.iter().map(|&index| Row::Mod { index }));
            }
            at += count;
        }
    }
}

pub fn split_groups<'a>(mods: impl IntoIterator<Item = &'a Mod>) -> Vec<String> {
    let mut closed: HashSet<(Category, &str)> = HashSet::new();
    let mut split: Vec<String> = Vec::new();
    let mut last: Option<(Category, &str)> = None;
    for m in mods {
        let now = (!m.sub.is_empty()).then_some((m.category, m.sub.as_str()));
        if now == last {
            continue;
        }
        if let Some(left) = last {
            closed.insert(left);
        }
        if let Some(entered) = now
            && closed.contains(&entered)
            && !split.iter().any(|name| name == entered.1)
        {
            split.push(entered.1.to_string());
        }
        last = now;
    }
    split
}

pub fn twins(mods: &[Mod]) -> Vec<bool> {
    let mut named: std::collections::HashMap<String, usize> = Default::default();
    for m in mods {
        *named.entry(m.name.to_ascii_lowercase()).or_default() += 1;
    }
    let shared = |m: &Mod| named[&m.name.to_ascii_lowercase()] > 1;
    mods.iter().map(shared).collect()
}

pub fn edge_of_run(mods: &[Mod], at: usize, top: bool) -> usize {
    let same = |other: &Mod| other.category == mods[at].category;
    match top {
        true => at - mods[..at].iter().rev().take_while(|m| same(m)).count(),
        false => at + mods[at + 1..].iter().take_while(|m| same(m)).count(),
    }
}

mod navigate;
#[cfg(test)]
mod tests;
