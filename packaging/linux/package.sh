#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
root=$PWD
version=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "rimstack-app"))')
tools=target/packaging/tools
app=target/packaging/AppDir
out=target/packages
mkdir -p "$tools" "$out"

fetch() { # name url
    [ -x "$tools/$1" ] && return
    curl --fail --location --silent --show-error --output "$tools/$1" "$2"
    chmod +x "$tools/$1"
}
fetch linuxdeploy https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage
fetch linuxdeploy-plugin-qt https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/1-alpha-20250213-1/linuxdeploy-plugin-qt-x86_64.AppImage

profile="${RIMSTACK_PROFILE:-dist}"
cargo build --profile "$profile" -p rimstack-app
rm -rf "$app"
mkdir -p "$app/usr/share/icons/hicolor/scalable/apps"
cp crates/app/assets/rimstack.svg "$app/usr/share/icons/hicolor/scalable/apps/"
printf '[Paths]\nPrefix = usr\nPlugins = plugins\nQml2Imports = qml\n' > "$app/qt.conf"

export QML_SOURCES_PATHS="$root/crates/app/qml"
qmake="${QMAKE:-$(command -v qmake6 || command -v qmake)}"
export QMAKE="$qmake"
if [ "$("$qmake" -query QT_INSTALL_PREFIX)" = /usr ]; then
    plugins="$root/target/packaging/qt-plugins"
    rm -rf "$plugins"
    for file in "$("$qmake" -query QT_INSTALL_PLUGINS)"/*/*.so; do
        kind=$(basename "$(dirname "$file")")
        case "$kind/$(basename "$file")" in
            */libq* | wayland-*/* | platforminputcontexts/libcompose* | platforminputcontexts/libibus*) mkdir -p "$plugins/$kind" && ln -s "$file" "$plugins/$kind/" ;;
        esac
    done
    printf '#!/bin/sh\n"%s" "$@" | sed "s|^QT_INSTALL_PLUGINS:.*|QT_INSTALL_PLUGINS:%s|"\n' "$qmake" "$plugins" > target/packaging/qmake
    chmod +x target/packaging/qmake
    export QMAKE="$root/target/packaging/qmake"
fi
export EXTRA_QT_MODULES="svg"
export EXTRA_PLATFORM_PLUGINS="libqwayland.so;libqoffscreen.so"
export LDAI_OUTPUT="$out/RimStack-$version-x86_64.AppImage"
export APPIMAGE_EXTRACT_AND_RUN=1
export NO_STRIP=1
PATH="$PWD/$tools:$PATH" linuxdeploy \
    --appdir "$app" \
    --executable "target/$profile/rimstack" \
    --desktop-file crates/app/assets/rimstack.desktop \
    --icon-file crates/app/assets/rimstack.svg \
    --plugin qt \
    --output appimage
echo "$LDAI_OUTPUT"
