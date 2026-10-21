use serde::Serialize;
use std::path::Path;

/// Milliseconds since 1970, or 0 when nothing is there. A folder's time moves
/// when an entry is added or removed.
pub fn changed_at(path: &Path) -> u64 {
    let written = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let since = written.and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok());
    since.map_or(0, |d| d.as_millis() as u64)
}

pub fn newest<'a>(paths: impl IntoIterator<Item = &'a Path>) -> u64 {
    paths.into_iter().map(changed_at).max().unwrap_or(0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stamps {
    pub mods: u64,
    pub saves: u64,
    pub log: u64,
    /// 0 while it holds the load order last saved or read here.
    pub mods_config: u64,
    pub theme: u64,
    pub language: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Noticed {
    pub mods: bool,
    pub saves: bool,
    pub log: bool,
    pub mods_config: bool,
    /// The list changed while the game ran, or right after a launch from here.
    pub mods_config_by_game: bool,
    pub theme: bool,
    pub language: bool,
}

/// Seconds after a launch from here when a list is still taken as the game's own:
/// a fast start can end before a look sees the process.
const LAUNCH_WINDOW: u64 = 120;

/// What a look is told about the game.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Game {
    pub running: bool,
    /// Seconds since 1970 when RimStack last started it; 0 for never.
    pub launched: u64,
    /// Seconds since 1970 now.
    pub now: u64,
}

impl Game {
    /// The game rewrites ModsConfig.xml as it starts, and may be gone again
    /// before a look finds its process.
    fn owns_list(&self, seen_running: bool) -> bool {
        let recent = self.launched > 0 && self.now.saturating_sub(self.launched) <= LAUNCH_WINDOW;
        self.running || seen_running || recent
    }
}

/// The first look reports nothing.
#[derive(Debug, Default)]
pub struct Watcher {
    before: Option<Stamps>,
    mods_pending: bool,
    /// An earlier look found the game running: a list seen after it stops is its own.
    game_seen: bool,
}

impl Watcher {
    pub fn look(&mut self, mut now: Stamps, game: Game, in_the_way: bool) -> Noticed {
        let Some(before) = self.before else {
            self.before = Some(now);
            self.game_seen = game.running;
            return Noticed::default();
        };
        let mut noticed = Noticed::default();
        // Steam keeps writing its record while it downloads: wait until a look finds it quiet.
        if now.mods != before.mods {
            self.mods_pending = true;
        } else if self.mods_pending && !in_the_way {
            self.mods_pending = false;
            noticed.mods = true;
        }
        noticed.saves = now.saves != before.saves;
        noticed.theme = now.theme != before.theme;
        noticed.language = now.language != before.language;
        // The game writes its log while it runs and ModsConfig.xml as it starts:
        // both wait until it stops. The list is then the game's own, and still when
        // a fast start ends before any look sees the process: two minutes count from
        // the launch RimStack recorded. Only then is it named a write from outside.
        if now.log != before.log {
            match game.running {
                true => now.log = before.log,
                false => noticed.log = true,
            }
        }
        if now.mods_config != before.mods_config && now.mods_config != 0 {
            match game.running {
                true => now.mods_config = before.mods_config,
                false => {
                    noticed.mods_config = true;
                    noticed.mods_config_by_game = game.owns_list(self.game_seen);
                }
            }
        }
        self.before = Some(now);
        self.game_seen = game.running;
        noticed
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn at(mods: u64, log: u64, mods_config: u64) -> Stamps {
        Stamps {
            mods,
            log,
            mods_config,
            ..Stamps::default()
        }
    }

    fn game(running: bool, launched: u64, now: u64) -> Game {
        Game {
            running,
            launched,
            now,
        }
    }

    #[test]
    fn the_first_look_only_takes_note_and_an_unchanged_one_finds_nothing() {
        let mut watcher = Watcher::default();
        assert_eq!(
            watcher.look(at(1, 1, 0), Game::default(), false),
            Noticed::default()
        );
        assert_eq!(
            watcher.look(at(1, 1, 0), Game::default(), false),
            Noticed::default()
        );
    }

    #[test]
    fn changed_mods_are_read_once_quiet_for_a_look_and_nothing_is_in_the_way() {
        let mut watcher = Watcher::default();
        watcher.look(at(1, 0, 0), Game::default(), false);
        assert!(!watcher.look(at(2, 0, 0), Game::default(), false).mods);
        assert!(!watcher.look(at(3, 0, 0), Game::default(), false).mods);
        assert!(!watcher.look(at(3, 0, 0), Game::default(), true).mods);
        assert!(watcher.look(at(3, 0, 0), Game::default(), false).mods);
        assert!(!watcher.look(at(3, 0, 0), Game::default(), false).mods);
    }

    #[test]
    fn the_log_and_the_list_written_while_the_game_runs_wait_until_it_stops() {
        let mut watcher = Watcher::default();
        watcher.look(at(0, 10, 0), Game::default(), false);
        assert!(!watcher.look(at(0, 11, 5), game(true, 0, 0), false).log);
        assert!(
            !watcher
                .look(at(0, 12, 5), game(true, 0, 0), false)
                .mods_config
        );
        let stopped = watcher.look(at(0, 12, 5), game(false, 0, 0), false);
        assert!(stopped.log && stopped.mods_config);
        assert!(stopped.mods_config_by_game);
        assert_eq!(
            watcher.look(at(0, 12, 5), Game::default(), false),
            Noticed::default()
        );
    }

    #[test]
    fn a_list_rewritten_right_after_a_launch_is_the_game_s_own_once_it_is_gone() {
        let mut watcher = Watcher::default();
        // Started a moment ago, exited before any look saw its process.
        watcher.look(at(0, 0, 5), game(false, 3_000, 3_001), false);
        let noticed = watcher.look(at(0, 0, 9), game(false, 3_000, 3_008), false);
        assert!(noticed.mods_config && noticed.mods_config_by_game);
    }

    #[test]
    fn past_two_minutes_a_launch_no_longer_owns_a_write_to_the_list() {
        let mut watcher = Watcher::default();
        watcher.look(at(0, 0, 5), game(false, 3_000, 3_001), false);
        let noticed = watcher.look(at(0, 0, 9), game(false, 3_000, 3_121), false);
        assert!(noticed.mods_config);
        assert!(!noticed.mods_config_by_game);
    }

    #[test]
    fn a_list_written_with_no_launch_and_no_game_running_is_outside() {
        let mut watcher = Watcher::default();
        watcher.look(at(0, 0, 5), game(false, 0, 3_000), false);
        let noticed = watcher.look(at(0, 0, 9), game(false, 0, 3_002), false);
        assert!(noticed.mods_config);
        assert!(!noticed.mods_config_by_game);
    }

    #[test]
    fn a_list_back_to_the_saved_one_is_no_change_to_offer() {
        let mut watcher = Watcher::default();
        watcher.look(at(0, 0, 5), Game::default(), false);
        assert!(
            !watcher
                .look(at(0, 0, 0), Game::default(), false)
                .mods_config
        );
        assert!(
            watcher
                .look(at(0, 0, 9), Game::default(), false)
                .mods_config
        );
    }

    #[test]
    fn saves_the_theme_and_the_language_are_noticed_at_once() {
        let mut watcher = Watcher::default();
        watcher.look(Stamps::default(), game(true, 0, 0), true);
        let changed = Stamps {
            saves: 1,
            theme: 2,
            language: 3,
            ..Stamps::default()
        };
        let noticed = watcher.look(changed, game(true, 0, 0), true);
        assert!(noticed.saves && noticed.theme && noticed.language);
    }

    #[test]
    fn a_missing_file_has_no_time_and_a_new_entry_changes_its_folder() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(changed_at(&tmp.path().join("none")), 0);
        let before = changed_at(tmp.path());
        assert!(before > 0);
        // File times can be coarse: the sleep makes the new entry's time later.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::create_dir(tmp.path().join("mod")).unwrap();
        let after = changed_at(tmp.path());
        assert!(after > before);
        assert_eq!(
            newest([tmp.path(), tmp.path().join("none").as_path()]),
            after
        );
        assert_eq!(newest([]), 0);
    }
}
