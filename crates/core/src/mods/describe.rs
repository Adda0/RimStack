use crate::base::sentence::Sentence;
use crate::base::translate::named;
use crate::mods::checks::{self, Problem};
use crate::mods::details;
use crate::mods::library::{Library, ModId};
use crate::mods::list::Mod;
use crate::mods::rules::{Kind, Rule};
use crate::rimworld::code::{Aim, Facts};
use crate::rimworld::dotnet::Does;
use crate::rimworld::game::When;
use crate::rimworld::workshopdb::Names;
use crate::say;
use crate::say_n;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trouble {
    pub error: bool,
    pub words: Sentence,
    pub short: Sentence,
    pub fix: Fix,
    /// What `Note::ignored` holds to ignore it; empty for a game version, which has `Fix::Trust`.
    pub key: String,
    pub kind: TroubleKind,
}

/// What a trouble is about: the Problems panel lists each kind under a heading of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TroubleKind {
    NeededOff,
    NeededMissing,
    NotInstalled,
    Incompatible,
    OutOfOrder,
    GameVersion,
    NoVersions,
    NoWorkshopId,
    GoneFromWorkshop,
    Duplicate,
    XmlBroken,
    PatchMisses,
    OwnHarmony,
    DllClash,
    LoadFoldersCase,
}

