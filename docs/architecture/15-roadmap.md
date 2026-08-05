# 15 — Build order and milestones

Sequenced so that each milestone is independently useful and each one de-risks the next.
The riskiest assumptions are tested early: **M2 proves the parser can handle real
workshop content**, and **M8 proves the game abstraction is real**.

## M0 — Skeleton (contracts before code)

- Cargo workspace, all crates present as stubs.
- `smd-core` contracts complete (§ `03`).
- `cargo xtask lint` with the size and dependency-direction gates **live from commit one**
  — a size limit added later is a limit that never gets enforced.
- CI matrix green on an empty build.

*Exit:* `cargo test` passes; adding a crate with an upward dependency fails the build.

## M1 — File-level conflicts, CLI only

- `smd-fs`: path normalisation, Steam library enumeration, hashing.
- `smd-ingest`: descriptors, `DirectoryScan` order source.
- `smd-index`: overlay table.
- `smd-rules`: `core.file_overwrite`.
- `smd-report`: table + JSON.

*Exit:* `smd scan --mods-dir …` lists same-path conflicts on a real playset.
**First point of real user value.**

## M2 — Clausewitz parser and symbols

- `smd-syntax` + `smd-clausewitz`, event-driven, with recovery.
- Fact extraction driven by `profiles/stellaris.toml`.
- `DefinitionTable`, symbol resolution, `body_hash`.
- `core.symbol_collision`.
- Fuzzing wired up.

*Exit:* parses a 200-mod workshop directory with zero panics and <0.5% files producing
diagnostics. **If this milestone is painful, the whole design is in question — which is
why it is second.**

## M3 — Real load order and the Paradox rules

- `LauncherV2Sqlite` and `DlcLoadJson` order sources with the fallback chain.
- Winner resolution, `replace_path` shadowing, `live` flags.
- `pdx.event_id_dup`, `pdx.namespace_share`, `pdx.defines_overlap`,
  `pdx.replace_path_shadow`.

*Exit:* correct winner reported for a hand-verified 30-mod playset.

## M4 — Localisation, dependencies, versions

- `smd-locyml`; `LocTable`; `core.loc_collision`.
- `dep.missing`, `dep.order`, `meta.version_drift`, `meta.descriptor_mismatch`.
- Markdown report.

*Exit:* the full v1 rule catalogue from `08` is implemented.

## M5 — Policy and calibration

- `smd-policy`: weights, modifiers, bands, explanation catalogue.
- Suppression layers.
- `cargo xtask calibrate` plus the first known-good/known-bad corpora.

*Exit:* zero false positives at ≥high on the known-good corpus.
**The tool becomes trustworthy here, not at M4.**

## M6 — Performance

- `redb` parse cache, content-addressed.
- Rayon parallelism across all stages; map-reduce indexing.
- Criterion benches and the synthetic corpus generator.

*Exit:* the budget in `12-performance.md` is met on the reference workload.

## M7 — GUI

- Tauri shell, `smd-api` command surface, generated TS types.
- Overview, Conflicts (virtualised + facets + detail pane), Mods, Report, Settings.
- Progress, cancellation, diagnostics badge.

*Exit:* a non-technical tester diagnoses a seeded conflict without instruction.
**That test, not feature completeness, is the exit criterion.**

## M8 — Second game

- CK3 or EU4 profile as TOML only.
- Any Stellaris-specific leakage discovered is fixed by moving it into profile data.
- Game picker in the GUI.

*Exit:* CK3 conflicts detected with **zero new Rust in `smd-rules` or above**. If this
requires engine changes, the abstraction failed and the fix is architectural, not a
patch. Do this before the API stabilises, while changing it is still cheap.

## M9 — Depth

- Side-by-side diff pane.
- Load-order sandbox with live re-resolution.
- `core.dangling_reference` (needs `ReferenceTable`).
- Impact analysis in the detail pane.
- `meta.patch_candidate`.

## M10 — Ecosystem

- HTML report.
- PyO3 binding + wheels.
- GitHub Action for mod authors (`smd scan --preset ci --fail-on critical`).
- Plugin SDK documentation for external rules.

## Deferred, deliberately

| Feature | Why not yet |
| --- | --- |
| Automatic patch generation | Needs semantic merge, not just detection. Irony already does a version of this. Wrong second problem to solve. |
| Writing load order back to the launcher | Breaks the read-only guarantee; needs a backup/undo design first. |
| Workshop metadata fetching | Network access changes the trust and privacy story. Needs its own design. |
| Full Clausewitz semantic validation | CWTools' territory; integrate rather than duplicate. |
| Non-Paradox games | Only after M8 proves the seam. |

## The failure mode this order is designed to avoid

Building the GUI early. It is the most visible part and the most tempting to start with,
and it would lock the `smd-api` surface before the engine's shape is known — every later
engine change would then drag UI churn behind it. The CLI is the engine's real first
consumer, and it stays the fastest way to test everything the engine does.
