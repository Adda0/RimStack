use super::App;
use crate::changes::Changes;
use rimstack_core::base::locks::Shared;
use rimstack_core::kept::foreign::{self, Kept};
use rimstack_core::kept::notes::{self, Note};
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::Category;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(serde::Serialize)]
pub struct TagOffer {
    tag: String,
    text: String,
}

/// The swatches of `Theme.qml`, by name, with their colour on a dark theme.
const SWATCHES: [(&str, &str); 13] = [
    ("Violet", "#9a8fc2"),
    ("Slate", "#8d99a6"),
    ("Blue", "#6fa3c7"),
    ("Green", "#86b06b"),
    ("Amber", "#d2a94e"),
    ("Coral", "#c98277"),
    ("Teal", "#7fb8b0"),
    ("Sand", "#c9a36a"),
    ("Grey", "#a3a3ad"),
    ("Rose", "#d58fb3"),
    ("Lime", "#b5c95a"),
    ("Sky", "#7cc4e8"),
    ("Orange", "#e09a5c"),
];

/// The swatch nearest a colour another manager painted with, so it reads in every theme.
/// Black, RimPy's way of painting nothing, and greys are none.
fn nearest_swatch(colour: &str) -> Option<&'static str> {
    let [r, g, b] = foreign::rgb(colour)?.map(i32::from);
    if r.max(g).max(b) - r.min(g).min(b) < 24 {
        return None;
    }
    let distance = |(_, hex): &&(&str, &str)| {
        let [sr, sg, sb] = foreign::rgb(hex).unwrap_or_default().map(i32::from);
        (r - sr).pow(2) + (g - sg).pow(2) + (b - sb).pow(2)
    };
    SWATCHES.iter().min_by_key(distance).map(|(name, _)| *name)
}

/// Tags not here yet go last, a note not here yet goes below this one, a colour only where there is none.
fn merge(note: &mut Note, kept: &Kept) {
    for tag in &kept.tags {
        if !note.tags.iter().any(|had| had.eq_ignore_ascii_case(tag)) {
            note.tags.push(tag.clone());
        }
    }
    if !kept.note.is_empty() && !note.text.contains(&kept.note) {
        note.text = match note.text.trim().is_empty() {
            true => kept.note.clone(),
            false => format!("{}\n\n{}", note.text.trim_end(), kept.note),
        };
    }
    if note.colour.is_empty() {
        note.colour = nearest_swatch(&kept.colour).unwrap_or_default().to_string();
    }
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

    /// An empty name takes the colour off.
    pub fn set_colour(&self, ids: &[ModId], name: &str) -> bool {
        let name = name.trim().to_string();
        self.change_notes(ids, Changes::NOTES, |note| note.colour = name.clone())
    }

    pub fn set_pinned(&self, ids: &[ModId], on: bool) -> bool {
        self.change_notes(ids, Changes::NOTES, |note| note.pinned = on)
    }

    /// How many mods sit under the group `name`.
    pub fn group_size(&self, name: &str) -> usize {
        self.notes().reading().in_group(name).len()
    }

    /// Takes the tag `name` off every mod grouped under it; they stay where they are.
    pub fn delete_group(&self, name: &str) -> bool {
        let keys = self.notes().reading().in_group(name);
        self.change_notes_of(&keys, Changes::NOTES, |note| note.remove_tag(name))
    }

    /// Takes over the tags, notes and colours another manager kept, matched by mod folder;
    /// nothing kept here is lost. Gives how many mods got something new.
    pub fn adopt(&self, kept: &[Kept]) -> usize {
        let loaded = self.library();
        let Ok((library, _)) = &*loaded else {
            return 0;
        };
        let real = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mut by_path: HashMap<PathBuf, String> = HashMap::new();
        // A folder name, as another machine's path still ends in it; none when two mods share it.
        let mut by_folder: HashMap<&str, Option<String>> = HashMap::new();
        for m in &library.installed {
            let Some(key) = m.key() else { continue };
            by_path.insert(real(&m.path), key.clone());
            let shared = by_folder.insert(m.folder.as_str(), Some(key)).is_some();
            if shared {
                by_folder.insert(m.folder.as_str(), None);
            }
        }
        let key_of = |k: &Kept| {
            let folder = || {
                by_folder
                    .get(k.folder.file_name()?.to_str()?)
                    .cloned()
                    .flatten()
            };
            by_path.get(&real(&k.folder)).cloned().or_else(folder)
        };
        let found: Vec<(String, &Kept)> =
            kept.iter().filter_map(|k| Some((key_of(k)?, k))).collect();
        drop(loaded);
        let mut all = self.notes().writing();
        let mut changed = 0;
        for (key, kept) in found {
            let before = all.get(&key).cloned().unwrap_or_default();
            let mut note = before.clone();
            merge(&mut note, kept);
            if note != before {
                all.set(&key, note);
                changed += 1;
            }
        }
        if changed > 0 {
            if let Err(problem) = all.write(&self.notes_file()) {
                rimstack_core::problem!("{problem}");
            }
            drop(all);
            self.publish(Changes::NOTES);
        }
        changed
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

    #[test]
    fn another_manager_s_tags_notes_and_colours_are_added_to_what_is_here() {
        use rimstack_core::kept::foreign::Kept;
        let (tmp, app) = installed();
        let id = roads(&app);
        app.set_note(id, "travel", "far");
        let kept = |folder: std::path::PathBuf, tags: &[&str], note: &str, colour: &str| Kept {
            folder,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            note: note.into(),
            colour: colour.into(),
        };
        let found = [
            // By its path here, and by its folder name from another machine.
            kept(
                tmp.path().join("game/Mods/roads"),
                &["TRAVEL", "must"],
                "breaks saves",
                "#ff0000",
            ),
            kept("/elsewhere/Core".into(), &["core"], "", "#000000"),
            kept("/elsewhere/gone".into(), &["lost"], "", ""),
        ];
        assert_eq!(app.adopt(&found), 2);
        let notes = rimstack_core::kept::notes::Notes::read(&app.notes_file());
        let roads = notes.get("ann.roads").unwrap();
        assert_eq!(roads.tags, ["travel", "must"]);
        assert_eq!(roads.text, "far\n\nbreaks saves");
        assert_eq!(roads.colour, "Orange");
        let core = notes.get("ludeon.rimworld").unwrap();
        assert_eq!(
            (core.tags.clone(), core.colour.as_str()),
            (vec!["core".to_string()], "")
        );
        assert_eq!(app.adopt(&found), 0, "taken over once");
    }

    #[test]
    fn the_swatches_are_those_of_the_theme() {
        let theme = include_str!("../../qml/Theme.qml");
        for (name, dark) in super::SWATCHES {
            assert!(
                theme.contains(&format!("[\"{name}\", \"{dark}\"")),
                "{name} {dark}"
            );
        }
        let listed = theme
            .split("swatches: [")
            .nth(1)
            .unwrap()
            .split(']')
            .count();
        assert!(
            listed > super::SWATCHES.len(),
            "a swatch of Theme.qml is missing here"
        );
    }
}
