//! Written by the Loading Progress mod as `StartupImpactData.xml`, beside the
//! game's `Config` folder.

use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Measured {
    pub loading_ms: f64,
    /// Keyed by folded package id.
    pub by_mod: HashMap<String, f64>,
}

pub fn parse(xml: &str) -> Option<Measured> {
    let doc = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}')).ok()?;
    let named = |node: roxmltree::Node, name: &str| {
        let child = node.children().find(|n| n.has_tag_name(name))?;
        Some(child.text()?.trim().to_string())
    };
    let all = || doc.descendants();
    let loading = all().find(|n| n.has_tag_name("loadingTime"))?;
    let mods = all().find(|n| n.has_tag_name("mods"))?;
    let one = |node: roxmltree::Node| {
        let id = crate::rimworld::id::fold(&named(node, "modPackageId")?);
        Some((id, named(node, "totalImpact")?.parse().ok()?))
    };
    Some(Measured {
        loading_ms: loading.text()?.trim().parse().ok()?,
        by_mod: mods.children().filter_map(one).collect(),
    })
}

/// A missing or unreadable file gives an empty `Measured`.
pub fn read(config_dir: &Path) -> Measured {
    let text = std::fs::read_to_string(config_dir.join("StartupImpactData.xml"));
    text.ok().and_then(|xml| parse(&xml)).unwrap_or_default()
}

pub fn seconds(ms: f64) -> String {
    match ms {
        ms if ms < 100.0 => format!("{ms:.0} ms"),
        ms if ms < 10_000.0 => format!("{:.1} s", ms / 1000.0),
        ms => format!("{:.0} s", ms / 1000.0),
    }
}

impl Measured {
    /// Milliseconds a mod took; `None` for one that was not measured.
    pub fn of(&self, package_id: &str) -> Option<f64> {
        self.by_mod
            .get(&crate::rimworld::id::fold(package_id))
            .copied()
    }
}

pub fn in_words(measured: &Measured, package_id: &str) -> String {
    match measured.of(package_id) {
        Some(ms) => format!("{} of {}", seconds(ms), seconds(measured.loading_ms)),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}<?xml version=\"1.0\"?><StartupImpactSession><sessionData>
        <loadingTime>33058.1</loadingTime>
        <metrics><keys><li>a</li></keys><values><li>1</li></values></metrics>
        <totalImpact>7012.5</totalImpact>
        <mods>
          <li><modName>Big</modName><modPackageId>Author.Big</modPackageId><totalImpact>1240.4</totalImpact></li>
          <li><modName>Small</modName><modPackageId>a.small</modPackageId><totalImpact>42</totalImpact></li>
          <li><modName>Broken</modName><totalImpact>1</totalImpact></li>
        </mods></sessionData></StartupImpactSession>";

    #[test]
    fn each_mod_s_time_is_read_with_the_whole() {
        let measured = parse(SAMPLE).unwrap();
        assert_eq!(measured.by_mod.len(), 2);
        assert_eq!(in_words(&measured, "author.BIG"), "1.2 s of 33 s");
        assert_eq!(in_words(&measured, "a.small"), "42 ms of 33 s");
        assert_eq!(in_words(&measured, "not.there"), "");
        assert_eq!(measured.of("A.Small_steam"), None);
        assert_eq!(
            measured.of("A.Small").map(seconds).as_deref(),
            Some("42 ms")
        );
    }

    #[test]
    fn a_missing_or_broken_file_is_nothing_measured() {
        assert!(parse("<nope/>").is_none());
        assert_eq!(
            read(tempfile::tempdir().unwrap().path()),
            Measured::default()
        );
    }
}
