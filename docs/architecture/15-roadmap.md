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
| 2 | M0.5 — Ground truth, versioned, and the measured workload | 10–15 |
| 3 | M1 — First release: inventory, verified order, same-path conflicts | 18–26 |
| 4 | M2 — Parser, symbols, and concurrency shape | 17–25 |
| 5 | M3 — Filename precedence, `replace_path`, and the Paradox rules | 7–11 |
| 6 | M4 — Localisation, dependencies, versions | 7–10 |
| 7 | M8 — Second game | 6–10 *(15–25 if the abstraction leaks)* |
| 8 | M5 — Policy and calibration against labelled ground truth | 18–27 |
| 9 | M6 — Performance tuning against the measured budget | 12–20 |
| 10 | M7 — GUI | **25–40** |
| 11 | M9 — Depth | 20–30 |
| 12 | M10 — Ecosystem | 12–20 |
| — | RE — Release engineering *(a minimal slice inside M1; the rest alongside, from M5)* | 8–12 |
| | **Total** | **~165–255 days · 8–12 months** |

The October review (`docs/REVIEW-2026-10.md`, H30–H34) moved days, not scope, between
rows: the launcher order source and a release-validation slice moved into M1, the
workload measurement into M0.5, a golden corpus into M2, and a labelled ground-truth set
into M5. M0.5 also grew in substance: 42 items, each needing a stamped observation, and
eight-plus probes each run in an isolated playset in both orders. The net is roughly +10
to +20 days, and every one of them buys an exit criterion that a stub cannot pass.

The ten-bullet version of this document read like a quarter of work. It is closer to a
year, and an unstated factor of four is how a project gets abandoned at M6 with nothing
shipped. Three consequences worth taking as decisions rather than discovering:

- **M7 is the most under-scoped item**, by a wide margin. Five virtualised views, a
  result-set query surface (§ `11`), a diff pane, an IPC type-generation seam, and a
  usability test with a real non-technical tester who will find things.
- **M1 is the first shippable release, and it is deliberately small** — the smallest
  output that is true without the parser, the resolver, or the policy layer (§ M1). If the
  schedule has to be cut, it is cut after a milestone that shipped, not by compressing the
  one in progress. *Shippable* means the release gate in § RE passed, not that the
  feature list is done.
- **The GUI is the branch point.** M0–M6 produce a complete engine and CLI with no GUI
  commitment spent; the M7 checkpoint below is where that money is either spent or saved.

Nothing here starts until the contract items in `docs/REVIEW-2026-08.md` § A are settled.
They are type definitions, and every one of them costs an order of magnitude more once
code depends on it. The H-gates in `docs/REVIEW-2026-10.md` attach to the milestones
named in that register; a milestone does not close with one of its H-gates open.

## How an exit criterion is written

The August plan's exit criteria could each be met by doing nothing: a parser that emits
no facts produces no diagnostics; a policy that scores everything Low produces no false
positives at ≥high; a budget can be "met" on a workload nobody measured. An exit
criterion that a stub passes is not a criterion. So every exit below obeys four rules
(H30):

1. **A stub fails it.** Every criterion has a positive floor — facts emitted, conflicts
   found, labelled cases detected — alongside any ceiling on errors. Ceilings alone are
   satisfied by silence.
2. **Its inputs exist before it is judged.** A criterion that needs a measurement names
   the earlier milestone that produced it. No milestone measures the thing its own exit
   is graded against and then grades itself.
3. **It names its evidence artefact.** A checked-in file, a CI job, or a manifest diff —
   not "verified". If the artefact cannot be pointed to in review, the box is unticked.
4. **It is graded against something the author did not write.** Where the criterion is
   about correctness, at least one input comes from the running game (M0.5 probes) or
   from a real playset labelled by hand (§ `13` "Labelled ground truth"), not only from
   fixtures written to exhibit the shape under test.

## M0 — Skeleton (contracts before code) · 5–8 days

- Cargo workspace, all crates present as stubs.
- `smd-core` contracts complete (§ `03`), including the § A items from the review register
  — they are itemised there and are not duplicated here.
- `cargo xtask lint` with the size, dependency-direction, and layer-map gates **live from
  commit one** — a size limit added later is a limit that never gets enforced.
