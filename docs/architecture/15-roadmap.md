# 15 — Build order and milestones

Sequenced so that each milestone is independently useful and each one de-risks the next.
The riskiest assumptions are tested early: **M0.5 proves the game behaves as the design
assumes**, **M2 proves the parser can handle real workshop content**, and **M8 proves the
game abstraction is real**.

Milestone ids are **stable labels, not sequence numbers**. M8 and M6 moved; their names
did not, because other documents cite them. The order below is the order they are built
in.

## Schedule, honestly

One experienced Rust developer, working days, excluding certificate and notarisation lead
times and excluding the re-run of review R5.

| Order | Milestone | Days |
| --- | --- | --- |
| 1 | M0 — Skeleton | 5–8 |
| 2 | M0.5 — Ground truth | 4–6 |
| 3 | M1 — File-level conflicts, CLI | 12–18 |
| 4 | M2 — Parser, symbols, and concurrency shape | 15–22 |
| 5 | M3 — Real load order and the Paradox rules | 10–15 |
| 6 | M4 — Localisation, dependencies, versions | 7–10 |
| 7 | M8 — Second game | 6–10 *(15–25 if the abstraction leaks)* |
| 8 | M5 — Policy and calibration | 15–22 |
| 9 | M6 — Performance measurement and tuning | 12–20 |
| 10 | M7 — GUI | **25–40** |
| 11 | M9 — Depth | 20–30 |
| 12 | M10 — Ecosystem | 12–20 |
| — | RE — Release engineering *(runs alongside, from M5)* | 8–12 |
| | **Total** | **~155–235 days · 8–11 months** |

The ten-bullet version of this document read like a quarter of work. It is closer to a
year, and an unstated factor of four is how a project gets abandoned at M6 with nothing
shipped. Three consequences worth taking as decisions rather than discovering:

- **M7 is the most under-scoped item**, by a wide margin. Five virtualised views, a
  result-set query surface (§ `11`), a diff pane, an IPC type-generation seam, and a
  usability test with a real non-technical tester who will find things.
- **M1 is the first shippable release.** If the schedule has to be cut, it is cut after a
  milestone that shipped, not by compressing the one in progress.
- **The GUI is the branch point.** M0–M6 produce a complete engine and CLI with no GUI
  commitment spent; the M7 checkpoint below is where that money is either spent or saved.

Nothing here starts until the contract items in `docs/REVIEW-2026-08.md` § A are settled.
They are type definitions, and every one of them costs an order of magnitude more once
code depends on it.

## M0 — Skeleton (contracts before code) · 5–8 days

- Cargo workspace, all crates present as stubs.
- `smd-core` contracts complete (§ `03`), including the § A items from the review register
  — they are itemised there and are not duplicated here.
- `cargo xtask lint` with the size, dependency-direction, and layer-map gates **live from
  commit one** — a size limit added later is a limit that never gets enforced.
- `LICENSE-MIT` and `LICENSE-APACHE` present (§ `14`).
- CI matrix green on an empty build.

*Exit:* `cargo test` passes; adding a crate with an upward dependency fails the build.

## M0.5 — Ground truth · 4–6 days · **blocks M1**

The design was written without a Stellaris installation to hand, and `docs/VERIFY.md`
records every claim that depends on one. Those claims are not background detail: load
order direction, `replace_path` semantics, and per-directory merge behaviour determine
whether the tool's output is correct or confidently inverted.

- Acquire the game, the launcher, and a ~200-mod workshop playset representative of what
  users actually run.
- Build probe mods: two minimal mods defining the same object with distinguishable
  values, in each of the directory shapes under question.
- Load in **both orders**, observe in game, record which value the game used.
- Answer **every** `VERIFY.md` checkbox, with the finding recorded inline in that file —
  not in an issue, not in a commit message.
- Land the merge-mode and `replace_path` answers in `profiles/stellaris.toml`, replacing
  the `merge = "unknown"` placeholders (§ `10`).
- Re-run review R5 (Paradox domain accuracy) against the documented prior art. It checks a
  different thing than the probe mods do: prior art against the design, versus the design
  against a running game. Neither substitutes for the other.

*Exit:* no unticked box in `VERIFY.md`; no `merge = "unknown"` in the profile that is not
also an open item there.

This milestone is cheap and it is blocking, which is the entire point. Every day of M1–M4
built on a wrong load-order direction is a day that has to be re-verified afterwards.

## M1 — File-level conflicts, CLI only · 12–18 days

