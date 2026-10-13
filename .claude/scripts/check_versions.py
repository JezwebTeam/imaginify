#!/usr/bin/env python3
"""Check that every Imaginify version spot agrees, and print release notes.

The version lives in five files plus the changelog. Missing one ships an
installer whose "Add/Remove Programs" entry or .pkg receipt disagrees with the
app's title bar, and macOS Installer refuses to "upgrade" to a pkg whose
version it thinks is older - so all of them must move together.

    python3 .claude/scripts/check_versions.py                 # all spots agree?
    python3 .claude/scripts/check_versions.py --expect 1.0.2  # ...and equal 1.0.2?
    python3 .claude/scripts/check_versions.py --notes 1.0.2   # print that CHANGELOG section

Used by lint.yml (every push), release.yml (tag must match) and /release.
Exit status: 0 = consistent, 1 = mismatch or missing entry, 2 = could not run.
Python 3.9 compatible (the macOS system python).
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")

# (file, description, regex with one capture group for the version)
SPOTS = [
    ("imaginify.py", "__version__", r'^__version__\s*=\s*"([^"]+)"'),
    ("installer_windows.iss", "#define MyAppVersion", r'^#define\s+MyAppVersion\s+"([^"]+)"'),
    ("build_installer_macos.sh", "APP_VERSION", r'^APP_VERSION="([^"]+)"'),
    ("installer_macos/distribution.xml", "pkg-ref version", r'<pkg-ref\s+id="com\.jezweb\.imaginify"\s+version="([^"]+)"'),
]

# `## [1.0.1] - 2026-07-02`; an `## [Unreleased]` section above it is allowed.
HEADING = re.compile(r"^## \[(\d+\.\d+\.\d+)\](?:\s+-\s+(\d{4}-\d{2}-\d{2}))?\s*$")


def repo_root():
    try:
        out = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True,
                             text=True, check=True).stdout.strip()
        return Path(out)
    except (OSError, subprocess.CalledProcessError):
        return Path(__file__).resolve().parents[2]


def read_spots(root):
    found = []
    for rel, label, pattern in SPOTS:
        path = root / rel
        if not path.is_file():
            found.append((rel, label, None, "file missing"))
            continue
        matches = re.findall(pattern, path.read_text(encoding="utf-8"), re.MULTILINE)
        if len(matches) != 1:
            found.append((rel, label, None, "expected 1 match, found %d" % len(matches)))
        else:
            found.append((rel, label, matches[0], None))
    vj = root / "version.json"
    try:
        found.append(("version.json", '"version"', json.loads(vj.read_text(encoding="utf-8"))["version"], None))
    except (OSError, ValueError, KeyError) as exc:
        found.append(("version.json", '"version"', None, "unreadable: %s" % exc))
    return found


def changelog_sections(root):
    """Return [(version, date, body_lines)] in file order."""
    path = root / "CHANGELOG.md"
    if not path.is_file():
        return None
    sections, cur = [], None
    for line in path.read_text(encoding="utf-8").splitlines():
        m = HEADING.match(line)
        if m:
            cur = (m.group(1), m.group(2), [])
            sections.append(cur)
        elif line.startswith("## "):
            cur = None  # [Unreleased] or any other level-2 heading
        elif cur is not None:
            cur[2].append(line)
    return sections


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--expect", help="every spot must equal this version")
    ap.add_argument("--notes", metavar="X.Y.Z", help="print this version's CHANGELOG section and exit")
    args = ap.parse_args()
    root = repo_root()

    sections = changelog_sections(root)
    if args.notes:
        for ver, _date, body in sections or []:
            if ver == args.notes:
                text = "\n".join(body).strip()
                if not text:
                    print("check_versions: CHANGELOG section %s is empty" % ver, file=sys.stderr)
                    return 1
                print(text)
                return 0
        print("check_versions: no `## [%s]` section in CHANGELOG.md" % args.notes, file=sys.stderr)
        return 1

    if args.expect and not SEMVER.match(args.expect):
        print("check_versions: --expect must be X.Y.Z, got %r" % args.expect, file=sys.stderr)
        return 2

    fail = False
    spots = read_spots(root)
    values = set()
    for rel, label, value, err in spots:
        if err:
            print("FAIL %-34s %-22s %s" % (rel, label, err))
            fail = True
        elif not SEMVER.match(value):
            print("FAIL %-34s %-22s %r is not X.Y.Z" % (rel, label, value))
            fail = True
        else:
            print("ok   %-34s %-22s %s" % (rel, label, value))
            values.add(value)

    if len(values) > 1:
        print("FAIL version spots disagree: %s" % ", ".join(sorted(values)))
        fail = True
    version = args.expect or (values.pop() if len(values) == 1 else None)
    if args.expect and values and values != {args.expect}:
        print("FAIL expected %s everywhere" % args.expect)
        fail = True

    if sections is None:
        print("FAIL CHANGELOG.md missing")
        fail = True
    elif version:
        if not sections:
            print("FAIL CHANGELOG.md has no `## [X.Y.Z]` section")
            fail = True
        elif sections[0][0] != version:
            print("FAIL CHANGELOG.md newest section is %s, not %s" % (sections[0][0], version))
            fail = True
        elif not "\n".join(sections[0][2]).strip():
            print("FAIL CHANGELOG.md section %s is empty" % version)
            fail = True
        else:
            print("ok   %-34s %-22s %s" % ("CHANGELOG.md", "newest section", version))

    print("Versions %s." % ("INCONSISTENT" if fail else "consistent: %s" % version))
    return 1 if fail else 0


if __name__ == "__main__":
    sys.exit(main())
