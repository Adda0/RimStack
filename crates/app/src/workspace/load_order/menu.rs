use super::details::{Details, FixSaid, NotInstalled};
use super::{LoadOrder, Side};
use crate::App;
use crate::ui::theme::SWATCHES;
use crate::workspace::upkeep::Line;
use rimstack_core::base::sentence::Sentence;
use rimstack_core::mods::library::ModId;
use rimstack_core::mods::list::Row;
use rimstack_core::{say, say_n};
use serde::Serialize;

/// The window keeps one handler for each kind (`doers`).
#[derive(Serialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Action {
    MoveSelected,
    MoveAcross {
        active: bool,
        index: usize,
    },
    OpenFolder {
        path: String,
    },
    OpenUrl {
        url: String,
    },
    CopyText {
        value: String,
    },
    ApplyFix {
        mod_id: Option<ModId>,
        fix: FixSaid,
    },
    Subscribe {
        workshop_id: String,
    },
    CopyToLocal {
        ids: Vec<ModId>,
        ask: Ask,
    },
    Redownload {
        mod_id: ModId,
        ask: Ask,
    },
    /// A copy kept before the Workshop updated the mod, by that update's time.
    RestoreKept {
        mod_id: ModId,
        updated: u64,
        ask: Ask,
    },
    Unsubscribe {
        mod_id: ModId,
    },
    UpdateFromGit {
        mod_id: ModId,
    },
    PutInSub {
        mod_id: ModId,
        tag: String,
    },
    NewSub {
        mod_id: ModId,
    },
    /// One entry, as the tags in use may be many: opens the groups in a menu of their own.
    GroupMenu {
        mod_id: ModId,
    },
    /// One entry, as the swatches are many: opens them in a menu of their own, searchable.
    ColourMenu {
        mod_id: ModId,
        ids: Vec<ModId>,
    },
    Trust {
        ids: Vec<ModId>,
        yes: bool,
    },
    Bin {
        name: String,
        path: String,
        mod_id: ModId,
        kind: crate::workspace::upkeep::BinKind,
        /// DDS textures RimStack made in it, for the question.
        made: usize,
    },
    ToggleSub {
        active: bool,
        row: usize,
    },
    RemoveSub {
        active: bool,
        row: usize,
    },
    /// Paints the mods' rows with a swatch, by name; an empty name takes the colour off.
    Paint {
        ids: Vec<ModId>,
        name: String,
    },
    /// Takes the tag off every mod grouped under it.
    DeleteSub {
        tag: String,
    },
}

#[derive(Serialize)]
pub struct Ask {
    pub title: Sentence,
    pub lines: Vec<Line>,
    pub label: Sentence,
}

fn unset(flag: &bool) -> bool {
    !flag
}

fn empty(icon: &&str) -> bool {
    icon.is_empty()
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Sentence>,
    #[serde(skip_serializing_if = "unset")]
    pub line: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Sentence>,
    #[serde(skip_serializing_if = "empty")]
    pub icon: &'static str,
    #[serde(skip_serializing_if = "unset")]
    pub off: bool,
    #[serde(skip_serializing_if = "unset")]
    pub danger: bool,
    /// The swatch the entry is drawn in, by name.
    #[serde(skip_serializing_if = "empty")]
    pub swatch: &'static str,
    #[serde(skip_serializing_if = "unset")]
    pub ticked: bool,
    /// One of the entries the menu's search field narrows to.
    #[serde(skip_serializing_if = "unset")]
    pub pick: bool,
    #[serde(flatten)]
    pub action: Option<Action>,
}

impl Entry {
    fn item(text: Sentence, icon: &'static str, action: Action) -> Self {
        Entry {
            text: Some(text),
            icon,
            action: Some(action),
            ..Entry::default()
        }
    }

    fn title(text: Sentence) -> Self {
        Entry {
            title: Some(text),
            ..Entry::default()
        }
    }

    fn off(self, off: bool) -> Self {
        Entry { off, ..self }
    }

    fn danger(self) -> Self {
        Entry {
            danger: true,
            ..self
        }
    }

