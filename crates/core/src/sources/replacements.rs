//! Format: gzipped JSON with a `rules` array.

use crate::base::error::Error;
use crate::said;

use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub workshop_id: String,
    pub name: String,
    pub author: String,
    pub versions: Vec<String>,
}

/// Keyed by the replaced mod's Workshop id.
pub fn parse(json: &str) -> Result<HashMap<String, Replacement>, Error> {
    // RimSort writes the file with a byte order mark.
    let json = json.strip_prefix('\u{feff}').unwrap_or(json);
    let root: Value = serde_json::from_str(json)
        .map_err(|e| Error::format(said!("the list is not JSON: {}", e)))?;
    let rules = root
        .get("rules")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::format(said!("the list has no rules")))?;
    let text = |rule: &Value, key: &str| rule.get(key).and_then(Value::as_str).map(String::from);
    let replacement = |rule: &Value| -> Option<(String, Replacement)> {
        let versions = rule.get("newVersions").and_then(Value::as_array);
        let version = |v: &Value| v.as_str().map(String::from);
        Some((
            text(rule, "oldWorkshopId")?,
            Replacement {
                workshop_id: text(rule, "newWorkshopId")?,
                name: text(rule, "newName")?,
                author: text(rule, "newAuthor").unwrap_or_default(),
                versions: versions.map_or(Vec::new(), |v| v.iter().filter_map(version).collect()),
            },
        ))
    };
    Ok(rules.iter().filter_map(replacement).collect())
}

/// A missing or unreadable file reads as none: the list is advice, not a failure.
pub fn read(path: &Path) -> HashMap<String, Replacement> {
    let Ok(file) = std::fs::File::open(path) else {
        return HashMap::new();
    };
    let mut json = String::new();
    if flate2::read::GzDecoder::new(file)
        .read_to_string(&mut json)
        .is_err()
    {
        return HashMap::new();
    }
    parse(&json).unwrap_or_default()
}

const URL: &str =
    "https://raw.githubusercontent.com/emipa606/UseThisInstead/main/replacements.json.gz";

pub fn file_in(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("replacements.json.gz")
}

/// A download that does not parse never replaces the file at `to`.
pub fn download(to: &Path) -> Result<HashMap<String, Replacement>, Error> {
    let agent = crate::base::web::agent(60);
    let mut response = agent.get(URL).call().map_err(crate::base::web::failed)?;
    let bytes = response
        .body_mut()
        .read_to_vec()
        .map_err(crate::base::web::failed)?;
    let mut json = String::new();
    flate2::read::GzDecoder::new(bytes.as_slice())
        .read_to_string(&mut json)
        .map_err(|e| Error::format(said!("the list could not be unpacked: {}", e)))?;
    let list = parse(&json)?;
    crate::base::atomic_file::write(to, bytes)?;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const LIST: &str = r#"{"rules": [
        {"oldWorkshopId": "1076950211", "oldName": "Therapy", "oldPackageId": "Orion.Therapy",
         "newWorkshopId": "3509488682", "newName": "Therapy (Continued)", "newAuthor": "Zaljerem",
         "newPackageId": "Orion.Therapy", "oldVersions": ["1.5"], "newVersions": ["1.6"]},
        {"oldWorkshopId": "5", "newName": "No id for the new one"}
    ]}"#;

    #[test]
    fn replacements_are_found_by_the_old_workshop_id() {
        let found = parse(LIST).unwrap();
        assert_eq!(found.len(), 1);
        let new = &found["1076950211"];
        assert_eq!(
            (new.workshop_id.as_str(), new.name.as_str()),
            ("3509488682", "Therapy (Continued)")
        );
        assert_eq!(new.author, "Zaljerem");
        assert_eq!(new.versions, ["1.6"]);
        assert_eq!(parse(&format!("\u{feff}{LIST}")).unwrap().len(), 1);
        assert!(parse("[]").is_err());
        assert!(parse("nope").is_err());
    }

    #[test]
    #[ignore = "needs RimSort's copy of the list"]
    fn rimsort_s_real_list_is_read() {
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
        let path = home.join(".local/share/RimSort/dbs/UseThisInstead/replacements.json.gz");
        let mut json = String::new();
        let file = std::fs::File::open(&path).unwrap();
        let outcome = flate2::read::GzDecoder::new(file).read_to_string(&mut json);
        println!("gunzip: {outcome:?}, {} bytes", json.len());
        println!("parse: {:?}", parse(&json).map(|m| m.len()));
        assert!(read(&path).len() > 100);
    }

    #[test]
    fn the_gzipped_file_is_read_and_a_missing_or_broken_one_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("replacements.json.gz");
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(LIST.as_bytes()).unwrap();
        std::fs::write(&path, gz.finish().unwrap()).unwrap();
        assert_eq!(read(&path).len(), 1);
        assert!(read(&dir.path().join("none.gz")).is_empty());
        std::fs::write(&path, "not gzip").unwrap();
        assert!(read(&path).is_empty());
    }
}
