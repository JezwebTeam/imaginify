#!/usr/bin/env python3
"""Read-only post-release verification for an Imaginify release.

Run by the jezpress-release-verifier agent at the verify step of /release, after
release.yml has published. Never writes: no edits, no gh writes, no pushes.
A green workflow is not proof - a release can publish with an asset missing, a
stale installer version or empty notes - so everything is read back.

    python3 .claude/scripts/verify_release.py --version 1.1.0 --prev 1.0.1

Exit status: 0 = no FAIL, 1 = at least one FAIL, 2 = could not run.
Python 3.9 compatible (the macOS system python).
"""

import argparse
import importlib.util
import io
import json
import re
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MIN_SIZE = 1024 * 1024  # Tauri builds are ~3-15 MB; anything under 1 MB is broken


def expected_assets(v):
    return [
        "Imaginify-Setup-%s.exe" % v,
        "Imaginify-%s-windows-portable.exe" % v,
        "Imaginify-%s-macos-arm64.dmg" % v,
        "Imaginify-%s-macos-x86_64.dmg" % v,
    ]


class Results:
    def __init__(self):
        self.rows = []

    def add(self, status, check, detail=""):
        self.rows.append((status, check, detail))
        print("%-5s %-28s %s" % (status, check, detail), flush=True)

    def ok(self, cond, check, good, bad):
        self.add("PASS" if cond else "FAIL", check, good if cond else bad)
        return cond


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, cwd=str(ROOT), **kw)


def die(msg):
    print("verify_release: %s" % msg, file=sys.stderr)
    sys.exit(2)


def repo_slug():
    url = run(["git", "remote", "get-url", "origin"]).stdout.strip()
    m = re.search(r"github\.com[:/]([^/]+/[^/]+?)(?:\.git)?$", url)
    if not m:
        die("cannot read the GitHub repo from origin (%r)" % url)
    return m.group(1)


def gh_json(args):
    p = run(["gh"] + args)
    if p.returncode != 0:
        return None, (p.stderr or p.stdout).strip().splitlines()[-1:] or ["gh failed"]
    try:
        return json.loads(p.stdout), None
    except ValueError as exc:
        return None, [str(exc)]