- `LICENSE-MIT` and `LICENSE-APACHE` present (§ `14`).
- CI matrix green on an empty build.

*Exit:* `cargo test` passes; adding a crate with an upward dependency fails the build.

## M0.5 — Ground truth, versioned, and the measured workload · 10–15 days · **blocks M1**

The design was written without a Stellaris installation to hand, and `docs/VERIFY.md`
records every claim that depends on one. Those claims are not background detail: load
order direction, `replace_path` semantics, and per-directory merge behaviour determine
whether the tool's output is correct or confidently inverted.

- Acquire the game, the launcher, and a ~200-mod workshop playset representative of what
  users actually run.
- Build probe mods: two minimal mods defining the same object with distinguishable
  values, in each of the directory shapes under question — including differently named
  files in a FIOS directory (`common/traits`), and a mod file whose name sorts *below*
  the vanilla file it collides with, because "vanilla is always lowest" is an assumption,
  not a finding.
- Load in **both orders**, observe in game, record which value the game used. **Each run
  uses an isolated playset containing only the probe pair** (and vanilla); probe mods
  overwrite overlapping keys, and a third mod in the playset contaminates every answer it
  touches (`tools/probe-mods/README.md`). The run log records each run's canary
  readouts, which prove no other probe was loaded.
- Answer **every** `VERIFY.md` checkbox, with the finding recorded inline in that file —
  not in an issue, not in a commit message. **Each answer is versioned:** every item
  (V01–V42) carries an *Observed* record — game version and checksum, launcher version,
  OS, date, probe id — kept separate from its *Prior art* record. Prior art agreeing with
  a probe is corroboration; prior art alone is not an answer, and never ticks a box. The
  pass runs on game ≥ 4.5.2, the live version when this was written.
- **Measure the reference workload (H32).** Over three real 200–400 mod playsets: file
  count, byte total, and the by-extension histogram, replacing the provisional figures in
  § `12` "Reference workload". This was scheduled at M6, after M2 had already chosen a
  read strategy and a cache layout from the mean file size the measurement produces. The
  playsets are acquired here anyway; measuring them is an afternoon's script.
- Land the merge-mode and `replace_path` answers in `profiles/stellaris.toml`, replacing
  the `merge = "unknown"` placeholders (§ `10`).
- Re-run review R5 (Paradox domain accuracy) against the documented prior art. It checks a
  different thing than the probe mods do: prior art against the design, versus the design
  against a running game. Neither substitutes for the other.

*Exit:* no unticked box in `VERIFY.md` — every V-item has an Observed record of VERIFIED
or CONTRADICTED stamped with game version, checksum, and launcher version. **A single
INCONCLUSIVE item blocks M0.5**: it is re-probed with a changed design until it resolves,
and is never rounded to the answer the design hoped for. No `merge = "unknown"` in the
profile that is not also an open item there; § `12` "Reference workload" restated from
the three measured playsets, with the time and memory budget rows re-derived so each
column sums. *Evidence:* the `VERIFY.md`
diff, the probe-run log, the measured histogram checked in under `bench/workload/`.

The estimate is 10–15 days, not the August 4–6: eight-plus probes, each loaded in both
orders in its own isolated playset with a game restart per run, a live launcher schema
and locking pass on Windows, the workload measurement, and the R5 re-run. It is still the
cheapest milestone per decision it settles, and it is blocking, which is the entire
point. Every day of M1–M4 built on a wrong load-order direction is a day that has to be
re-verified afterwards.

## M1 — First release: inventory, verified order, same-path conflicts · 18–26 days

The smallest output that is **true without anything that is still unverified** (H34):
no parser, no object or key resolution, no merge-mode semantics, no scores. A same-path
collision is decided by the first two stages of the resolution pipeline (§ `07`
"Resolution pipeline": Stage A — file overlay, Stage B — surviving files) alone —
whole-file replacement by path holds in every directory regardless of `merge` — so it is
the one conflict class that can be reported before Stages C and D exist. Its verdicts
carry `Mechanism::SamePath` or `Mechanism::ReplacePath` and nothing finer. Everything in
the release is read-only, and everything it cannot establish it says it cannot establish.

