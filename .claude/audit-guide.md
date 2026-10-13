# Imaginify audit guide - feature + pre-release gates

Adapted from the JezPress plugin audit guide. Same machinery, different failure modes: Imaginify is a desktop app that edits users' own photos and ships as unsigned installers with no auto-update, so the gates guard **user files** and **builds that launch**.

Two gates:
- **Feature audit** - after any change to the image pipeline, threading, file output, packaging (build scripts, installers, `requirements.txt`) or `.github/workflows/`. Gates the *change*.
- **Pre-release audit** - before bumping the version (`/release` step 1). Gates the *ship*. Non-negotiable.

Light diff-review only (no reviewers) for purely cosmetic changes: copy, colours, README wording. **State which path you took** so the gate is visibly honoured.

## Two review layers

| Layer | Reviewer | Role |
|---|---|---|
| 1 | **Codex, GPT-6.1 Sol** via `.claude/scripts/codex_review.sh` (read-only sandbox, instructions in `.claude/codex-review-prompt.md`) | Broad first pass. Its CRITICAL/HIGH findings are fixed, and it re-reviews, before layer 2 runs. |
| 2 (final) | **`jezpress-code-reviewer` agent, Claude Fable** (`.claude/agents/`) | Reviews blind, then rules on every layer-1 finding. **Its verdict gates.** |

**When each layer runs:**
- **Pre-release audit:** both layers, always.
- **Feature audit on a high-risk diff** (anything that writes files, the save/convert path, threading, build scripts, installers, workflows): both layers.
- **Other feature audits** (UI layout, presets, labels): layer 1 alone. Brief layer 2 only if layer 1 returned `BLOCK` or you are unsure. State which path you took.
- **Codex unavailable** (exit 3 or 4; exit 2 is a usage error - fix the call): run layer 2 alone, brief it with `layer 1: unavailable`, and say so in the update and the ledger.

## Install check (third layer)

The plugins' staging check has no equivalent server here; the equivalent is installing the real build on a real machine. Before every tag (`/release` step 5, after the bump is pushed): run `release.yml` by hand on `main` (it builds every platform and uploads them as workflow artifacts, without publishing), then Abner installs the DMG or `.pkg` on his Mac - and the Setup `.exe` on a Windows machine when one is to hand - opens the app past the Gatekeeper / SmartScreen prompt, drops a few photos on it and processes them. Record which platforms were checked; `windows unchecked` is an honest ledger value, a guess is not.

## How to run a gate

1. **Isolate the diff.** `git diff <prev-tag>..HEAD` plus `git status --porcelain` for untracked files.
2. **Lint first and hand the result in.** `python3 -m py_compile` on changed `.py` files, `ruff check --select F,E9` and `shellcheck -S error` if installed, `python3 .claude/scripts/check_versions.py`. Put the results in the brief so neither reviewer repeats them.
3. **Run layer 1.** Write a findings-free brief (what changed and why, the lint results, a sanity checklist for the surfaces touched, the NOT-to-flag list) to a scratch file, then:
   ```bash
   .claude/scripts/codex_review.sh --range <prev>..HEAD [--uncommitted] --brief <brief-file> --out <scratch>/codex-round<N>.md
   ```
   `--out` must be outside the repo. Apply every CRITICAL/HIGH, keep a one-line fix history per finding in a separate history file, re-run with `brief + history` concatenated into a new file (never append to the brief), until no open CRITICAL/HIGH (4 rounds at most, then stop and ask).
4. **Dispatch `jezpress-code-reviewer`** (layer 2) with the scope, the brief, and - named separately, to open only after its blind pass - every layer-1 round report plus the history file.
5. **Read the verdict.** Surface every CRITICAL/HIGH to Abner, with its evidence label: `CONFIRMED` outweighs `PLAUSIBLE`, and a `PLAUSIBLE` finding on the save path is worth reproducing before acting.

## Acting on findings

- **CRITICAL / HIGH** - fix before declaring the change complete. Re-audit if the fix is material.
- **MEDIUM** - Abner's call: fix now or note it as known.
- **LOW** - commit message.

## Dispatch preference

Layer 1 is always `.claude/scripts/codex_review.sh`; layer 2 is always the committed `jezpress-code-reviewer` agent. Never substitute `general-purpose` for either. If subagents are unavailable, do a **manual self-review** against this guide and **say so explicitly** - a manual pass is not the independent gate, and Abner needs to know which one he got.