impl TroubleKind {
    pub fn heading(self) -> &'static str {
        match self {
            TroubleKind::NeededOff => named("Needed and switched off"),
            TroubleKind::NeededMissing => named("Needed and not installed"),
            TroubleKind::NotInstalled => named("Not installed"),
            TroubleKind::Incompatible => named("Cannot be used together"),
            TroubleKind::OutOfOrder => named("Out of order"),
            TroubleKind::GameVersion => named("Made for another game version"),
            TroubleKind::NoVersions => named("No supported versions"),
            TroubleKind::NoWorkshopId => named("No Workshop id"),
            TroubleKind::GoneFromWorkshop => named("Gone from the Workshop"),
            TroubleKind::Duplicate => named("Installed more than once"),
            TroubleKind::XmlBroken => named("XML that will not load"),
            TroubleKind::PatchMisses => named("Patches miss their defs"),
            TroubleKind::OwnHarmony => named("Ships its own 0Harmony.dll"),
            TroubleKind::DllClash => named("Dlls other active mods also ship"),
            TroubleKind::LoadFoldersCase => named("LoadFolders.xml letter case"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    None,
    Activate(String),
    Subscribe(String),
    Search(String),
    Deactivate(usize),
    /// Put the mod right after (true) or right before (false) the one at the index.
    Move(usize, bool),
    Trust,
}

pub fn warned_for_version(own: &[Trouble]) -> bool {
    own.iter().any(|t| t.fix == Fix::Trust)
}

impl Fix {
    pub fn brings_a_mod(&self) -> bool {
        matches!(self, Fix::Activate(_) | Fix::Subscribe(_) | Fix::Search(_))
    }

    pub fn action(&self) -> &'static str {
        match self {
            Fix::None => "",
            Fix::Activate(_) => "activate",
            Fix::Subscribe(_) => "subscribe",
            Fix::Search(_) => "search",
            Fix::Deactivate(_) => "deactivate",
            Fix::Move(..) => "move",
            Fix::Trust => "trust",
        }
    }

    pub fn target(&self) -> String {
        match self {
            Fix::None | Fix::Trust => String::new(),
            Fix::Activate(id) | Fix::Subscribe(id) | Fix::Search(id) => id.clone(),
            Fix::Deactivate(at) => at.to_string(),
            Fix::Move(at, after) => format!("{at}:{}", if *after { "after" } else { "before" }),
        }
    }

    pub fn page(&self) -> Option<String> {
        let Fix::Search(id) = self else {
            return None;
        };
        let text: String = id
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    char::from(b).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect();
        Some(format!(
            "https://steamcommunity.com/workshop/browse/?appid={}&searchtext={text}",
            crate::rimworld::paths::APP_ID
        ))
    }

    pub fn label(&self) -> &'static str {
        match self {
            Fix::None => "",
            Fix::Activate(_) => named("Activate it"),
            Fix::Subscribe(_) => named("Workshop page"),
            Fix::Search(_) => named("Search the Workshop"),
            Fix::Deactivate(_) => named("Deactivate the other"),
            Fix::Move(..) => named("Move it there"),
            Fix::Trust => named("Ignore"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    pub needed: Vec<String>,
    pub version_only: usize,
    pub other_warnings: usize,
}

pub fn tally(troubles: &[Vec<Trouble>]) -> Tally {
    let mut tally = Tally::default();
    for own in troubles {
        for trouble in own {
            match &trouble.fix {
                Fix::Activate(id) if !tally.needed.contains(id) => tally.needed.push(id.clone()),
                Fix::Trust => tally.version_only += 1,
                _ => {}
            }
        }
        let other = |t: &Trouble| !t.error && t.fix != Fix::Trust;
        tally.other_warnings += usize::from(own.iter().any(other));
    }
    tally
}

#[derive(Debug, Clone, Copy)]
pub enum Versions<'a> {
    CheckedBut(&'a HashSet<String>),
    Ignored,
}

impl Versions<'_> {
    fn trusts(self, package_id: &str) -> bool {
        match self {
            Versions::CheckedBut(trusted) => {
                trusted.contains(&crate::rimworld::id::fold(package_id))
            }
            Versions::Ignored => true,
        }
    }
}

// The game's own parts are of the game's version whatever they list: a test build
// ships expansions that still name the last one.
fn official(library: &Library, m: &Mod) -> bool {
    let installed = m.installed.and_then(|i| library.get(i));
    installed.is_some_and(|i| i.source == crate::rimworld::paths::Source::Official)
}

/// The newest version listed by each mod `troubles` warns of as made for another game version.
pub fn outdated(library: &Library, mods: &[Mod], versions: Versions) -> Vec<Option<String>> {
    let abouts = library.abouts(mods);
    let found = checks::check(&abouts, &library.game_version, |key| library.name_of(key));
    let each = found.into_iter().zip(&abouts).zip(mods);
    each.map(|((problems, about), m)| {
        let supports = problems.into_iter().find_map(|p| match p {
            Problem::GameVersion { supports } => Some(supports),
            _ => None,
        })?;
        let excused = official(library, m) || about.is_some_and(|a| versions.trusts(&a.package_id));
        let newest = details::newest(&supports).map(String::from);
        newest.filter(|_| !excused)
    })
    .collect()
}

pub fn version_ignored(library: &Library, mods: &[Mod], versions: Versions) -> Vec<bool> {
    let abouts = library.abouts(mods);
    let found = checks::check(&abouts, &library.game_version, |key| library.name_of(key));
    let trusted = |package_id: &str| versions.trusts(package_id);
    let mismatch = |problems: &Vec<Problem>| {
        let version = |p: &Problem| matches!(p, Problem::GameVersion { .. });
        problems.iter().any(version)
    };
    let each = found.iter().zip(&abouts);
    each.map(|(problems, about)| {
        mismatch(problems) && about.is_some_and(|a| trusted(&a.package_id))
    })
    .collect()
}

/// Display name for an official DLC by its folded package id, or None if not a known DLC.
fn dlc_name(folded_id: &str) -> Option<&'static str> {
    match folded_id {
        "ludeon.rimworld.royalty" => Some("Royalty"),
        "ludeon.rimworld.ideology" => Some("Ideology"),
        "ludeon.rimworld.biotech" => Some("Biotech"),
        "ludeon.rimworld.anomaly" => Some("Anomaly"),
        "ludeon.rimworld.odyssey" => Some("Odyssey"),
        _ => None,
    }
}

pub fn missing(workshop: &Names, id: &str) -> (String, Fix) {
    match workshop.get(&crate::rimworld::id::fold(id)) {
        Some(item) => (item.name.clone(), Fix::Subscribe(item.workshop_id.clone())),
        None => (id.to_string(), Fix::Search(id.to_string())),
    }
}

/// From a mod whose About.xml was read, a property RimSort looks for that it does not have:
/// a copy of a Workshop item names the item in `About/PublishedFileId.txt`; a local one without
/// it can never be checked for updates. Ignorable by the mod's package id and `Trouble::key`.
pub fn lacking(library: &Library, m: &crate::mods::list::Mod) -> Option<Trouble> {
    let installed = library.get(m.installed?)?;
    if installed.source != crate::rimworld::paths::Source::Local
        || installed.published
        || installed.about.is_err()
    {
        return None;
    }
    Some(Trouble {
        error: false,
        words: say!(
            "Names no Workshop item: it has no About/PublishedFileId.txt, so a check cannot tell its updates."
        ),
        short: say!("No Workshop id"),
        fix: Fix::None,
        key: "no-workshop-id".to_string(),
        kind: TroubleKind::NoWorkshopId,
    })
}

/// About.xml listing no game version: the game logs it and its mod list marks the mod as made for
/// another version. Official content lists none and is not asked to.
pub fn unversioned(library: &Library, m: &Mod) -> Option<Trouble> {
    let about = library.get(m.installed?)?.about.as_ref().ok()?;
    if official(library, m) || !about.supported_versions.is_empty() {
        return None;
    }
    Some(Trouble {
        error: false,
        words: say!(
            "Names no game version in About.xml under supportedVersions: the game logs it as an error and marks the mod as made for another version."
        ),
        short: say!("No supported versions"),
        fix: Fix::None,
        key: "no-versions".to_string(),
        kind: TroubleKind::NoVersions,
    })
}

pub fn troubles(
    library: &Library,
    mods: &[Mod],
    versions: Versions,
    workshop: &Names,
) -> Vec<Vec<Trouble>> {
    let abouts = library.abouts(mods);
    let mut found = checks::check(&abouts, &library.game_version, |key| library.name_of(key));
    for ((problems, about), m) in found.iter_mut().zip(&abouts).zip(mods) {
        if official(library, m) || about.is_some_and(|a| versions.trusts(&a.package_id)) {
            problems.retain(|p| !matches!(p, Problem::GameVersion { .. }));
        }
    }
    let missing = |id: &str| missing(workshop, id);
    let in_words = |problem: Problem| -> Trouble {
        let key = match &problem {
            Problem::Requires { id, .. } => {
                format!(
                    "needs:{}",
                    crate::rimworld::id::fold(&crate::rimworld::id::split_steam(id).0)
                )
            }
            Problem::Incompatible { with } => {
                format!("not-with:{}", crate::rimworld::id::fold(&mods[*with].id))
            }
            Problem::GameVersion { .. } => String::new(),
        };
        let (words, short, fix, kind) = match &problem {
            Problem::Requires {
                id,
                name: Some(name),
            } => (
                say!("Needs {}, which is installed but switched off.", name),
                say!("Needs {}", name),
                Fix::Activate(crate::rimworld::id::fold(
                    &crate::rimworld::id::split_steam(id).0,
                )),
                TroubleKind::NeededOff,
            ),
            Problem::Requires { id, name: None } => {
                let folded_id = crate::rimworld::id::fold(&crate::rimworld::id::split_steam(id).0);
                if let Some(dlc) = dlc_name(&folded_id) {
                    (
                        say!("Needs the {} DLC, which is not installed.", dlc),
                        say!("Missing the {} DLC", dlc),
                        Fix::None,
                        TroubleKind::NeededMissing,
                    )
                } else {
                    let (name, fix) = missing(id);
                    (
                        say!("Needs {}, which is not installed.", name),
                        say!("Missing {}", name),
                        fix,
                        TroubleKind::NeededMissing,
                    )
                }
            }
            Problem::Incompatible { with } => (
                say!("Cannot be used together with {}.", mods[*with].name),
                say!("Not with {}", mods[*with].name),
                Fix::Deactivate(*with),
                TroubleKind::Incompatible,
            ),
            Problem::GameVersion { supports } => (
                say!(
                    "Made for RimWorld {}, not {}. Many such mods still work.",
                    supports.join(", "),
                    library.game_version
                ),
                say!("Made for {}", details::newest(supports).unwrap_or_default()),
                Fix::Trust,
                TroubleKind::GameVersion,
            ),
        };
        Trouble {
            error: problem.is_error(),
            words,
            short,
            fix,
            key,
            kind,
        }
    };
    let mut all: Vec<Vec<Trouble>> = found
        .into_iter()
        .map(|problems| problems.into_iter().map(in_words).collect())
        .collect();
    for (m, own) in mods.iter().zip(&mut all) {
        if m.installed.is_none() {
            own.push(Trouble {
                error: true,
                words: say!("On the list, but not installed."),
                short: say!("Not installed"),
                fix: missing(&m.id).1,
                key: "missing".to_string(),
                kind: TroubleKind::NotInstalled,
            });
        }
        if let Some(lack) = lacking(library, m) {
            own.push(lack);
        }
        own.extend(unversioned(library, m));
    }
    all
}

/// What `is:incomplete` marks: a mod missing a property RimSort looks for — unreadable About.xml
/// (no packageId it can be known by), or a local copy that names no Workshop item, unless that
/// warning is ignored for it (by folded package id).
pub fn incomplete(
    installed: Option<&crate::mods::library::Installed>,
    folded_id: &str,
    ignored: &HashMap<String, HashSet<String>>,
) -> bool {
    let Some(m) = installed else {
        return false;
    };
    if m.about.is_err() {
        return true;
    }
    let lacks_item = m.source == crate::rimworld::paths::Source::Local && !m.published;
    lacks_item
        && !ignored
            .get(folded_id)
            .is_some_and(|keys| keys.contains("no-workshop-id"))
}

/// Takes out the problems ignored, by folded package id; an incompatibility ignored on
/// either side is gone from both.
pub fn without_ignored(
    all: &mut [Vec<Trouble>],
    mods: &[Mod],
    ignored: &HashMap<String, HashSet<String>>,
) {
    let folded: Vec<String> = mods
        .iter()
        .map(|m| crate::rimworld::id::fold(&m.id))
        .collect();
    let ignores = |at: usize, key: &str| {
        ignored
            .get(&folded[at])
            .is_some_and(|keys| keys.contains(key))
    };
    for (me, own) in all.iter_mut().enumerate() {
        own.retain(|t| {
            let other_side = match t.fix {
                Fix::Deactivate(with) => ignores(with, &format!("not-with:{}", folded[me])),
                _ => false,
            };
            t.key.is_empty() || !(ignores(me, &t.key) || other_side)
        });
    }
}

/// For each active mod: its id names another installed folder, and the game reads only one
/// of the copies. Of the entries naming one id, only the first is read.
pub fn duplicates(library: &Library, mods: &[Mod]) -> Vec<Vec<Trouble>> {
    let entries: Vec<String> = mods.iter().map(|m| m.id.clone()).collect();
    let read = library.in_use(&entries);
    mods.iter()
        .map(|m| duplicate(library, m, &read).into_iter().collect())
        .collect()
}

fn duplicate(library: &Library, m: &Mod, read: &HashMap<ModId, String>) -> Option<Trouble> {
    let i = m.installed?;
    let copies = library.copies(i)?;
    let key = library.get(i)?.key()?;
    let place = |at: ModId| {
        library
            .get(at)
            .map(|o| format!("{} ({})", o.folder, o.source_label()))
    };
    let others: Vec<String> = copies
        .all
        .iter()
        .copied()
        .filter(|&o| o != i)
        .filter_map(place)
        .collect();
    let all = others.join(", ");
    let why = |winner: ModId| library.loaded_why(winner, &m.id).map(|why| why.words());
    let trouble = if read.contains_key(&i) {
        Trouble {
            error: false,
            words: say!(
                "Its id {} is installed more than once: {}. For \"{}\" the game loads this copy — {}.",
                key,
                all,
                m.id,
                why(i)?.inside()
            ),
            short: say!("another copy is installed"),
            fix: Fix::None,
            key: "duplicate".to_string(),
            kind: TroubleKind::Duplicate,
        }
    } else {
        let winner = copies.all.iter().copied().find(|o| read.contains_key(o))?;
        Trouble {
            error: false,
            words: say!(
                "Its id {} is installed more than once: {}. The order reaches \"{}\" first: the game loads {}, and reads nothing of this copy.",
                key,
                all,
                read[&winner],
                place(winner)?
            ),
            short: say!("the game loads another copy"),
            fix: Fix::None,
            key: "duplicate".to_string(),
            kind: TroubleKind::Duplicate,
        }
    };
    Some(trouble)
}

pub fn code(facts: &Facts) -> Vec<Sentence> {
    if facts.types == 0 {
        return Vec::new();
    }
    let some = |count: usize, said: Sentence| (count > 0).then_some(said);
    let with = |counted: Sentence, each: [Option<Sentence>; 3]| {
        let each: Vec<Sentence> = each.into_iter().flatten().collect();
        match each.is_empty() {
            true => counted,
            false => say!("{}: {}", counted.inside(), Sentence::listed(each).inside()),
        }
    };
    let types = [
        some(facts.drawing, say!("{} drawing", facts.drawing)),
        some(facts.adding, say!("{} adding to the game", facts.adding)),
        None,
    ];
    let mut parts = vec![with(say_n!(facts.types, "{} type", "{} types"), types)];
    if facts.patches > 0 {
        let aimed = [
            some(facts.on_playing, say!("{} while playing", facts.on_playing)),
            some(
                facts.on_interface,
                say!("{} on the interface", facts.on_interface),
            ),
            some(facts.on_loading, say!("{} on loading", facts.on_loading)),
        ];
        let patched = say_n!(facts.patches, "{} patched method", "{} patched methods");
        parts.push(with(patched, aimed));
    }
    parts
}

pub fn also_patching<'a>(
    mine: &[Aim],
    others: &'a [(&'a str, &[Aim])],
) -> Vec<Vec<(&'a str, Does)>> {
    let same =
        |a: &Aim, b: &Aim| !a.target.is_empty() && a.target == b.target && a.method == b.method;
    let patching = |aim: &Aim| {
        others
            .iter()
            .flat_map(|(name, theirs)| {
                theirs
                    .iter()
                    .filter(|t| same(t, aim))
                    .map(|t| (*name, t.does))
            })
            .collect()
    };
    mine.iter().map(patching).collect()
}

