---
name: release
description: Run the full Imaginify release pipeline end to end - two-layer pre-release audit loop (Codex GPT-6.1 Sol layer 1, then jezpress-code-reviewer on Fable as the final reviewer), version bump in every spot + CHANGELOG, commit and push, a CI test build of every platform that Abner installs by hand, tag + push (release.yml builds the Windows .exe + Setup and the macOS arm64 + x86_64 DMGs and publishes the GitHub release), a read-only release-verifier gate, a LEDGER append and the Google Chat announcement. Use when the user says "release X.Y.Z", "ship X.Y.Z", or "roll X.Y.Z". Pass the target version (and optionally a one-line summary) as the argument.
argument-hint: X.Y.Z [one-line changelog summary]
---

# Release pipeline - Imaginify

Target version: **$ARGUMENTS** (first token = `X.Y.Z`; remainder, if any, = changelog hint).

Adapted from the JezPress plugin `/release`. The product is the GitHub release, built by `.github/workflows/release.yml` on a `vX.Y.Z` tag. `git push`, the tag push and `gh workflow run` auto-approve; `ask`-gated commands (`curl`, `gh release edit/upload/delete`, destructive git) prompt, by design.

**Drive it end to end without asking for go-aheads.** Stop and ask Abner **only** here:

| Stop | Step |
|---|---|
| Rust or Node missing and cannot be installed without him | 0 |
| Either audit layer still has an open CRITICAL/HIGH after round 4 | 1 |
| A finding is a product trade-off, not a code defect (the fix changes behaviour users rely on) | 1 |
| The CI test build is ready: ask Abner to install it (always - he installs by hand) | 5 |
| The install check fails and the fix is not obvious | 5 |
| Verifier FAIL that its table says cannot be fixed in place (anything wrong inside a published tag) | 8 |

Facts:
- Repo `JezwebTeam/imaginify` · Tauri 2 app: `core/` (Rust pipeline), `src-tauri/` (shell), `ui/` (window) · Rust pinned by `rust-toolchain.toml` · bundle id `com.jezweb.imaginify` · Inno Setup `AppId` fixed in `installer_windows.iss` (never change either).
- Version spots (all move together, plus `CHANGELOG.md`): `src-tauri/tauri.conf.json`, `core/Cargo.toml`, `src-tauri/Cargo.toml`, the two workspace entries in `Cargo.lock`, `package.json`, `package-lock.json` (two fields), `installer_windows.iss` `MyAppVersion`, `version.json`. `python3 .claude/scripts/check_versions.py` checks them all.
- Audit details: `.claude/audit-guide.md`.

## 0. Preflight - do this FIRST

```bash
git switch main && git fetch origin && git status -sb      # clean, not behind origin/main
gh auth status
export PATH="$HOME/.cargo/bin:$PATH"; cargo --version && node --version
python3 .claude/scripts/check_versions.py                  # current spots agree before touching them
npm ci                                                     # if node_modules/ is missing
```

**Workflow pushes:** the `origin` remote is HTTPS. Pushing a change under `.github/workflows/` over HTTPS + OAuth needs the `workflow` scope; if the push is refused for that, use the SSH remote (`git@github.com:JezwebTeam/imaginify.git`) rather than debugging credentials.

## 1. Pre-release audit loop (MANDATORY gate - BEFORE the bump)

Audit first, bump after. Both layers automatic - **do not ask**. Details: `.claude/audit-guide.md`.

