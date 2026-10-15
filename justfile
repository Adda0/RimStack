# RimStack: what is run again and again while working on it.
# `just` alone lists the recipes.

app := "target/release/rimstack"

default:
    @just --list --unsorted

# Build the release app
build:
    cargo build --release -q -p rimstack-app
    tools/qml-import.sh

# Build and open RimStack on the installed mods
run *args: build
    {{app}} {{args}}

# Open on 2,300 made-up mods instead of the installed ones
demo *args: build
    {{app}} --demo {{args}}

# Format the Rust code
fmt:
    cargo fmt

# Clippy on every target, then qmllint on the QML
lint: build
    cargo clippy -q --all-targets
    tools/lint-qml.sh
    @# 21 are known (items reached through a Loader, a Repeater or `parent`, and the app's own types, which qmllint cannot see into): a new one is a mistake.
    @test "$(tools/lint-qml.sh 2>&1 | grep -c '^Warning')" -le 21 || { echo "qmllint: more warnings than the 21 known"; exit 1; }

# Both crates' tests; `just test list::` runs those matching a name
test *filter:
    cargo test --workspace -q {{filter}}

# The package for this system, into target/packages: an AppImage, a zip and installer, or a dmg
package:
    {{ if os() == "windows" { "pwsh packaging/windows/package.ps1" } else { "packaging/" + os() + "/package.sh" } }}

# Known advisories against the crates in Cargo.lock
audit:
    cargo audit

# After a text was added or changed: wrap it and write words.json and English.json again
words:
    python3 tools/words.py

# Texts each language file lacks, or holds and no longer needs
languages:
    #!/usr/bin/env python3
    import json, glob, os
    words = set(json.load(open("crates/app/assets/words.json", encoding="utf-8")))
    bad = 0
    for path in sorted(glob.glob("crates/app/assets/languages/*.json")):
        own = set(json.load(open(path, encoding="utf-8")))
        lacks, extra = sorted(words - own), sorted(own - words)
        bad += len(lacks) + len(extra)
        print(os.path.basename(path), "ok" if not lacks and not extra else "")
        for text in lacks: print("  lacks:", text)
        for text in extra: print("  no longer used:", text)
    raise SystemExit(1 if bad else 0)

# Every action of the Mods screen, with no window, on a copy of the game's settings; `just check Français` in that language
check language="": build
    RIMSTACK_LANGUAGE="{{language}}" tools/check-app.sh

# A picture of one section, with no window and its own settings folder: `just shot 4 /tmp/logs.png --view=1`
shot section out *args: build
    #!/usr/bin/env bash
    set -euo pipefail
    scratch="$(mktemp -d)"
    trap 'rm -rf "$scratch"' EXIT
    mkdir -p "$scratch/settings/RimStack"
    printf '{ "setupDone": "yes" }' > "$scratch/settings/RimStack/settings.json"
    XDG_CONFIG_HOME="$scratch/settings" QT_QPA_PLATFORM=offscreen {{app}} --section={{section}} --shot={{out}} {{args}}

# The scripted ten-second scroll on made-up mods, in a real window: frames, slow frames, the worst
scroll: build
    {{app}} --selftest 2>&1 | grep -E "scroll frames|_ms="

# Everything before a commit: format, lint, tests, build, the app's checks (in English; the screens check switches language)
ready: fmt lint test audit words languages
    just check
