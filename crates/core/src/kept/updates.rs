use crate::rimworld::{paths::Source, workshop::Page};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    /// The ids the API hides (removed, private or unlisted), with what their page showed when
    /// last looked at; `None` not looked at yet.
    pub hidden: BTreeMap<String, Option<Looked>>,
    pub checkouts: BTreeMap<String, Remote>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Remote {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Looked {
    pub page: Page,
    /// Seconds since 1970.
    pub at: u64,
}

/// A live page is looked at again after this long: the item may be removed since. A gone one
/// is not: it stays gone until the API shows the item again.
pub const LIVE_FOR: u64 = 30 * 24 * 60 * 60;

impl LastCheck {
    /// A missing or damaged file is no check yet.
    pub fn read(file: &Path) -> Self {
        let Some(mut json) = crate::base::atomic_file::read_json(file) else {
            return Self::default();
        };
        // Before pages were kept, a file listed the ids found gone, without the live ones.
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
            kept.hidden.entry(id.to_string()).or_insert(Some(gone));
        }
        kept
    }

    pub fn write(&self, file: &Path) -> Result<(), crate::base::error::Error> {
        let text = serde_json::to_string(self)
            .map_err(|e| crate::base::error::Error::format(format!("{}: {e}", file.display())))?;
        crate::base::atomic_file::write(file, text)
    }

    /// Takes the ids the API hides now. A page is looked at when not yet, or seen live longer
    /// than `LIVE_FOR` ago, oldest first, until `page` gives `None` (refused): the next check
    /// goes on from there. An id no longer hidden drops what its page showed.
    pub fn look_at_hidden(
        &mut self,
        hidden: &[String],
        now: u64,
        mut page: impl FnMut(&str) -> Option<Page>,
    ) {
        let mut kept: BTreeMap<String, Option<Looked>> = hidden
            .iter()
            .map(|id| (id.clone(), self.hidden.get(id).copied().flatten()))
            .collect();
        let mut asked: Vec<(Option<u64>, String)> = kept
            .iter()
            .filter(|(_, looked)| due(looked, now))
            .map(|(id, looked)| (looked.map(|l| l.at), id.clone()))
            .collect();
        asked.sort();
        for (_, id) in asked {
            let Some(seen) = page(&id) else {
                break;
            };
            kept.insert(
                id,
                Some(Looked {
                    page: seen,
                    at: now,
                }),
            );
        }
        self.hidden = kept;
    }

    /// The hidden ids whose page is Steam's error page: removed or private.
    pub fn gone(&self) -> impl Iterator<Item = &String> {
        let gone = self.hidden.iter().filter(|(_, looked)| {
            matches!(
                looked,
                Some(Looked {
                    page: Page::Gone,
                    ..
                })
            )
        });
        gone.map(|(id, _)| id)
    }

    /// The hidden ids whose page is to be looked at on the next check.
    pub fn hidden_unchecked(&self, now: u64) -> usize {
        self.hidden
            .values()
            .filter(|looked| due(looked, now))
            .count()
    }
}

fn due(looked: &Option<Looked>, now: u64) -> bool {
    match looked {
        None => true,
        Some(Looked {
            page: Page::Live,
            at,
        }) => now.saturating_sub(*at) >= LIVE_FOR,
        Some(Looked {
            page: Page::Gone, ..
        }) => false,
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
            hidden: BTreeMap::from([
                (
                    "200".to_string(),
                    Some(Looked {
                        page: Page::Gone,
                        at: 9,
                    }),
                ),
                ("300".to_string(), None),
            ]),
            checkouts: BTreeMap::from([
                (
                    "/mods/a".to_string(),
                    Remote {
                        behind: Some(3),
                        problem: None,
                    },
                ),
                (
                    "/mods/b".to_string(),
                    Remote {
                        behind: None,
                        problem: Some("offline".into()),
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
        assert!(older.hidden.is_empty());
        // Before pages were kept, the ids found gone were listed on their own.
        std::fs::write(&file, r#"{"checked":5,"gone":["200"]}"#).unwrap();
        let older = LastCheck::read(&file);
        assert_eq!(older.gone().collect::<Vec<_>>(), ["200"]);
        assert_eq!(older.hidden_unchecked(5), 0);
    }

    const DAY: u64 = 24 * 60 * 60;

    fn looked(page: Page, at: u64) -> Option<Looked> {
        Some(Looked { page, at })
    }

    #[test]
    fn a_hidden_page_is_looked_at_once_until_stale_and_a_refusal_leaves_the_rest_for_later() {
        let now = 400 * DAY;
        let mut check = LastCheck {
            hidden: BTreeMap::from([
                ("1".to_string(), looked(Page::Live, now - 10 * DAY)),
                ("2".to_string(), looked(Page::Live, now - 31 * DAY)),
                ("3".to_string(), looked(Page::Gone, now - 300 * DAY)),
                ("4".to_string(), None),
                ("6".to_string(), looked(Page::Gone, now - DAY)),
            ]),
            ..LastCheck::default()
        };
        // Steam's pages as had: 4 is gone, 5 unlisted; the second page asked is refused.
        let hidden = ["1", "2", "3", "4", "5"].map(String::from);
        let mut asked = Vec::new();
        check.look_at_hidden(&hidden, now, |id| {
            asked.push(id.to_string());
            (asked.len() == 1).then_some(Page::Gone)
        });
        assert_eq!(
            asked,
            ["4", "5"],
            "the never looked at first; 1 is fresh, 3 stays gone"
        );
        assert_eq!(check.gone().collect::<Vec<_>>(), ["3", "4"]);
        assert!(!check.hidden.contains_key("6"), "answered with result 1");
        assert_eq!(check.hidden["2"], looked(Page::Live, now - 31 * DAY));
        assert_eq!(check.hidden_unchecked(now), 2);

        let later = now + DAY;
        let mut asked = Vec::new();
        check.look_at_hidden(&hidden, later, |id| {
            asked.push(id.to_string());
            Some(Page::Live)
        });
        assert_eq!(asked, ["5", "2"], "the rest, the stale one last");
        assert_eq!(check.hidden["2"], looked(Page::Live, later));
        assert_eq!(check.hidden_unchecked(later), 0);

        // 3 shown by the API again drops its verdict; hidden once more, it is looked at anew.
        check.look_at_hidden(&["1", "2", "4", "5"].map(String::from), later, |_| None);
        assert!(!check.hidden.contains_key("3"));
        check.look_at_hidden(&hidden, later, |id| {
            assert_eq!(id, "3");
            None
        });
        assert_eq!(check.hidden["3"], None);
        assert_eq!(check.gone().collect::<Vec<_>>(), ["4"]);
        // A month on, the live ones are stale too; the gone one never is.
        let mut asked = 0;
        check.look_at_hidden(&hidden, later + 30 * DAY, |_| {
            asked += 1;
            None
        });
        assert_eq!((asked, check.hidden_unchecked(later + 30 * DAY)), (1, 4));
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
}