**Write one context brief** to `<scratch>/brief.md`, reused for both layers. It must contain **no reviewer findings**. Findings and fixes go in `<scratch>/l1-history.md` (one line per finding: `r1 H-1 <summary> -> fixed in core/src/output.rs:42`). The brief covers:
- what the release changes and why,
- lint results: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p imaginify-core`, `node --check ui/main.js`, `shellcheck -S error` (or "not installed"), `check_versions.py`,
- a sanity checklist for the surfaces touched (file output, decode/encode modes and formats, size budgets, Tauri commands and capabilities, the window JS, installers, workflows),
- the NOT-to-flag list from the agent file.

**Previous tag:** `v1.0.1` was the last Python release; 2.0.0 is the first Tauri one, so its diff is the whole rewrite.

**1a. Layer 1 - Codex.**

```bash
.claude/scripts/codex_review.sh --range v<PREV>..HEAD --uncommitted --brief <scratch>/brief.md --out <scratch>/codex-round<N>.md
```

Apply **every** CRITICAL/HIGH, record each in `l1-history.md`, re-lint, and re-run with `--brief` pointing at `cat brief.md l1-history.md > codex-brief-r<N>.md` until no open CRITICAL/HIGH. Exit 2 = usage error (fix the call; NOT "unavailable"). Exit 3 or 4 = Codex unavailable: note `layer 1: unavailable` and go to 1b - never skip 1b.

**1b. Layer 2 - `jezpress-code-reviewer` (Fable, final).** Dispatch with the scope (`git diff v<PREV>..HEAD`, `git diff HEAD`, untracked files from `git status --porcelain`), `brief.md`, and - named separately, to open only after its blind pass - **every** layer-1 round report and `l1-history.md`. If Fable's usage limit is hit, re-dispatch with `model: "opus"` and record `fable unavailable -> opus`.

**Loop on layer 2:**
1. Keep a round log covering both layers, e.g. `codex r1: PASS WITH FIXES (H-1 ...) -> fixed; codex r2: PASS; fable r1: PASS`. Step 9 records it.
2. `BLOCK` / `PASS WITH FIXES` -> apply every CRITICAL/HIGH, re-lint, re-dispatch layer 2 with the fix diff and its previous findings. MEDIUMs: apply when cheap, else list in the commit message. LOWs: commit message.
3. Exit on a layer-2 `PASS`, or when every CRITICAL/HIGH from the last round is applied **and** the fix was not material - say which.
4. Round 4 still open, or a product trade-off -> **stop and ask Abner** with the options.

If subagents are unavailable, do a manual self-review against `.claude/audit-guide.md` and label it `manual`.

## 2. Bump the version in every spot

```bash
V=X.Y.Z
python3 - "$V" <<'PY'
import json, re, sys
v = sys.argv[1]
for p in ("src-tauri/tauri.conf.json", "package.json", "version.json"):
    d = json.load(open(p)); d["version"] = v
    open(p, "w").write(json.dumps(d, indent=2) + "\n")
for p in ("core/Cargo.toml", "src-tauri/Cargo.toml"):
    s = open(p).read()
    open(p, "w").write(re.sub(r'(?m)\A(\[package\]\nname = "[^"]+"\nversion = )"[^"]+"', r'\g<1>"%s"' % v, s, count=1))
p = "installer_windows.iss"; s = open(p).read()
open(p, "w").write(re.sub(r'(?m)^(#define\s+MyAppVersion\s+)"[^"]+"', r'\g<1>"%s"' % v, s, count=1))
PY
npm install --package-lock-only --silent     # package-lock.json
cargo update --workspace --offline           # the Cargo.lock entries
```

Also update `version.json` `"notes"` to a one-line summary.

## 3. CHANGELOG.md (+ README.md when it is stale)

- Add `## [X.Y.Z] - <YYYY-MM-DD>` at the top (Australia/Sydney date), bullets grounded in the actual work, including the audit-loop fixes and the summary argument. **Don't fabricate.** This section becomes the GitHub release body and the Chat card.
- **ASCII punctuation only** - straight quotes and hyphens.
- If the release changes what users see, update `README.md` in the same commit.

Then: `python3 .claude/scripts/check_versions.py --expect X.Y.Z` must print `Versions consistent`.

## 4. Commit + push `main` (no tag yet)

Write the message to a file, then commit:

```bash
# .git/COMMITMSG.txt: subject "Release X.Y.Z", body = the changelog bullets + MEDIUM/LOW notes, ending with the Co-Authored-By line
git -c user.name='Abner' -c user.email='abner@jezweb.net' commit -F .git/COMMITMSG.txt
rm -f .git/COMMITMSG.txt
git push origin main
```

`lint.yml` runs on this push (cargo fmt + clippy -D warnings + pipeline tests, `node --check`, helper scripts + shellcheck, version spots). Watch it: `gh run list --workflow=lint.yml --limit 1` then `gh run watch <id> --exit-status`. A red lint means stop and fix before step 5.

## 5. CI test build + install check (MANDATORY - before the tag)

Build every platform from `main` without publishing:

