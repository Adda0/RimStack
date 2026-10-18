use super::App;
use crate::changes::Changes;
use rimstack_core::base::locks::Shared;
use rimstack_core::kept::notes::{self, Note};
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::Category;

#[derive(serde::Serialize)]
pub struct TagOffer {
    tag: String,
    text: String,
}

impl App {
    /// An id from an earlier reading changes nothing; so does a change that leaves every note as it was.
    pub fn change_notes(
        &self,
        ids: &[ModId],
        changes: Changes,
        change: impl Fn(&mut Note),
    ) -> bool {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return false;
        };
        let keys: Vec<String> = ids
            .iter()
            .filter_map(|&id| library.get(id)?.key())
            .collect();
        drop(loaded);
        self.change_notes_of(&keys, changes, change)
    }

    fn change_notes_of(
        &self,
        keys: &[String],
        changes: Changes,
        change: impl Fn(&mut Note),
    ) -> bool {
        if keys.is_empty() {
            return false;
        }
        let mut all = self.notes().writing();
        let mut differs = false;
        for key in keys {
            let before = all.get(key).cloned().unwrap_or_default();
            let mut note = before.clone();
            change(&mut note);
            differs |= note != before;
            all.set(key, note);
        }
        if !differs {
            return false;
        }
        let written = all.write(&self.notes_file());
        drop(all);
        if let Err(problem) = written {
            rimstack_core::problem!("{problem}");
        }
        self.publish(changes);
        true
    }

    pub fn set_note(&self, id: ModId, tags: &str, text: &str) -> bool {
        let tags = notes::tags_in(tags);
        let text = text.trim().to_string();
        self.change_notes(&[id], Changes::NOTES, |note| {
            note.tags = tags.clone();
            note.text = text.clone();
        })
    }

    pub fn set_note_text(&self, id: ModId, text: &str) -> bool {
        let text = text.trim().to_string();
        self.change_notes(&[id], Changes::NOTES, |note| note.text = text.clone())
    }

    /// Each tag of the comma-separated `text` the mod has in no case yet goes last.
    pub fn add_tags(&self, id: ModId, text: &str) -> bool {
        let tags = notes::tags_in(text);
        self.change_notes(&[id], Changes::NOTES, |note| {
            for tag in &tags {
                if !note.tags.iter().any(|had| had.eq_ignore_ascii_case(tag)) {
                    note.tags.push(tag.clone());
                }
            }
        })
    }

    pub fn clear_tags(&self, id: ModId) -> bool {
        self.change_notes(&[id], Changes::NOTES, |note| note.tags.clear())
    }

    pub fn add_tag(&self, ids: &[ModId], tag: &str) {
        let tag = tag.trim().to_string();
        if tag.is_empty() {
            return;
        }
        self.change_notes(ids, Changes::NOTES, |note| {
            if !note.tags.contains(&tag) {
                note.tags.push(tag.clone());
            }
        });
    }

    /// An empty name takes them out.
    pub fn set_group(&self, ids: &[ModId], name: &str) -> bool {
        let name = name.trim().to_string();
        self.change_notes(ids, Changes::NOTES, |note| note.set_group(&name))
    }

    pub fn remove_tag(&self, ids: &[ModId], tag: &str) -> bool {
        self.change_notes(ids, Changes::NOTES, |note| note.remove_tag(tag))
    }

    /// For the tag typed after the last comma of `text`: each tag to offer, from those not in
    /// `had`, with the whole text that adds it.
    pub fn tag_offers(&self, had: &str, text: &str) -> Vec<TagOffer> {
        let before = match text.rfind(',') {
            Some(at) => format!("{} ", &text[..=at]),
            None => String::new(),
        };
        let had = notes::tags_in(had);
        let tags = self.notes().reading().tags_for(text, &had, 8);
        tags.into_iter()
            .map(|tag| TagOffer {
                text: format!("{before}{tag}"),
                tag,
            })
            .collect()
    }

    /// An empty or unknown label leaves it to RimStack.
    pub fn set_category(&self, id: ModId, label: &str) {
        let known = Category::from_label(label)
            .map_or("", Category::key)
            .to_string();
        self.change_notes(&[id], Changes::NOTES, |note| note.category = known.clone());
    }

    pub fn set_version_ok(&self, ids: &[ModId], ok: bool) {
        let changes = Changes::NOTES | Changes::PROBLEMS;
        self.change_notes(ids, changes, |note| note.version_ok = ok);
    }

    /// `key` as `Trouble::key` gives it.
    pub fn ignore_problem(&self, package_id: &str, key: &str, ignore: bool) {
        let changes = Changes::NOTES | Changes::PROBLEMS;
        self.change_notes_of(&[package_id.to_string()], changes, |note| {
            note.ignored.retain(|k| k != key);
            if ignore {
                note.ignored.push(key.to_string());
            }
        });
    }

    pub fn warn_again(&self, ids: &[ModId]) {
        let changes = Changes::NOTES | Changes::PROBLEMS;
        self.change_notes(ids, changes, |note| note.ignored.clear());
    }

    /// A rule of your own: `then` loads after `first`.
    pub fn change_rule(&self, first: ModId, then: ModId, add: bool) {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return;
        };
        let key = |id: ModId| library.get(id)?.key();
        let (Some(first), Some(then)) = (key(first), key(then)) else {
            return;
        };
        let mut yours = self.user_rules().writing();
        let changed = match add {
            true => yours.add(&first, &then),
            false => yours.remove(&first, &then),
        };
        if !changed {
            return;
        }
        let written = yours.write(&self.user_rules_file());
        drop(yours);
        if let Err(problem) = written {
            rimstack_core::problem!("{problem}");
        }
        self.publish(Changes::RULES | Changes::PROBLEMS);
    }
}

