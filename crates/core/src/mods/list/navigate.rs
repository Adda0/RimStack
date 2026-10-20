use super::{GroupedList, Row};
use crate::mods::library::ModId;

impl GroupedList {
    pub fn installed_on(&self, row: usize) -> Option<ModId> {
        match self.rows.get(row)? {
            Row::Mod { index } => self.mods[*index].installed,
            _ => None,
        }
    }

    /// `from` may be one above the first row or one below the last.
    pub fn step(&self, from: isize, by: isize) -> Option<usize> {
        let mut row = from.checked_add(by)?;
        while let Some(at) = usize::try_from(row).ok().filter(|at| *at < self.rows.len()) {
            if self.installed_on(at).is_some() {
                return Some(at);
            }
            row = row.checked_add(by)?;
        }
        None
    }

    pub fn installed_between(&self, a: usize, b: usize) -> Vec<ModId> {
        let rows = a.min(b)..=a.max(b);
        rows.filter_map(|row| self.installed_on(row)).collect()
    }

    /// Every installed mod with a row, top to bottom.
    pub fn shown_installed(&self) -> Vec<ModId> {
        (0..self.rows.len())
            .filter_map(|row| self.installed_on(row))
            .collect()
    }

    pub fn beside(&self, row: usize) -> Option<ModId> {
        let near = [
            row.checked_add(1),
            row.checked_sub(1),
            row.checked_add(2),
            row.checked_sub(2),
        ];
        near.into_iter()
            .flatten()
            .find_map(|at| self.installed_on(at))
    }

    pub fn position_of(&self, id: ModId) -> Option<usize> {
        self.mods.iter().position(|m| m.installed == Some(id))
    }

    /// Nothing when it is filtered out or under a folded heading.
    pub fn row_of(&self, position: usize) -> Option<usize> {
        let shows = |row: &Row| matches!(row, Row::Mod { index } if *index == position);
        self.rows.iter().position(shows)
    }

    fn row_of_mod(&self, id: ModId) -> Option<usize> {
        self.row_of(self.position_of(id)?)
    }

    /// More than one row lands on the nearest installed mod short of an end, so a page
    /// or a whole list's length goes as far as there is to go.
    pub fn step_from(&self, id: Option<ModId>, by: isize) -> Option<usize> {
        let last = self.rows.len().checked_sub(1)? as isize;
        let at_end = if by > 0 { -1 } else { last + 1 };
        let from = id.and_then(|id| self.row_of_mod(id));
        let to = from
            .map_or(at_end, |row| row as isize)
            .saturating_add(by)
            .clamp(0, last);
        let ahead = if by < 0 { -1 } else { 1 };
        self.step(to - ahead, ahead)
            .or_else(|| self.step(to + ahead, -ahead))
    }

    pub fn beside_mod(&self, id: ModId) -> Option<ModId> {
        self.beside(self.row_of_mod(id)?)
    }

    pub fn others_through(&self, from: ModId, to: ModId) -> Option<Vec<ModId>> {
        let (a, b) = (self.row_of_mod(from)?, self.row_of_mod(to)?);
        let between = self.installed_between(a, b).into_iter();
        Some(between.filter(|id| *id != from).collect())
    }

    /// Nothing unless both are in the list.
    pub fn in_load_order(&self, a: ModId, b: ModId) -> Option<(ModId, ModId)> {
        match self.position_of(a)? < self.position_of(b)? {
            true => Some((a, b)),
            false => Some((b, a)),
        }
    }

    pub fn with_keys(
        &self,
        keys: &[String],
        key_of: &dyn Fn(ModId) -> Option<String>,
    ) -> Vec<ModId> {
        let keyed: Vec<(ModId, String)> = self
            .mods
            .iter()
            .filter_map(|m| m.installed)
            .filter_map(|id| Some((id, key_of(id)?)))
            .collect();
        let named = |key: &String| keyed.iter().find(|(_, k)| k == key).map(|(id, _)| *id);
        keys.iter().filter_map(named).collect()
    }

    pub fn below(&self, id: ModId, other: ModId) -> Option<(usize, usize)> {
        let (at, theirs) = (self.position_of(id)?, self.position_of(other)?);
        (theirs > at).then_some((at, theirs))
    }

    /// Nothing when that would leave the list.
    pub fn nudged(&self, id: ModId, by: isize) -> Option<(usize, usize)> {
        let at = self.position_of(id)?;
        let to = at
            .checked_add_signed(by)
            .filter(|to| *to < self.mods.len())?;
        Some((at, to))
    }
}