- `smd-fs`: path normalisation, Steam library enumeration, hashing.
- `smd-ingest`: descriptors, `DirectoryScan` order source.
- `smd-index`: overlay table.
- `smd-rules`: `core.file_overwrite`.
- `smd-report`: table + JSON, redacted by default.

*Exit:* `smd scan --mods-dir …` lists same-path conflicts on a real playset.
**First point of real user value, and the first release.**

## M2 — Parser, symbols, and concurrency shape · 15–22 days

- `smd-syntax` + `smd-clausewitz`, event-driven, with recovery and an iterative,
  depth-capped descent.
- Fact extraction driven by `profiles/stellaris.toml`.
- `DefinitionTable`, symbol resolution, `body_hash`.
- `core.symbol_collision`, `core.symbol_deleted`.
- Fuzzing wired up.

And four things that are **structural, not optimisations**, which is why they are here and
not in M6:

| Shape | Why it cannot be retrofitted |
| --- | --- |
| The cache key, complete: `content_hash ‖ parser_id ‖ parser_version ‖ CONTRACT_VERSION ‖ profile_hash` | Entries written before the key is versioned are unversioned *by construction* — there is no way to tell later whether a cached fact was produced under the current contract |
| Arena ownership and lifetimes for the fact stream | Deciding who owns a fact's storage after the code exists is a rewrite of every extractor |
| `Send`-clean fact emission (`RawFact` string-based, `Fact` interned and never serialised) | A fact type that captures a scan-local interner cannot cross a thread or a cache boundary, and the compiler only says so once you try |
| Cancellation threaded through every outer loop | Cancellation reaches a stage only if every loop between here and there checks for it. Adding it later means touching all of them anyway |

None of these are acquired by adding `par_` in front of an iterator later, which is the
assumption that put them in a performance milestone in the first place.

*Exit:* parses a 200-mod workshop directory with zero panics and <0.5% files producing
diagnostics. Determinism holds across a thread-count sweep, and a randomised
cancel-under-load stress test passes. **If this milestone is painful, the whole design is
in question — which is why it is early.**

## M3 — Real load order and the Paradox rules · 10–15 days

- `LauncherV2Sqlite` (with the WAL-safe copy protocol, § `05`) and `DlcLoadJson` order
  sources, with the fallback chain and its loud schema-mismatch diagnostic.
- Winner resolution, `replace_path` shadowing, `live` flags.
- `pdx.event_id_dup`, `pdx.namespace_share`, `pdx.defines_overlap`,
  `pdx.replace_path_shadow`.

*Exit:* correct winner reported for a hand-verified 30-mod playset, including one verified
against a launcher that is open while the scan runs.

## M4 — Localisation, dependencies, versions · 7–10 days

- `smd-locyml`; `LocTable`; `core.loc_collision`.
- `dep.missing`, `dep.order`, `meta.version_drift`, `meta.descriptor_mismatch`.
- Markdown report.

*Exit:* the full v1 rule catalogue from `08` is implemented.

## M8 — Second game · 6–10 days *(15–25 if the abstraction leaks)*

Moved here from the end. The stated rationale for this milestone was always "do it before
the API stabilises, while changing it is still cheap" — and the previous sequence put it
*after* M7 had shipped five views built on that API, which is precisely when it is no
longer cheap.

- CK3 or EU4 profile as TOML only.
- Any Stellaris-specific leakage discovered is fixed by moving it into profile data.
- Game selection in the CLI; the GUI picker follows at M7 for free.

*Exit:* CK3 conflicts detected with **zero new Rust in `smd-rules` or above**. If this
requires engine changes, the abstraction failed and the fix is architectural, not a patch
— which is exactly why the wide estimate range exists and why finding out now is worth
the interruption.

## M5 — Policy and calibration · 15–22 days

- `smd-policy`: weights, modifiers, bands, explanation catalogue.
- Suppression layers, **and their persistence**: settings and user suppressions read and
  written under the OS config dir, versioned, human-editable, and shared as a file.
- `cargo xtask calibrate` with the committed regression corpus, the ranking metric, and
  the 20% holdout; `cargo xtask ground-truth` with its checked-in manifest (§ `13`).
- **Start the Apple Developer enrolment and the code-signing certificate paperwork here.**
  Both have lead times measured in weeks, and neither blocks anything until it is the only
  thing blocking a release.

*Exit:* zero false positives at ≥high on the known-good corpus, holdout reported
separately. **The tool becomes trustworthy here, not at M4.**

## M6 — Performance measurement and tuning · 12–20 days

