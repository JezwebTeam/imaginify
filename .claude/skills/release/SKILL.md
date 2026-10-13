---
name: release
description: Run the full Imaginify release pipeline end to end - two-layer pre-release audit loop (Codex GPT-6.1 Sol layer 1, then jezpress-code-reviewer on Fable as the final reviewer), version bump in all five spots + CHANGELOG, commit and push, a CI test build of every platform that Abner installs by hand, tag + push (release.yml builds Windows .exe + Setup, macOS arm64 + x86_64 DMG/.pkg and publishes the GitHub release), a read-only release-verifier gate, and a LEDGER append. Use when the user says "release X.Y.Z", "ship X.Y.Z", or "roll X.Y.Z". Pass the target version (and optionally a one-line summary) as the argument.
argument-hint: X.Y.Z [one-line changelog summary]
---

# Release pipeline - Imaginify

Target version: **$ARGUMENTS** (first token = `X.Y.Z`; remainder, if any, = changelog hint).

Adapted from the JezPress plugin `/release`. The product is the GitHub release, built by `.github/workflows/release.yml` on a `vX.Y.Z` tag. `git push`, the tag push and `gh workflow run` auto-approve; `ask`-gated commands (`curl`, `gh release edit/upload/delete`, destructive git) prompt, by design.

**Drive it end to end without asking for go-aheads.** Stop and ask Abner **only** here:

| Stop | Step |
|---|---|
| `.venv` missing or below Python 3.10 (needs a python.org / Homebrew Python 3.10+ to create it) | 0 |
| Either audit layer still has an open CRITICAL/HIGH after round 4 | 1 |
| A finding is a product trade-off, not a code defect (the fix changes behaviour users rely on) | 1 |
| The CI test build is ready: ask Abner to install it (always - he installs by hand) | 5 |
| The install check fails and the fix is not obvious | 5 |
| Verifier FAIL that its table says cannot be fixed in place (anything wrong inside a published tag) | 8 |

Facts:
- Repo `JezwebTeam/imaginify` · app `imaginify.py` · Python floor 3.10 · bundle id `com.jezweb.imaginify` · Inno Setup `AppId` fixed in `installer_windows.iss` (never change it).
- Version spots (all five move together, plus `CHANGELOG.md`): `imaginify.py` `__version__`, `version.json` `"version"`, `installer_windows.iss` `MyAppVersion`, `build_installer_macos.sh` `APP_VERSION`, `installer_macos/distribution.xml` `pkg-ref version`. `python3 .claude/scripts/check_versions.py` checks them all.
- Audit details: `.claude/audit-guide.md`.

## 0. Preflight - do this FIRST

```bash
git switch main && git fetch origin && git status -sb      # clean, not behind origin/main
gh auth status
.venv/bin/python -c "import sys, PIL, customtkinter; assert sys.version_info >= (3, 10); print(sys.version)"
python3 .claude/scripts/check_versions.py                  # current spots agree before touching them
```

Use `python3`, never `python`. The macOS system `python3` is 3.9: fine for `check_versions.py` / `verify_release.py`, wrong for the app - anything that imports `imaginify.py` uses `.venv/bin/python`. If `.venv` is missing, create it with a 3.10+ interpreter (`python3.12 -m venv .venv && .venv/bin/python -m pip install -r requirements.txt`); if no such interpreter exists, stop and ask.

**Workflow pushes:** the `origin` remote is HTTPS. Pushing a change under `.github/workflows/` over HTTPS + OAuth needs the `workflow` scope; if the push is refused for that, use the SSH remote (`git@github.com:JezwebTeam/imaginify.git`) rather than debugging credentials.

## 1. Pre-release audit loop (MANDATORY gate - BEFORE the bump)

Audit first, bump after. Both layers automatic - **do not ask**. Details: `.claude/audit-guide.md`.

**Write one context brief** to `<scratch>/brief.md`, reused for both layers. It must contain **no reviewer findings**. Findings and fixes go in `<scratch>/l1-history.md` (one line per finding: `r1 H-1 <summary> -> fixed in imaginify.py:142`). The brief covers:
- what the release changes and why,
- lint results for the changed files: `python3 -m py_compile`, `ruff check --select F,E9`, `shellcheck -S error` (or "not installed"), `check_versions.py`,
- a sanity checklist for the surfaces touched (save path / formats / modes, threading, file output, build scripts, installers, workflows),
- the NOT-to-flag list from the agent file.

**First release from this pipeline:** the previous tag is `v1.0.1`.

**1a. Layer 1 - Codex.**

```bash
.claude/scripts/codex_review.sh --range v<PREV>..HEAD --uncommitted --brief <scratch>/brief.md --out <scratch>/codex-round<N>.md
```

Apply **every** CRITICAL/HIGH, record each in `l1-history.md`, re-lint, and re-run with `--brief` pointing at `cat brief.md l1-history.md > codex-brief-r<N>.md` until no open CRITICAL/HIGH. Exit 2 = usage error (fix the call; NOT "unavailable"). Exit 3 or 4 = Codex unavailable: note `layer 1: unavailable` and go to 1b - never skip 1b.

**1b. Layer 2 - `jezpress-code-reviewer` (Fable, final).** Dispatch with the scope (`git diff v<PREV>..HEAD`, `git diff HEAD`, untracked files from `git status --porcelain`), `brief.md`, and - named separately, to open only after its blind pass - **every** layer-1 round report and `l1-history.md`.