#[cfg(test)]
mod tests {
    use crate::changes::Changes;
    use crate::workspace::tests::installed;
    use rimstack_core::base::locks::Shared;
    use rimstack_core::mods::library::ModId;
    use std::sync::{Arc, Mutex};

    fn roads(app: &crate::App) -> ModId {
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();

        library.ids().find(|(_, m)| m.folder == "roads").unwrap().0
    }

    #[test]
    fn a_note_is_kept_on_disk_and_told() {
        let (_tmp, app) = installed();
        let heard = Arc::new(Mutex::new(Changes::NONE));
        let into = heard.clone();
        app.listen(move |changes| *into.lock().unwrap() |= changes);
        let id = roads(&app);
        app.set_note(id, "travel, maps", " far ");
        app.add_tag(&[id], "travel");
        let kept = rimstack_core::kept::notes::Notes::read(&app.notes_file());
        let note = kept.get("ann.roads").unwrap();
        assert_eq!(
            (note.tags.clone(), note.text.as_str()),
            (vec!["travel".to_string(), "maps".to_string()], "far")
        );
        assert!(heard.lock().unwrap().contains(Changes::NOTES));
    }

    #[test]
    fn tags_are_added_once_in_any_case_offered_only_when_missing_and_cleared() {
        let (_tmp, app) = installed();
        let id = roads(&app);
        let core = {
            let loaded = app.library();
            let (library, _) = loaded.as_ref().as_ref().unwrap();
            library.ids().find(|(_, m)| m.folder == "Core").unwrap().0
        };
        app.set_note(core, "trade, Travel", "");
        app.set_note(id, "travel", "far");
        assert!(app.add_tags(id, "TRAVEL, maps , "));
        assert!(!app.add_tags(id, "Maps"));
        let note = || app.notes().reading().get("ann.roads").cloned().unwrap();
        assert_eq!(note().tags, ["travel", "maps"]);
        let offered: Vec<String> = app
            .tag_offers("travel, maps", "x, T")
            .into_iter()
            .map(|o| o.text)
            .collect();
        assert_eq!(offered, ["x, trade"]);
        assert!(app.set_note_text(id, "near"));
        assert!(app.clear_tags(id));
        assert_eq!((note().tags.len(), note().text), (0, "near".to_string()));
    }

    #[test]
    fn an_id_from_an_earlier_reading_changes_nothing() {
        let (_tmp, app) = installed();
        app.set_note(ModId::new(0, 99), "lost", "lost");
        assert!(app.notes().reading().is_empty());
    }

    #[test]
    fn a_rule_of_your_own_is_added_once_and_removed() {
        let (_tmp, app) = installed();
        let loaded = app.library();
        let (library, _) = loaded.as_ref().as_ref().unwrap();
        let core = library.ids().find(|(_, m)| m.folder == "Core").unwrap().0;
        let id = roads(&app);
        app.change_rule(core, id, true);
        app.change_rule(core, id, true);
        assert_eq!(app.user_rules().reading().len(), 1);
        app.change_rule(core, id, false);
        assert_eq!(app.user_rules().reading().len(), 0);
    }
}