    /// One of the entries the menu's search field narrows to.
    fn pick(self) -> Self {
        Entry { pick: true, ..self }
    }

    /// A swatch, named in its own colour.
    fn swatch(name: &'static str, ticked: bool, action: Action) -> Self {
        Entry {
            swatch: name,
            ticked,
            ..Entry::item(Sentence::said(name, Vec::new()), "", action)
        }
    }

    /// Carries a swatch along, as the row menu's colour entry shows the colour in use.
    fn painted(self, name: &'static str) -> Self {
        Entry {
            swatch: name,
            ..self
        }
    }
}

fn separated(groups: Vec<Vec<Entry>>) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    for group in groups.into_iter().filter(|group| !group.is_empty()) {
        if !entries.is_empty() {
            entries.push(Entry {
                line: true,
                ..Entry::default()
            });
        }
        entries.extend(group);
    }
    entries
}

/// The colour in use as one of the theme's swatch names, empty when it is not one.
fn colour_of(colour: &str) -> &'static str {
    SWATCHES
        .iter()
        .find(|(name, _, _)| *name == colour)
        .map_or("", |(name, _, _)| *name)
}

pub struct Clicked<'a> {
    pub active: bool,
    pub index: usize,
    pub id: Option<ModId>,
    pub selected: &'a [ModId],
}

struct Subject {
    name: String,
    path: String,
    url: String,
    package_id: String,
    take_off: Option<Sentence>,
    fixes: Vec<FixSaid>,
    resubscribe: Vec<String>,
    workshop: bool,
    workshop_page: String,
    steam_page: String,
    repository: String,
    group_choices: Option<Vec<String>>,
    sub: String,
    colour: String,
    version_ok: bool,
    offer_trust: bool,
    can_bin: bool,
    dds_made: usize,
}

impl Subject {
    fn gone(gone: NotInstalled) -> Self {
        Subject {
            name: gone.name,
            package_id: gone.package_id,
            take_off: Some(gone.take_off),
            fixes: gone.fixes,
            resubscribe: gone.resubscribe,
            ..Subject::unknown()
        }
    }

    fn held(about: Details) -> Self {
        let repository = match about.git {
            true => rimstack_core::sources::git::page(std::path::Path::new(&about.path))
                .unwrap_or_default(),
            false => String::new(),
        };
        Subject {
            name: about.name,
            path: about.path,
            url: about.url,
            package_id: about.package_id,
            workshop: about.workshop,
            workshop_page: about.workshop_page.unwrap_or_default(),
            steam_page: about.steam_page.unwrap_or_default(),
            repository,
            group_choices: Some(about.group_choices),
            sub: about.sub,
            colour: about.colour,
            version_ok: about.version_ok,
            offer_trust: about.offer_trust,
            can_bin: about.can_bin,
            dds_made: about.dds_made,
            ..Subject::unknown()
        }
    }

    fn unknown() -> Self {
        Subject {
            name: String::new(),
            path: String::new(),
            url: String::new(),
            package_id: String::new(),
            take_off: None,
            fixes: Vec::new(),
            resubscribe: Vec::new(),
            workshop: false,
            workshop_page: String::new(),
            steam_page: String::new(),
            repository: String::new(),
            group_choices: None,
            sub: String::new(),
            colour: String::new(),
            version_ok: false,
            offer_trust: false,
            can_bin: false,
            dds_made: 0,
        }
    }
}

impl LoadOrder {
    /// The chosen mods whose version warning the player can set aside; none for fewer than two.
    pub fn version_warned(&self, chosen: &[ModId]) -> Vec<ModId> {
        let list = self.list(Side::Active);
        let warned = |id: &&ModId| {
            let own = list
                .position_of(**id)
                .and_then(|at| self.troubles().get(at));
            own.is_some_and(|own| rimstack_core::mods::describe::warned_for_version(own))
        };
        chosen.iter().filter(warned).copied().collect()
    }