- **Read-only playset inventory.** `smd-fs`: path normalisation, Steam library
  enumeration, hashing. `smd-ingest`: descriptors, discovered mods, the vanilla install.
  Every source listed with its identity, path, version, and descriptor diagnostics, all
  read through the input limits of § `05` "Trust boundaries". Archive mods are
  inventoried, not read: their content is reported as unread, with a completeness
  warning, until the archive boundary (H23) closes.
- **Verified enabled order — moved here from M3.** `LauncherV2Sqlite` with the consistent
  snapshot protocol (§ `05` "Snapshotting `launcher-v2.sqlite`"), then `DlcLoadJson`. The report states, per source, **whether it is enabled and how that
  is known**, separately from **whether its position is known** (§ `03`, activation
  certainty versus order confidence; § `05` "Load order"). When neither order source verifies, the release
  lists overlaps **without a winner** and says so; `DirectoryScan` alone never yields a
  winner and never marks a discovered mod as enabled.
- **Same-path conflicts only.** `smd-index`: the file overlay and `SurvivingFiles`.
  `smd-rules`: `core.file_overwrite`. `replace_path` shadowing of **vanilla** is stated;
  shadowing of earlier *mods* is reported as undetermined, with the
  `ResolutionGap::ReplacePathScope` gap, until M0.5's answer lands in the profile.
- **Bounded evidence.** Each finding carries participants, mod-relative paths, sizes, and
  content hashes — never file bodies — with a stated cap on participants rendered per
  finding and on total output, and an explicit "… and N more" when it binds.
- **Completeness warnings.** Every file that was unreadable, changed during the scan, or
  over the size cap is counted and listed (`ParseCompleteness::Unparsed`, § `03`); the
  report header prints `ScanCompleteness` and both certainty badges — activation and order
  (`Finding.certainty`, § `08` "Completeness and certainty") — even with zero findings. A
  partial scan never prints "no conflicts".
- **Safe static output.** `smd-report`: table, JSON, and Markdown, redacted by default,
  with every mod-supplied string escaped for its sink (§ `11` "Output safety"; § `14`
  "What the app is allowed to touch" and "Privacy"). No HTML until M10, and not before
  H27 closes.
- **No severity and no ranking.** M1 reports mechanism, participants, and outcome. Policy
  arrives at M5, and a score assigned before calibration exists is a guess printed in
  bold.