The shapes landed in M2. This milestone is measurement, and the changes measurement
justifies.

- SQLite parse cache with batched writes; the stat-keyed layer in front of it (§ `05`).
- Tuning: pool sizing, chunk sizes, buffer pooling, the mmap threshold if one is
  warranted.
- Criterion benches, split CPU-only and full-pipeline (§ `13`), plus the synthetic corpus
  generator.
- Re-derive the reference workload from measurements of three real playsets, and restate
  the budget rows against it.

*Exit:* the budget in `12-performance.md` is met on the **measured** reference workload,
with every column summing to its stated total.

## M7 — GUI · 25–40 days

- Tauri shell, `smd-api` command surface, generated TS types.
- The result-set query surface: `OpenResultSet` / `Page` / `GroupedQuery`, with epochs
  (§ `11`).
- Overview, Conflicts (virtualised + facets + detail pane), Mods, Report, Settings.
- **The side-by-side diff pane**, moved here from M9. The exit criterion below is a trust
  test, and the diff is the feature the design says produces trust; running the test on a
  build without it is not a test of anything.
- Load-order sandbox as a query override, with the findings table greying while findings
  re-derive.
- Progress, cancellation, diagnostics badge.
- UI string externalisation and one non-English locale, matching the explanation
  catalogue's shape. Retrofitting i18n across five views costs several times what doing it
  once at the start of them does.

**Decision checkpoint, at the start of this milestone.** Tauri is provisional (ADR-0001).
Before the shell work begins, re-take the decision against the browser-first alternative —
CLI plus a self-contained interactive HTML report, escalating to `smd-cli serve` for the
live-engine features. By this point the engine is complete and measured, the packaging
costs are known (§ `14`), and the choice can be made on evidence instead of on preference.
Choosing the alternative removes the WebView2 trilemma, the WebKitGTK skew, the AppImage
weight, the notarisation prerequisite, and the IPC type seam; it costs the live reorder
sandbox and on-demand diff fetch unless `serve` is built.

*Exit:* a non-technical tester diagnoses a seeded conflict without instruction.
**That test, not feature completeness, is the exit criterion.**

## M9 — Depth · 20–30 days

- Reference extraction behind `Capabilities::REFERENCES`, with a typed reference map —
  gated **off** for v1, and this is where the gate opens.
- `core.dangling_reference` (needs `ReferenceTable`).
- Impact analysis in the detail pane.
- `meta.patch_candidate`.
- Intra-source ordering, if M0.5 confirmed it matters.

## M10 — Ecosystem · 12–20 days

- HTML report (self-contained, redacted by default).
- PyO3 binding + wheels.
- GitHub Action for mod authors (`smd scan --preset ci --fail-on critical`).
- Plugin SDK documentation for external rules.

## RE — Release engineering · 8–12 days, from M5 onward

Not a milestone that happens on the afternoon of the tag.

- Per-target build matrix, artefact naming, checksums, provenance attestations.
- Windows signing; macOS signing and notarisation in CI (blocked on the M5 paperwork).
- The updater manifest, its signing key handling, and the opt-in flow.
- A download page that routes a user to the right artefact — including the
  no-WebView2-and-offline case, which routes to the CLI (§ `14`).

## Deferred, deliberately

| Feature | Why not yet |
| --- | --- |
| Reference extraction and impact analysis | Untyped extraction matches `owner`, `root`, `value`, `yes`; impact becomes noise and every score inflates. v1 is the **file and symbol layers**. Opens at M9 with a type map |
| Automatic patch generation | Needs semantic merge, not just detection. Irony already does a version of this. Wrong second problem to solve |
| Writing load order back to the launcher | Breaks the read-only guarantee; needs a backup/undo design first |
| Workshop metadata fetching | Network access changes the trust and privacy story. Needs its own design, and the Steamworks Web API is the only permitted route (§ `14`) |
| Full Clausewitz semantic validation | CWTools' territory; integrate rather than duplicate |
| Non-Paradox games | Only after M8 proves the seam |

## The failure mode this order is designed to avoid

Building the GUI early. It is the most visible part and the most tempting to start with,
and it would lock the `smd-api` surface before the engine's shape is known — every later
engine change would then drag UI churn behind it. The CLI is the engine's real first
consumer, and it stays the fastest way to test everything the engine does.

The second failure mode, and the reason for the schedule table: believing the first one.
A plan that says "ten milestones" and means "a year" produces the same abandonment as
starting with the GUI, one milestone later.