    pub fn row_menu(&mut self, app: &App, at: &Clicked) -> Vec<Entry> {
        let side = Side::of(at.active);
        let subject = match self.not_installed(app, side, at.index) {
            Some(gone) => Subject::gone(gone),
            None => at
                .id
                .map(|id| self.details(app, id))
                .map_or_else(Subject::unknown, Subject::held),
        };
        let many = at.selected.len() > 1;
        let warned = if many {
            self.version_warned(at.selected)
        } else {
            Vec::new()
        };
        let mut groups = vec![self.moving(at, &subject, many)];
        groups.push(Self::opening(&subject));
        groups.push(Self::fetching(at, &subject));
        if subject.workshop {
            groups.push(Self::workshop(app, at, &subject));
        } else if !subject.workshop_page.is_empty() {
            groups.push(Self::workshop_page(&subject));
        }
        if !subject.repository.is_empty() {
            groups.push(Self::git(at, &subject));
        }
        groups.push(Self::filing(at, &subject));
        groups.push(Self::colouring(at, &subject));
        groups.push(Self::last(at, &subject, many, &warned));
        separated(groups)
    }

    fn moving(&self, at: &Clicked, subject: &Subject, many: bool) -> Vec<Entry> {
        let count = at.selected.len();
        let text = subject
            .take_off
            .clone()
            .unwrap_or_else(|| match (many, at.active) {
                (true, true) => say_n!(count, "Deactivate {} mod", "Deactivate {} mods"),
                (true, false) => say_n!(count, "Activate {} mod", "Activate {} mods"),
                (false, true) => say!("Deactivate"),
                (false, false) => say!("Activate"),
            });
        let action = if many {
            Action::MoveSelected
        } else {
            Action::MoveAcross {
                active: at.active,
                index: at.index,
            }
        };
        let icon = if at.active { "minus" } else { "plus" };
        vec![Entry::item(text, icon, action).off(self.previewing())]
    }

    fn opening(subject: &Subject) -> Vec<Entry> {
        let mut group = Vec::new();
        if !subject.path.is_empty() {
            let action = Action::OpenFolder {
                path: subject.path.clone(),
            };
            group.push(Entry::item(say!("Open folder"), "folder-open", action));
        }
        if !subject.url.is_empty() {
            let action = Action::OpenUrl {
                url: subject.url.clone(),
            };
            group.push(Entry::item(say!("Mod's own page"), "link", action));
        }
        let copy = |value: &str| Action::CopyText {
            value: value.to_string(),
        };
        group.push(Entry::item(say!("Copy name"), "copy", copy(&subject.name)));
        if !subject.package_id.is_empty() {
            let text = say!("Copy package id");
            group.push(Entry::item(text, "copy", copy(&subject.package_id)));
        }
        group
    }

    fn fetching(at: &Clicked, subject: &Subject) -> Vec<Entry> {
        let mut group = Vec::new();
        for fix in &subject.fixes {
            let action = Action::ApplyFix {
                mod_id: at.id,
                fix: fix.clone(),
            };
            group.push(Entry::item(
                Sentence::named(fix.label),
                "external-link",
                action,
            ));
        }
        for workshop_id in &subject.resubscribe {
            let action = Action::Subscribe {
                workshop_id: workshop_id.clone(),
            };
            group.push(Entry::item(say!("Subscribe again"), "download", action));
        }
        group
    }

    /// A local copy of a Workshop item: its page only; copying, downloading again and
    /// unsubscribing stay with Workshop mods.
    fn workshop_page(subject: &Subject) -> Vec<Entry> {
        let page = subject.workshop_page.clone();
        let open = |url: &str| Action::OpenUrl {
            url: url.to_string(),
        };
        vec![
            Entry::title(say!("Steam Workshop")),
            Entry::item(
                say!("Page in Steam"),
                "external-link",
                open(&subject.steam_page),
            ),
            Entry::item(say!("Page in the browser"), "external-link", open(&page)),
            Entry::item(say!("Copy link"), "copy", Action::CopyText { value: page }),
        ]
    }

