#!/usr/bin/env python3

import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/release.yml"
LINUX = ROOT / "packaging/linux/package.sh"
QT_HOSTS = ["linux_x64", "windows_x86", "mac_x64"]


def fetch(url: str) -> str:
    request = urllib.request.Request(url, headers={"User-Agent": "RimStack ci-versions"})
    with urllib.request.urlopen(request, timeout=30) as answer:
        return answer.read().decode()


def version_key(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in version.split("."))


def qt_versions(host: str) -> set[str]:
    listing = fetch(f"https://download.qt.io/online/qtsdkrepository/{host}/desktop/")
    found = set()
    for digits in re.findall(r'href="qt6_6(\d+)/"', listing):
        minor, patch = (digits[:-1], digits[-1])
        found.add(f"6.{int(minor)}.{patch}")
    return found


def newest_qt() -> str:
    everywhere = set.intersection(*(qt_versions(host) for host in QT_HOSTS))
    return max(everywhere, key=version_key)


def newest_aqt() -> str:
    return json.loads(fetch("https://pypi.org/pypi/aqtinstall/json"))["info"]["version"]


def newest_release(repository: str) -> str:
    releases = json.loads(fetch(f"https://api.github.com/repos/{repository}/releases?per_page=20"))
    tags = [r["tag_name"] for r in releases if not r["draft"] and r["tag_name"] != "continuous"]
    return tags[0]


def main() -> int:
    check = "--check" in sys.argv[1:]
    workflow, linux = WORKFLOW.read_text(), LINUX.read_text()
    moved = []

    qt_now = re.search(r'QT_VERSION: "([\d.]+)"', workflow).group(1)
    qt = newest_qt()
    if version_key(qt) > version_key(qt_now):
        workflow = workflow.replace(f'QT_VERSION: "{qt_now}"', f'QT_VERSION: "{qt}"')
        moved.append(f"Qt {qt_now} → {qt}")

    aqt_now = re.search(r'AQT_SOURCE: "([^"]+)"', workflow).group(1)
    aqt = newest_aqt()
    if version_key(aqt) > (3, 3, 0) and aqt_now != f"aqtinstall=={aqt}":
        workflow = workflow.replace(f'AQT_SOURCE: "{aqt_now}"', f'AQT_SOURCE: "aqtinstall=={aqt}"')
        moved.append(f"aqtinstall {aqt_now} → {aqt}")

    for repository in ["linuxdeploy/linuxdeploy", "linuxdeploy/linuxdeploy-plugin-qt"]:
        pattern = rf"github\.com/{repository}/releases/download/([^/]+)/"
        now = re.search(pattern, linux).group(1)
        newest = newest_release(repository)
        if newest != now:
            linux = linux.replace(f"{repository}/releases/download/{now}/", f"{repository}/releases/download/{newest}/")
            moved.append(f"{repository.split('/')[1]} {now} → {newest}")

    print("\n".join(moved) if moved else "everything is current")
    if not check:
        WORKFLOW.write_text(workflow)
        LINUX.write_text(linux)
    return 0


if __name__ == "__main__":
    sys.exit(main())