**Loop on layer 2:**
1. Keep a round log covering both layers, e.g. `codex r1: PASS WITH FIXES (H-1 CMYK->PNG) -> fixed; codex r2: PASS; fable r1: PASS`. Step 9 records it.
2. `BLOCK` / `PASS WITH FIXES` -> apply every CRITICAL/HIGH, re-lint, re-dispatch layer 2 with the fix diff and its previous findings. MEDIUMs: apply when cheap, else list in the commit message. LOWs: commit message.
3. Exit on a layer-2 `PASS`, or when every CRITICAL/HIGH from the last round is applied **and** the fix was not material - say which.
4. Round 4 still open, or a product trade-off -> **stop and ask Abner** with the options.

If subagents are unavailable, do a manual self-review against `.claude/audit-guide.md` and label it `manual`.

## 2. Bump the version in all five spots

Set `X.Y.Z` in `imaginify.py`, `version.json` (also update its `"notes"` to a one-line summary), `installer_windows.iss`, `build_installer_macos.sh`, `installer_macos/distribution.xml`.

## 3. CHANGELOG.md (+ README.md when it is stale)

- Add `## [X.Y.Z] - <YYYY-MM-DD>` at the top (Australia/Sydney date), bullets grounded in the actual work, including the audit-loop fixes and the summary argument. **Don't fabricate.** This section becomes the GitHub release body.
- **ASCII punctuation only** - straight quotes and hyphens.
- If the release changes what users see, or `README.md` is out of date (its "Installation status" section said downloads were not published), update it in the same commit.

Then: `python3 .claude/scripts/check_versions.py --expect X.Y.Z` must print `Versions consistent`.

## 4. Commit + push `main` (no tag yet)

Write the message to a file, then commit:

```bash
# .git/COMMITMSG.txt: subject "Release X.Y.Z", body = the changelog bullets + MEDIUM/LOW notes, ending with the Co-Authored-By line
git -c user.name='Abner' -c user.email='abner@jezweb.net' commit -F .git/COMMITMSG.txt
rm -f .git/COMMITMSG.txt
git push origin main
```

`lint.yml` runs on this push (py_compile on 3.10 + 3.13, ruff, shellcheck, version spots, ). Watch it: `gh run list --workflow=lint.yml --limit 1` then `gh run watch <id> --exit-status`. A red lint means stop and fix before step 5.

## 5. CI test build + install check (MANDATORY - before the tag)

Build every platform from `main` without publishing:

```bash
gh workflow run release.yml --ref main
gh run list --workflow=release.yml --limit 1 --json databaseId,status -q '.[0]'
gh run watch <id> --exit-status        # ~10-15 min
gh run download <id> -D <scratch>/build-X.Y.Z
```

**Stop and ask Abner** to install from `<scratch>/build-X.Y.Z/`: the `.dmg` or `.pkg` for his Mac's architecture (and the Setup `.exe` on a Windows machine if one is to hand), open it past the Gatekeeper / SmartScreen prompt, drop a few photos and a folder on the window, process them in two formats, and confirm the title bar says `vX.Y.Z`. Record what he checked (`mac + windows` / `mac (windows unchecked)`).

A failure -> fix on `main`, re-run the audit if the fix is material (step 1), commit, push, repeat step 5. The version stays `X.Y.Z` - nothing is tagged yet.

## 6. Tag + push

```bash
git tag -a vX.Y.Z -m "Imaginify X.Y.Z"
git push origin vX.Y.Z
```

## 7. Watch the release workflow

```bash
gh run list --workflow=release.yml --limit 1 --json databaseId,status,headBranch -q '.[0]'
gh run watch <id> --exit-status
```

It re-checks every version spot against the tag, builds all three platform jobs again from the tag, then publishes `Imaginify X.Y.Z` with six installers and `SHA256SUMS.txt`. A failure before the `release` job means nothing was published: fix on `main`, delete the local and remote tag **only if no release exists** (`gh release view vX.Y.Z` fails), re-tag. After a release exists, never move the tag - ship the next patch.

## 8. Verify the release (MANDATORY gate - blocks step 9)

Dispatch `jezpress-release-verifier`, foreground:

> Verify release `X.Y.Z` of Imaginify (previous `PREV`). Repo root: `<repo>`.

- **PASS** -> continue. Report WARNs in the summary.
- **FAIL** -> apply the fix the verifier names, re-dispatch, loop until PASS.
- **FAIL that cannot be fixed in place** -> stop and tell Abner.

## 9. Append to `.claude/LEDGER.md`

One row, newest first, in the ledger's columns:

```
| X.Y.Z | <YYYY-MM-DD> | <audit value> - codex xN, fable xN | <one-line summary> | <install check> | published + verified |
```

Build the Audit column from the step-1 round log using the legend in `LEDGER.md` - never reconstruct or round up a verdict. Commit (`Record X.Y.Z in the ledger`) and push to `main`.

## 10. Report to Abner

- what shipped and the release URL,
- **every audit round's verdict** for both layers and what was fixed (the audit-ran confirmation he requires),
- what the install check covered,
- the release-verifier result with its WARNs,
- that the ledger row is committed and pushed.

Then tell him to `/clear` before the next task.