/// The reason a colliding patch is named with: two transpilers on one method, or a prefix
/// that can skip the original, likely clash; postfixes alone are likely safe neighbours.
fn clash(aside: Does, others: &[(&str, Does)]) -> Sentence {
    let names = Sentence::listed(others.iter().map(|(n, _)| (*n).into()).collect());
    if aside.transpiler && others.iter().any(|(_, d)| d.transpiler) {
        return say!("{} (likely to clash: both rewrite it)", names.inside());
    }
    if aside.skips || others.iter().any(|(_, d)| d.skips) {
        return say!(
            "{} (likely to clash: a prefix may skip the original)",
            names.inside()
        );
    }
    let postfixes = |d: &Does| d.postfix && !d.prefix && !d.transpiler && !d.finalizer;
    if postfixes(&aside) && others.iter().all(|(_, d)| postfixes(d)) {
        return say!("{} (unlikely to clash: only adds after)", names.inside());
    }
    say!("{} (may clash)", names.inside())
}

pub fn aims(aims: &[Aim], also: &[Vec<(&str, Does)>], most: usize) -> Vec<(Sentence, Sentence)> {
    let note = |aim: &Aim, others: &[(&str, Does)]| {
        let when = match aim.when {
            Some(When::Playing) => Some(say!("while playing")),
            Some(When::Interface) => Some(say!("interface")),
            Some(When::Loading) => Some(say!("loading")),
            None => None,
        };
        // Harmony's own names for what a patch does: not put in words.
        let does = [
            (aim.does.prefix, "prefix"),
            (aim.does.postfix, "postfix"),
            (aim.does.transpiler, "transpiler"),
            (aim.does.finalizer, "finalizer"),
        ];
        let does = does.iter().filter(|d| d.0).map(|d| Sentence::from(d.1));
        let said = Sentence::listed(when.into_iter().chain(does).collect());
        match others.is_empty() {
            true => said,
            false => {
                let ranked = clash(aim.does, others);
                say!("{}; also {}", said.inside(), ranked.inside())
            }
        }
    };
    let none = Vec::new();
    let others = |at: usize| also.get(at).unwrap_or(&none);
    let mut order: Vec<usize> = (0..aims.len()).collect();
    order.sort_by_key(|&at| others(at).is_empty());
    let mut lines: Vec<(Sentence, Sentence)> = order
        .iter()
        .take(most)
        .map(|&at| {
            let aim = &aims[at];
            let name = match aim.target.is_empty() {
                true => say!("Target named in code"),
                false => Sentence::Plain(format!("{}.{}", aim.target, aim.method)),
            };
            (name, note(aim, others(at)))
        })
        .collect();
    if aims.len() > most {
        lines.push((
            say!("{} more", aims.len() - most),
            Sentence::listed(Vec::new()),
        ));
    }
    lines
}

