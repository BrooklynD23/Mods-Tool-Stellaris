# Stellaris Mod Doctor — design documentation

Design phase, with the M0 skeleton in place: `smd-core` contracts as stubs and the
`xtask lint` gates. These documents are the specification the implementation is built
against. Where a document describes a gate or a guarantee that is not built yet, the
lint ledger in [architecture/13-testing.md](architecture/13-testing.md) says so.

## Read in this order

| # | Document | Answers |
| --- | --- | --- |
| 1 | [architecture/01-overview.md](architecture/01-overview.md) | What the tool is, its layers, its principles |
| 2 | [adr/0001-tech-stack.md](adr/0001-tech-stack.md) | Why Rust + Tauri, and what was rejected |
| 3 | [adr/0002-extensibility-contract.md](adr/0002-extensibility-contract.md) | How "add or remove without touching anything else" is enforced |
| 4 | [architecture/02-modules-and-crates.md](architecture/02-modules-and-crates.md) | Crate layout, responsibilities, size gates |
| 5 | [architecture/03-data-contracts.md](architecture/03-data-contracts.md) | The shared types everything speaks |
| 6 | [architecture/04-process-architecture.md](architecture/04-process-architecture.md) | Threads, stages, channels, cancellation, engine⇄UI boundary |
| 7 | [architecture/05-ingest.md](architecture/05-ingest.md) | Discovery, descriptors, load order |
| 8 | [architecture/06-parsing.md](architecture/06-parsing.md) | Clausewitz and localisation parsing, caching |
| 9 | [architecture/07-index-and-resolver.md](architecture/07-index-and-resolver.md) | The model of what the game sees; who wins |
| 10 | [architecture/08-rule-engine.md](architecture/08-rule-engine.md) | The `Rule` plugin contract and the v1 catalogue |
| 11 | [architecture/09-policy-and-scoring.md](architecture/09-policy-and-scoring.md) | Severity as data; explanation catalogue |
| 12 | [architecture/10-game-profiles.md](architecture/10-game-profiles.md) | The multi-game seam |
| 13 | [architecture/11-gui.md](architecture/11-gui.md) | Screens, virtualisation, state ownership |
| 14 | [architecture/12-performance.md](architecture/12-performance.md) | Budget and the decisions that meet it |
| 15 | [architecture/13-testing.md](architecture/13-testing.md) | Levels, fixtures, calibration harness |
| 16 | [architecture/14-packaging.md](architecture/14-packaging.md) | Artefacts, compatibility matrix, permissions |
| 17 | [architecture/15-roadmap.md](architecture/15-roadmap.md) | Milestones, exit criteria, deferrals |
| — | [REVIEW-2026-08.md](REVIEW-2026-08.md) | **The M0 gate — what must be settled before the first line of code** |
| — | [REVIEW-2026-10.md](REVIEW-2026-10.md) | **The October register — the H-gates on resolution, completeness, boundaries, and exit criteria** |
| — | [VERIFY.md](VERIFY.md) | **Assumptions that must be confirmed against a live install** |
| — | [GROUND-TRUTH-PRIOR-ART.md](GROUND-TRUTH-PRIOR-ART.md) | What documented prior art says about those assumptions — corroboration, not observation |

## The design in one page

**Problem.** Stellaris mods overwrite each other's files, objects, and localisation keys.
The launcher shows none of it, and the failure usually surfaces hours into a game.

**Approach.** Read the playset, build a model of what the game will actually load, find
every place two mods contend, decide who wins, and rank the outcomes by how likely they
are to matter. Ranking arrives at M5; the first release (M1) reports conflicts and their
certainty unscored (`REVIEW-2026-10.md`, H34).

**Stack.** Rust engine (parse throughput, one static binary), Tauri + Svelte GUI (dense
data UI without an Electron-sized download), TOML for everything game-specific.

**Structure.** Four layers with downward-only dependencies:

```
Presentation   smd-gui · smd-cli · smd-report
Analysis       smd-rules (detects) · smd-policy (values)
Model          smd-ingest · smd-parse · smd-index
Foundation     smd-core (contracts) · smd-fs · smd-profiles
```

**Extension.** Three seams and nothing else: a **parser**, a **rule**, or a **game
profile**. Each is one self-registering file. Severity, prose, and folder semantics are
data, so tuning the tool never touches its logic.

**Key idea.** The hard problem is not finding collisions — it is separating the
intentional overwrite from the silent breakage. Three things do that work:
`MergeMode` (what a collision *means* in that directory), `body_hash` (identical copies
are not conflicts), and the reference index (who was relying on the thing that got
replaced).

**Stance.** Read-only. Deterministic. Never aborts on a bad file. Never says a mod is
broken — says what got discarded, and shows you the bytes.
