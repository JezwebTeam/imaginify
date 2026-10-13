# `.claude/` - Imaginify dev bundle

Committed, repo-safe knowledge and tooling that Claude Code loads when you open this project. Adapted from the JezPress plugin bundle (`jezpress-booking-calendar`): the same audit gates, ledger and `/release` discipline, rebuilt for a Python + CustomTkinter desktop app packaged with PyInstaller. Everything WordPress-specific is gone.

## What's here

| File | Purpose |
|---|---|
| `audit-guide.md` | The feature + pre-release gates: two review layers and the install check. |
| `codex-review-prompt.md` | Standing instructions for the **layer-1** reviewer (OpenAI Codex, GPT-6.1 Sol), tuned to the Rust pipeline / Tauri shell / installer failure modes. Read by `scripts/codex_review.sh`. |
| `agents/jezpress-code-reviewer.md` | The **final (layer-2)** reviewer (Claude Fable): reviews blind, rules on every layer-1 finding, gives the gating verdict. |
| `agents/jezpress-release-verifier.md` | Read-only post-publish gate (Sonnet) at `/release` step 8: runs `verify_release.py` and holds the ledger row on any FAIL. |
| `skills/release/SKILL.md` | The `/release X.Y.Z` skill: audit loop -> bump -> push -> CI test build + hand install check -> tag -> release.yml publishes -> verifier -> LEDGER. |
| `scripts/codex_review.sh` | Runs layer 1: Codex in a read-only sandbox over a diff range plus a brief. Exit 2 = usage error, 3 = Codex not installed / logged in, 4 = Codex run failed. Identical to the plugin fleet's copy except one wording line. |
| `scripts/check_versions.py` | Checks the five version spots + newest `CHANGELOG.md` section agree (`--expect X.Y.Z` for a release; `--notes X.Y.Z` prints release notes). Used by `lint.yml`, `release.yml` and `/release`. |
| `scripts/verify_release.py` | The deterministic checks behind the verifier: tag, version spots at the tag, CHANGELOG, workflow conclusion, release title/notes, every installer asset present and full-size, SHA256SUMS against GitHub's digests, Latest pointer. |
| `LEDGER.md` | Append-only release + audit log, one row per shipped version. |
| `settings.json` | Scoped permission allowlist: routine dev, the build scripts and the release steps auto-approve; `curl`/`wget`, `pip install`, `gh repo`, `gh release edit/upload/delete` and destructive git still prompt. |

> `settings.local.json` is the harness's machine-local permission cache: git-ignored, never part of this bundle. It records approved commands verbatim, so it can hold secrets.

## CI (`.github/workflows/`)

| Workflow | When | What |
|---|---|---|
| `lint.yml` | every push / PR | `cargo fmt --check`, `cargo clippy -D warnings`, pipeline tests, `node --check` on the window JS, helper-script syntax + shellcheck, version spots |
| `release.yml` | tag `vX.Y.Z` (publishes) or "Run workflow" (builds only, artifacts) | version gate -> Windows exe + Inno Setup installer, macOS arm64 + x86_64 DMG -> GitHub release with SHA256SUMS |

## Hard rule: NO SECRETS in this folder

The repo is on GitHub. Never put credentials, tokens, webhook URLs or signing material here. Machine-local secrets live in `~/.claude/memory/` and are read at run time only.
