use crate::rimworld::paths::Source;
use crate::rimworld::workshop::{Details, Page, Version};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

/// Kept in `updates.json`; all empty before the first check.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LastCheck {
    /// Seconds since 1970 when Steam last answered.
    pub checked: Option<u64>,
    pub asked: Option<u64>,
    /// Seconds since 1970 when each Workshop item was last updated, by Workshop id.
    pub now: BTreeMap<String, u64>,
    /// Each one's `hcontent_file`, where Steam gave it; a check before RimStack kept them has none.
    pub manifests: BTreeMap<String, u64>,
    /// Asked and not answered: unlisted by their authors, which Steam answers only to a login.
    pub unanswered: BTreeSet<String>,
    /// What the public page of each `unanswered` id showed, and when; one not looked at yet
    /// has none.
    pub looked: BTreeMap<String, Looked>,
    pub checkouts: BTreeMap<String, Remote>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Remote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    /// Why the last update of the checkout did not apply, as `Sentence::to_kept` writes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Looked {
    pub page: Page,
    /// Seconds since 1970.
    pub at: u64,
}

/// A live page is looked at again after this long: the item may be removed since. A gone one
/// is not: it stays gone until Steam answers for the item again.
pub const LIVE_FOR: u64 = 30 * 24 * 60 * 60;

fn due(looked: Option<&Looked>, now: u64) -> bool {
    match looked {
        None => true,
        Some(Looked {
            page: Page::Gone, ..
        }) => false,
        Some(Looked {
            page: Page::Live,
            at,
        }) => now.saturating_sub(*at) >= LIVE_FOR,
    }
}

impl LastCheck {
    pub fn answered(&mut self, asked: &[String], now: &[Details], at: u64) {
        self.checked = Some(at);
        self.asked = Some(asked.len() as u64);
        self.now = now.iter().map(|d| (d.id.clone(), d.updated)).collect();
        self.manifests = now
            .iter()
            .filter(|d| d.manifest != 0)
            .map(|d| (d.id.clone(), d.manifest))
            .collect();
        let answered: BTreeSet<&String> = now.iter().map(|d| &d.id).collect();
        let unanswered = asked.iter().filter(|id| !answered.contains(id));
        self.unanswered = unanswered.cloned().collect();
    }

    pub fn offered(&self) -> HashMap<String, Version> {
        let version = |(id, &updated): (&String, &u64)| {
            let manifest = self.manifests.get(id).copied().unwrap_or(0);
            (id.clone(), Version { manifest, updated })
        };
        self.now.iter().map(version).collect()
    }

    /// A missing or damaged file is no check yet.
    pub fn read(file: &Path) -> Self {
        let Some(mut json) = crate::base::atomic_file::read_json(file) else {
            return Self::default();
        };
        // Before verdicts were kept, a file listed the ids found gone, without the live ones.
        let older = json.get_mut("gone").map(serde_json::Value::take);
        let mut kept: Self = serde_json::from_value(json).unwrap_or_default();
        let ids = older.as_ref().and_then(serde_json::Value::as_array);
        for id in ids
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
        {
            let gone = Looked {
                page: Page::Gone,
                at: kept.checked.unwrap_or(0),
            };
            kept.looked.entry(id.to_string()).or_insert(gone);
        }
        kept
    }

    /// Looks at the pages of the unanswered ids not looked at yet, or seen live longer than
    /// `LIVE_FOR` ago, oldest first, until `page` gives `None` (refused): the next check goes
    /// on from there. An id Steam answers again drops what its page showed.
    pub fn look_at_hidden(&mut self, now: u64, mut page: impl FnMut(&str) -> Option<Page>) {
        let unanswered = &self.unanswered;
        self.looked.retain(|id, _| unanswered.contains(id));
        let mut asked: Vec<(Option<u64>, &String)> = (unanswered.iter())
            .filter(|id| due(self.looked.get(*id), now))
            .map(|id| (self.looked.get(id).map(|l| l.at), id))
            .collect();
        asked.sort();
        let mut seen = Vec::new();
        for (_, id) in asked {
            let Some(page) = page(id) else {
                break;
            };
            seen.push((id.clone(), Looked { page, at: now }));
        }
        self.looked.extend(seen);
    }

