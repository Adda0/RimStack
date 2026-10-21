use super::{LoadOrder, Side, rules};
use crate::App;
use rimstack_core::base::locks::Shared;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::describe::{self, Trouble, TroubleKind};
use rimstack_core::mods::details::{self, Clash, Named, OwnRule, Slow};
use rimstack_core::mods::library::{Library, ModId};
use rimstack_core::mods::list::{Category, Mod};
use rimstack_core::mods::tree::folder_tree;
use rimstack_core::rimworld::loadfolders;
use rimstack_core::rimworld::paths::Source;
use rimstack_core::say;
use serde::Serialize;

const MOST_PATCHES: usize = 200;

#[derive(Serialize)]
pub struct FixSaid {
    action: &'static str,
    target: String,
    label: &'static str,
    page: Option<String>,
}

impl FixSaid {
    fn of(fix: &rimstack_core::mods::describe::Fix) -> Self {
        Self {
            action: fix.action(),
            target: fix.target(),
            label: fix.label(),
            page: fix.page(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemSaid {
    /// -1 for a mod that is not installed.
    mod_id: i64,
    #[serde(skip)]
    trust: bool,
    #[serde(skip)]
    kind: TroubleKind,
    name: String,
    error: bool,
    group: &'static str,
    words: Sentence,
    short: Sentence,
    fix: FixSaid,
    /// With `ignore`, what `App::ignore_problem` is given; empty when it cannot be ignored.
    package_id: String,
    ignore: String,
}

impl ProblemSaid {
    fn of(m: &Mod, trouble: &Trouble) -> Self {
        Self {
            mod_id: m.installed.map_or(-1, |i| i64::from(i.to_qml())),
            trust: trouble.fix == rimstack_core::mods::describe::Fix::Trust,
            kind: trouble.kind,
            name: m.name.clone(),
            error: trouble.error,
            group: trouble.kind.heading(),
            words: trouble.words.clone(),
            short: trouble.short.clone(),
            fix: FixSaid::of(&trouble.fix),
            package_id: m.id.clone(),
            ignore: trouble.key.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct Detected {
    name: String,
    why: Sentence,
    code: bool,
    /// The rule as a `communityRules.json` entry, to send upstream; shown for `code`.
    community: String,
}

#[derive(Serialize)]
pub struct Branch {
    folder: String,
    files: usize,
}

#[derive(Serialize, Default)]
pub struct Files {
    tree: Vec<Branch>,
    assemblies: String,
}

#[derive(Serialize)]
pub struct Patched {
    method: Sentence,
    note: Sentence,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    name: String,
    source: &'static str,
    workshop: bool,
    official: bool,
    contains: Sentence,
    path: String,
    repository: String,
    folder: String,
    preview: Option<String>,
    author: String,
    package_id: String,
    version: String,
    game_versions: String,
    description: String,
    description_rich: String,
    url: String,
    load_time: String,
    unreadable: Option<Sentence>,
    tags: String,
    tag_list: Vec<String>,
    note: String,
    category_chosen: String,
    pinned: bool,
    version_ok: bool,
    problems_ignored: usize,
    made_for: Option<String>,
    replaced_by: Option<String>,
    replaced_by_id: Option<String>,
    replaced_by_versions: Option<String>,
    /// Every installed copy of its package id, this one included; empty for only one.
    copies: Vec<Copy>,
    updated: Option<u64>,
    megabytes: Option<f64>,
    requires: Vec<Named>,
    needed_by: Vec<Named>,
    /// This mod and the active ones needing it, directly or through another;
    /// empty when no active mod needs it.
    off_with: Vec<ModId>,
    translates: Option<Sentence>,
    /// Removing it mid-save, judged on what it adds; only for a mod in the newest save.
    removal: Option<Sentence>,
    removal_harms: bool,
    load_folders: Vec<Sentence>,
    code: Sentence,
    sub: String,
    patches: Vec<Patched>,
    category: Option<&'static str>,
    problems: Vec<ProblemSaid>,
    your_rules: Vec<OwnRule>,
    detected: Vec<Detected>,
    loses_to: Vec<Clash>,
    wins_over: Vec<Clash>,
    conflict_marks: std::collections::BTreeMap<ModId, rimstack_core::mods::details::Mark>,
    /// Active mods loading later whose defs replace this mod's, and the other way round.
    defs_replaced_by: Vec<rimstack_core::mods::defs::DefClash>,
    defs_replaces: Vec<rimstack_core::mods::defs::DefClash>,
    /// The mods not installed that its patches or defs are for; a note, not a problem.
    xml_leaves_out: Option<Sentence>,
    loads_after: Vec<Named>,
    loads_before: Vec<Named>,
    required_above: usize,
    placement: Option<Sentence>,
    offer_trust: bool,
    /// The ways it may be deleted, the whole mod first; empty for the game's own.
    bin_choices: Vec<crate::workspace::upkeep::BinChoice>,
    group_choices: Vec<String>,
    /// How many mods sit under its group, this one too.
    group_size: usize,
    workshop_page: Option<String>,
    steam_page: Option<String>,
    /// Its `Mod_*` files of the game's Config.
    settings_files: Vec<String>,
}

/// One installed folder naming this mod's package id.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Copy {
    mod_id: ModId,
    source: &'static str,
    folder: String,
    path: String,
    /// Its `modVersion`; empty when the About.xml names none.
    version: String,
    megabytes: Option<f64>,
    /// This is the copy the game reads for the load order.
    used: bool,
    /// Why the game reads this one; `None` while another is read.
    note: Option<Sentence>,
    /// The order entry naming this copy instead, offered as "Use this copy"; `None` while
    /// the game cannot be told.
    use_as: Option<String>,
}

/// An active row whose mod is not installed, in place of its details for its menu.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotInstalled {
    name: String,
    take_off: Sentence,
    package_id: String,
    fixes: Vec<FixSaid>,
    resubscribe: Vec<String>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Textures {
    count: usize,
    dds_made: usize,
}

#[derive(Serialize)]
pub struct ProblemGroup {
    name: &'static str,
    error: bool,
    trusted: usize,
    shared: bool,
    lines: Vec<ProblemSaid>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    game_version: String,
    categories: Vec<&'static str>,
    troubles: Vec<ProblemSaid>,
    to_get: Vec<ProblemSaid>,
    /// The Workshop ids among them, for SteamCMD.
    to_download: Vec<String>,
    trouble_groups: Vec<ProblemGroup>,
    needed: Vec<String>,
    version_only: usize,
    other_warnings: usize,
    slowest: Vec<Slow>,
    load_times: bool,
    installed: usize,
    missing: usize,
    unreadable: usize,
    read_ms: u128,
    works_on: usize,
    trusted: Vec<Named>,
    community_mods: Option<usize>,
    community_time: Option<i64>,
}

impl LoadOrder {
    pub fn not_installed(&self, app: &App, side: Side, index: usize) -> Option<NotInstalled> {
        let m = self.list(side).mods().get(index)?;
        if m.installed.is_some() {
            return None;
        }
        let (name, fix) = describe::missing(&app.workshop_names(), &m.id);
        let resubscribe = match &fix {
            describe::Fix::Subscribe(workshop_id) => vec![workshop_id.clone()],
            _ => Vec::new(),
        };
        Some(NotInstalled {
            name,
            take_off: say!("Take off the list"),
            package_id: m.id.clone(),
            fixes: vec![FixSaid::of(&fix)],
            resubscribe,
        })
    }

    pub fn details(&mut self, app: &App, id: ModId) -> Details {
        let loaded = self.reading(app);
        let Ok((library, _)) = &*loaded else {
            return Details::default();
        };
        let Some(m) = library.get(id) else {
            return Details::default();
        };
        let workshop = m.source == Source::Workshop;
        let note = m.key().and_then(|k| app.notes().reading().get(&k).cloned());
        let note = note.unwrap_or_default();
        let replacements = app.replacements();
        let replaced = replacements.get(&m.folder).filter(|_| workshop);
        let steam_record = app.steam_record();
        let (times, sizes) = &*steam_record;
        let mut said = Details {
            workshop,
            official: m.source == Source::Official,
            name: m.display_name().to_string(),
            source: m.source_label(),
            contains: m.contains(),
            path: m.path.to_string_lossy().into_owned(),
            repository: rimstack_core::sources::git::page(&m.path).unwrap_or_default(),
            folder: m.folder.clone(),
            preview: m.preview.as_ref().map(|p| p.to_string_lossy().into_owned()),
            tags: note.tags.join(", "),
            tag_list: note.tags.clone(),
            version_ok: note.version_ok,
            pinned: note.pinned,
            problems_ignored: note.ignored.len(),
            replaced_by: replaced.map(|new| new.name.clone()),
            replaced_by_id: replaced.map(|new| new.workshop_id.clone()),
            replaced_by_versions: replaced.map(|new| new.versions.join(", ")),
            updated: times.get(&m.folder).copied().filter(|_| workshop),
            megabytes: sizes
                .get(&m.folder)
                .filter(|_| workshop)
                .map(|bytes| *bytes as f64 / 1_048_576.0),
            requires: details::required(library.requirements(m)),
            needed_by: details::needed_by(library.needed_by(m), self.active.mods()),
            off_with: self.off_with(library, id),
            translates: library.translated(m).map(|(name, installed)| {
                if installed {
                    Sentence::from(name)
                } else {
                    say!("{} (not installed)", name)
                }
            }),
            load_folders: self.load_folders(library, m),
            code: Sentence::joined("\n", describe::code(&m.code.facts)),
            ..Details::default()
        };
        match &m.about {
            Ok(about) => {
                said.author = about.authors.join(", ");
                said.package_id = about.package_id.clone();
                said.version = about.mod_version.clone().unwrap_or_default();
                said.game_versions = about.supported_versions.join(", ");
                said.description = about.description.clone().unwrap_or_default();
                said.description_rich = rimstack_core::base::markup::rich(&said.description);
                said.url = about.url.clone().unwrap_or_default();
                said.load_time =
                    rimstack_core::rimworld::startup::in_words(&app.startup(), &about.package_id);
                said.made_for = details::made_for(&about.supported_versions, &library.game_version);
            }
            Err(reason) => said.unreadable = Some(reason.sentence().clone()),
        }
        let save_ids = app.newest_save();
        let in_newest_save = m
            .key()
            .zip(save_ids.as_ref().as_ref())
            .is_some_and(|(key, ids)| ids.contains(&key));
        if in_newest_save
            && let Some(found) = app.conflicts().reading().as_ref()
            && let Some(verdict) = found.removal.get(&id)
        {
            said.removal = Some(verdict.said());
            said.removal_harms = verdict.harms();
        }
        if let Some(copies) = library.copies(id) {
            let active: Vec<String> = self.active.mods().iter().map(|x| x.id.clone()).collect();
            let read = library.in_use(&active);
            let entry = copies.all.iter().find_map(|at| read.get(at));
            said.copies = (copies.all.iter().copied())
                .filter_map(|at| {
                    let copy = library.get(at)?;
                    let used = read.contains_key(&at);
                    let written = library.written_for(at);
                    let note = read
                        .get(&at)
                        .and_then(|entry| library.loaded_why(at, entry))
                        .map(rimstack_core::mods::library::Loaded::words);
                    let version = copy
                        .about
                        .as_ref()
                        .ok()
                        .and_then(|about| about.mod_version.clone())
                        .unwrap_or_default();
                    let megabytes = sizes
                        .get(&copy.folder)
                        .filter(|_| copy.source == Source::Workshop)
                        .map(|bytes| *bytes as f64 / 1_048_576.0);
                    Some(Copy {
                        mod_id: at,
                        source: copy.source_label(),
                        folder: copy.folder.clone(),
                        path: copy.path.to_string_lossy().into_owned(),
                        version,
                        megabytes,
                        used,
                        note,
                        use_as: (!used && entry.is_some()).then_some(written).flatten(),
                    })
                })
                .collect();
        }
        let chosen = Category::from_kept(&note.category);
        said.category = chosen.or(library.category_of(id)).map(Category::label);
        said.note = note.text;
        said.category_chosen = chosen.map_or("", Category::label).to_string();
        let listed = [&self.active, &self.inactive]
            .into_iter()
            .find_map(|list| Some((list, list.mods().iter().find(|x| x.installed == Some(id))?)));
        said.sub = listed.map(|(_, x)| x.sub.clone()).unwrap_or_default();
        let beside = listed
            .map(|(list, x)| list.subs(x.category))
            .unwrap_or_default();
        let mut choices: Vec<String> = note.tags.iter().cloned().chain(beside).collect();
        choices.dedup();
        let mut seen = std::collections::HashSet::new();
        choices.retain(|tag| *tag != said.sub && seen.insert(tag.clone()));
        said.group_choices = choices;
        said.group_size = app.group_size(&said.sub);
        let whole = m.source != Source::Official && self.active.position_of(id).is_none();
        let made = app.texture_manifest().made_in(&m.path);
        said.bin_choices = crate::workspace::upkeep::bin_choices(made, workshop, whole);
        // A local copy has the page of the item it names, of its subscribed twin, or of its
        // package id in the Workshop database.
        let item = match m.source {
            Source::Workshop => Some(m.folder.clone()),
            Source::Official => None,
            _ => rimstack_core::rimworld::about::published_file_id(&m.path)
                .or_else(|| {
                    let twin = library.copies(id)?.all.into_iter().find(|&twin| {
                        twin != id
                            && library
                                .get(twin)
                                .is_some_and(|t| t.source == Source::Workshop)
                    })?;
                    Some(library.get(twin)?.folder.clone())
                })
                .or_else(|| Some(app.workshop_names().get(&m.key()?)?.workshop_id.clone())),
        };
        if let Some(item) = item {
            said.workshop_page = Some(format!(
                "https://steamcommunity.com/sharedfiles/filedetails/?id={item}"
            ));
            said.steam_page = Some(format!("steam://url/CommunityFilePage/{item}"));
        }
        if let Some(config) = &app.paths().config {
            let folders: std::collections::HashSet<&str> = library
                .installed
                .iter()
                .map(|m| m.folder.as_str())
                .collect();
            said.settings_files =
                rimstack_core::rimworld::mod_settings::of(config, &m.folder, |f| {
                    folders.contains(f)
                })
                .iter()
                .filter_map(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .collect();
        }
        let others: Vec<(&str, &[rimstack_core::rimworld::code::Aim])> = self
            .active
            .mods()
            .iter()
            .filter_map(|other| other.installed.filter(|&i| i != id))
            .filter_map(|i| library.get(i))
            .map(|other| (other.display_name(), other.code.aims.as_slice()))
            .collect();
        let also = describe::also_patching(&m.code.aims, &others);
        said.patches = describe::aims(&m.code.aims, &also, MOST_PATCHES)
            .into_iter()
            .map(|(method, note)| Patched { method, note })
            .collect();
        if let Some(at) = self.active.position_of(id) {
            let listed = &self.active.mods()[at];
            said.category = Some(listed.category.label());
            let own = self.troubles.get(at).map(Vec::as_slice).unwrap_or_default();
            said.problems = own.iter().map(|t| ProblemSaid::of(listed, t)).collect();
            said.offer_trust = said.problems.iter().any(|t| t.trust);
            let found = rules::all_rules(app, &mut self.detected, library, self.active.mods());
            let near = details::neighbours(&found, at, self.active.mods());
            let required = |n: &Named| said.requires.iter().any(|r| r.name == n.name);
            let (above, others): (Vec<Named>, Vec<Named>) =
                near.after.into_iter().partition(required);
            said.required_above = above.len();
            said.loads_after = others;
            said.loads_before = near.before;
            said.your_rules = self.own_rules(app, library, at);
            said.detected = self.detected_about(app, library, at);
            let clashes = self.texture_conflicts(app, id);
            said.loses_to = clashes.loses;
            said.wins_over = clashes.wins;
            said.conflict_marks = clashes.marks;
            let defs = self.def_clashes(app, id);
            said.defs_replaced_by = defs.replaced_by;
            said.defs_replaces = defs.replaces;
            said.xml_leaves_out = app
                .conflicts()
                .reading()
                .as_ref()
                .and_then(|known| known.xml_found.get(&id))
                .and_then(rimstack_core::mods::xmlcheck::leaves_out);
            said.placement = near.bottom.then(|| say!("The community rules keep this mod near the bottom of the list, below everything that does not have to follow it."));
        } else if let Some((_, listed)) = listed
            && let Some(lack) = describe::lacking(library, listed)
            && !note.ignored.contains(&lack.key)
        {
            said.problems = vec![ProblemSaid::of(listed, &lack)];
        }
        said
    }

    pub fn textures(app: &App, id: ModId) -> Textures {
        let loaded = app.library();
        let Some(m) = loaded.as_ref().as_ref().ok().and_then(|(l, _)| l.get(id)) else {
            return Textures::default();
        };
        Textures {
            count: app.texture_count(&m.path),
            dds_made: app.texture_manifest().made_in(&m.path),
        }
    }

    pub fn files(app: &App, id: ModId) -> Files {
        let loaded = app.library();
        let Some(m) = loaded.as_ref().as_ref().ok().and_then(|(l, _)| l.get(id)) else {
            return Files::default();
        };
        Files {
            tree: folder_tree(&m.path)
                .into_iter()
                .map(|(folder, files)| Branch { folder, files })
                .collect(),
            assemblies: m.assemblies().join(", "),
        }
    }

    fn off_with(&self, library: &rimstack_core::mods::library::Library, id: ModId) -> Vec<ModId> {
        let mods = self.active.mods();
        let Some(at) = self.active.position_of(id) else {
            return Vec::new();
        };
        let mut off = vec![false; mods.len()];
        off[at] = true;
        let off = rimstack_core::mods::library::with_needing(&library.requires_within(mods), off);
        let ids: Vec<ModId> = (mods.iter().zip(off))
            .filter_map(|(m, off)| m.installed.filter(|_| off))
            .collect();
        match ids.len() {
            1 => Vec::new(),
            _ => ids,
        }
    }

    /// Relative to the mod; `(the mod's own folder)` for its root.
    fn load_folders(
        &self,
        library: &rimstack_core::mods::library::Library,
        m: &rimstack_core::mods::library::Installed,
    ) -> Vec<Sentence> {
        let active: std::collections::HashSet<String> = self
            .active
            .mods()
            .iter()
            .filter_map(|x| library.get(x.installed?)?.key())
            .collect();
        let found = loadfolders::folders(&m.path, &library.game_version, &|k| active.contains(k));
        found
            .iter()
            .map(|folder| match folder.strip_prefix(&m.path) {
                Ok(inside) if !inside.as_os_str().is_empty() => {
                    Sentence::Plain(inside.to_string_lossy().into_owned())
                }
                _ => say!("(the mod's own folder)"),
            })
            .collect()
    }

    fn detected_about(
        &mut self,
        app: &App,
        library: &rimstack_core::mods::library::Library,
        at: usize,
    ) -> Vec<Detected> {
        let mods = self.active.mods();
        let found = self.detected.of(app, library, mods);
        found
            .iter()
            .filter(|d| d.then == at)
            .map(|d| Detected {
                name: mods[d.first].name.clone(),
                why: d.why.clone(),
                code: d.code,
                community: rimstack_core::mods::community::entry_for(
                    &mods[d.then].id,
                    &mods[d.first].id,
                    &mods[d.first].name,
                    &d.why.put(&rimstack_core::base::sentence::English),
                ),
            })
            .collect()
    }

    fn own_rules(
        &self,
        app: &App,
        library: &rimstack_core::mods::library::Library,
        at: usize,
    ) -> Vec<OwnRule> {
        let mods = self.active.mods();
        let yours = app.user_rules().reading();
        details::own_rules(&yours.rules(&library.abouts(mods)), at, mods)
    }

    fn texture_conflicts(&self, app: &App, id: ModId) -> details::Clashes {
        let known = app.conflicts().reading();
        let Some(known) = known.as_ref() else {
            return Default::default();
        };
        let listed = |id: ModId| {
            let at = self.active.position_of(id)?;
            Some((at, self.active.mods()[at].name.clone()))
        };
        details::clashes(&known.members, &known.overlaps, id, &listed)
    }

    /// Active mods defining the same defs; the one loading later's win.
    fn def_clashes(&self, app: &App, id: ModId) -> rimstack_core::mods::defs::DefClashes {
        let known = app.conflicts().reading();
        let Some(known) = known.as_ref() else {
            return Default::default();
        };
        let listed = |id: ModId| {
            let at = self.active.position_of(id)?;
            Some((at, self.active.mods()[at].name.clone()))
        };
        rimstack_core::mods::defs::of_mod(&known.members, &known.def_collisions, id, &listed)
    }

    /// What taking the switched-off mods, still in the newest save, out of it would do; empty
    /// for nothing said.
    pub fn left_save(&self, app: &App, library: &Library) -> Sentence {
        let session = app.session();
        let deactivated = session.changes().deactivated;
        let save_ids = app.newest_save();
        let Some(save_ids) = save_ids.as_ref().as_ref() else {
            return Sentence::default();
        };
        let known = app.conflicts().reading();
        let Some(found) = known.as_ref() else {
            return Sentence::default();
        };
        let mut names = Vec::new();
        for id in deactivated {
            let key = rimstack_core::rimworld::id::split_steam(id).0;
            if !save_ids.contains(&key) {
                continue;
            }
            let Some((mod_id, m)) = library
                .ids()
                .find(|(_, m)| m.key().as_deref() == Some(key.as_str()))
            else {
                continue;
            };
            let Some(verdict) = found.removal.get(&mod_id) else {
                continue;
            };
            names.push((m.display_name().to_string(), verdict.harms()));
        }
        let (harms, safe): (Vec<_>, Vec<_>) = names.into_iter().partition(|(_, harms)| *harms);
        let named = |which: Vec<String>| {
            let list = which.join(", ");
            (!list.is_empty()).then_some(list)
        };
        Sentence::joined(
            " ",
            [
                named(harms.into_iter().map(|(name, _)| name).collect()).map(|list| {
                    say!(
                        "Taking {} out of the newest save is likely to damage it.",
                        list
                    )
                }),
                named(safe.into_iter().map(|(name, _)| name).collect())
                    .map(|list| say!("Taking {} out of it is likely safe.", list)),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
    }

    pub fn overview(&self, app: &App) -> Overview {
        let loaded = self.reading(app);
        let Ok((library, took)) = &*loaded else {
            return Overview::default();
        };
        let mods = self.active.mods();
        let each = mods.iter().zip(&self.troubles);
        let said = |(m, own): (&Mod, &Vec<Trouble>)| -> Vec<ProblemSaid> {
            own.iter().map(|t| ProblemSaid::of(m, t)).collect()
        };
        let troubles: Vec<ProblemSaid> = each.clone().flat_map(said).collect();
        let to_get = each
            .clone()
            .flat_map(|(m, own)| {
                own.iter()
                    .filter(|t| t.fix.brings_a_mod())
                    .map(move |t| ProblemSaid::of(m, t))
            })
            .collect();
        let mut trouble_groups: Vec<ProblemGroup> =
            rimstack_core::base::grouped::by_key(each.flat_map(said), |t| t.kind)
                .into_iter()
                .map(|(kind, lines)| {
                    let trusted = lines.iter().filter(|t| t.trust).count();
                    ProblemGroup {
                        name: kind.heading(),
                        error: lines.first().is_some_and(|t| t.error),
                        trusted,
                        shared: trusted > 1 && trusted == lines.len(),
                        lines,
                    }
                })
                .collect();
        trouble_groups.sort_by_key(|group| !group.error);
        let trusted = app
            .notes()
            .reading()
            .version_ok()
            .map(|key| {
                let found = library
                    .ids()
                    .find(|(_, m)| m.key().as_deref() == Some(key))
                    .map(|(id, _)| id);
                Named {
                    name: library.name_of(key).unwrap_or_else(|| key.to_string()),
                    note: match found {
                        Some(_) => Sentence::default(),
                        None => say!("not installed"),
                    },
                    trouble: false,
                    mod_id: found,
                }
            })
            .collect();
        let community = app.community();
        let tally = describe::tally(&self.troubles);
        Overview {
            game_version: library.game_version.clone(),
            categories: Category::ALL
                .into_iter()
                .filter(|c| *c != Category::Game)
                .map(Category::label)
                .collect(),
            troubles,
            to_get,
            to_download: self.missing_workshop_ids(),
            trouble_groups,
            needed: tally.needed,
            version_only: tally.version_only,
            other_warnings: tally.other_warnings,
            slowest: details::slowest(mods, 5),
            load_times: mods.iter().any(|m| m.load_micros.is_some()),
            installed: library.installed.len(),
            missing: library.missing,
            unreadable: library.unreadable,
            read_ms: took.as_millis(),
            works_on: app.works_on().len(),
            trusted,
            community_mods: (*community).as_ref().map(|c| c.len()),
            community_time: (*community).as_ref().and_then(|c| c.timestamp),
        }
    }
}
