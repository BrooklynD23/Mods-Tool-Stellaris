# Kanban — Stellaris Mod Doctor

Last updated: 2026-11-22 · branch `work` · last green CI: run 44521481527 (windows-latest, ubuntu-latest, lint)

**Updating this board.** Every agent that finishes a slice updates this file in the same
commit as the slice: move the card to its new column, add the commit sha, and bump
"Last updated". One card per slice. A card moves to **Verified** only with evidence
(a test, a CI run, or an owner observation), named on the card. Detail lives in
[`docs/HANDOFF.md`](docs/HANDOFF.md); this board is the index.

Columns: **To do** (code, unblocked) · **In progress** · **Built, untested** ·
**Verified** · **Blocked on owner** · **Deferred**.

## In progress

_(empty)_

## To do

Code that needs nothing from the owner. Ordered by value.

- [ ] Tests for the scaffolded rules: M2 `core.symbol_*`, M3 `pdx.*`, M4 `core.loc_collision` + `dep.*` + `meta.*` (owner deferred testing on 2026-11-19)
- [ ] RuleCases for `dep.*` / `meta.*`: extend `PlaysetBuilder` to set dependencies and descriptors first
- [ ] H30 golden extraction corpus, one fixture per `[[dir]]` shape (M2 exit)
- [ ] `locyml` golden corpus and fuzz target
- [ ] Lint gates H35–H39 still unbuilt: struct-literal ban, rule ↔ `explain/*`, placeholders, `[[dir]]` golden coverage, `--selftest`/`registry.lock`, Mermaid from `layers.toml`, `dir-audit`
- [ ] Move remaining Paradox-family literals into profile data: `mods_subdir`, `descriptor.mod`, `ugc_`/`pdx_` prefixes (M8 leakage list)
- [ ] Index: `winning` bitset, `reorder`, a merge scope separate from the domain
- [ ] On-disk parse cache (SQLite) and layer-1 stat cache (doc 06, doc 12)

## Built, untested

Compiles, passes lint and existing tests on Windows CI; behaviour unproven on real data.

- [ ] M1 remainder: H22 final-path check, H50 incomplete blocks `Deleted`, H6 strict profile loader, H12 qualifier sentences, wildcard lint, fuzz scaffolding — `1b0804d`
- [ ] M2 parser: `smd-parse`, `DefinitionTable`, `core.symbol_collision`, `core.symbol_deleted` — `e4a6e75`
- [ ] M3 resolution: precedence decider (H19), event/namespace/define tables, `pdx.event_id_dup`, `pdx.namespace_share`, `pdx.defines_overlap`, `pdx.replace_path_shadow` — `1ff27f8`
- [ ] M4: `dep.missing`, `dep.order`, `meta.version_drift`, `meta.descriptor_mismatch` — `d1d8eb5`; `locyml`, `LocTable`, `core.loc_collision`, additive `residual_lost` — `1ff27f8`
- [ ] M5 policy: `smd-policy`, `smd scan --score`, `smd suppress` — `d1d8eb5`
- [ ] M8 second game: `profiles/ck3.toml`, `--game` — `d1d8eb5`
- [ ] RE: `.github/workflows/release.yml` (never triggered), `.github/workflows/fuzz.yml` (never run)
- [ ] Windows reparse/cloud-placeholder handling in `smd-fs/src/sys_other.rs` (type-checked; never met a real OneDrive, dedup or junction file)

## Verified

- [x] M0 skeleton, M0.5 prior art, M1 path-level slice — merged to `main` via PR #3
- [x] `smd locate`: Steam libraries, GOG, Xbox, registry, OS defaults, every candidate shown — CI 44521481527
- [x] Discovery origins: `mod_file`, `workshop_dir`, `unregistered_dir`, `loose_archive`, `launcher_db_only` — CI + synthetic smoke run
- [x] Launcher join on `gameRegistryId`/`steamId`/`pdxId`/path; unmatched ⇒ `Unknown` — tests
- [x] Namespaced mod keys `steam:`/`pdx:`/`h-`; one launcher DB read per scan — tests
- [x] Lazy hashing (hash only contended paths): 5.4 s → 1.8–2.5 s on a synthetic 50-mod tree — measured
- [x] `dev.bat` one-click runner; survives `(x86)`, `&`, empty args — real `cmd.exe` probes
- [x] `ModDoctor.ps1`: UTF-8, real exit codes, `smd locate` integration — PowerShell 5.1 parse check
- [x] CI: clippy on Windows, `--no-fail-fast`, duplicate steps removed — CI 44521481527

## Blocked on owner

- [ ] Update Stellaris to ≥ 4.5.2 (installed: 4.4.6); every M0.5 probe is specified on 4.5.2+
- [ ] Run the probe mods in `tools/probe-mods/`, record in `docs/VERIFY.md` — unblocks `merge` for localisation (V20, V39) and `on_actions` (V18)
- [ ] Full `dev.bat` run on the gaming PC; send `reports\latest.md` — first real test of all 13 rules
- [ ] Dump `launcher-v2.sqlite` schema (H43); observe `-wal`/`-shm` open vs closed (H48); confirm `gameRegistryId`/`pdxId`/`archivePath` contents and `pdx_<id>.mod` naming
- [ ] Three real playsets for the M1 exit, ≥ 20 same-path conflicts hand-checked each; native 71-mod timing (M6 input)
- [ ] Confirm the Game Pass user-dir candidate and `[version] verified_range = ["4.*"]`
- [ ] Labelled ground truth over three playsets (M5 calibration exit)
- [ ] A CK3 install to prove the M8 exit
- [ ] Code-signing certificate and release-pipeline decision (H33)
- [ ] GUI decision for M7: Tauri + Svelte vs browser-first (ADR-0001)

## Deferred

- [ ] M6 performance tuning — needs the native measured workload
- [ ] M7 GUI — needs the owner's GUI decision
- [ ] M9 depth, M10 ecosystem — after M1–M8 exits
- [ ] macOS — deferred by owner decision 2026-10-29