    /// The unanswered ids whose page is Steam's error page: removed or private.
    pub fn gone(&self) -> impl Iterator<Item = &String> {
        self.looked
            .iter()
            .filter(|(id, looked)| looked.page == Page::Gone && self.unanswered.contains(*id))
            .map(|(id, _)| id)
    }

    /// The unanswered ids whose page is to be looked at on the next check.
    pub fn hidden_unchecked(&self, now: u64) -> usize {
        let due = |id: &&String| due(self.looked.get(*id), now);
        self.unanswered.iter().filter(due).count()
    }

    /// A refusal stays while the remote is as far ahead as when it was refused:
    /// an update tried again would meet it again.
    pub fn keep_refusals(&mut self, before: &BTreeMap<String, Remote>) {
        for (folder, now) in &mut self.checkouts {
            let Some(then) = before.get(folder) else {
                continue;
            };
            if then.refused.is_some() && then.behind == now.behind {
                now.refused.clone_from(&then.refused);
            }
        }
    }

    pub fn write(&self, file: &Path) -> Result<(), crate::base::error::Error> {
        crate::base::atomic_file::write_json(file, self)
    }
}

/// Workshop folders belong to Steam, so a checkout there is asked only on request.
pub fn asks_git(source: Source, checkout: bool, git_for_workshop: bool) -> bool {
    checkout
        && match source {
            Source::Local => true,
            Source::Workshop => git_for_workshop,
            Source::Official => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_is_kept_and_read_back_and_an_older_file_still_reads() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("updates.json");
        assert_eq!(LastCheck::read(&file), LastCheck::default());
        let check = LastCheck {
            checked: Some(10),
            asked: Some(2),
            now: BTreeMap::from([("100".to_string(), 7)]),
            manifests: BTreeMap::from([("100".to_string(), 70)]),
            unanswered: BTreeSet::from(["200".to_string(), "300".to_string()]),
            looked: BTreeMap::from([(
                "300".to_string(),
                Looked {
                    page: Page::Gone,
                    at: 9,
                },
            )]),
            checkouts: BTreeMap::from([
                (
                    "/mods/a".to_string(),
                    Remote {
                        behind: Some(3),
                        problem: None,
                        refused: Some("local changes".into()),
                    },
                ),
                (
                    "/mods/b".to_string(),
                    Remote {
                        behind: None,
                        problem: Some("offline".into()),
                        refused: None,
                    },
                ),
            ]),
        };
        check.write(&file).unwrap();
        assert_eq!(LastCheck::read(&file), check);
        std::fs::write(
            &file,
            r#"{"checked":5,"asked":1,"now":{"9":4},"checkouts":{"/m":{"behind":0}}}"#,
        )
        .unwrap();
        let older = LastCheck::read(&file);
        assert_eq!(
            (older.checked, older.now["9"], older.checkouts["/m"].behind),
            (Some(5), 4, Some(0))
        );
        assert_eq!(
            older.offered()["9"],
            Version {
                manifest: 0,
                updated: 4
            }
        );
    }

    #[test]
    fn an_answer_keeps_each_version_and_what_steam_said_nothing_of() {
        let details = |id: &str, manifest| Details {
            id: id.to_string(),
            updated: 5,
            manifest,
            ..Details::default()
        };
        let asked = ["1".to_string(), "2".to_string(), "3".to_string()];
        let mut check = LastCheck::default();
        check.answered(&asked, &[details("1", 11), details("2", 0)], 99);
        assert_eq!((check.checked, check.asked), (Some(99), Some(3)));
        let offered = check.offered();
        assert_eq!(
            offered["1"],
            Version {
                manifest: 11,
                updated: 5
            }
        );
        assert_eq!(offered["2"].manifest, 0);
        assert_eq!(check.unanswered, BTreeSet::from(["3".to_string()]));
    }

    #[test]
    fn a_refusal_lasts_until_the_remote_moves() {
        let refused = |behind| Remote {
            behind: Some(behind),
            problem: None,
            refused: Some("local changes".into()),
        };
        let before = BTreeMap::from([
            ("/same".to_string(), refused(2)),
            ("/moved".to_string(), refused(2)),
        ]);
        let fresh = |behind| Remote {
            behind: Some(behind),
            ..Remote::default()
        };
        let mut check = LastCheck {
            checkouts: BTreeMap::from([
                ("/same".to_string(), fresh(2)),
                ("/moved".to_string(), fresh(3)),
                ("/new".to_string(), fresh(1)),
            ]),
            ..LastCheck::default()
        };
        check.keep_refusals(&before);
        assert_eq!(check.checkouts["/same"], refused(2));
        assert_eq!(check.checkouts["/moved"].refused, None);
        assert_eq!(check.checkouts["/new"].refused, None);
    }

    #[test]
    fn a_local_checkout_is_asked_and_a_workshop_one_only_on_request() {
        assert!(asks_git(Source::Local, true, false));
        assert!(!asks_git(Source::Workshop, true, false));
        assert!(asks_git(Source::Workshop, true, true));
    }

    #[test]
    fn nothing_but_a_checkout_of_a_mod_is_asked() {
        for on in [false, true] {
            assert!(!asks_git(Source::Local, false, on));
            assert!(!asks_git(Source::Workshop, false, on));
            assert!(!asks_git(Source::Official, true, on));
        }
    }

    #[test]
    fn older_files_with_only_the_gone_ids_still_load() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("updates.json");
        std::fs::write(
            &file,
            r#"{"checked":5,"unanswered":["200"],"gone":["200"]}"#,
        )
        .unwrap();
        let older = LastCheck::read(&file);
        assert_eq!(older.gone().collect::<Vec<_>>(), ["200"]);
        assert_eq!(older.hidden_unchecked(5), 0);
    }

    const DAY: u64 = 24 * 60 * 60;

    fn looked(page: Page, at: u64) -> Looked {
        Looked { page, at }
    }

    #[test]
    fn a_hidden_page_is_looked_at_once_until_stale_and_a_refusal_leaves_the_rest_for_later() {
        let now = 400 * DAY;
        let mut check = LastCheck {
            unanswered: ["1", "2", "3", "4", "5"].map(String::from).into(),
            looked: BTreeMap::from([
                ("1".to_string(), looked(Page::Live, now - 10 * DAY)),
                ("2".to_string(), looked(Page::Live, now - 31 * DAY)),
                ("3".to_string(), looked(Page::Gone, now - 300 * DAY)),
                // Answered with result 1 since: its verdict goes.
                ("6".to_string(), looked(Page::Gone, now - DAY)),
            ]),
            ..LastCheck::default()
        };
        // Steam's pages as had: 4 is gone, 5 unlisted; the second page asked is refused.
        let mut asked = Vec::new();
        check.look_at_hidden(now, |id| {
            asked.push(id.to_string());
            (asked.len() == 1).then_some(Page::Gone)
        });
        assert_eq!(
            asked,
            ["4", "5"],
            "the never looked at first; 1 is fresh, 3 stays gone"
        );
        assert_eq!(check.gone().collect::<Vec<_>>(), ["3", "4"]);
        assert!(!check.looked.contains_key("6"));
        assert_eq!(check.looked["2"], looked(Page::Live, now - 31 * DAY));
        assert_eq!(check.hidden_unchecked(now), 2);
        let later = now + DAY;
        let mut asked = Vec::new();
        check.look_at_hidden(later, |id| {
            asked.push(id.to_string());
            Some(Page::Live)
        });
        assert_eq!(asked, ["5", "2"], "the rest, the stale one last");
        assert_eq!(check.looked["2"], looked(Page::Live, later));
        assert_eq!(check.hidden_unchecked(later), 0);
        // A month on, the live ones are stale too: the gone one never is.
        assert_eq!(check.hidden_unchecked(later + 30 * DAY), 3);
    }
}
