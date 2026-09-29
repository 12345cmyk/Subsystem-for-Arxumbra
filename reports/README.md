# Subsystem-for-Arxumbra — Version Dossiers (`reports/`)

## Tracked Versions (Newest First)
- `v0.0.0.0.0.0-s9` — `reports/v0.0.0.0.0.0-s9.md` (Cargo package `0.0.9`)
- `v0.0.0.0.0.0-s8` — `reports/v0.0.0.0.0.0-s8.md` (Cargo package `0.0.8`)
- `v0.0.0.0.0.0-s7` — historical entry in `reports/registry.json` (no standalone markdown dossier; pre-s8 dossiers were never written and must not be fabricated retroactively)
- `v0.0.0.0.0.0-s6` — historical entry in `reports/registry.json` (no standalone markdown dossier)
- `v0.0.0.0.0.0-s5` … `v0.0.0.0.0.0-s0` — historical asset-less releases in `reports/registry.json`

## Purpose and Schema (`schema: 1`)
This directory stores the machine-readable release registry (`reports/registry.json`) and per-version engineering dossiers (`reports/<version>.md`).
- `reports/registry.json` is an append-only ledger using `"schema": 1`.
- Entries for shipped versions (`"status": "released"`) are immutable forever.
- Pre-`v0.0.0.0.0.0-s8` releases (`s0` through `s7`) are seeded from verified GitHub API metadata; `v0.0.0.0.0.0-s7` retains `"report": "reports/v0.0.0.0.0.0-s7.md"` for schema uniformity even though no pre-s8 markdown dossier was ever authored.

## Two-Phase Append Procedure for Next Version (`v0.0.0.0.0.0-s10`)
1. **Phase 1 (Pre-tag / Tagged Commit):**
   - Create `reports/v0.0.0.0.0.0-s10.md` with `report_version: 1` and fill all pre-release sections (code stats, local gates, fixation hashes). Mark post-release fields with `STATUS: PENDING RELEASE RUN`.
   - Append a new entry to `reports/registry.json` with `"status": "candidate"`.
   - Run local gates and dry-run CI on `master`, then tag the exact green commit.
2. **Phase 2 (Post-release Commit on `master`):**
   - Update `reports/v0.0.0.0.0.0-s10.md` to `report_version: 2 (FINAL)` with run ID, job durations, asset sizes, API digests, and local verification results.
   - Transition the `v0.0.0.0.0.0-s10` entry in `reports/registry.json` from `"candidate"` to `"released"`.
   - Commit Phase 2 to `master` without moving the annotated tag.