    fn workshop(app: &App, at: &Clicked, subject: &Subject) -> Vec<Entry> {
        let page = subject.workshop_page.clone();
        let open = |url: &str| Action::OpenUrl {
            url: url.to_string(),
        };
        let copy = app.copy_offer(at.selected);
        let copying = copy.ids.is_empty();
        let ask = Ask {
            title: copy.title,
            lines: copy.lines,
            label: say!("Copy"),
        };
        let mut group = vec![
            Entry::title(say!("Steam Workshop")),
            Entry::item(
                say!("Page in Steam"),
                "external-link",
                open(&subject.steam_page),
            ),
            Entry::item(say!("Page in the browser"), "external-link", open(&page)),
            Entry::item(
                say!("Copy link"),
                "copy",
                Action::CopyText {
                    value: page.clone(),
                },
            ),
            Entry::item(
                say!("Copy to the game's Mods folder…"),
                "copy",
                Action::CopyToLocal { ids: copy.ids, ask },
            )
            .off(copying),
        ];
        if let Some(id) = at.id {
            if app.steamcmd().is_some() {
                let ask = Ask {
                    title: say!("Download {} again", subject.name),
                    lines: vec![
                        Line::Said(say!(
                            "SteamCMD downloads the mod again, and the new copy replaces this folder:"
                        )),
                        Line::Path {
                            path: subject.path.clone(),
                        },
                    ],
                    label: say!("Download"),
                };
                let action = Action::Redownload { mod_id: id, ask };
                group.push(Entry::item(
                    say!("Download again with SteamCMD…"),
                    "download",
                    action,
                ));
            }
            if !at.active {
                let action = Action::Unsubscribe { mod_id: id };
                group.push(Entry::item(say!("Unsubscribe…"), "minus", action).danger());
            }
            let kept = app.kept_copies_of(id);
            if !kept.is_empty() {
                group.push(Entry::title(say!("Kept copies")));
            }
            let local = app.paths().local_mods().unwrap_or_default();
            for updated in kept {
                let day = crate::workspace::logs::day(updated);
                let ask = Ask {
                    title: say!("Restore the copy of {}", day.clone()),
                    lines: vec![
                        Line::Said(say!(
                            "Kept before a launch the Workshop's update reached. It is copied into the game's Mods folder, where a copy wins over Steam's newer one:"
                        )),
                        Line::Path {
                            path: local.to_string_lossy().into_owned(),
                        },
                    ],
                    label: say!("Restore"),
                };
                let action = Action::RestoreKept {
                    mod_id: id,
                    updated,
                    ask,
                };
                group.push(Entry::item(
                    say!("Restore the copy of {}", day),
                    "history",
                    action,
                ));
            }
        }
        group
    }

    fn git(at: &Clicked, subject: &Subject) -> Vec<Entry> {
        let Some(id) = at.id else {
            return Vec::new();
        };
        let repository = Action::OpenUrl {
            url: subject.repository.clone(),
        };
        vec![
            Entry::title(say!("Git")),
            Entry::item(say!("Repository"), "link", repository),
            Entry::item(
                say!("Update"),
                "download",
                Action::UpdateFromGit { mod_id: id },
            ),
        ]
    }

    /// The row menu's filing group: one entry, as the tags in use may be many. The groups
    /// themselves open in a menu of their own, the same one the About tab's group button
    /// opens.
    fn filing(at: &Clicked, subject: &Subject) -> Vec<Entry> {
        let (Some(id), Some(_)) = (at.id, subject.group_choices.as_ref()) else {
            return Vec::new();
        };
        vec![Entry::item(
            say!("Group under a tag"),
            "tag",
            Action::GroupMenu { mod_id: id },
        )]
    }

    /// The groups beside the mod, a new one, out of its group, and its group deleted for
    /// every mod in it: opened by the row menu's one filing entry and the About tab's
    /// Change button alike.
    pub fn group_menu(&mut self, app: &App, id: ModId) -> Vec<Entry> {
        let subject = Subject::held(self.details(app, id));
        let at = Clicked {
            active: false,
            index: 0,
            id: Some(id),
            selected: &[],
        };
        Self::group_entries(&at, &subject)
    }