- **The M1 lint gates** (H36): the shared-enum wildcard and struct-literal bans, the
  rule↔explanation and placeholder checks, `[[dir]]` golden coverage, and the
  `--selftest`/`registry.lock` diff — the first rule, the first explanation, and the first
  release binary all appear in this milestone, so their gates do too (§ `13` "Lint
  guarantees").
- **The release-validation slice of RE** (H33): per-target CI artefacts with checksums,
  and the release gate in § RE run against them.

*Exit:* all of —

1. On a fixture playset with *k* seeded same-path collisions, exactly *k* are reported,
   with the correct winner where order is verified and no winner where it is not. A
   same-path finding with an `Unreadable` or `ChangedDuringScan` contribution never states
   a winner; a `DirectoryScan`-only playset yields only `Conditional` findings; a seeded
   unreadable file produces a completeness warning and suppresses any "clean" verdict. A
   stub reporting nothing fails on the first clause.
2. On **three real playsets** from M0.5, the enabled set and order match what the
   launcher shows, including one run with the launcher open, and a hand-checked sample of
   ≥ 20 reported same-path conflicts per playset is correct against the files on disk.
3. The release gate in § RE passes on the artefacts CI built — not on a local
   `cargo build`.
4. These gates are closed: H1 (Stages A and B), H6, and H9 from the resolver set (H7,
   the skeleton reconciliation, closes before M1 starts — REVIEW-2026-10);
   H11 (path-level), H50, H12 (contract), H14, H15, H16 (sources and
   paths), H17 from the integrity set; H20–H26 and H29 from the boundary set, with the
   adversarial fixtures of § `13` "Adversarial inputs" passing; H33–H36 and H39 from
   this milestone's own. H20 and H29 need the live-install answers M0.5 produced.

*Evidence:* the fixture assertions in CI; the three per-playset check sheets under
`tests/release/`; the release-gate log attached to the draft GitHub Release.
**First point of real user value, and the first release.**

## M2 — Parser, symbols, and concurrency shape · 17–25 days

- `smd-syntax` + `smd-clausewitz`, event-driven, with recovery and an iterative,
  depth-capped descent.
- Fact extraction driven by `profiles/stellaris.toml`.
- `DefinitionTable`, symbol resolution, `body_hash`.
- `core.symbol_collision`, `core.symbol_deleted`.
- The `AdditiveSplit` extractor for `merge_by_key_additive_body` directories (H4); the
  rule that reads it lands at M4.
- The parse-cache key including the effective extraction class, and cached completeness
  and diagnostics replayed on warm scans (H10, H11).
- Fuzzing wired up.

And four things that are **structural, not optimisations**, which is why they are here and
not in M6:

| Shape | Why it cannot be retrofitted |
| --- | --- |
| The cache key, complete: `content_hash ‖ parser_id ‖ parser_version ‖ CONTRACT_VERSION ‖ profile_hash ‖ ExtractionClass` (§ `06`, H10) | Entries written before the key is versioned are unversioned *by construction* — there is no way to tell later whether a cached fact was produced under the current contract |
| Arena ownership and lifetimes for the fact stream | Deciding who owns a fact's storage after the code exists is a rewrite of every extractor |
| `Send`-clean fact emission (`RawFact` string-based, `Fact` interned and never serialised) | A fact type that captures a scan-local interner cannot cross a thread or a cache boundary, and the compiler only says so once you try |
| Cancellation threaded through every outer loop | Cancellation reaches a stage only if every loop between here and there checks for it. Adding it later means touching all of them anyway |

None of these are acquired by adding `par_` in front of an iterator later, which is the
assumption that put them in a performance milestone in the first place.

- **A golden extraction corpus** (H30; § `13` "Golden corpus and fact floors"):
  synthetic files covering every `[[dir]]` shape in `profiles/stellaris.toml`, each with
  a checked-in expected fact list.

*Exit:* all of —

1. **Positive floors.** The golden corpus extracts its expected facts exactly — no fact
   missing, none extra. On the 200-mod workshop directory, every file the profile
   classifies as script yields ≥ 1 fact or a diagnostic explaining why not, and the
   definition count lands within the range predicted from the M0.5 histogram. An empty
   parser fails both.
2. **Ceilings.** Zero panics; < 0.5% of files producing diagnostics; and no file
   silently skipped — a skip is a diagnostic and counts against the ceiling.
3. Determinism holds across a thread-count sweep, and a randomised cancel-under-load
   stress test passes.

*Evidence:* the golden corpus snapshot; the per-playset fact and diagnostic counts
checked in beside the M0.5 histogram. **If this milestone is painful, the whole design is
in question — which is why it is early.**

## M3 — Filename precedence, `replace_path`, and the Paradox rules · 7–11 days

The launcher order source moved to M1. What is left is the part of winner resolution that
needs the parser: object and key resolution over the surviving files, and the precedence
tuple's filename axis (§ `07`).

- Key resolution by `merge`, the precedence tuple, `replace_path` shadowing, `live`
  flags.
- `pdx.event_id_dup`, `pdx.namespace_share`, `pdx.defines_overlap`,
  `pdx.replace_path_shadow`.
- Explanation and advice keyed to the precedence axis that decided (H19): "change the load
  order" only where `rank` decided, a filename where the FIOS/LIOS axis did.

*Exit:* correct winner reported for a hand-verified 30-mod playset, which must include a
FIOS directory (`common/traits`) case, a differently-named-files case, and one mod
shipping two files that define the same key — without them the filename axis of the
tuple and the `Contender` winner are untested (H2) — and every winner matches the M0.5 probe
observation for that directory shape.

## M4 — Localisation, dependencies, versions · 7–10 days

- `smd-locyml`; `LocTable`; `core.loc_collision`, with `localisation/replace/` as the
  higher precedence tier (H3).
- Additive-body resolution over `AdditiveSplit` facts (H4).
- `dep.missing`, `dep.order`, `meta.version_drift`, `meta.descriptor_mismatch`.
- Markdown report.

*Exit:* the full v1 rule catalogue from `08` is implemented, every rule passing all five
`RuleCase` kinds, and a `localisation/replace/` case and an `on_actions`-shaped
additive case resolve as the M0.5 probes observed.

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

## M5 — Policy and calibration against labelled ground truth · 18–27 days

- `smd-policy`: weights, modifiers, bands, explanation catalogue.
- Suppression layers, **and their persistence**: settings and user suppressions read and
  written under the OS config dir, versioned, human-editable, and shared as a file.
- `cargo xtask calibrate` with the committed regression corpus, the ranking metric, and
  the 20% holdout; `cargo xtask ground-truth` with its checked-in manifest (§ `13`).
- **Start the Apple Developer enrolment and the code-signing certificate paperwork here.**
  Both have lead times measured in weeks, and neither blocks anything until it is the only
  thing blocking a release.

- **A labelled ground-truth set** (H31; § `13` "Labelled ground truth") over three real
  playsets, in two parts: a precision sample drawn from the tool's findings, and a recall
  census drawn *independently* of them — seeded conflicts from a synthetic mod pair added
  to each playset, plus conflicts listed by a reference enumerator sharing no code with
  the tool. Each item is labelled by hand — real conflict, benign, or wrong — against the
  files and, where M0.5 established it, the running game.

*Exit:* all of —

1. **Recall floor.** Every cited `known_bad` entry detected at ≥high; every seeded
   conflict in the census reported (and the seeded near-miss not); *and* ≥ 80% of the
   independently enumerated real conflicts scored ≥high — a real conflict the tool did not
   report at all counts as a miss.
2. **Precision ceiling.** Zero false positives at ≥high on `known_good`, *and* ≤ 10% of
   ≥high findings in the precision sample labelled benign or wrong.
3. **Ranking.** Mean reciprocal rank of the seeded incompatibility reported, and not
   lower than the previous calibration run.
4. **Holdout.** All three reported on the 20% holdout separately; a holdout
   more than one fixture worse than the tuning set is an overfit, not a pass.

Lowering every weight satisfies clause 2 and fails clause 1; raising every weight does
the reverse; rescaling both shows up in clause 3. That is the point of having all four.
**The tool becomes trustworthy here, not at M4** — and "trustworthy" means measured
against labels, not against the absence of loud findings.

## M6 — Performance tuning against the measured budget · 12–20 days

The shapes landed in M2. This milestone is measurement, and the changes measurement
justifies.

- SQLite parse cache with batched writes; the stat-keyed layer in front of it (§ `06`).
- Tuning: pool sizing, chunk sizes, buffer pooling, the mmap threshold if one is
  warranted.
- Criterion benches, split CPU-only and full-pipeline (§ `13`), plus the synthetic corpus
  generator, its histogram taken from the workload M0.5 measured.

The reference workload is **not** measured here. It was, in the August plan — which meant
M2 chose a read strategy and a cache layout from a provisional mean file size, and M6
then graded itself against a budget it had just written. It is measured at M0.5 and
restated there (H32).

*Exit:* the budget in `12-performance.md`, as restated at M0.5, is met on the measured
workload on the reference machine, with every column summing to its stated total. If the
budget has to move, it moves in a reviewed change to § `12` *before* the run that is
graded against it. *Evidence:* the `bench-io` trend run on the Windows runner and a
reference-machine run log.

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

## RE — Release engineering · 8–12 days, a slice in M1 and the rest from M5 onward

Not a milestone that happens on the afternoon of the tag. The August plan promised a
release at M1 and started release engineering at M5, so the first release had no
artefact pipeline behind it. The slice that validates an artefact moves into M1; signing,
notarisation, and the updater stay here.

**The release gate (H33), for every release including M1.** No tag is published until,
on the artefacts CI produced for that tag:

1. Checksums verify, and each target's binary runs `--selftest` clean against
   `registry.lock`.
2. Each target's binary scans the fixture playset and its output matches the golden
   report byte for byte.
3. The Windows binary scans the three M0.5 real playsets on a machine with the game and
   launcher installed, and the `cargo xtask ground-truth` manifest diff is empty or
   explained in the release notes (§ `13`).

Then:

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
