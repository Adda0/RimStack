#!/bin/sh
# Lints the QML with Qt's qmllint against the module the last build made.
# Build first: `cargo build -p rimstack-app --release`.
set -eu
cd "$(dirname "$0")/.."
lint=${QMLLINT:-/usr/lib/qt6/bin/qmllint}
tools/qml-import.sh
find crates/app/qml -name '*.qml' -print0 | xargs -0 "$lint" -I target/qml-import "$@"