    fn group_entries(at: &Clicked, subject: &Subject) -> Vec<Entry> {
        let (Some(id), Some(choices)) = (at.id, &subject.group_choices) else {
            return Vec::new();
        };
        let mut group = vec![Entry::title(say!("Group under a tag"))];
        for tag in choices {
            let action = Action::PutInSub {
                mod_id: id,
                tag: tag.clone(),
            };
            group.push(Entry::item(tag.clone().into(), "tag", action).pick());
        }
        group.push(Entry::item(
            say!("A new tag…"),
            "plus",
            Action::NewSub { mod_id: id },
        ));
        if !subject.sub.is_empty() {
            let action = Action::PutInSub {
                mod_id: id,
                tag: String::new(),
            };
            group.push(Entry::item(
                say!("Remove the tag {}", subject.sub),
                "",
                action,
            ));
            let tag = subject.sub.clone();
            group.push(
                Entry::item(
                    say!("Delete the group {}…", tag),
                    "trash-2",
                    Action::DeleteSub { tag },
                )
                .danger(),
            );
        }
        group
    }

    /// Paints the chosen mods' rows, as RimSort and RimPy users do to spot mods at a glance:
    /// one entry, showing the colour in use, whose menu narrows the swatches as one types.
    fn colouring(at: &Clicked, subject: &Subject) -> Vec<Entry> {
        let (Some(id), true) = (at.id, subject.group_choices.is_some()) else {
            return Vec::new();
        };
        let ids = match at.selected.contains(&id) {
            true => at.selected.to_vec(),
            false => vec![id],
        };
        let item = Entry::item(
            say!("Colour"),
            "palette",
            Action::ColourMenu { mod_id: id, ids },
        );
        match subject.colour.is_empty() {
            true => vec![item],
            false => vec![item.painted(colour_of(&subject.colour))],
        }
    }

    /// The swatches for the chosen mods, one of the entries the menu's search narrows to,
    /// and taking the colour off: opened by the row menu's one colour entry.
    pub fn colour_menu(&mut self, app: &App, id: ModId, ids: &[ModId]) -> Vec<Entry> {
        let subject = Subject::held(self.details(app, id));
        let paint = |name: &str| Action::Paint {
            ids: ids.to_vec(),
            name: name.to_string(),
        };
        let mut group = vec![Entry::title(say!("Colour"))];
        for (name, _, _) in SWATCHES {
            let picked = Entry::swatch(name, name == subject.colour, paint(name)).pick();
            group.push(picked);
        }
        let none = Entry::item(say!("No colour"), "x", paint(""));
        group.push(none.off(subject.colour.is_empty()));
        group
    }

    fn last(at: &Clicked, subject: &Subject, many: bool, warned: &[ModId]) -> Vec<Entry> {
        let mut group = Vec::new();
        let trust = |ids: Vec<ModId>, yes| Action::Trust { ids, yes };
        let one = at.id.into_iter().collect::<Vec<_>>();
        if many && !warned.is_empty() {
            group.push(Entry::item(
                say_n!(
                    warned.len(),
                    "Ignore the game version mismatch of {} mod",
                    "Ignore the game version mismatch of {} mods"
                ),
                "check",
                trust(warned.to_vec(), true),
            ));
        } else if many && subject.version_ok {
            group.push(Entry::item(
                say_n!(
                    at.selected.len(),
                    "Warn about the game version of {} mod again",
                    "Warn about the game version of {} mods again"
                ),
                "",
                trust(at.selected.to_vec(), false),
            ));
        } else if subject.version_ok {
            let text = say!("Warn about its game version again");
            group.push(Entry::item(text, "", trust(one, false)));
        } else if subject.offer_trust {
            let text = say!("Ignore the game version mismatch");
            group.push(Entry::item(text, "check", trust(one, true)));
        }
        if let Some(id) = at.id {
            let kinds = crate::workspace::upkeep::BinKind::offered(
                subject.dds_made,
                subject.workshop,
                subject.can_bin,
            );
            for kind in kinds {
                let action = Action::Bin {
                    name: subject.name.clone(),
                    path: subject.path.clone(),
                    mod_id: id,
                    kind,
                    made: subject.dds_made,
                };
                let (text, icon) = kind.entry();
                group.push(Entry::item(text, icon, action).danger());
            }
        }
        group
    }

