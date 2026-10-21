use super::App;
use rimstack_core::base::elapsed::ago;
use rimstack_core::base::error::Error;
use rimstack_core::base::locks::Shared;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::kept::settings::Switch;
use rimstack_core::kept::steamcmd::Downloaded;
use rimstack_core::kept::updates::{LastCheck, Remote};
use rimstack_core::mods::library::{Installed, Library, ModId};
use rimstack_core::rimworld::{paths::Source, workshop};
use rimstack_core::say;
use rimstack_core::sources::downloads::{self, Asked, Behind, Replaceable, Repository};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Checkout {
    pub mod_id: ModId,
    pub name: String,
    pub workshop: bool,
    pub checked: bool,
    pub behind: Option<u64>,
    pub problem: Option<Sentence>,
}

impl Checkout {
    fn is_behind(&self) -> bool {
        self.behind.is_some_and(|n| n > 0) || self.problem.is_some() || !self.checked
    }
}

#[derive(Default)]
pub struct Updates {
    pub checked: Option<u64>,
    pub asked: Option<u64>,
    pub answered: usize,
    pub items: Vec<Behind>,
    /// The SteamCMD-downloaded copies the Workshop has newer: SteamCMD updates these, not Steam.
    pub steamcmd: Vec<Behind>,
    pub checkouts: Vec<Checkout>,
    pub problem: Option<Sentence>,
    /// The ids the API hides whose page is still to be looked at.
    pub hidden_unchecked: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BehindShown<'a> {
    #[serde(flatten)]
    pub row: &'a Behind,
    pub asked: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesShown<'a> {
    pub checked: Option<u64>,
    pub checked_said: Sentence,
    pub problem: Option<&'a Sentence>,
    pub count: usize,
    pub active: Vec<BehindShown<'a>>,
    pub inactive: Vec<BehindShown<'a>>,
    pub steamcmd: Vec<BehindShown<'a>>,
    pub checkouts: &'a [Checkout],
    pub checkouts_behind: Vec<&'a Checkout>,
    pub hidden_unchecked: usize,
}

pub fn updates_shown<'a>(updates: &'a Updates, asked: &HashSet<String>) -> UpdatesShown<'a> {
    let shown = |active: bool| {
        let rows = updates.items.iter().filter(move |row| row.active == active);
        rows.map(|row| BehindShown {
            row,
            asked: asked.contains(&row.id),
        })
        .collect()
    };
    UpdatesShown {
        checked: updates.checked,
        checked_said: match updates.checked {
            Some(at) => say!(
                "Checked {}",
                ago(super::now_secs().saturating_sub(at)).inside()
            ),
            None => say!("Never checked"),
        },
        problem: updates.problem.as_ref(),
        count: updates.items.len(),
        active: shown(true),
        inactive: shown(false),
        steamcmd: updates
            .steamcmd
            .iter()
            .map(|row| BehindShown { row, asked: false })
            .collect(),
        checkouts: &updates.checkouts,
        checkouts_behind: updates.checkouts.iter().filter(|c| c.is_behind()).collect(),
        hidden_unchecked: updates.hidden_unchecked,
    }
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Queue {
    pub items: Vec<Asked>,
    pub repositories: Vec<Repository>,
    pub note: Option<Sentence>,
    pub wanted: Vec<String>,
}

impl App {
    fn checked_file(&self) -> PathBuf {
        self.own_dir().join("updates.json")
    }

    pub(crate) fn steamcmd_file(&self) -> PathBuf {
        self.data_dir().join("steamcmd.json")
    }

    /// The mods SteamCMD put in the game's Mods folder, and when the Workshop's copy was last
    /// no newer. Recorded on download: nothing is walked to find out.
    pub fn steamcmd_downloaded(&self) -> Downloaded {
        Downloaded::read(&self.steamcmd_file())
    }

    pub fn record_steamcmd_downloads(&self, ids: &[String]) {
        let now = super::now_secs();
        let mut kept = self.steamcmd_downloaded();
        for id in ids {
            kept.at.insert(id.clone(), now);
        }
        if let Err(problem) = kept.write(&self.steamcmd_file()) {
            rimstack_core::problem!("{problem}");
        }
    }

