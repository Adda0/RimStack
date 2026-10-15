use std::path::Path;

const LAYERS: [(&str, &[&str]); 5] = [
    ("base", &["base"]),
    ("rimworld", &["base", "rimworld"]),
    ("mods", &["base", "rimworld", "mods"]),
    ("kept", &["base", "rimworld", "kept"]),
    ("sources", &["base", "rimworld", "mods", "kept", "sources"]),
];

fn files(dir: &Path, into: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => files(&path, into),
            false => into.push(path),
        }
    }
}

fn named(text: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    for (layer, _) in LAYERS {
        let direct = format!("crate::{layer}::");
        let grouped = |at: usize| {
            let rest = &text[at..];
            let end = rest.find("};").unwrap_or(rest.len());
            let inside = &rest[..end];
            inside.contains(&format!("{{{layer}::")) || inside.contains(&format!(" {layer}::"))
        };
        let in_group = text.match_indices("crate::{").any(|(at, _)| grouped(at));
        if text.contains(&direct) || in_group {
            found.push(layer);
        }
    }
    found
}

#[test]
fn no_layer_uses_one_above_it() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut wrong = Vec::new();
    for (layer, allowed) in LAYERS {
        let mut all = vec![src.join(format!("{layer}.rs"))];
        files(&src.join(layer), &mut all);
        for file in all {
            let text = std::fs::read_to_string(&file).unwrap();
            for used in named(&text)
                .into_iter()
                .filter(|used| !allowed.contains(used))
            {
                wrong.push(format!("{} uses {used}", file.display()));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_use_of_a_layer_is_found_written_either_way() {
    assert_eq!(named("use crate::mods::library::Library;"), ["mods"]);
    assert_eq!(
        named("use crate::{\n    base::error::Error,\n    kept::notes,\n};"),
        ["base", "kept"]
    );
    assert!(named("use std::collections::HashMap;").is_empty());
}
