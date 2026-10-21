//! RimSort's `communityRules.json`: downloaded only when asked, never shipped with RimStack.

use crate::base::error::Error;
use crate::said;

use crate::mods::rules::{self, Kind, Rule};
use crate::rimworld::about::About;
use crate::rimworld::id;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const URL: &str =
    "https://raw.githubusercontent.com/RimSort/Community-Rules-Database/main/communityRules.json";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entry {
    pub load_after: Vec<String>,
    pub load_before: Vec<String>,
    pub bottom: bool,
    pub top: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Community {
    /// Seconds since 1970.
    pub timestamp: Option<i64>,
    /// Keyed by folded package id.
    entries: HashMap<String, Entry>,
}

impl Community {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn pairs(&self) -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for (id, entry) in &self.entries {
            pairs.extend(
                entry
                    .load_after
                    .iter()
                    .map(|other| (id::fold(other), id.clone())),
            );
            pairs.extend(
                entry
                    .load_before
                    .iter()
                    .map(|other| (id.clone(), id::fold(other))),
            );
        }
        pairs.sort();
        pairs.dedup();
        pairs
    }

    /// It is a matter of category and makes no rule.
    pub fn loads_at_top(&self, package_id: &str) -> bool {
        let entry = self.entries.get(&id::fold(package_id));
        entry.is_some_and(|e| e.top)
    }

    pub fn rules(&self, mods: &[Option<&About>]) -> Vec<Rule> {
        let position = rules::positions(mods);
        let entry_of =
            |about: &Option<&About>| about.and_then(|a| self.entries.get(&id::fold(&a.package_id)));
        let bottom: Vec<bool> = mods
            .iter()
            .map(|about| entry_of(about).is_some_and(|e| e.bottom))
            .collect();

        let mut found = Vec::new();
        for (me, about) in mods.iter().enumerate() {
            let Some(entry) = entry_of(about) else {
                continue;
            };
            let mut add = |targets: &[String], kind: Kind| {
                for target in targets {
                    let Some(&other) = position.get(&id::split_steam(target).0) else {
                        continue;
                    };
                    if other == me {
                        continue;
                    }
                    let (first, then) = match kind {
                        Kind::CommunityBefore => (me, other),
                        _ => (other, me),
                    };
                    found.push(Rule { first, then, kind });
                }
            };
            add(&entry.load_after, Kind::CommunityAfter);
            add(&entry.load_before, Kind::CommunityBefore);
            if entry.bottom {
                let above = (0..mods.len()).filter(|&other| !bottom[other]);
                found.extend(above.map(|other| Rule {
                    first: other,
                    then: me,
                    kind: Kind::CommunityBottom,
                }));
            }
        }
        found
    }
}

/// A `communityRules.json` entry: `later` loads after `earlier`, both package ids folded as the
/// database keeps them; `earlier_name` the name a reviewer reads and `why` the comment, which
/// an own rule does not have. Reads back through `parse` as the same rule.
pub fn entry_for(later: &str, earlier: &str, earlier_name: &str, why: &str) -> String {
    let comment = if why.is_empty() {
        Value::Array(Vec::new())
    } else {
        Value::Array(vec![Value::String(why.to_string())])
    };
    let entry = serde_json::json!({
        id::fold(later): {
            "loadAfter": {
                id::fold(earlier): {
                    "name": [earlier_name],
                    "comment": comment,
                }
            }
        }
    });
    serde_json::to_string_pretty(&entry).unwrap_or_default()
}

/// The user's own rule in ids, not positions: `then` loads after `first`; `first_name` is the
/// name a reviewer of the pull request reads, the package id itself when no better is known.
#[derive(Debug, Clone)]
pub struct OwnRule {
    pub first: String,
    pub then: String,
    pub first_name: String,
}

/// What was merged into the upstream file's text: `added` are the rules the file did not know.
pub struct Merged {
    pub text: String,
    pub added: Vec<OwnRule>,
}

/// `earlier` under `loadAfter` of `then`, named for a reviewer, as pair builders for the
/// ordered writer: the target, the `loadAfter` pair, the whole entry of `then`.
fn target(earlier: &str, earlier_name: &str) -> (String, ordered::Value) {
    use ordered::Value as V;
    (
        id::fold(earlier),
        V::Object(vec![(
            "name".to_string(),
            V::Array(vec![V::String(earlier_name.to_string())]),
        )]),
    )
}

