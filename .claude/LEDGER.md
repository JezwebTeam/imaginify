# Release ledger - Imaginify

**Repo:** `JezwebTeam/imaginify`

> Dev-only, append-only **governance log**. One row per shipped version, **newest first**, appended at the ledger step of `/release` once the audit verdict is known and the release verifier has passed.
>
> **Why it exists:** the `Audit`, `Install check` and `Release` columns are the point - they are the only durable record that the gates ran and the ship steps completed (git tags and `CHANGELOG.md` don't capture that). Keep `Shipped` to **one line** - do not reproduce the changelog here.

## Audit column values
- `clean` - pre-release audit found no Critical/High.
- `fixes (N H)` / `fixes (N C)` - N High/Critical found and fixed before the tag; re-audited if material.
- `blocked -> fixed` - a Critical was present; shipped only after it was resolved (note what).
- `manual` - subagents unavailable, manual self-review only.
- Append the layers that ran, e.g. `fixes (2 H) - codex x2, fable x1`. Write `codex unavailable` when a layer could not run, so a partial gate is visible.
- `pre-ledger` - shipped before this pipeline existed; no recorded gate.

## Install check / Release column values
- **Install check**: `mac + windows` · `mac (windows unchecked)` · `none` (say why)
- **Release**: `published + verified` (release-verifier PASS) · `published` (verifier not run - say why) · `partial` (assets missing - say which)

## Ledger
| Version | Date | Audit | Shipped | Install check | Release |
|---|---|---|---|---|---|
| 1.1.0 | 2026-10-14 | blocked -> fixed (8 C, 2 H); 1 C accepted by Abner (crafted-file memory exhaustion inside a third-party decoder; worker-process isolation planned for 1.1.1); past the 4-round limit by Abner's choice - codex x7, fable x1 | First Tauri (Rust) release: never overwrites files, per-image crop with shapes, sRGB colour, animations kept, smaller downloads | mac (windows unchecked) | published + verified |
| 1.0.1 | 2026-07-02 | pre-ledger | Drag and drop of images and folders; Windows + macOS installer wizards | not recorded | partial (Windows Setup + portable .exe only; no macOS assets, no checksums) |
