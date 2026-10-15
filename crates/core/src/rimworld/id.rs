//! `ModsConfig.xml` writes ids in lower case, `About.xml` keeps the author's
//! spelling: compare them only folded.

pub fn fold(id: &str) -> String {
    id.trim().to_lowercase()
}

/// The game adds `_steam` to a Workshop copy's id when the mod is also installed locally.
pub fn split_steam(id: &str) -> (String, bool) {
    let folded = fold(id);
    match folded.strip_suffix("_steam") {
        Some(base) if !base.is_empty() => (base.to_string(), true),
        _ => (folded, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steam_suffix_is_split_off_only_at_the_end() {
        assert_eq!(split_steam("Foo.Bar_steam"), ("foo.bar".into(), true));
        assert_eq!(split_steam("foo.bar"), ("foo.bar".into(), false));
        assert_eq!(split_steam("my_steam.mod"), ("my_steam.mod".into(), false));
    }
}