def load_check_versions():
    spec = importlib.util.spec_from_file_location("check_versions", str(ROOT / ".claude/scripts/check_versions.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main():
    ap = argparse.ArgumentParser(description="Verify a published Imaginify release (read-only)")
    ap.add_argument("--version", required=True)
    ap.add_argument("--prev", help="previous released version (checks the new one supersedes it)")
    args = ap.parse_args()
    v = args.version
    if not re.match(r"^\d+\.\d+\.\d+$", v):
        die("--version must be X.Y.Z")
    tag = "v" + v
    if run(["gh", "--version"]).returncode != 0:
        die("gh CLI not available")
    slug = repo_slug()
    cv = load_check_versions()
    r = Results()

    # ---------------------------------------------------------------- tag + source
    remote = run(["git", "ls-remote", "--tags", "origin", "refs/tags/%s" % tag]).stdout.strip()
    r.ok(bool(remote), "tag on origin", tag, "%s is not on origin - push the tag" % tag)
    have_local = run(["git", "rev-parse", "-q", "--verify", "refs/tags/%s" % tag]).returncode == 0
    if not have_local:
        run(["git", "fetch", "-q", "origin", "refs/tags/%s:refs/tags/%s" % (tag, tag)])
    notes = ""
    arch = subprocess.run(["git", "archive", "--format=tar", tag], capture_output=True, cwd=str(ROOT))
    if arch.returncode != 0:
        r.add("FAIL", "tag source", "git archive %s failed: %s" % (tag, arch.stderr.decode()[-200:]))
    else:
        with tempfile.TemporaryDirectory() as tmp:
            with tarfile.open(fileobj=io.BytesIO(arch.stdout)) as tf:
                tf.extractall(tmp)
            spots = cv.read_spots(Path(tmp))
            bad = ["%s %s=%s" % (f, lbl, val or err) for f, lbl, val, err in spots if val != v]
            r.ok(not bad, "version spots at tag", "all %d = %s" % (len(spots), v), "; ".join(bad))
            secs = cv.changelog_sections(Path(tmp)) or []
            r.ok(bool(secs) and secs[0][0] == v and "\n".join(secs[0][2]).strip(), "CHANGELOG at tag",
                 "newest section %s" % v, "newest section is %s" % (secs[0][0] if secs else "missing"))
            notes = "\n".join(secs[0][2]).strip() if secs and secs[0][0] == v else ""
            readme = (Path(tmp) / "README.md").read_text(encoding="utf-8", errors="replace")
            if "Prebuilt downloads are not published yet" in readme:
                r.add("WARN", "README install status", "README.md still says prebuilt downloads are not published")
    if args.prev:
        def key(x):
            return tuple(int(p) for p in x.split("."))
        r.ok(key(v) > key(args.prev), "supersedes previous", "%s > %s" % (v, args.prev),
             "%s is not newer than %s" % (v, args.prev))

    # ---------------------------------------------------------------- workflow run
    runs, err = gh_json(["run", "list", "-R", slug, "--workflow", "release.yml", "--branch", tag,
                         "--limit", "1", "--json", "status,conclusion,url"])
    if err or not runs:
        r.add("WARN", "release workflow", "no run found for %s (%s)" % (tag, "; ".join(err or [])))
    else:
        r.ok(runs[0]["conclusion"] == "success", "release workflow", "success",
             "%s/%s %s" % (runs[0]["status"], runs[0]["conclusion"], runs[0]["url"]))

    # ---------------------------------------------------------------- GitHub release
    rel, err = gh_json(["api", "repos/%s/releases/tags/%s" % (slug, tag)])
    if err:
        r.add("FAIL", "GitHub release", "no release for %s (%s)" % (tag, "; ".join(err)))
        return finish(r, v)
    r.ok(not rel.get("draft") and not rel.get("prerelease"), "published", rel.get("html_url", ""),
         "draft=%s prerelease=%s" % (rel.get("draft"), rel.get("prerelease")))
    r.ok(rel.get("name") == "Imaginify %s" % v, "title", rel.get("name"), "title is %r" % rel.get("name"))

    body = rel.get("body") or ""
    first_bullet = next((ln.strip() for ln in notes.splitlines() if ln.strip()), "")
    r.ok(bool(first_bullet) and first_bullet in body, "notes from CHANGELOG", "body carries the %s section" % v,
         "release body does not contain the CHANGELOG section")
    nonascii = sorted({c for c in body if ord(c) > 127})
    if nonascii:
        r.add("WARN", "notes ASCII", "non-ASCII in body: %s" % " ".join("U+%04X" % ord(c) for c in nonascii[:8]))

    assets = {a["name"]: a for a in rel.get("assets", [])}
    want = expected_assets(v) + ["SHA256SUMS.txt"]
    missing = [n for n in want if n not in assets]
    extra = [n for n in assets if n not in want]
    r.ok(not missing, "assets present", "%d/%d" % (len(want), len(want)), "missing: %s" % ", ".join(missing))
    if extra:
        r.add("WARN", "unexpected assets", ", ".join(extra))
    small = ["%s (%d B)" % (n, assets[n]["size"]) for n in expected_assets(v)
             if n in assets and assets[n]["size"] < MIN_SIZE]
    r.ok(not small, "asset sizes", "every build >= %d MB" % (MIN_SIZE // 1048576), "too small: %s" % ", ".join(small))
    not_up = [n for n, a in assets.items() if a.get("state") != "uploaded"]
    r.ok(not not_up, "asset state", "all uploaded", "not uploaded: %s" % ", ".join(not_up))

    # ---------------------------------------------------------------- checksums
    if "SHA256SUMS.txt" in assets:
        with tempfile.TemporaryDirectory() as tmp:
            p = run(["gh", "release", "download", tag, "-R", slug, "-p", "SHA256SUMS.txt", "-D", tmp])
            sums_path = Path(tmp) / "SHA256SUMS.txt"
            if p.returncode != 0 or not sums_path.is_file():
                r.add("FAIL", "SHA256SUMS", "download failed: %s" % p.stderr.strip()[-200:])
            else:
                sums = {}
                for ln in sums_path.read_text().splitlines():
                    parts = ln.split()
                    if len(parts) == 2:
                        sums[parts[1].lstrip("*")] = parts[0]
                listed = [n for n in expected_assets(v) if n not in sums]
                r.ok(not listed, "SHA256SUMS lists all", "%d entries" % len(sums), "missing: %s" % ", ".join(listed))
                digests = {n: (a.get("digest") or "").replace("sha256:", "") for n, a in assets.items()}
                if any(digests.values()):
                    wrong = [n for n in expected_assets(v) if n in sums and digests.get(n) and digests[n] != sums[n]]
                    r.ok(not wrong, "SHA256SUMS match", "every listed hash matches GitHub's digest",
                         "mismatch: %s" % ", ".join(wrong))
                else:
                    r.add("SKIP", "SHA256SUMS match", "GitHub returned no asset digests to compare")

    # ---------------------------------------------------------------- latest
    latest, err = gh_json(["api", "repos/%s/releases/latest" % slug])
    if latest:
        if latest.get("tag_name") != tag:
            r.add("WARN", "latest release", "Latest is %s, not %s" % (latest.get("tag_name"), tag))
        else:
            r.add("PASS", "latest release", tag)
    return finish(r, v)


def finish(r, v):
    n = {k: sum(1 for s, _, _ in r.rows if s == k) for k in ("PASS", "WARN", "FAIL", "SKIP")}
    verdict = "FAIL" if n["FAIL"] else "PASS"
    print("\nRELEASE VERIFY: %s  imaginify %s  (%d pass, %d warn, %d fail, %d skip)"
          % (verdict, v, n["PASS"], n["WARN"], n["FAIL"], n["SKIP"]))
    return 1 if n["FAIL"] else 0


if __name__ == "__main__":
    sys.exit(main())