fn one(key: &str, value: (String, ordered::Value)) -> (String, ordered::Value) {
    let pair = vec![value];
    (key.to_string(), ordered::Value::Object(pair))
}

/// The own rules the upstream file does not ask for already, as a whole file again: each new rule
/// a `loadAfter` under `then`, kept ones untouched. A rule the file states in either direction is
/// skipped, as is a duplicate inside `own`. Fails when the text is not a rules file.
pub fn merge_own_rules(text: &str, own: &[OwnRule]) -> Result<Merged, Error> {
    let folded: Vec<OwnRule> = own
        .iter()
        .map(|rule| OwnRule {
            first: id::fold(&rule.first),
            then: id::fold(&rule.then),
            first_name: rule.first_name.clone(),
        })
        .filter(|rule| !rule.first.is_empty() && !rule.then.is_empty() && rule.first != rule.then)
        .collect();
    let mut known: std::collections::HashSet<String> = parse(text)?
        .pairs()
        .iter()
        .map(|(first, then)| format!("{first}\0{then}"))
        .collect();
    let mut root = ordered::parse(text)?;
    let rules = root
        .object_mut()
        .and_then(|pairs| ordered::pairs_mut(pairs, "rules").and_then(ordered::Value::object_mut))
        .ok_or_else(|| Error::format(said!("community rules have no \"rules\" list")))?;
    let mut added = Vec::new();
    for rule in folded {
        let mine = format!("{}\0{}", rule.first, rule.then);
        let opposite = format!("{}\0{}", rule.then, rule.first);
        if known.contains(&mine) || known.contains(&opposite) || !known.insert(mine) {
            continue;
        }
        let target = target(&rule.first, &rule.first_name);
        if let Some(value) = ordered::pairs_mut(rules, &rule.then) {
            let entry = value.object_mut().ok_or_else(|| {
                Error::format(said!("the entry of {} is not an object", rule.then))
            })?;
            if let Some(targets) = ordered::pairs_mut(entry, "loadAfter") {
                targets
                    .object_mut()
                    .ok_or_else(|| {
                        Error::format(said!("the loadAfter of {} is not an object", rule.then))
                    })?
                    .push(target);
            } else {
                entry.push(one("loadAfter", target));
            }
        } else {
            rules.push(one(&rule.then, one("loadAfter", target)));
        }
        added.push(rule);
    }
    let newline = text.ends_with('\n');
    Ok(Merged {
        text: ordered::write(&root, newline)?,
        added,
    })
}

pub fn parse(text: &str) -> Result<Community, Error> {
    let root: Value = serde_json::from_str(text)
        .map_err(|e| Error::format(said!("community rules are not JSON: {}", e)))?;
    let rules = root
        .get("rules")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::format(said!("community rules have no \"rules\" list")))?;

    let ids = |entry: &Value, key: &str| -> Vec<String> {
        let targets = entry.get(key).and_then(Value::as_object);
        targets.map_or(Vec::new(), |t| t.keys().cloned().collect())
    };
    // Written as `true` or as `{"value": true}`.
    let flag = |entry: &Value, key: &str| -> bool {
        let value = entry.get(key);
        let inner = value.and_then(|v| v.get("value")).or(value);
        inner.and_then(Value::as_bool).unwrap_or(false)
    };
    let entries = rules
        .iter()
        .map(|(package_id, entry)| {
            let entry = Entry {
                load_after: ids(entry, "loadAfter"),
                load_before: ids(entry, "loadBefore"),
                bottom: flag(entry, "loadBottom"),
                top: flag(entry, "loadTop"),
            };
            (id::fold(package_id), entry)
        })
        .collect();
    Ok(Community {
        timestamp: root.get("timestamp").and_then(Value::as_i64),
        entries,
    })
}

/// One file per game version, from NoVersionWarning (github.com/emipa606/NoVersionWarning).
pub fn works_on_url(game_version: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/emipa606/NoVersionWarning/main/{game_version}/ModIdsToFix.xml"
    )
}

pub fn works_on_file(data_dir: &Path, game_version: &str) -> PathBuf {
    data_dir.join(format!("worksOn-{game_version}.xml"))
}