    pub fn last_check(&self) -> LastCheck {
        LastCheck::read(&self.checked_file())
    }

    fn keep_check(&self, check: &LastCheck) {
        if let Err(problem) = check.write(&self.checked_file()) {
            rimstack_core::problem!("{problem}");
        }
    }

    /// Last time's answer stays when Steam is not reached.
    pub fn check_updates(&self) -> Result<Updates, Error> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let mut kept = self.last_check();
        let downloaded = self.steamcmd_downloaded();
        let mut ids: HashSet<String> = library
            .workshop_mods()
            .keys()
            .map(|id| id.to_string())
            .collect();
        ids.extend(downloaded.at.keys().cloned());
        let ids: Vec<String> = ids.into_iter().collect();
        let problem = match workshop::fetch_details(&ids) {
            Ok(now) => {
                let at = super::now_secs();
                kept.checked = Some(at);
                kept.asked = Some(ids.len() as u64);
                kept.now = now
                    .items
                    .iter()
                    .map(|d| (d.id.clone(), d.updated))
                    .collect();
                kept.look_at_hidden(&now.hidden, at, workshop::pages());
                None
            }
            Err(problem) => Some(problem.sentence().clone()),
        };
        kept.checkouts = self.checkouts_asked(library);
        self.keep_check(&kept);
        let mut report = self.updates_of(library, &kept);
        report.problem = problem;
        Ok(report)
    }

    /// `None` if there was none.
    pub fn kept_updates(&self) -> Option<Updates> {
        let kept = self.last_check();
        if kept == LastCheck::default() {
            return None;
        }
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().ok()?;
        Some(self.updates_of(library, &kept))
    }

    pub fn checkout_updated(&self, folder: &Path) {
        let mut kept = self.last_check();
        let caught_up = Remote {
            behind: Some(0),
            problem: None,
        };
        let folder = folder.to_string_lossy().into_owned();
        kept.checkouts.insert(folder, caught_up);
        self.keep_check(&kept);
    }

    fn updates_of(&self, library: &Library, kept: &LastCheck) -> Updates {
        let on_disk = &self.steam_record().0;
        let now: HashMap<String, u64> = kept.now.clone().into_iter().collect();
        let active = self.active_now(library);
        let items = downloads::items(library, &active);
        let behind = downloads::behind(&items, on_disk, &now);
        let downloaded = self.steamcmd_downloaded();
        let fetched: HashMap<String, u64> = downloaded.at.clone().into_iter().collect();
        let copies = downloads::steamcmd_items(library, &active, &fetched);
        let steamcmd = downloads::behind(&copies, &fetched, &now);
        if kept.checked.is_some() {
            *self.behind().writing() = Some(
                behind
                    .iter()
                    .chain(&steamcmd)
                    .map(|row| row.id.clone())
                    .collect(),
            );
            *self.removed().writing() = Some(kept.gone().cloned().collect());
        }
        Updates {
            checked: kept.checked,
            asked: kept.asked,
            answered: now.len(),
            items: behind,
            steamcmd,
            checkouts: self.checkouts_of(library, &kept.checkouts),
            problem: None,
            hidden_unchecked: kept.hidden_unchecked(super::now_secs()),
        }
    }

    fn asked_of_git<'a>(&self, library: &'a Library) -> Vec<(ModId, &'a Installed)> {
        let git_for_workshop = self.settings().switch(Switch::GitForWorkshop);
        downloads::checkouts(library, git_for_workshop).collect()
    }

    fn checkouts_asked(&self, library: &Library) -> BTreeMap<String, Remote> {
        use rayon::prelude::*;
        let asked = self.asked_of_git(library);
        let answers: Vec<(String, Remote)> = asked
            .par_iter()
            .map(|(_, m)| {
                let said = match rimstack_core::sources::git::behind(&m.path) {
                    Ok(count) => Remote {
                        behind: Some(count as u64),
                        problem: None,
                    },
                    // Kept as a sentence, put in words when it is shown.
                    Err(problem) => Remote {
                        behind: None,
                        problem: Some(problem.sentence().to_kept().to_string()),
                    },
                };
                (m.path.to_string_lossy().into_owned(), said)
            })
            .collect();
        answers.into_iter().collect()
    }

    fn checkouts_of(&self, library: &Library, answers: &BTreeMap<String, Remote>) -> Vec<Checkout> {
        let asked = self.asked_of_git(library).into_iter();
        asked
            .map(|(id, m)| {
                let said = answers.get(m.path.to_string_lossy().as_ref());
                Checkout {
                    mod_id: id,
                    name: m.display_name().to_string(),
                    workshop: m.source == Source::Workshop,
                    checked: said.is_some(),
                    behind: said.and_then(|s| s.behind),
                    problem: said.and_then(|s| s.problem.as_deref().map(Sentence::from_kept)),
                }
            })
            .collect()
    }

    pub fn look_up(&self, pasted: &str) -> Result<Queue, Error> {
        let mut ids = workshop::ids_in(pasted);
        if let Some(pairs) = rimstack_core::mods::share::decode(pasted) {
            for (_, id) in pairs {
                if let Some(id) = id
                    && !ids.contains(&id)
                {
                    ids.push(id);
                }
            }
        }
        let local = self.paths().local_mods();
        let repositories = downloads::repositories(pasted, local.as_deref());
        if ids.is_empty() {
            let note = match repositories.is_empty() {
                true => say!("No Workshop link, mod id or Git link found"),
                false => Sentence::default(),
            };
            return Ok(Queue {
                repositories,
                note: Some(note),
                ..Queue::default()
            });
        }
        let ids = workshop::expand_collections(&ids)?;
        let answered = workshop::fetch_details(&ids)?.items;
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().map_err(Clone::clone)?;
        let mods = library.workshop_mods();
        let items = downloads::asked(&ids, &answered, &|id| mods.contains_key(id));
        let wanted = items
            .iter()
            .filter(|item| item.title.is_some() && !item.installed)
            .map(|item| item.id.clone())
            .collect();
        Ok(Queue {
            items,
            repositories,
            note: None,
            wanted,
        })
    }

    /// The user's own Steam WebAPI key, kept in RimStack's settings.
    pub fn steam_key(&self) -> Option<String> {
        self.settings().get("steamKey").map(String::from)
    }

    /// Blocks on the network; the key goes only to api.steampowered.com.
    pub fn search_workshop(&self, text: &str) -> Result<Vec<workshop::Found>, Error> {
        let Some(key) = self.steam_key() else {
            return Err(Error::refused(say!("no Steam WebAPI key kept")));
        };
        workshop::query_files(&key, text.trim(), 1)
    }

    pub fn replaceable(&self) -> Vec<Replaceable> {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return Vec::new();
        };
        let active = self.active_now(library);
        let items = downloads::items(library, &active);
        downloads::replaceable(&items, &self.replacements(), &self.replacements_ignored())
    }

    /// The load order with each active mod that has an installed replacement swapped for it,
    /// and how many were; `None` when there is none to swap.
    pub fn with_replacements(&self) -> Option<(Vec<String>, usize)> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().ok()?;
        let key = |id: ModId| library.get(id)?.key();
        let swaps: Vec<(String, String)> = self
            .replaceable()
            .into_iter()
            .filter(|row| row.active)
            .filter_map(|row| Some((key(row.mod_id)?, key(row.new_mod_id?)?)))
            .collect();
        if swaps.is_empty() {
            return None;
        }
        let list = downloads::swapped(self.session().current(), &swaps);
        Some((list, swaps.len()))
    }

    /// With `None`, every one set aside is suggested again.
    pub fn ignore_replacement(&self, id: Option<ModId>) -> Result<(), Error> {
        let mut ignored: Vec<String> = match id {
            Some(_) => self.replacements_ignored().into_iter().collect(),
            None => Vec::new(),
        };
        if let Some(id) = id.and_then(|id| self.workshop_id(id)) {
            ignored.push(id);
        }
        ignored.sort();
        ignored.dedup();
        self.set_setting("replacementsIgnored", &ignored.join(","))?;
        self.publish(crate::changes::Changes::SETTINGS);
        Ok(())
    }

    /// A Workshop mod's id: the name of its folder.
    pub fn workshop_id(&self, id: ModId) -> Option<String> {
        let loaded = self.library();
        let (library, _) = loaded.as_ref().as_ref().ok()?;
        let m = library.get(id).filter(|m| m.source == Source::Workshop)?;
        Some(m.folder.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    #[test]
    fn a_local_checkout_is_listed_by_its_mod_with_what_its_remote_said_last() {
        let (tmp, app) = installed();
        let roads = tmp.path().join("game/Mods/roads");
        std::fs::create_dir(roads.join(".git")).unwrap();
        app.rescan();
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let id = library.ids().find(|(_, m)| m.folder == "roads").unwrap().0;

        let report = app.updates_of(library, &LastCheck::default());
        assert_eq!(report.checkouts.len(), 1);
        let row = &report.checkouts[0];
        assert_eq!((row.mod_id, row.checked, row.behind), (id, false, None));
        let shown = updates_shown(&report, &HashSet::new());
        assert_eq!(shown.checkouts_behind.len(), 1, "not asked yet");

        let mut kept = LastCheck::default();
        let remote = Remote {
            behind: Some(2),
            problem: None,
        };
        kept.checkouts
            .insert(roads.to_string_lossy().into_owned(), remote);
        let report = app.updates_of(library, &kept);
        let row = &report.checkouts[0];
        assert_eq!(
            (row.checked, row.behind, row.workshop),
            (true, Some(2), false)
        );
        assert_eq!(crate::qml::values::as_shown(&row)["modId"], id.to_qml());
        drop(loaded);

        app.checkout_updated(&roads);
        let caught_up = app.kept_updates().unwrap();
        assert_eq!(caught_up.checkouts[0].behind, Some(0));
        assert!(
            updates_shown(&caught_up, &HashSet::new())
                .checkouts_behind
                .is_empty()
        );
    }

    /// Only the SteamCMD record tells its downloads apart from copies made by hand, so an
    /// update can never overwrite a copy the user made for a purpose of their own.
    #[test]
    fn a_steamcmd_download_is_behind_when_the_workshop_is_newer_than_its_download() {
        let (tmp, app) = installed();
        let mods = tmp.path().join("game/Mods");
        let hand = mods.join("800/About");
        std::fs::create_dir_all(&hand).unwrap();
        std::fs::write(
            hand.join("About.xml"),
            "<ModMetaData><packageId>Jo.Hats</packageId><name>Hats</name></ModMetaData>",
        )
        .unwrap();
        std::fs::write(hand.join("PublishedFileId.txt"), "800\n").unwrap();
        let fetched = mods.join("700/About");
        std::fs::create_dir_all(&fetched).unwrap();
        std::fs::write(
            fetched.join("About.xml"),
            "<ModMetaData><packageId>Jo.Roads</packageId><name>Roads</name></ModMetaData>",
        )
        .unwrap();
        std::fs::write(fetched.join("PublishedFileId.txt"), "700\n").unwrap();
        app.rescan();
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let later = super::super::now_secs() + 100;
        let said = LastCheck {
            now: BTreeMap::from([("700".to_string(), later), ("800".to_string(), later)]),
            ..LastCheck::default()
        };
        assert!(app.updates_of(library, &said).steamcmd.is_empty());

        app.record_steamcmd_downloads(&["700".to_string(), "999".to_string()]);
        let before = super::super::now_secs() - 100;
        assert_eq!(
            app.updates_of(library, &said).steamcmd.len(),
            1,
            "the copy made by hand is not the record's to update, and 999 is not installed"
        );
        let up = LastCheck {
            now: BTreeMap::from([("700".to_string(), before)]),
            ..LastCheck::default()
        };
        assert!(
            app.updates_of(library, &up).steamcmd.is_empty(),
            "the record was made after the Workshop's update"
        );
    }

    #[test]
    fn a_local_mod_has_no_workshop_id_and_ignoring_none_clears_the_list() {
        let (_tmp, app) = installed();
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let roads = library.ids().find(|(_, m)| m.folder == "roads").unwrap().0;
        drop(loaded);
        assert_eq!(app.workshop_id(roads), None);
        app.set_setting("replacementsIgnored", "1,2").unwrap();
        app.ignore_replacement(Some(roads)).unwrap();
        assert_eq!(app.replacements_ignored().len(), 2);
        app.ignore_replacement(None).unwrap();
        assert!(app.replacements_ignored().is_empty());
    }
}
