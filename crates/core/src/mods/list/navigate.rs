use super::{GroupedList, Row};
use crate::mods::library::ModId;

impl GroupedList {
    pub fn installed_on(&self, row: usize) -> Option<ModId> {
        match self.rows.get(row)? {
            Row::Mod { index } => self.mods[*index].installed,
            _ => None,
        }
    }

    /// The mod a row stands for: its own, or the first one a folded heading hides.
    pub fn stop_on(&self, row: usize) -> Option<ModId> {
        self.installed_on(row).or_else(|| self.hidden_under(row))
    }

    fn hidden_under(&self, row: usize) -> Option<ModId> {
        let (category, sub, from) = match *self.rows.get(row)? {
            Row::Header {
                category,
                collapsed: true,
                ..
            } => {
                let before = self.rows[..row]
                    .iter()
                    .rev()
                    .find_map(|shown| match *shown {
                        Row::Mod { index } | Row::Sub { first: index, .. } => Some(index),
                        Row::Header { .. } => None,
                    });
                (category, None, before.unwrap_or(0))
            }
            Row::Sub {
                first,
                collapsed: true,
                ..
            } => (
                self.mods[first].category,
                Some(&self.mods[first].sub),
                first,
            ),
            _ => return None,
        };
        let under = |(at, m): &(usize, &super::Mod)| {
            self.matches(*at) && sub.is_none_or(|sub| m.sub == *sub)
        };
        self.mods
            .iter()
            .enumerate()
            .skip(from)
            .skip_while(|(_, m)| m.category != category)
            .take_while(|(_, m)| m.category == category)
            .filter(under)
            .find_map(|(_, m)| m.installed)
    }

    /// `from` may be one above the first row or one below the last.
    pub fn step(&self, from: isize, by: isize) -> Option<usize> {
        let mut row = from.checked_add(by)?;
        while let Some(at) = usize::try_from(row).ok().filter(|at| *at < self.rows.len()) {
            if self.stop_on(at).is_some() {
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

    /// Its row, or that of the folded heading it is under; nothing when filtered out.
    pub fn place_of(&self, id: ModId) -> Option<usize> {
        let at = self.position_of(id)?;
        if let Some(row) = self.row_of(at) {
            return Some(row);
        }
        let m = &self.mods[at];
        let mut place = None;
        for (row, shown) in self.rows.iter().enumerate() {
            match *shown {
                Row::Mod { index } | Row::Sub { first: index, .. } if index > at => break,
                Row::Header {
                    category,
                    collapsed: true,
                    ..
                } if category == m.category => place = Some(row),
                Row::Sub {
                    first,
                    collapsed: true,
                    ..
                } if self.mods[first].category == m.category && self.mods[first].sub == m.sub => {
                    place = Some(row)
                }
                _ => {}
            }
        }
        place.filter(|_| self.matches(at))
    }

    /// More than one row lands on the nearest installed mod short of an end, so a page
    /// or a whole list's length goes as far as there is to go.
    pub fn step_from(&self, id: Option<ModId>, by: isize) -> Option<usize> {
        let last = self.rows.len().checked_sub(1)? as isize;
        let at_end = if by > 0 { -1 } else { last + 1 };
        let from = id.and_then(|id| self.place_of(id));
        let to = from
            .map_or(at_end, |row| row as isize)
            .saturating_add(by)
            .clamp(0, last);
        let ahead = if by < 0 { -1 } else { 1 };
        self.step(to - ahead, ahead)
            .or_else(|| self.step(to + ahead, -ahead))
    }

    pub fn beside_mod(&self, id: ModId) -> Option<ModId> {
        self.beside(self.place_of(id)?)
    }

    pub fn others_through(&self, from: ModId, to: ModId) -> Option<Vec<ModId>> {
        let (a, b) = (self.place_of(from)?, self.place_of(to)?);
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

    /// The moves, `(from, to)` in the order to make them, that take each mod `by` places
    /// and keep their order; nothing when one would leave the list or is not in it.
    pub fn nudged(&self, ids: &[ModId], by: isize) -> Option<Vec<(usize, usize)>> {
        let mut at: Vec<usize> = ids
            .iter()
            .map(|id| self.position_of(*id))
            .collect::<Option<_>>()?;
        at.sort_unstable();
        at.dedup();
        if by > 0 {
            at.reverse();
        }
        let to = |from: usize| {
            from.checked_add_signed(by)
                .filter(|to| *to < self.mods.len())
        };
        at.into_iter().map(|from| Some((from, to(from)?))).collect()
    }
}
