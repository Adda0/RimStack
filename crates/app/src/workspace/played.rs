use super::App;
use rimstack_core::base::locks::Shared;
use rimstack_core::kept::{lists, orders};
use rimstack_core::rimworld::id::split_steam;
use rimstack_core::rimworld::saves;
use std::collections::HashSet;
use std::sync::Arc;

/// The ids a save, a saved list or the kept order history names, folded as
/// the game folds ids.
fn read_played(app: &App) -> HashSet<String> {
    let folded = |ids: Vec<String>| -> Vec<String> {
        ids.into_iter().map(|id| split_steam(&id).0).collect()
    };
    let mut named: HashSet<String> = app
        .saves_dir()
        .into_iter()
        .flat_map(|dir| saves::list(&dir))
        .filter_map(|save| save.header.ok())
        .flat_map(|header| folded(header.mod_ids))
        .collect();
    let lists_dir = app.lists_dir();
    for list in lists::all(&lists_dir) {
        if let Ok(ids) = lists::load(&lists_dir, &list.name) {
            named.extend(folded(ids));
        }
    }
    for entry in orders::read(&app.orders_file()) {
        named.extend(folded(entry.ids));
    }
    named
}

impl App {
    /// Read on the first ask, again only once saves or saved lists change.
    pub fn played(&self) -> Arc<HashSet<String>> {
        if let Some(found) = self.played.reading().as_ref().cloned() {
            return found;
        }
        let found = Arc::new(read_played(self));
        *self.played.writing() = Some(found.clone());
        found
    }

    /// Saves or saved lists or the order history on disk changed.
    pub fn forget_played(&self) {
        *self.played.writing() = None;
    }
}

#[cfg(test)]
mod tests {
    use crate::workspace::tests::installed;

    fn save(dir: &std::path::Path, name: &str, ids: &[&str]) {
        std::fs::create_dir_all(dir).unwrap();
        let ids = ids
            .iter()
            .map(|id| format!("<li>{id}</li>"))
            .collect::<String>();
        std::fs::write(
            dir.join(format!("{name}.rws")),
            format!("<savegame><meta><modIds>{ids}</modIds></meta></savegame>"),
        )
        .unwrap();
    }

    #[test]
    fn the_mods_of_saves_saved_lists_and_kept_orders_count_as_played() {
        let (tmp, app) = installed();
        save(
            &tmp.path().join("config/Saves"),
            "old",
            &["ludeon.rimworld", "ann.roads_steam"],
        );
        let played = app.played();
        assert_eq!(played.len(), 2, "{played:?}");
        assert!(played.contains("ann.roads") && played.contains("ludeon.rimworld"));

        app.keep_list("mine", Some(vec!["ann.hats".to_string()]), false)
            .unwrap();
        let played = app.played();
        assert!(played.contains("ann.hats") && played.contains("ann.roads"));

        // A damaged save's header counts nothing, but reads as no save at all.
        std::fs::write(tmp.path().join("config/Saves/broken.rws"), "not a save").unwrap();
        app.publish(crate::changes::Changes::SAVES);
        let played = app.played();
        assert!(played.contains("ann.hats") && !played.contains("not a save"));
    }
}