/// The folded package ids in a `ModIdsToFix.xml` text.
pub fn works_on(xml: &str) -> Result<std::collections::HashSet<String>, Error> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let doc = crate::base::xml::parse(xml)
        .map_err(|e| Error::format(said!("the list is not XML: {}", e)))?;
    if !doc.root_element().has_tag_name("ModIdsToFix") {
        return Err(Error::format(said!("the list is not a ModIdsToFix file")));
    }
    let items = doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("li"));
    Ok(items
        .filter_map(|n| n.text())
        .map(id::fold)
        .filter(|s| !s.is_empty())
        .collect())
}

/// Empty when none is kept, or the kept one cannot be read.
pub fn read_works_on(path: &Path) -> std::collections::HashSet<String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    works_on(&text).unwrap_or_default()
}

/// A file that does not parse never replaces a good one.
pub fn download_works_on(
    game_version: &str,
    to: &Path,
) -> Result<std::collections::HashSet<String>, Error> {
    let agent = crate::base::web::agent(30);
    // No list is published for a game version until someone starts one:
    // that is an empty list, not a failure.
    let text = match agent.get(works_on_url(game_version)).call() {
        Ok(mut response) => response
            .body_mut()
            .read_to_string()
            .map_err(crate::base::web::failed)?,
        Err(ureq::Error::StatusCode(404)) => "<ModIdsToFix></ModIdsToFix>".to_string(),
        Err(problem) => return Err(crate::base::web::failed(problem)),
    };
    let parsed = works_on(&text)?;
    crate::base::atomic_file::write(to, &text)?;
    Ok(parsed)
}

pub fn file_in(data_dir: &Path) -> PathBuf {
    data_dir.join("communityRules.json")
}

pub fn read(path: &Path) -> Result<Option<Community>, Error> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::file(path, e)),
    }
}

/// A file that does not parse never replaces a good one, and neither does
/// half a file.
pub fn store(text: &str, to: &Path) -> Result<Community, Error> {
    let parsed = parse(text)?;
    crate::base::atomic_file::write(to, text)?;
    Ok(parsed)
}

pub fn download(to: &Path) -> Result<Community, Error> {
    let agent = crate::base::web::agent(30);
    let mut response = agent.get(URL).call().map_err(crate::base::web::failed)?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(crate::base::web::failed)?;
    store(&text, to)
}

// The rules file is written by RimSort as Python's `json.dumps(indent=4)`: keys in the order
// they were added, four spaces a level, non-ASCII escaped. `merge_own_rules` keeps that shape,
// so a published diff shows only the rules it adds.
mod ordered {
    use crate::base::error::Error;
    use crate::said;
    use serde::de::{MapAccess, SeqAccess, Visitor};

    /// JSON as `serde_json::Value`, but an object keeps the order of its keys.
    pub enum Value {
        Null,
        Bool(bool),
        Number(serde_json::Number),
        String(String),
        Array(Vec<Self>),
        Object(Vec<(String, Self)>),
    }

    impl Value {
        pub fn object_mut(&mut self) -> Option<&mut Vec<(String, Self)>> {
            match self {
                Self::Object(pairs) => Some(pairs),
                _ => None,
            }
        }
    }