```bash
gh workflow run release.yml --ref main
gh run list --workflow=release.yml --limit 1 --json databaseId,status -q '.[0]'
gh run watch <id> --exit-status        # ~15-25 min (cold Rust cache is the slow part)
gh run download <id> -D <scratch>/build-X.Y.Z
```

**Stop and ask Abner** to install from `<scratch>/build-X.Y.Z/`: the `.dmg` for his Mac's architecture (and the Setup `.exe` on a Windows machine if one is to hand - ideally one that had 1.x installed, to see it upgrade in place), open it past the Gatekeeper / SmartScreen prompt, drop a few photos and a folder on the window, crop one, process them in two formats, and confirm the title shows `vX.Y.Z`. Record what he checked (`mac + windows` / `mac (windows unchecked)`).

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

It re-checks every version spot against the tag, builds all three platform jobs again from the tag, then publishes `Imaginify X.Y.Z` with four installers and `SHA256SUMS.txt`. A failure before the `release` job means nothing was published: fix on `main`, delete the local and remote tag **only if no release exists** (`gh release view vX.Y.Z` fails), re-tag. After a release exists, never move the tag - ship the next patch.

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

## 10. Google Chat announcement (automatic - no approval prompt)

Only after step 8 PASSES. Posts to the JezPress releases space (Abner, 2026-10-13), threaded one parent card per app per day like the plugins: thread key `imaginify-<YYYY-MM-DD>` (Australia/Sydney date). The webhook URL lives only in `~/.claude/memory/jezpress-google-chat-webhook.md` - read it at run time, **never** write it into a committed file, a commit message or the transcript.

```bash
V=X.Y.Z
DAY="$(TZ=Australia/Sydney date +%F)"
TMP="$(mktemp -d)"
WEBHOOK="$(grep -oE 'https://chat\.googleapis\.com/v1/spaces/[^ )`"]+' ~/.claude/memory/jezpress-google-chat-webhook.md | head -n1)"
[ -n "$WEBHOOK" ] || { echo "webhook not found"; exit 1; }
python3 .claude/scripts/check_versions.py --notes "$V" > "$TMP/notes.txt" || { echo "no CHANGELOG section for $V"; exit 1; }
python3 - "$V" "$DAY" "$TMP/notes.txt" > "$TMP/chat.json" <<'PY'
import html, json, sys
v, day, notes_file = sys.argv[1], sys.argv[2], sys.argv[3]
notes = html.escape(open(notes_file).read().strip())  # textParagraph renders HTML
url = "https://github.com/JezwebTeam/imaginify/releases/tag/v" + v
print(json.dumps({
    "thread": {"threadKey": "imaginify-" + day},
    "cardsV2": [{"cardId": "imaginify-" + v, "card": {
        "header": {"title": "Imaginify " + v + " released", "subtitle": "Desktop image tool - Windows + macOS"},
        "sections": [
            {"header": "What's new", "widgets": [{"textParagraph": {"text": notes.replace("\n", "<br>")}}]},
            {"widgets": [{"buttonList": {"buttons": [{"text": "Download", "onClick": {"openLink": {"url": url}}}]}}]},
        ]}}],
}, ensure_ascii=True))
PY
CODE="$(curl -sS -o "$TMP/reply.json" -w '%{http_code}' -X POST \
  -H 'Content-Type: application/json; charset=UTF-8' \
  --data-binary @"$TMP/chat.json" \
  "${WEBHOOK}&messageReplyOption=REPLY_MESSAGE_FALLBACK_TO_NEW_THREAD")"
echo "Chat HTTP $CODE"
[ "$CODE" = 200 ] || cat "$TMP/reply.json"   # the reply never contains the URL
rm -rf "$TMP"
```

Run it as ONE command (shell variables do not survive between tool calls). Expect `Chat HTTP 200`; otherwise the reply is printed - fix the payload. Do not retry blindly - a second 200 posts a duplicate card.

## 11. Report to Abner

- what shipped and the release URL,
- **every audit round's verdict** for both layers and what was fixed (the audit-ran confirmation he requires),
- what the install check covered,
- the release-verifier result with its WARNs,
- that the ledger row is committed and pushed, and the Chat card posted (HTTP status),

Then tell him to `/clear` before the next task.