pub fn rule(rule: &Rule, mods: &[Mod]) -> Sentence {
    let Some(by) = rule.declared_by() else {
        let (first, then) = (&mods[rule.first].name, &mods[rule.then].name);
        return match rule.kind {
            Kind::Yours => say!("your rule puts {} before {}", first, then),
            _ => say!("the community rules put {} before {}", first, then),
        };
    };
    let other = if by == rule.first {
        rule.then
    } else {
        rule.first
    };
    let (by, other) = (&mods[by].name, &mods[other].name);
    match rule.kind {
        Kind::Requires => say!("{} requires {}", by, other),
        Kind::LoadAfter | Kind::ForceLoadAfter => say!("{} asks to load after {}", by, other),
        _ => say!("{} asks to load before {}", by, other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_fix_has_its_workshop_page_and_its_text_is_encoded() {
        let page = Fix::Search("Ann.Big Roads&co".into()).page().unwrap();
        assert!(page.ends_with("searchtext=Ann.Big%20Roads%26co"));
        assert!(page.contains("appid=294100"));
        assert_eq!(Fix::Trust.page(), None);
        assert_eq!(Fix::Subscribe("1".into()).page(), None);
    }

    #[test]
    fn a_mod_is_warned_for_its_version_when_one_of_its_problems_is_trusted_away() {
        let trouble = |fix| Trouble {
            error: false,
            words: Sentence::default(),
            short: Sentence::default(),
            fix,
            key: String::new(),
            kind: TroubleKind::GameVersion,
        };
        assert!(warned_for_version(&[
            trouble(Fix::None),
            trouble(Fix::Trust)
        ]));
        assert!(!warned_for_version(&[trouble(Fix::Search("a".into()))]));
        assert!(!warned_for_version(&[]));
    }
    use crate::mods::library::{build, scan};
    use crate::rimworld::paths::{ModRoot, Source};
    use std::fs;

    fn two_mods() -> (tempfile::TempDir, Library) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Mods");
        let abouts = [
            (
                "old",
                "<ModMetaData><packageId>a.old</packageId><name>Old</name><supportedVersions><li>1.5</li></supportedVersions><modDependencies><li><packageId>gone.lib</packageId></li></modDependencies></ModMetaData>",
            ),
            (
                "fine",
                "<ModMetaData><packageId>a.fine</packageId><name>Fine</name><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>",
            ),
        ];
        for (folder, about) in abouts {
            fs::create_dir_all(root.join(folder).join("About")).unwrap();
            fs::write(root.join(folder).join("About/About.xml"), about).unwrap();
            fs::write(root.join(folder).join("About/PublishedFileId.txt"), "1000").unwrap();
        }
        let roots = [ModRoot {
            path: root,
            source: Source::Local,
        }];
        let active = [
            "a.old".to_string(),
            "a.fine".to_string(),
            "not.here".to_string(),
        ];
        (tmp, build(scan(&roots), &active, "1.6"))
    }

    #[test]
    fn each_mod_s_problems_are_put_in_words_errors_marked() {
        let (_tmp, library) = two_mods();
        let found = troubles(
            &library,
            &library.active,
            Versions::CheckedBut(&HashSet::new()),
            &Names::new(),
        );
        let said: Vec<(String, String)> = found[0]
            .iter()
            .map(|t| (t.words.to_string(), t.short.to_string()))
            .collect();
        let old: Vec<(bool, &str, &str, &Fix)> = found[0]
            .iter()
            .zip(&said)
            .map(|(t, (words, short))| (t.error, words.as_str(), short.as_str(), &t.fix))
            .collect();
        assert_eq!(
            old,
            [
                (
                    true,
                    "Needs gone.lib, which is not installed.",
                    "Missing gone.lib",
                    &Fix::Search("gone.lib".to_string())
                ),
                (
                    false,
                    "Made for RimWorld 1.5, not 1.6. Many such mods still work.",
                    "Made for 1.5",
                    &Fix::Trust
                ),
            ]
        );
        assert!(found[1].is_empty());
        assert_eq!(found[2][0].short.to_string(), "Not installed");
        assert_eq!(found[2][0].fix, Fix::Search("not.here".to_string()));
    }

    #[test]
    fn a_mod_listing_no_game_version_is_warned_once_not_as_made_for_another() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Mods");
        let abouts = [
            (
                "bare",
                "<ModMetaData><packageId>a.bare</packageId><name>Bare</name><supportedVersions/></ModMetaData>",
            ),
            (
                "none",
                "<ModMetaData><packageId>a.none</packageId><name>None</name></ModMetaData>",
            ),
        ];
        for (folder, about) in abouts {
            fs::create_dir_all(root.join(folder).join("About")).unwrap();
            fs::write(root.join(folder).join("About/About.xml"), about).unwrap();
            fs::write(root.join(folder).join("About/PublishedFileId.txt"), "1000").unwrap();
        }
        let roots = [ModRoot {
            path: root,
            source: Source::Local,
        }];
        let active = ["a.bare".to_string(), "a.none".to_string()];
        let library = build(scan(&roots), &active, "1.6");
        let found = troubles(
            &library,
            &library.active,
            Versions::CheckedBut(&HashSet::new()),
            &Names::new(),
        );
        for own in &found {
            let keys: Vec<&str> = own.iter().map(|t| t.key.as_str()).collect();
            assert_eq!(keys, ["no-versions"], "{own:?}");
        }
    }

    #[test]
    fn an_ignored_problem_is_taken_out_and_an_incompatibility_from_both_sides() {
        let (_tmp, library) = two_mods();
        let mut found = troubles(
            &library,
            &library.active,
            Versions::CheckedBut(&HashSet::new()),
            &Names::new(),
        );
        assert_eq!(found[0][0].key, "needs:gone.lib");
        assert_eq!(found[2][0].key, "missing");
        let not_with = |with: usize| Trouble {
            error: true,
            words: Sentence::default(),
            short: Sentence::default(),
            fix: Fix::Deactivate(with),
            key: format!(
                "not-with:{}",
                crate::rimworld::id::fold(&library.active[with].id)
            ),
            kind: TroubleKind::Incompatible,
        };
        found[0].push(not_with(1));
        found[1].push(not_with(0));
        let ignored = HashMap::from([
            (
                "a.fine".to_string(),
                HashSet::from(["not-with:a.old".to_string()]),
            ),
            (
                "not.here".to_string(),
                HashSet::from(["missing".to_string()]),
            ),
        ]);
        without_ignored(&mut found, &library.active, &ignored);
        assert_eq!(found[0].len(), 2, "its need and its version stay");
        assert!(found[1].is_empty() && found[2].is_empty());
    }

    #[test]
    fn the_game_s_own_parts_are_not_warned_about_for_the_version_they_list() {
        let tmp = tempfile::tempdir().unwrap();
        let about = "<ModMetaData><packageId>ludeon.rimworld.royalty</packageId><name>Royalty</name><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>";
        let mut roots = Vec::new();
        for (folder, source) in [("Data", Source::Official), ("Mods", Source::Local)] {
            let root = tmp.path().join(folder);
            fs::create_dir_all(root.join("Royalty/About")).unwrap();
            let own = about.replace("ludeon.rimworld.royalty", &format!("a.{folder}"));
            fs::write(root.join("Royalty/About/About.xml"), own).unwrap();
            if source == Source::Local {
                // A copy put in Mods keeps its PublishedFileId.txt, like a redownload.
                fs::write(root.join("Royalty/About/PublishedFileId.txt"), "2941000\n").unwrap();
            }
            roots.push(ModRoot { path: root, source });
        }
        let active = ["a.Data".to_string(), "a.Mods".to_string()];
        let library = build(scan(&roots), &active, "1.7");
        let nobody = HashSet::new();
        let found = troubles(
            &library,
            &library.active,
            Versions::CheckedBut(&nobody),
            &Names::new(),
        );
        assert!(found[0].is_empty());
        assert_eq!(found[1].len(), 1);
        assert_eq!(found[1][0].fix, Fix::Trust);
    }

    #[test]
    fn a_fix_has_an_action_a_target_and_a_button() {
        let fix = Fix::Activate("a.fine".to_string());
        assert_eq!(
            (fix.action(), fix.target().as_str()),
            ("activate", "a.fine")
        );
        let other = Fix::Deactivate(3);
        assert_eq!(
            (other.action(), other.target().as_str()),
            ("deactivate", "3")
        );
        assert_eq!(
            (Fix::Trust.action(), Fix::Trust.target().as_str()),
            ("trust", "")
        );
        assert_eq!(Fix::None.label(), "");
    }

    #[test]
    fn problems_are_counted_by_what_solves_them() {
        let trouble = |error: bool, fix: Fix| Trouble {
            error,
            words: Sentence::default(),
            short: Sentence::default(),
            fix,
            key: String::new(),
            kind: TroubleKind::GameVersion,
        };
        let needs = |id: &str| trouble(true, Fix::Activate(id.to_string()));
        let counted = tally(&[
            vec![needs("a.lib"), trouble(false, Fix::Trust)],
            vec![needs("a.lib"), needs("b.lib")],
            vec![trouble(false, Fix::None), trouble(false, Fix::None)],
            vec![trouble(false, Fix::Trust)],
            vec![],
        ]);
        assert_eq!(counted.needed, ["a.lib", "b.lib"]);
        assert_eq!(counted.version_only, 2);
        assert_eq!(counted.other_warnings, 1);
    }

    #[test]
    fn a_trusted_mod_gets_no_version_warning_and_keeps_its_other_problems() {
        let (_tmp, library) = two_mods();
        let trusted: HashSet<String> = ["a.old".to_string()].into();
        let none = Names::new();
        let found = troubles(
            &library,
            &library.active,
            Versions::CheckedBut(&trusted),
            &none,
        );
        assert_eq!(found[0].len(), 1);
        assert!(found[0][0].error && found[0][0].short.to_string() == "Missing gone.lib");
        let found = troubles(&library, &library.active, Versions::Ignored, &none);
        assert_eq!(found[0].len(), 1);
        let ignored = version_ignored(&library, &library.active, Versions::Ignored);
        assert_eq!(ignored[..2], [true, false]);
        let nobody = HashSet::new();
        let checked = version_ignored(&library, &library.active, Versions::CheckedBut(&nobody));
        assert!(checked.iter().all(|ignored| !ignored));
        let item = crate::rimworld::workshopdb::Item {
            workshop_id: "42".into(),
            name: "Gone Library".into(),
        };
        let known: Names = [("gone.lib".to_string(), item)].into();
        let found = troubles(&library, &library.active, Versions::Ignored, &known);
        assert_eq!(
            found[0][0].words.to_string(),
            "Needs Gone Library, which is not installed."
        );
        assert_eq!(found[0][0].fix, Fix::Subscribe("42".to_string()));
    }

    #[test]
    fn a_local_copy_naming_no_workshop_item_is_flagged_until_ignored() {
        let (tmp, library) = two_mods();
        let nobody: HashMap<String, HashSet<String>> = HashMap::new();
        // Both local mods name a Workshop item: nothing is missing.
        assert!(library.inactive.iter().all(|m| {
            let installed = library.get(m.installed.expect("installed"));
            !incomplete(installed, &m.id, &nobody)
        }));

        std::fs::remove_file(tmp.path().join("Mods/old/About/PublishedFileId.txt")).unwrap();
        std::fs::remove_file(tmp.path().join("Mods/fine/About/About.xml")).unwrap();
        let root = tmp.path().join("Mods");
        let roots = [ModRoot {
            path: root,
            source: Source::Local,
        }];
        let active = ["a.old".to_string()];
        let library = build(scan(&roots), &active, "1.6");

        let lack = lacking(&library, &library.active[0]).unwrap();
        assert_eq!(lack.short.to_string(), "No Workshop id");
        assert!(!lack.error && lack.key == "no-workshop-id");
        assert!(lack.words.to_string().contains("PublishedFileId.txt"));
        // A copy that names no item is incomplete until its warning is ignored.
        let old = library.active[0].installed.unwrap();
        assert!(incomplete(library.get(old), "a.old", &nobody));
        let ignored: HashMap<String, HashSet<String>> = HashMap::from([(
            "a.old".to_string(),
            HashSet::from(["no-workshop-id".to_string()]),
        )]);
        assert!(!incomplete(library.get(old), "a.old", &ignored));
        // A mod whose About.xml does not read has no id to ignore anything by.
        let unreadable = library
            .inactive
            .iter()
            .find(|m| m.installed.is_some() && m.id.is_empty())
            .unwrap();
        let unreadable_id = unreadable.installed.unwrap();
        assert!(incomplete(library.get(unreadable_id), "", &ignored));
        assert!(lacking(&library, unreadable).is_none());
    }

    #[test]
    fn a_mod_is_outdated_unless_trusted_or_version_warnings_are_off() {
        let (_tmp, library) = two_mods();
        let nobody = HashSet::new();
        let found = outdated(&library, &library.active, Versions::CheckedBut(&nobody));
        assert_eq!(found, [Some("1.5".to_string()), None, None]);
        let trusted: HashSet<String> = ["a.old".to_string()].into();
        let found = outdated(&library, &library.active, Versions::CheckedBut(&trusted));
        assert_eq!(found, [None, None, None]);
        let found = outdated(&library, &library.active, Versions::Ignored);
        assert_eq!(found, [None, None, None]);
    }

    #[test]
    fn code_and_rules_read_as_plain_lines() {
        let code = |facts: &Facts| -> Vec<String> {
            code(facts).iter().map(Sentence::to_string).collect()
        };
        let aims =
            |aims_of: &[Aim], also: &[Vec<(&str, Does)>], most: usize| -> Vec<(String, String)> {
                aims(aims_of, also, most)
                    .iter()
                    .map(|(name, note)| (name.to_string(), note.to_string()))
                    .collect()
            };
        assert!(code(&Facts::default()).is_empty());
        let facts = Facts {
            types: 1,
            drawing: 1,
            patches: 2,
            on_playing: 1,
            ..Facts::default()
        };
        assert_eq!(
            code(&facts),
            ["1 type: 1 drawing", "2 patched methods: 1 while playing"]
        );
        let aim = |method: &str, when, prefix| Aim {
            target: "Verse.Pawn".into(),
            method: method.into(),
            does: crate::rimworld::dotnet::Does {
                prefix,
                postfix: true,
                ..Default::default()
            },
            when,
        };
        let patched = [
            aim("Tick", Some(When::Playing), true),
            aim("Draw", None, false),
            aim("Kill", Some(When::Playing), false),
        ];
        assert_eq!(
            aims(&patched, &[], 2),
            [
                (
                    "Verse.Pawn.Tick".to_string(),
                    "while playing, prefix, postfix".to_string()
                ),
                ("Verse.Pawn.Draw".to_string(), "postfix".to_string()),
                ("1 more".to_string(), "".to_string())
            ]
        );
        let unknown = Aim {
            target: String::new(),
            ..aim("", None, true)
        };
        let theirs = [
            aim("Kill", None, true),
            aim("Other", None, true),
            unknown.clone(),
        ];
        assert!(
            also_patching(std::slice::from_ref(&unknown), &[("Second", &theirs)])
                .iter()
                .all(Vec::is_empty)
        );
        assert_eq!(
            aims(&[unknown], &[], 9),
            [(
                "Target named in code".to_string(),
                "prefix, postfix".to_string()
            )]
        );
        let others: [(&str, &[Aim]); 2] = [("Second", &theirs), ("Third", &patched[1..2])];
        let also = also_patching(&patched, &others);
        let named = |also: &[Vec<(&str, Does)>]| {
            also.iter()
                .map(|one| one.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(named(&also), [vec![], vec!["Third"], vec!["Second"]]);
        assert_eq!(
            aims(&patched, &also, 2),
            [
                (
                    "Verse.Pawn.Draw".to_string(),
                    "postfix; also Third (unlikely to clash: only adds after)".to_string()
                ),
                (
                    "Verse.Pawn.Kill".to_string(),
                    "while playing, postfix; also Second (may clash)".to_string()
                ),
                ("1 more".to_string(), "".to_string())
            ]
        );
        let (_tmp, library) = two_mods();
        let mods = &library.active;
        let stated = Rule {
            first: 1,
            then: 0,
            kind: Kind::LoadAfter,
        };
        assert_eq!(
            rule(&stated, mods).to_string(),
            "Old asks to load after Fine"
        );
        let yours = Rule {
            first: 1,
            then: 0,
            kind: Kind::Yours,
        };
        assert_eq!(
            rule(&yours, mods).to_string(),
            "your rule puts Fine before Old"
        );
    }
    #[test]
    fn a_dlc_not_installed_is_said_as_such_with_no_fix_and_one_switched_off_can_be_activated() {
        let tmp = tempfile::tempdir().unwrap();
        let add = |root: &str, folder: &str, about: &str| {
            let at = tmp.path().join(root).join(folder).join("About");
            fs::create_dir_all(&at).unwrap();
            fs::write(at.join("About.xml"), about).unwrap();
            fs::write(at.join("PublishedFileId.txt"), "2941000\n").unwrap();
        };
        add(
            "Data",
            "Royalty",
            "<ModMetaData><packageId>Ludeon.RimWorld.Royalty</packageId><name>Royalty</name></ModMetaData>",
        );
        add(
            "Mods",
            "genes",
            "<ModMetaData><packageId>a.genes</packageId><name>Genes</name><supportedVersions><li>1.6</li></supportedVersions><modDependencies><li><packageId>ludeon.rimworld.biotech</packageId></li><li><packageId>ludeon.rimworld.royalty</packageId></li></modDependencies></ModMetaData>",
        );
        let roots =
            [("Data", Source::Official), ("Mods", Source::Local)].map(|(folder, source)| ModRoot {
                path: tmp.path().join(folder),
                source,
            });
        let library = build(scan(&roots), &["a.genes".to_string()], "1.6");
        let found = troubles(
            &library,
            &library.active,
            Versions::CheckedBut(&HashSet::new()),
            &Names::new(),
        );
        let said: Vec<(bool, String, String, &Fix, &str)> = found[0]
            .iter()
            .map(|t| {
                let (words, short) = (t.words.to_string(), t.short.to_string());
                (t.error, words, short, &t.fix, t.key.as_str())
            })
            .collect();
        assert_eq!(
            said,
            [
                (
                    true,
                    "Needs the Biotech DLC, which is not installed.".to_string(),
                    "Missing the Biotech DLC".to_string(),
                    &Fix::None,
                    "needs:ludeon.rimworld.biotech"
                ),
                (
                    true,
                    "Needs Royalty, which is installed but switched off.".to_string(),
                    "Needs Royalty".to_string(),
                    &Fix::Activate("ludeon.rimworld.royalty".to_string()),
                    "needs:ludeon.rimworld.royalty"
                ),
            ]
        );
    }

    #[test]
    fn an_active_mod_of_an_id_installed_twice_is_warned_which_copy_the_game_loads() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = [
            (tmp.path().join("Mods"), Source::Local),
            (tmp.path().join("workshop"), Source::Workshop),
        ]
        .map(|(path, source)| ModRoot { path, source });
        let about = |id: &str| format!("<ModMetaData><packageId>{id}</packageId></ModMetaData>");
        let add = |at: usize, folder: &str, id: &str| {
            let abouts = roots[at].path.join(folder).join("About");
            fs::create_dir_all(&abouts).unwrap();
            fs::write(abouts.join("About.xml"), about(id)).unwrap();
        };
        add(0, "hats", "Ann.Hats");
        add(1, "1", "ann.hats");
        add(1, "2", "Ann.Roads");
        add(1, "3", "ann.roads");
        add(1, "4", "ann.free");
        let active = [
            "ann.hats".to_string(),
            "ann.hats_steam".to_string(),
            "ann.roads".to_string(),
            "ann.free".to_string(),
            "gone.mod".to_string(),
        ];
        let library = build(scan(&roots), &active, "1.7");
        let found = duplicates(&library, &library.active);
        let said =
            |own: &[Trouble]| -> Vec<String> { own.iter().map(|t| t.words.to_string()).collect() };
        assert_eq!(
            said(&found[0]),
            [
                "Its id ann.hats is installed more than once: 1 (Workshop). For \"ann.hats\" the game loads this copy — a copy in the game's Mods folder comes before a Workshop one."
            ]
        );
        assert_eq!(found[0][0].short.to_string(), "another copy is installed");
        assert_eq!(
            said(&found[1]),
            [
                "Its id ann.hats is installed more than once: hats (Local). The order reaches \"ann.hats\" first: the game loads hats (Local), and reads nothing of this copy."
            ]
        );
        assert_eq!(found[1][0].short.to_string(), "the game loads another copy");
        assert_eq!(
            said(&found[2]),
            [
                "Its id ann.roads is installed more than once: 3 (Workshop). For \"ann.roads\" the game loads this copy — its folder name sorts first in its folder."
            ]
        );
        assert_eq!(found[2][0].short.to_string(), "another copy is installed");
        assert!(found[2][0].fix == Fix::None);
        assert!(found[3].is_empty());
        assert!(found[4].is_empty());
    }

    #[test]
    fn the_copy_named_by_a_second_entry_of_its_id_is_warned_the_game_never_reads_it() {
        let tmp = tempfile::tempdir().unwrap();
        let roots = [
            (tmp.path().join("Mods"), Source::Local),
            (tmp.path().join("workshop"), Source::Workshop),
        ]
        .map(|(path, source)| ModRoot { path, source });
        let about = |id: &str| format!("<ModMetaData><packageId>{id}</packageId></ModMetaData>");
        let add = |at: usize, folder: &str, id: &str| {
            let abouts = roots[at].path.join(folder).join("About");
            fs::create_dir_all(&abouts).unwrap();
            fs::write(abouts.join("About.xml"), about(id)).unwrap();
        };
        add(0, "hats", "Ann.Hats");
        add(1, "1", "ann.hats");
        let active = [
            "ann.hats_steam".to_string(),
            "ann.hats".to_string(),
            "gone.mod".to_string(),
        ];
        let library = build(scan(&roots), &active, "1.7");
        let found = duplicates(&library, &library.active);
        let said =
            |own: &[Trouble]| -> Vec<String> { own.iter().map(|t| t.words.to_string()).collect() };
        assert_eq!(found.len(), 3);
        assert_eq!(
            said(&found[0]),
            [
                "Its id ann.hats is installed more than once: hats (Local). For \"ann.hats_steam\" the game loads this copy — the load order names it with \"_steam\"."
            ]
        );
        assert_eq!(found[0][0].short.to_string(), "another copy is installed");
        assert_eq!(
            said(&found[1]),
            [
                "Its id ann.hats is installed more than once: 1 (Workshop). The order reaches \"ann.hats_steam\" first: the game loads 1 (Workshop), and reads nothing of this copy."
            ]
        );
        assert_eq!(found[1][0].short.to_string(), "the game loads another copy");
        assert!(found[2].is_empty());
    }

    // Inactive duplicates are found with `is:duplicate`; only the active side's rows get
    // this trouble, which `duplicates` is called for.
}