    /// `key`'s value inside `pairs`, for writing.
    pub fn pairs_mut<'v>(pairs: &'v mut [(String, Value)], key: &str) -> Option<&'v mut Value> {
        pairs
            .iter_mut()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }
    pub fn parse(text: &str) -> Result<Value, Error> {
        serde_json::from_str(text)
            .map_err(|e| Error::format(said!("community rules are not JSON: {}", e)))
    }

    /// Four spaces a level, as the upstream file keeps it; `newline` ends the file with one.
    pub fn write(root: &Value, newline: bool) -> Result<String, Error> {
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut bytes = Vec::new();
        let mut writer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
        serde::Serialize::serialize(root, &mut writer)
            .map_err(|e| Error::format(said!("the merged rules cannot be written: {}", e)))?;
        let mut text = String::from_utf8(bytes)
            .map_err(|e| Error::format(said!("the merged rules are not UTF-8: {}", e)))?;
        text = escape(&text);
        if newline {
            text.push('\n');
        }
        Ok(text)
    }

    impl serde::Serialize for Value {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            use serde::ser::{SerializeMap, SerializeSeq};
            match self {
                Value::Null => serializer.serialize_unit(),
                Value::Bool(given) => serializer.serialize_bool(*given),
                Value::Number(number) => number.serialize(serializer),
                Value::String(given) => serializer.serialize_str(given),
                Value::Array(items) => {
                    let mut seq = serializer.serialize_seq(Some(items.len()))?;
                    for item in items {
                        seq.serialize_element(item)?;
                    }
                    seq.end()
                }
                Value::Object(pairs) => {
                    let mut map = serializer.serialize_map(Some(pairs.len()))?;
                    for (key, value) in pairs {
                        map.serialize_key(key)?;
                        map.serialize_value(value)?;
                    }
                    map.end()
                }
            }
        }
    }

    /// Python escapes every non-ASCII character in a dumped string with `\uXXXX`.
    fn escape(text: &str) -> String {
        if text.bytes().all(|b| b.is_ascii() && b != 0x7f) {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len() + 16);
        for ch in text.chars() {
            if ch.is_ascii() && ch != '\u{7f}' {
                out.push(ch);
                continue;
            }
            let mut units = [0; 2];
            for unit in ch.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
        out
    }

    impl<'de> serde::Deserialize<'de> for Value {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Builds;
            impl<'de> Visitor<'de> for Builds {
                type Value = Value;

                fn expecting(&self, out: &mut std::fmt::Formatter) -> std::fmt::Result {
                    out.write_str("JSON")
                }

                fn visit_bool<E>(self, given: bool) -> Result<Value, E> {
                    Ok(Value::Bool(given))
                }

                fn visit_u64<E>(self, given: u64) -> Result<Value, E> {
                    Ok(Value::Number(given.into()))
                }

                fn visit_i64<E>(self, given: i64) -> Result<Value, E> {
                    Ok(Value::Number(given.into()))
                }

                fn visit_f64<E: serde::de::Error>(self, given: f64) -> Result<Value, E> {
                    match serde_json::Number::from_f64(given) {
                        Some(number) => Ok(Value::Number(number)),
                        None => Err(E::custom("a number that is not real")),
                    }
                }

                fn visit_str<E>(self, given: &str) -> Result<Value, E> {
                    Ok(Value::String(given.to_string()))
                }

                fn visit_string<E>(self, given: String) -> Result<Value, E> {
                    Ok(Value::String(given))
                }

                fn visit_none<E>(self) -> Result<Value, E> {
                    Ok(Value::Null)
                }

                fn visit_unit<E>(self) -> Result<Value, E> {
                    Ok(Value::Null)
                }

                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
                    let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0));
                    while let Some(item) = seq.next_element()? {
                        items.push(item);
                    }
                    Ok(Value::Array(items))
                }

                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
                    let mut pairs = Vec::with_capacity(map.size_hint().unwrap_or(0));
                    while let Some((name, value)) = map.next_entry()? {
                        pairs.push((name, value));
                    }
                    Ok(Value::Object(pairs))
                }
            }
            deserializer.deserialize_any(Builds)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn order_and_readable_shape_survive_a_round_trip() {
            let text = "{\"z\": 1, \"a\": [{\"b\": \"b\"}, null]}";
            let parsed = parse(text).unwrap();
            let Value::Object(pairs) = &parsed else {
                panic!("not an object");
            };
            assert_eq!(pairs[0].0, "z");
            assert_eq!(pairs[1].0, "a");
            assert_eq!(
                write(&parsed, false).unwrap(),
                "{\n    \"z\": 1,\n    \"a\": [\n        {\n            \"b\": \"b\"\n        },\n        null\n    ]\n}"
            );
            assert!(write(&parsed, true).unwrap().ends_with('\n'));
        }

        #[test]
        fn non_ascii_is_escaped_as_python_writes_it() {
            let parsed = parse("{\"name\": \"Ääblaçké\"}").unwrap();
            assert_eq!(
                write(&parsed, false).unwrap(),
                "{\n    \"name\": \"\\u00c4\\u00e4bla\\u00e7k\\u00e9\"\n}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "timestamp": 1789239946,
        "rules": {
            "Author.Patch": {
                "loadAfter": {"big.mod": {"name": ["Big Mod"]}, "not.installed": {"name": ["x"]}},
                "loadBefore": {"late.mod_steam": {"name": ["Late"], "comment": "why"}}
            },
            "krkr.rocketman": {"loadBottom": {"value": true, "comment": "last"}},
            "old.style": {"loadBottom": true},
            "top.mod": {"loadTop": {"value": true}}
        }
    }"#;

    fn about(id: &str) -> About {
        About {
            package_id: id.to_string(),
            ..About::default()
        }
    }

    fn rules_for(ids: &[&str]) -> Vec<Rule> {
        let abouts: Vec<About> = ids.iter().map(|id| about(id)).collect();
        let refs: Vec<Option<&About>> = abouts.iter().map(Some).collect();
        parse(SAMPLE).unwrap().rules(&refs)
    }

    #[test]
    fn parses_the_timestamp_and_counts_the_mods() {
        let community = parse(SAMPLE).unwrap();
        assert_eq!(community.timestamp, Some(1789239946));
        assert_eq!(community.len(), 4);
    }

    #[test]
    fn load_after_and_before_become_rules_between_listed_mods() {
        let found = rules_for(&["author.patch", "Big.Mod", "late.mod"]);
        assert_eq!(
            found,
            vec![
                Rule {
                    first: 1,
                    then: 0,
                    kind: Kind::CommunityAfter
                },
                Rule {
                    first: 0,
                    then: 2,
                    kind: Kind::CommunityBefore
                },
            ]
        );
        assert_eq!(found[0].declared_by(), None);
    }

    #[test]
    fn a_bottom_mod_follows_every_mod_that_is_not_one() {
        let found = rules_for(&["krkr.rocketman", "a.a", "old.style", "b.b"]);
        let pairs: Vec<(usize, usize)> = found.iter().map(|r| (r.first, r.then)).collect();
        assert_eq!(pairs, vec![(1, 0), (3, 0), (1, 2), (3, 2)]);
        assert!(found.iter().all(|r| r.kind == Kind::CommunityBottom));
    }

    #[test]
    fn every_pair_in_the_file_can_be_listed() {
        let pairs = parse(SAMPLE).unwrap().pairs();
        let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(
            pairs,
            [
                pair("author.patch", "late.mod_steam"),
                pair("big.mod", "author.patch"),
                pair("not.installed", "author.patch"),
            ]
        );
    }

    #[test]
    fn load_top_marks_the_mod_and_makes_no_rule() {
        assert!(rules_for(&["a.a", "top.mod"]).is_empty());
        let community = parse(SAMPLE).unwrap();
        assert!(community.loads_at_top("Top.Mod"));
        assert!(!community.loads_at_top("krkr.rocketman"));
        assert!(!community.loads_at_top("unknown.mod"));
    }

    #[test]
    fn own_rules_merge_keeping_the_file_and_skipping_what_it_knows() {
        let upstream = "{\n    \"timestamp\": 1,\n    \"rules\": {\n        \"b.mod\": {\n            \"loadAfter\": {\n                \"a.mod\": {\n                    \"name\": [\n                        \"A mod\"\n                    ]\n                }\n            }\n        }\n    }\n}\n";
        let own = |first: &str, then: &str, first_name: &str| OwnRule {
            first: first.to_string(),
            then: then.to_string(),
            first_name: first_name.to_string(),
        };
        let merged = merge_own_rules(
            upstream,
            &[
                own("new.first", "New.Then", "New first"),
                own("a.mod", "b.mod", "Already there"),
                own("b.mod", "a.mod", "The file wants the opposite"),
            ],
        )
        .unwrap();
        assert_eq!(merged.added.len(), 1);
        assert_eq!(merged.added[0].then, "new.then");
        assert!(merged.text.ends_with('\n'));
        // What was there is kept where it was; the new entry is appended to the rules.
        assert!(merged.text.find("\"b.mod\"").unwrap() < merged.text.find("\"new.then\"").unwrap());
        let rules = parse(&merged.text).unwrap();
        assert_eq!(
            rules.pairs(),
            [
                ("a.mod".to_string(), "b.mod".to_string()),
                ("new.first".to_string(), "new.then".to_string())
            ]
        );
        // The added entry carries the name a reviewer reads and no comment.
        let writing = "\"new.then\": {\n            \"loadAfter\": {\n                \"new.first\": {\n                    \"name\": [\n                        \"New first\"\n                    ]\n                }\n            }\n        }";
        assert!(merged.text.contains(writing), "{}", merged.text);
        // Again over the merged text: nothing new.
        assert!(
            merge_own_rules(&merged.text, &merged.added)
                .unwrap()
                .added
                .is_empty()
        );
    }

    #[test]
    fn an_own_rule_joins_an_existing_mod_and_its_load_after() {
        let upstream = "{\n    \"rules\": {\n        \"then.mod\": {\n            \"bottom\": true,\n            \"loadAfter\": {\n                \"earlier.mod\": {\n                    \"name\": [\n                        \"Earlier\"\n                    ]\n                }\n            }\n        },\n        \"newer.mod\": {}\n    }\n}";
        let merged = merge_own_rules(
            upstream,
            &[
                OwnRule {
                    first: "one".into(),
                    then: "then.mod".into(),
                    first_name: "One".into(),
                },
                OwnRule {
                    first: "two".into(),
                    then: "then.mod".into(),
                    first_name: "Two".into(),
                },
                OwnRule {
                    first: "three".into(),
                    then: "newer.mod".into(),
                    first_name: "Three".into(),
                },
            ],
        )
        .unwrap();
        assert_eq!(merged.added.len(), 3);
        let text = merged.text;
        assert!(text.find("\"earlier.mod\"").unwrap() < text.find("\"one\"").unwrap());
        assert!(text.find("\"one\"").unwrap() < text.find("\"two\"").unwrap());
        assert!(text.contains("\"bottom\": true"));
        assert_eq!(
            parse(&text).unwrap().pairs(),
            [
                ("earlier.mod".to_string(), "then.mod".to_string()),
                ("one".to_string(), "then.mod".to_string()),
                ("three".to_string(), "newer.mod".to_string()),
                ("two".to_string(), "then.mod".to_string())
            ]
        );
        assert!(merge_own_rules("<html>", &[]).is_err());
    }

    #[test]
    fn an_exported_entry_reads_back_as_the_rule_it_copies() {
        let entry = entry_for("B.Mod", "A.Mod", "A mod", "its defs are patched");
        assert!(entry.contains("\"b.mod\""));
        assert!(entry.contains("\"A mod\""));
        assert!(entry.contains("its defs are patched"));
        let file = format!("{{\"timestamp\": 0, \"rules\": {entry}}}");
        let pairs = parse(&file).unwrap().pairs();
        assert_eq!(pairs, [("a.mod".to_string(), "b.mod".to_string())]);
    }

    #[test]
    fn text_that_is_not_the_rules_file_is_refused() {
        let said = |text| parse(text).unwrap_err().to_string();
        assert!(said("<html>").contains("not JSON"));
        assert!(said("{}").contains("no \"rules\""));
    }

    #[test]
    fn a_file_never_downloaded_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(&file_in(dir.path())), Ok(None));
    }

    #[test]
    fn storing_keeps_the_file_and_reads_back_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(&dir.path().join("RimStack"));
        let stored = store(SAMPLE, &path).unwrap();
        assert_eq!(read(&path), Ok(Some(stored)));
    }

    #[test]
    fn a_bad_download_does_not_replace_a_good_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = file_in(dir.path());
        store(SAMPLE, &path).unwrap();
        assert!(store("404: Not Found", &path).is_err());
        assert_eq!(read(&path).unwrap().unwrap().len(), 4);
    }

    #[test]
    fn the_works_on_list_gives_folded_ids_and_refuses_other_files() {
        let xml = "\u{feff}<?xml version=\"1.0\"?>\n<ModIdsToFix>\n  <!-- A mod -->\n  <li>Baby.Muffalump</li>\n  <li> zilla.fertility </li>\n  <li></li>\n</ModIdsToFix>";
        let found = works_on(xml).unwrap();
        assert_eq!(found.len(), 2);
        assert!(found.contains("baby.muffalump") && found.contains("zilla.fertility"));
        assert!(works_on("404: Not Found").is_err());
        assert!(works_on("<other><li>a.b</li></other>").is_err());
        let dir = tempfile::tempdir().unwrap();
        assert!(read_works_on(&works_on_file(dir.path(), "1.6")).is_empty());
    }

    #[test]
    #[ignore = "needs the network"]
    fn the_real_works_on_list_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = works_on_file(dir.path(), "1.6");
        let found = download_works_on("1.6", &path).unwrap();
        println!("{} mods", found.len());
        assert!(found.len() > 50);
        assert_eq!(read_works_on(&path), found);
    }

    #[test]
    #[ignore = "needs the network"]
    fn the_real_file_downloads_and_parses() {
        let dir = tempfile::tempdir().unwrap();
        let community = download(&file_in(dir.path())).unwrap();
        println!(
            "{} mods, timestamp {:?}",
            community.len(),
            community.timestamp
        );
        assert!(community.len() > 100);
    }
}
