//! They outrank the community's rules and the mods' own.

use crate::base::error::Error;
use crate::mods::rules::{self, Kind, Rule};
use crate::rimworld::about::About;
use crate::rimworld::id;
use serde_json::{Value, json};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserRules {
    pairs: Vec<(String, String)>,
}

impl UserRules {
    /// A damaged file is no rules.
    pub fn read(path: &Path) -> Self {
        let Some(Value::Array(items)) = crate::base::atomic_file::read_json(path) else {
            return Self::default();
        };
        let pair = |item: &Value| -> Option<(String, String)> {
            let first = item.get("first")?.as_str()?;
            Some((id::fold(first), id::fold(item.get("then")?.as_str()?)))
        };
        Self {
            pairs: items.iter().filter_map(pair).collect(),
        }
    }

    pub fn write(&self, path: &Path) -> Result<(), Error> {
        let items: Vec<Value> = self
            .pairs
            .iter()
            .map(|(first, then)| json!({ "first": first, "then": then }))
            .collect();
        let text = serde_json::to_string_pretty(&Value::Array(items))
            .map_err(|e| Error::format(format!("{}: {e}", path.display())))?;
        crate::base::atomic_file::write(path, text)
    }

    pub fn add(&mut self, first: &str, then: &str) -> bool {
        let (first, then) = (id::fold(first), id::fold(then));
        if first == then || first.is_empty() || then.is_empty() {
            return false;
        }
        if self.pairs.contains(&(first.clone(), then.clone())) {
            return false;
        }
        self.pairs
            .retain(|pair| *pair != (then.clone(), first.clone()));
        self.pairs.push((first, then));
        true
    }

    pub fn import(&mut self, pairs: &[(String, String)]) -> usize {
        let new = |(first, then): &&(String, String)| self.add(first, then);
        pairs.iter().filter(new).count()
    }

    pub fn remove(&mut self, first: &str, then: &str) -> bool {
        let wanted = (id::fold(first), id::fold(then));
        let before = self.pairs.len();
        self.pairs.retain(|pair| *pair != wanted);
        self.pairs.len() != before
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    pub fn rules(&self, mods: &[Option<&About>]) -> Vec<Rule> {
        let position = rules::positions(mods);
        let rule = |(first, then): &(String, String)| {
            Some(Rule {
                first: *position.get(first)?,
                then: *position.get(then)?,
                kind: Kind::Yours,
            })
        };
        self.pairs.iter().filter_map(rule).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn about(id: &str) -> About {
        About {
            package_id: id.to_string(),
            ..About::default()
        }
    }

    #[test]
    fn a_rule_is_added_once_and_replaces_its_opposite() {
        let mut yours = UserRules::default();
        assert!(yours.add("A.a", "b.b"));
        assert!(!yours.add("a.a", "B.B"));
        assert_eq!(yours.len(), 1);
        assert!(yours.add("b.b", "a.a"));
        assert_eq!(yours.len(), 1);
        assert!(!yours.add("a.a", "a.a"));
        assert!(!yours.add("", "a.a"));
        assert!(yours.remove("B.b", "a.a"));
        assert!(!yours.remove("b.b", "a.a"));
        assert!(yours.is_empty());
    }

    #[test]
    fn importing_counts_only_the_rules_that_are_new() {
        let mut yours = UserRules::default();
        yours.add("a.a", "b.b");
        let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
        let added = yours.import(&[pair("A.a", "b.b"), pair("b.b", "c.c"), pair("x.x", "x.x")]);
        assert_eq!(added, 1);
        assert_eq!(yours.len(), 2);
    }

    #[test]
    fn rules_apply_between_listed_mods_and_are_the_strongest() {
        let mut yours = UserRules::default();
        yours.add("late.mod", "early.mod");
        yours.add("early.mod", "not.listed");
        let abouts = [about("Early.Mod"), about("late.mod")];
        let refs: Vec<Option<&About>> = abouts.iter().map(Some).collect();
        let found = yours.rules(&refs);
        assert_eq!(
            found,
            [Rule {
                first: 1,
                then: 0,
                kind: Kind::Yours
            }]
        );
        assert!(found[0].broken());
        assert_eq!(found[0].declared_by(), None);
        assert!(Kind::Yours.strength() > Kind::CommunityAfter.strength());
    }

    #[test]
    fn rules_read_back_as_written_and_damage_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data").join("rules.json");
        let mut yours = UserRules::default();
        yours.add("a.a", "b.b");
        yours.write(&path).unwrap();
        assert_eq!(UserRules::read(&path), yours);
        std::fs::write(&path, "{").unwrap();
        assert!(UserRules::read(&path).is_empty());
    }
}
