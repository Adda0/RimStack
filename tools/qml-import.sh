#!/bin/sh
# Makes target/qml-import: the RimStack.Ui module as Qt's QML tools look
# for it, the types the newest build generated beside the QML sources
# themselves, and crates/app/.qmlls.ini pointing the editor's qmlls at
# it. tools/lint-qml.sh and the build run it.
set -eu
cd "$(dirname "$0")/.."
built=$(ls -dt target/*/build/rimstack-app-*/out/qt-build-utils/qml_modules/RimStack/Ui 2>/dev/null | head -1)
[ -n "$built" ] || { echo "no built QML module: build the app first" >&2; exit 2; }
module=target/qml-import/RimStack/Ui
mkdir -p "$module"
cp "$built/qmldir" "$built"/*.qmltypes "$module/"
ln -sfn "$PWD/crates/app/qml" "$module/qml"
# qmlls takes an import path only whole: written for this checkout.
printf '[General]\nimportPaths="%s/target/qml-import"\nno-cmake-calls=true\n' "$PWD" > crates/app/.qmlls.ini