    pub fn sub_menu(&self, active: bool, row: usize) -> Vec<Entry> {
        let Some(Row::Sub { collapsed, .. }) = self.list(Side::of(active)).rows().get(row) else {
            return Vec::new();
        };
        let fold = if *collapsed {
            say!("Show its mods")
        } else {
            say!("Hide its mods")
        };
        vec![
            Entry::item(fold, "", Action::ToggleSub { active, row }),
            Entry::item(
                say!("Take this tag off these mods"),
                "x",
                Action::RemoveSub { active, row },
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::installed;

    fn mod_at(root: &std::path::Path, folder: &str, id: &str) {
        let about = root.join(folder).join("About");
        std::fs::create_dir_all(&about).unwrap();
        let xml =
            format!("<ModMetaData><packageId>{id}</packageId><name>{id}</name></ModMetaData>");
        std::fs::write(about.join("About.xml"), xml).unwrap();
    }

    fn id_of(order: &LoadOrder, key: &str) -> (Side, usize, ModId) {
        [Side::Active, Side::Inactive]
            .into_iter()
            .find_map(|side| {
                let at = order.list(side).mods().iter().position(|m| m.id == key)?;
                Some((side, at, order.list(side).mods()[at].installed?))
            })
            .unwrap()
    }

    fn shown(entries: &[Entry]) -> Vec<String> {
        let each = |e: &Entry| match (&e.title, &e.text, e.line) {
            (Some(title), _, _) => format!("# {title}"),
            (_, Some(text), _) => text.to_string(),
            _ => "-".to_string(),
        };
        entries.iter().map(each).collect()
    }

    fn menu(order: &mut LoadOrder, app: &App, key: &str, selected: &[&str]) -> Vec<Entry> {
        let (side, index, id) = id_of(order, key);
        let chosen: Vec<ModId> = selected.iter().map(|k| id_of(order, k).2).collect();
        let at = Clicked {
            active: side == Side::Active,
            index,
            id: Some(id),
            selected: &chosen,
        };
        order.row_menu(app, &at)
    }

    #[test]
    fn a_local_mod_offers_its_folder_a_tag_and_the_bin() {
        let (_tmp, app) = installed();
        let mut order = LoadOrder::read(&app, true, None);
        let entries = menu(&mut order, &app, "ann.roads", &["ann.roads"]);
        let wanted: Vec<String> = [
            "Activate",
            "-",
            "Open folder",
            "Copy name",
            "Copy package id",
            "-",
            "Group under a tag",
            "-",
            "Colour",
            "-",
            "Move to the bin…",
        ]
        .iter()
        .map(|text| text.to_string())
        .collect();
        wanted.extend(SWATCHES.iter().map(|(name, _, _)| name.to_string()));
        wanted.push("No colour".to_string());
        wanted.extend([
            "-".to_string(),
            "Move to the bin…".to_string(),
            "Delete its files, but keep the .dds textures…".to_string(),
        ]);
        assert_eq!(shown(&entries), wanted);
        assert!(entries.last().unwrap().danger);
        assert!(matches!(
            entries[0].action,
            Some(Action::MoveAcross {
                active: false,
                index: 0
            })
        ));
    }

    #[test]
    fn a_workshop_mod_offers_steam_and_can_be_unsubscribed_only_while_inactive() {
        let (tmp, app) = installed();
        mod_at(tmp.path(), "workshop/900002", "Ann.Shop");
        app.rescan();
        let mut order = LoadOrder::read(&app, true, None);
        let entries = menu(&mut order, &app, "ann.shop", &["ann.shop"]);
        let texts = shown(&entries);
        assert!(texts.contains(&"# Steam Workshop".to_string()));
        assert!(texts.contains(&"Page in Steam".to_string()));
        assert!(texts.contains(&"Unsubscribe…".to_string()));
        assert!(!texts.contains(&"Download again with SteamCMD…".to_string()));
        let copy = entries
            .iter()
            .find(|e| matches!(e.action, Some(Action::CopyToLocal { .. })))
            .unwrap();
        assert!(!copy.off);
        let unsubscribe = entries
            .iter()
            .find(|e| matches!(e.action, Some(Action::Unsubscribe { .. })))
            .unwrap();
        assert!(unsubscribe.danger);
    }

    #[test]
    fn a_git_mod_offers_its_repository_and_an_update() {
        let (tmp, app) = installed();
        let root = tmp.path().join("game/Mods/roads");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(
            root.join(".git/config"),
            "[remote \"origin\"]\n\turl = https://github.com/ann/roads.git\n",
        )
        .unwrap();
        app.rescan();
        let mut order = LoadOrder::read(&app, true, None);
        let entries = menu(&mut order, &app, "ann.roads", &["ann.roads"]);
        let texts = shown(&entries);
        let at = texts.iter().position(|t| t == "# Git").unwrap();
        assert_eq!(texts[at + 1..at + 3], ["Repository", "Update"]);
        assert!(matches!(
            &entries[at + 1].action,
            Some(Action::OpenUrl { url }) if url == "https://github.com/ann/roads"
        ));
    }

    #[test]
    fn a_mod_among_several_chosen_moves_them_all() {
        let (tmp, app) = installed();
        mod_at(tmp.path(), "game/Mods/hats", "Ann.Hats");
        app.rescan();
        let mut order = LoadOrder::read(&app, true, None);
        let entries = menu(&mut order, &app, "ann.roads", &["ann.roads", "ann.hats"]);
        assert_eq!(
            entries[0].text.as_ref().unwrap().to_string(),
            "Activate 2 mods"
        );
        assert!(matches!(entries[0].action, Some(Action::MoveSelected)));
    }

    #[test]
    fn a_tag_heading_offers_to_fold_and_to_untag() {
        let (_tmp, app) = installed();
        let mut order = LoadOrder::read(&app, true, None);
        let (_, _, roads) = id_of(&order, "ann.roads");
        app.set_group(&[roads], "Stone");
        order.sync(&app);
        let row = order
            .list(Side::Inactive)
            .rows()
            .iter()
            .position(|row| matches!(row, Row::Sub { .. }))
            .unwrap();
        assert_eq!(
            shown(&order.sub_menu(false, row)),
            ["Hide its mods", "Take this tag off these mods"]
        );
        order.toggle_sub(&app, Side::Inactive, row);
        assert_eq!(shown(&order.sub_menu(false, row))[0], "Show its mods");
        assert!(order.sub_menu(false, 99).is_empty());
    }

    #[test]
    fn the_row_menu_holds_the_groups_in_one_entry() {
        let (tmp, app) = installed();
        mod_at(tmp.path(), "game/Mods/hats", "Ann.Hats");
        app.rescan();
        let mut order = LoadOrder::read(&app, true, None);
        let (_, _, roads) = id_of(&order, "ann.roads");
        app.set_group(&[roads], "Stone");
        order.sync(&app);
        let entries = menu(&mut order, &app, "ann.roads", &["ann.roads"]);
        let texts = shown(&entries);
        assert_eq!(
            texts
                .iter()
                .filter(|t| t.as_str() == "Group under a tag")
                .count(),
            1,
            "one entry, not a title with a Group under for every tag"
        );
        assert!(!texts.contains(&"A new tag…".to_string()));
        let group = entries
            .iter()
            .find(|e| {
                e.text
                    .as_ref()
                    .is_some_and(|t| t.to_string() == "Group under a tag")
            })
            .unwrap();
        assert_eq!(group.icon, "tag");
        let Action::GroupMenu { mod_id } = group.action.as_ref().unwrap() else {
            panic!("opens the groups");
        };
        assert_eq!(*mod_id, roads);
        // The same entries the About tab's Change button opens: the bare tag names, narrowed
        // by a search field. The mod's own tag is offered too: grouping it under Stone again
        // is a no-op that keeps it there.
        let mut wanted = vec!["# Group under a tag".to_string()];
        let choices = order.details(&app, roads).group_choices;
        for tag in &choices {
            wanted.push(tag.clone());
        }
        wanted.extend([
            "A new tag…".to_string(),
            "Remove the tag Stone".to_string(),
            "Delete the group Stone…".to_string(),
        ]);
        assert_eq!(shown(&order.group_menu(&app, roads)), wanted);
        let picks = order
            .group_menu(&app, roads)
            .into_iter()
            .filter(|e| e.pick)
            .map(|e| e.text.unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(picks, choices, "the groups alone are the picks");
    }

    #[test]
    fn a_mods_menu_paints_the_chosen_mods_with_a_swatch() {
        let (_tmp, app) = installed();
        let mut order = LoadOrder::read(&app, true, None);
        let entries = menu(&mut order, &app, "ann.roads", &["ann.roads"]);
        let texts = shown(&entries);
        let at = texts.iter().position(|t| t == "Colour").unwrap();
        let swatches: Vec<String> = SWATCHES
            .iter()
            .map(|(name, _, _)| name.to_string())
            .collect();
        let painting = |entry: &Entry| -> (Vec<ModId>, String) {
            match &entry.action {
                Some(Action::Paint { ids, name }) => (ids.clone(), name.clone()),
                _ => panic!("paints"),
            }
        };
        let (_, _, roads) = id_of(&order, "ann.roads");
        let Some(Action::ColourMenu { mod_id, ids }) = &entries[at].action else {
            panic!("opens the swatches in a menu of their own");
        };
        assert_eq!(
            mod_id, &roads,
            "the colour entry tells whose colour is in use"
        );
        assert_eq!(ids, &vec![roads], "the selected mods, none more");
        assert!(
            entries[at].swatch.is_empty(),
            "no colour in use, no swatch shown"
        );
        let opened = order.colour_menu(&app, *mod_id, ids);
        let texts = shown(&opened);
        assert_eq!(texts[0], "# Colour");
        assert_eq!(
            texts[1..1 + swatches.len()].to_vec(),
            swatches[..],
            "the swatches live in the menu of their own"
        );
        assert!(
            opened[1..1 + swatches.len()].iter().all(|e| e.pick),
            "the swatches alone are narrowed by the search field"
        );
        let none = 1 + swatches.len();
        assert_eq!(texts[none], "No colour");
        assert!(opened[none].off);
        assert!(!opened[none].pick);
        let rose = opened
            .iter()
            .position(|e| e.text.as_ref().is_some_and(|t| t.to_string() == "Rose"))
            .unwrap();
        assert!(!opened[rose].ticked);
        let (ids, name) = painting(&opened[rose]);
        assert_eq!(name, "Rose");
        app.set_colour(&ids, &name);
        order.sync(&app);
        let entries = menu(&mut order, &app, "ann.roads", &["ann.roads"]);
        assert_eq!(order.details(&app, roads).colour, "Rose");
        let colour = entries
            .iter()
            .position(|e| e.text.as_ref().is_some_and(|t| t.to_string() == "Colour"))
            .unwrap();
        assert_eq!(
            entries[colour].swatch, "Rose",
            "the entry shows the colour in use"
        );
        let opened = order.colour_menu(&app, roads, &[roads]);
        let rose = opened
            .iter()
            .position(|e| e.text.as_ref().is_some_and(|t| t.to_string() == "Rose"))
            .unwrap();
        assert!(opened[rose].ticked);
        let none = opened
            .iter()
            .position(|e| {
                e.text
                    .as_ref()
                    .is_some_and(|t| t.to_string() == "No colour")
            })
            .unwrap();
        assert!(!opened[none].off);
        let (ids, name) = painting(&opened[none]);
        assert!(name.is_empty());
        app.set_colour(&ids, &name);
        order.sync(&app);
        assert_eq!(order.details(&app, roads).colour, "");
    }
}
