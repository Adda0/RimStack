use crate::App;
use rimstack_core::base::locks::Shared;
use rimstack_core::kept::settings::Switch;
use rimstack_core::mods::describe::{self, Trouble};
use rimstack_core::mods::infer;
use rimstack_core::mods::library::{Library, ModId};
use rimstack_core::mods::list::Mod;
use rimstack_core::mods::ordering::{self, Sources};
use rimstack_core::mods::rules::Rule;
use rimstack_core::mods::sort;
use rimstack_core::rimworld::{loadfolders, paths::Source};
use std::collections::HashSet;

type Kept = (Vec<Option<ModId>>, Vec<ModId>, Vec<infer::Inferred>);

#[derive(Default, Clone)]
pub struct Detected {
    kept: Option<Kept>,
}

/// More than this freezes the window.
const MOST_READ_NOW: usize = 3;

impl Detected {
    /// Empty until the active mods' files were first read.
    pub fn of(&mut self, app: &App, library: &Library, mods: &[Mod]) -> Vec<infer::Inferred> {
        let known = app.conflicts().reading();
        let Some(read) = known.as_ref() else {
            return Vec::new();
        };
        let list: Vec<Option<ModId>> = mods.iter().map(|m| m.installed).collect();
        if let Some((for_list, for_read, rules)) = &self.kept {
            if *for_list == list && *for_read == read.members {
                return rules.clone();
            }
        }
        let active: HashSet<String> = mods
            .iter()
            .filter_map(|m| library.get(m.installed?)?.key())
            .collect();
        let installed = |m: &Mod| m.installed.and_then(|i| library.get(i));
        let mut read_now: Vec<(usize, infer::Scanned)> = Vec::new();
        for (at, m) in mods.iter().enumerate() {
            let unread = m.installed.is_some_and(|i| !read.scanned.contains_key(&i));
            if let (true, Some(found), true) =
                (unread, installed(m), read_now.len() < MOST_READ_NOW)
            {
                let loaded = loadfolders::folders(&found.path, &library.game_version, &|k| {
                    active.contains(k)
                });
                read_now.push((at, infer::scan(&loaded)));
            }
        }
        let nothing = infer::Scanned::default();
        let scanned: Vec<&infer::Scanned> = mods
            .iter()
            .enumerate()
            .map(|(at, m)| {
                let fresh = read_now
                    .iter()
                    .find(|(place, _)| *place == at)
                    .map(|(_, s)| s);
                let kept = m.installed.and_then(|i| read.scanned.get(&i));
                fresh.or(kept).unwrap_or(&nothing)
            })
            .collect();
        let names: Vec<String> = mods.iter().map(|m| m.name.clone()).collect();
        let official: Vec<bool> = mods
            .iter()
            .map(|m| installed(m).is_some_and(|i| i.source == Source::Official))
            .collect();
        let roots: Vec<std::path::PathBuf> = mods
            .iter()
            .map(|m| installed(m).map(|i| i.path.clone()).unwrap_or_default())
            .collect();
        let rules = infer::infer(&scanned, &names, &official, &roots);
        if read_now.is_empty() {
            self.kept = Some((list, read.members.clone(), rules.clone()));
        }
        rules
    }
}

fn with_sources<T>(
    app: &App,
    from_files: &[infer::Inferred],
    with: impl FnOnce(Sources) -> T,
) -> T {
    let yours = app.user_rules().reading();
    let community = app.community();
    let settings = app.settings();
    with(Sources {
        yours: &yours,
        community: (*community)
            .as_ref()
            .filter(|_| settings.switch(Switch::CommunityRules)),
        from_files,
        hints: settings.switch(Switch::FileHints),
    })
}

pub fn all_rules(app: &App, detected: &mut Detected, library: &Library, mods: &[Mod]) -> Vec<Rule> {
    let from_files = detected.of(app, library, mods);
    with_sources(app, &from_files, |sources| {
        ordering::all_rules(library, mods, sources)
    })
}

pub fn sorted_by_category(
    app: &App,
    detected: &mut Detected,
    library: &Library,
    mods: Vec<Mod>,
) -> (Vec<Mod>, sort::Sorted) {
    let from_files = detected.of(app, library, &mods);
    with_sources(app, &from_files, |sources| {
        ordering::sorted_by_category(library, mods, sources)
    })
}

pub fn troubles(app: &App, library: &Library, mods: &[Mod]) -> (Vec<Vec<Trouble>>, Vec<bool>) {
    let excused: HashSet<String> = app
        .notes()
        .reading()
        .version_ok()
        .map(String::from)
        .collect();
    let mut trusted = (*app.works_on()).clone();
    trusted.extend(excused.iter().cloned());
    let off = {
        let settings = app.settings();
        !rimstack_core::mods::checks::version_warnings_shown(
            !settings.switch(Switch::VersionWarnings),
            settings.get("versionAsideFor"),
            &library.game_version,
        )
    };
    let versions = match off {
        true => describe::Versions::Ignored,
        false => describe::Versions::CheckedBut(&trusted),
    };
    // The community's word is not an ignored warning; yours is.
    let by_you = match off {
        true => describe::Versions::Ignored,
        false => describe::Versions::CheckedBut(&excused),
    };
    let workshop = app.workshop_names();
    (
        describe::troubles(library, mods, versions, &workshop),
        describe::version_ignored(library, mods, by_you),
    )
}
