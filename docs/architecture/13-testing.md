# 13 — Testing strategy

The tool makes claims about other people's software. A false "this mod overwrites that
one" is worse than no tool, because a user will act on it. Testing is weighted
accordingly.

## Levels

| Level | Scope | Tool | Runs |
| --- | --- | --- | --- |
| Unit | One function, one module | `cargo test` | every commit |
| Property | Parser invariants | `proptest` | every commit |
| Fuzz | Parser robustness | `cargo-fuzz` | nightly, 30 min |
| Golden | Profile classification, report output, **extraction (expected fact lists)** | `insta` snapshots | every commit |
| Integration | Whole pipeline over a fixture playset | `cargo test --test pipeline` | every commit |
| Determinism | Same input, varied thread counts, identical bytes out | feature-gated test | every commit |
| Cache integrity | Identical bytes under two directories with different domains → two cache entries, two fact sets; a warm scan of a malformed file replays its diagnostics and its `Partial` (§ `06`, H10/H11) | `cargo test --test pipeline` | every commit |
| Concurrency stress | Randomised cancellation under load; thread-count sweep 1…2×cores | feature-gated test | every commit |
| Calibration | Regression against a committed symbolic corpus | `cargo xtask calibrate` | every PR touching rules or policy |
| Ground truth | The developer's real subscribed mods vs a checked-in manifest | `cargo xtask ground-truth` | locally, before a release |
| Labelled ground truth | Hand-labelled findings from three real playsets | `cargo xtask calibrate --labelled` | every PR touching rules or policy, from M5 |
| Release gate | CI-built artefacts, per target, plus the real playsets | `--selftest`, golden report, ground-truth diff | every tag, from M1 |
| Bench (CPU) | Lexer, extraction, index build — no IO | `criterion`, Linux | every PR, 10% gate |
| Bench (pipeline) | Full scan, IO included | `criterion`, **Windows** | every PR, trend-reported |
| E2E | GUI flows | `tauri-driver` + WebdriverIO (Win/Linux only) | pre-release |

## Fixtures

`smd-testkit` builds mod trees in code, so a test reads as its own specification:

```rust
let playset = Playset::new()
    .mod_("ethics_civics")
        .file("common/traits/00_species_traits.txt", r#"
            trait_super_adaptive = { cost = 2 }
        "#)
    .mod_("trait_overhaul")
        .file("common/traits/00_species_traits.txt", r#"
            trait_super_adaptive = { cost = 4 }
        "#)
    .build();

let report = scan(&playset);
assert_finding!(report, "core.symbol_collision",
    symbol = "trait_super_adaptive", winner = "trait_overhaul", severity >= High);
```

No fixture directories to hunt through, no `.gitignore` games, and a reviewer can see
the input and the expected output in one screen.

**No copyrighted content in the repo.** The performance corpus is *generated* from a seed
by `smd-testkit`, not vendored. The line is drawn explicitly, because "minimal" is not the
same as "not theirs":

| In fixtures | Rule |
| --- | --- |
| Symbol names (`trait_super_adaptive`) | **Yes** — they are the interface being described, and a test that renames them tests nothing |
| Paths (`common/traits/00_species_traits.txt`) | **Yes** — same reason |
| Mod names, in corpus metadata | **Yes** — naming the mods is what makes a citation checkable |
| File bodies | **Synthetic only.** Written by hand to exhibit the shape under test. Never copied from a mod or from the game, not even two lines, not even reformatted |
| Localisation values | Synthetic. `"Super Adaptive"` is a fact about the game's interface; a paragraph of flavour text is someone's writing |

The distinction is that names and paths are the vocabulary a conflict is *stated in*,
while bodies are content. Nothing in this tool's tests needs real content, so the rule
costs nothing and removes the question permanently.

## Parser testing

Property tests (`proptest`) assert three invariants over generated Clausewitz-ish input:

1. **Never panics.** Any byte sequence at all.
2. **Depth balance.** Emitted `block_open` and `block_close` events pair up, including
   after error recovery.
3. **Span containment.** Every span lies within the input; every child span lies within
   its parent's.

Fuzzing seeds from the fixture corpus plus deliberately malformed variants (truncated
mid-block, unterminated quotes, 10k-deep nesting, embedded NULs, Windows-1252 bytes,
BOM-in-the-middle). Any crash is a release blocker; the recovery *behaviour* is
snapshot-tested so error handling cannot silently change.

### Adversarial inputs

The parser is not the only reader of attacker-shaped bytes. Every input that crosses a
trust boundary (§ `05` "Trust boundaries") has a fuzz target and a committed adversarial
fixture set, and both run before M1 ships:

| Input | Fuzz target | Adversarial fixtures | Gate |
| --- | --- | --- | --- |
| `.mod` descriptors, Steam VDF, launcher JSON | one each, size- and depth-capped readers | oversize, over-deep, truncated, wrong-encoding | H24 |
| Archive reader | entry walk | path-escaping entries, link entries, ratio and decompressed-size bombs, overlapping entries, central-directory/local-header mismatch | H23 |
| Filesystem tree | — | links, junctions, reparse points incl. cloud placeholders, FIFOs, descriptor roots escaping the mod, case/NFC identity collisions, over-wide and over-deep trees | H22 |
| `launcher-v2.sqlite` | — | WAL-only commit, torn copy, corrupt page, checkpoint mid-copy | H20 |
| Report sinks | — | a hostile-names playset: ANSI/OSC escapes, bidi controls, Markdown link and HTML injection, JSON-in-HTML breakout | H25 |
| Every export and diagnostic | — | a redaction install whose user name and paths are searched for in every output | H26 |

### Golden corpus and fact floors

All three invariants above, and the fuzz target, are satisfied by a parser that returns
nothing. So does "< 0.5% of files produce diagnostics". Robustness gates are ceilings,
and a ceiling is met by silence; the parser also needs **floors** (H30):

| Floor | Asserts | Fails an empty parser because |
| --- | --- | --- |
| Golden extraction corpus | One synthetic file per `[[dir]]` shape in each shipped profile, with a checked-in expected `RawFact` list; extraction must match it exactly — nothing missing, nothing extra | Every expected fact is missing |
| Per-file accounting | Every file the profile classifies as parseable ends with ≥ 1 fact, or a `Diagnostic`, or an explicit `ParseCompleteness` other than `Complete` (§ `03`). A file with none of the three is a test failure | Every file has none of the three |
| Count against the measured workload | On a real playset, definition and loc-key counts land within ±25% of the figure predicted from the M0.5 histogram (§ `12`) | The count is zero |

The golden corpus follows the fixture rule above — synthetic bodies, real names and
paths — and the coverage stance below already requires every `[[dir]]` to appear in it.

## Rule testing

Cases are part of the `Rule` trait, not a convention about file layout:

```rust
pub trait Rule: Sync {
    // …
    fn cases(&self) -> &[RuleCase];
}

pub struct RuleCase {
    pub name:   &'static str,
    pub kind:   CaseKind,        // Positive | NearMiss | OrderUnknown | Deletion | Incomplete
    pub build:  fn(&mut PlaysetBuilder),
    pub expect: Expectation,     // finding with given subject/role, or explicitly none
}
```

There is no `cases` default, so **a rule with no cases does not compile**, and one
generated harness runs every case of every registered rule. This is the difference
between checking behaviour and checking file existence: a gate satisfied by
`#[test] fn positive_symbol_collision() {}` is not a gate, and an empty test module is
exactly what a hurried contributor writes.

Five kinds, all mandatory:

| Kind | Asserts |
| --- | --- |
| `Positive` | The finding fires, with the expected subject and participant roles |
| `NearMiss` | It does **not** fire — same symbol with identical bodies, a benign path, or a `MergeByKey` directory where only the file names differ |
| `OrderUnknown` | With no order source, the rule degrades (reports without a winner) rather than guessing one |
| `Deletion` | The 1→0 case: a symbol that existed in exactly one source and now exists in none |
| `Incomplete` | An input in the subject's scope is `Partial` or `Unparsed`: the rule makes no definitive negative (no `Outcome::Deleted`, no stated winner over an unparsed contribution) and the finding's `certainty.completeness` is not `Complete` (§ `08` "Completeness and certainty") |

The near-miss case is the one that separates a rule from a false-positive generator. The
deletion case is the one the catalogue is structurally blind to: every v1 rule is a
"≥2 of something" predicate, so no amount of two-participant testing can reach the state
where the count is zero — and a thing that used to exist and now exists nowhere is
precisely what users open this tool to find. Requiring the case per rule is what forces
`core.symbol_deleted` and the `all_dead()` query to stay implemented.

Cases are a required review item for any new rule, and `Expectation` is checked against
the finding, not against a count, so a rule that fires for the wrong reason still fails.

## Calibration, and what it can and cannot prove

Described in `09-policy-and-scoring.md`. The mechanism is sound and the claim usually
made for it is not: **a hand-written reconstruction of what the author believes an
incompatibility looks like, fed to a tool the same author wrote to detect it, measures
self-consistency.** It is circular, and it is circular in the direction that feels like
success. Two dozen playsets can be overfitted in an afternoon.

So the harness is split by what it is actually evidence of.

### 1. Regression corpus — committed, symbolic, gates every PR

```
tests/corpora/
├── known_good/          # combinations documented as working
│   ├── gigas_acot.toml
│   └── ...
└── known_bad/           # documented incompatibilities, with a citation each
    ├── two_trait_overhauls.toml
    └── ...
```

Each `.toml` describes mods and files symbolically (via `smd-testkit`) and **must** carry
a `citation` — a mod page note, a bug report, a forum thread, with a date. The field is
enforced by the loader, not by review: an entry without one fails to parse. An
uncited `known_bad` is the author's opinion wearing a test's clothing, and it is
indistinguishable from evidence once it is in the file.

What this corpus proves is **that a change did not break what worked before**. That is
worth gating on and is all it is gated on:

```
known_good: 14 playsets, 2 false positives at >=high  (target: 0)
known_bad:  9 playsets, 9 detected at >=high          (target: 9)
```

Two guards against the ways this number lies:

- **A ranking metric alongside the threshold.** Report mean reciprocal rank of the seeded
  incompatibility within its playset's findings, not just "detected at ≥high". A
  threshold metric is trivially satisfied by scaling every weight upward until everything
  clears the band; ranking is invariant to that, so a rescaling that fakes a pass shows
  up as a flat MRR.
- **A 20% holdout.** Corpus entries are split by a hash of their id; tuning uses 80%, and
  the harness reports the holdout separately. Divergence between the two is the
  measurement of overfitting, which otherwise has none.

Small-corpus arithmetic is stated so nobody reads more into a run than it holds: at
14 playsets, one fixture flipping is a 7% swing, and the gate is binary. Corpus growth is
therefore a standing task, not a one-time setup.

### 2. Ground truth — local, real mods, never committed

`cargo xtask ground-truth` runs the tool over the **developer's own subscribed mods** and
writes a manifest of `(rule_id, subject_shape, score, mod_names, content_hashes)` — no
mod content, no file bodies, no paths outside mod-relative form. The manifest is checked
in; the mods are not, and cannot be.

This is the only artefact in the project that is evidence about *reality* rather than
about the author's model of it. It runs locally because the corpus is 1.2 GB of other
people's work that cannot be redistributed, and it is checked in because a diff in that
manifest is the earliest signal that a rule change moved something in the real world.
A release with an unexplained manifest diff does not ship.

### 3. Labelled ground truth — the trust gate

The regression corpus measures self-consistency; the manifest measures stability. Neither
measures *correctness*, and an exit criterion phrased only as "zero false positives at
≥high" is satisfied by a policy that scores everything Low. The M5 exit (§ `15`) is
therefore graded against a third set (H31), drawn from **two sources, because one
source cannot measure both directions**. A sample of the tool's own findings can say how
many of them are wrong; it cannot say what the tool missed, because a missed conflict is
never in the sample. Recall measured that way is recall among findings already produced —
a tool that reports nothing scores perfectly on it.

- **Precision sample — from the tool's output.** Findings sampled from the ground-truth
  manifest of three real playsets (the M0.5 playsets), stratified by rule and by band so
  the sample is not all `core.file_overwrite` at Low. This sample measures precision and
  nothing else.
- **Recall census — independent of the tool's output.** Two parts, both enumerated
  without consulting what the tool reported:
  1. **Seeded conflicts.** A pair of synthetic seed mods, appended to each real playset,
     carrying a known list of conflicts by construction: one per directory shape in the
     profile, including a FIOS case, a LIOS case, a `replace_path` case, a deletion, and an
     additive near-miss that must *not* fire. Known by construction, so labelling is
     mechanical.
  2. **Independently enumerated conflicts.** A reference enumerator that shares no code
     with `smd-parse`, `smd-index`, or `smd-rules` — a deliberately naive lister of
     same-path files and of top-level keys defined in two or more sources, over a
     stratified random sample of directories — produces candidates, and a random sample of
     those is labelled by hand. Sharing the tool's code would share its blind spots.
  A census item labelled `real` that the tool did not report at all is a miss, and counts
  against recall exactly like one it reported Low.
- **Labels.** Each sampled finding or census item is labelled by hand — `real` (the loser's content is
  lost and something depends on it), `benign` (the overwrite is intended or harmless), or
  `wrong` (the tool's model is mistaken: wrong winner, wrong participants, no conflict) —
  with the evidence for the label recorded: the files, and where M0.5 established it, the
  in-game observation. Labels name mods and symbols and carry no file bodies, under the
  fixture rule.
- **Labelling is blind to the score.** The labeller sees the finding's subject and
  participants, not its band. A label written while looking at the band is a calibration
  of the labeller.
- **Metrics, all four reported together:** recall at ≥high over the **census** (seeded
  and enumerated, reported separately); precision at ≥high over the **sample** (fraction
  of ≥high labelled `benign` or `wrong`); MRR as above; and the same three on the
  holdout. Each one alone can be gamed by moving every weight in one direction; the
  set cannot.
- **Growth.** Every user-reported false positive or false negative that is reproduced
  becomes a labelled entry. The set is evidence about reality only to the extent it is
  renewed from reality.

### 4. What is still not measured

Neither of the first two observes the running game, and the third observes it only
through M0.5's probes. A finding confirmed by both is confirmed
as *"the tool's model says this, consistently, about real inputs"* — not as "this breaks
Stellaris". That gap closes only through M0.5's probe mods and user reports, and the
prose in `09` hedges precisely because of it.

## Cross-platform CI

| Job | Runner | Covers |
| --- | --- | --- |
| test-windows | `windows-latest` | Path casing, `\` separators, `%USERPROFILE%`, launcher DB, WAL copy |
| test-linux | `ubuntu-latest` | Case sensitivity, XDG paths, Proton prefix probing |
| test-macos | `macos-14` and `macos-latest` | Case-insensitive HFS+/APFS, `~/Documents`. **These are the macOS versions the project may claim to support** |
| lint | ubuntu | `clippy -D warnings`, `fmt --check`, `cargo xtask lint` — exactly the gates marked **live** in § "Lint guarantees" below |
| bench-cpu | ubuntu | Lexer, extraction, index build. 10% regression gate |
| bench-pipeline | **windows** | Full scan including IO. Trend-reported, no hard gate |
| ui | ubuntu | `vitest`, `tsc --noEmit`, generated-types drift check |
| fuzz | ubuntu, nightly | 30-minute `cargo-fuzz` run |

Windows is the primary correctness target — it is where the users are, and path handling
is where a cross-platform file tool actually breaks.

Two consequences of that which the bench split exists to fix. Every platform-sensitive
decision in this design — mmap versus `read()`, file locking, the stat cache's identity
fields, WAL copying — is budgeted on Windows, so benchmarking it on Linux measures a
different system. And a 10% gate on IO timings from a shared cloud runner is red on noise,
which means it gets disabled within a month and then protects nothing. Hence: **a hard
gate only where the measurement is CPU-bound and reproducible, and a tracked trend where
it is not.** A trend nobody can silence beats a gate everybody learns to override.

macOS is claimed only for the versions in that row. "macOS 11+" was never exercised
anywhere and would have been a support promise made by a document rather than by a test.

## Release validation

A release is a claim about an artefact, so it is validated on the artefact. The release
gate (§ `15` RE, H33) runs for every tag, M1 included, against the binaries CI built for
that tag — never against a developer's `cargo build`, which shares neither the build
profile nor the feature set with what a user downloads:

1. Checksums verify; `--selftest` on each target's binary diffs clean against
   `registry.lock`.
2. Each target's binary scans the fixture playset; output matches the golden report byte
   for byte.
3. The Windows binary scans the three M0.5 real playsets on a machine with the game and
   launcher installed — one of them with the launcher open — and the ground-truth
   manifest diff is empty or explained in the release notes.

The real-playset step cannot run in CI, for the same reason ground truth cannot: the
mods are not redistributable. Its log is attached to the draft release, and a draft
without one does not publish.

## Lint guarantees: live versus gated

The design documents describe more `xtask lint` gates than exist. On this branch
`xtask/src/lint.rs` runs **five**, and clippy holds two more by delegation. Every other
gate a document describes is scheduled here with an id and a milestone, and a document
citing one points at this table (H35). **A gate not marked live is a promise, not a
protection** — the same distinction `12` draws for budgets.

| Guarantee | Claimed in | Status | Gate · milestone |
| --- | --- | --- | --- |
| File > 300 code lines (warn at 200); code-line counting; `generated/` and all-`const` exemptions; two-halved per-file allowances | ADR-0002 §7, `02` | **Live** — `file-length` | — |
| Function > 60 lines, > 5 parameters | ADR-0002 §7, `02` | **Live** — clippy, thresholds in `clippy.toml`; delegation checked by `profile-audit` | — |
| Dependency edge contradicting `xtask/layers.toml`, validated against `cargo metadata` | ADR-0002 §7, `01`, `02` | **Live** — `dep-direction` | — |
| Game name in a crate without `game_names_allowed` | ADR-0002 §7, `08`, `10` | **Live** — `game-name` | — |
| Broken relative link in `docs/**/*.md` | REVIEW-2026-08 (cross-confirmed findings) | **Live** — `doc-link` | — |
| `[profile.release]` `panic = "unwind"` and the clippy delegation | ADR-0001, `04`, `08` | **Live** — `profile-audit` | — |
| A `Rule` impl with no `cases()` | ADR-0002 §7 | Compiler, not lint — `cases` has no default (§ "Rule testing") | lands with the `Rule` trait · M1 |
| `_ => {}` / `_ => None` over a shared enum | ADR-0002 §3, §7 | Not built | H36 · M1 |
| Struct literal of a shared `smd-core` struct outside `smd-core` | ADR-0002 §3, §7 | Not built | H36 · M1 |
| Every rule id ↔ every shipped `explain/*.toml`, both directions | ADR-0002 §7 | Not built | H36 · M1 |
| Every `{placeholder}` is a declared evidence key or in `smd_core::DERIVED_PLACEHOLDERS` | ADR-0002 §7, `08`, `09` | Not built (the derived set exists in `smd-core`; nothing reads it) | H36 · M1 |
| Every `[[dir]]` in a shipped profile appears in its golden classification list | ADR-0002 §7, § "Coverage stance" | Not built | H36 · M1 |
| `--selftest` matches `registry.lock`, per feature combination | ADR-0002 §7, § "Coverage stance" | Not built | H36 · M1 |
| rayon entry points (`par_iter`, `par_bridge`, `rayon::scope`, `rayon::spawn`) outside `smd-engine` | `04`, `08` | Not built | H37 · M2 |
| Two parsers claiming the same `(kind, glob)` at the same priority | ADR-0002 §7, `06` | Not built | H37 · M2 |
| Every rule id ↔ `weights.toml` `[base]`, both directions | ADR-0002 §7, `09` | Not built | H38 · M5 |
| Rule ids in `suppress.toml` and preset `disabled_rules` resolve | ADR-0002 §7 | Not built | H38 · M5 |
| Every `when.evidence` key declared by the named rule's `evidence()`; `Tag` values are variant names of the named enum | ADR-0002 §7, `08`, `09` | Not built | H38 · M5 |
| A declared evidence key no modifier and no explanation reads | `09` | Not built | H38 · M5 |
| Every `[[modifier]]` fires at least once across the corpus | `09` | Not built — `xtask calibrate` does not exist yet | H38 · M5 |
| Every non-best value of every `Certainty` axis has a `[caps]` row (also a load error) | `09` | Not built | H38 · M5 |
| `core.symbol_deleted` has a `variant.unverified` entry in every locale, its title without "deleted" | `09` | Not built | H38 · M5 |
| `{@html}` outside the one audited component | `11` | Not built — no GUI or HTML renderer exists yet | H27 · M7 / M10 |
| A winner-naming explanation lacks a variant for a `Mechanism` its rule can emit; a reorder suggestion outside the `load_order` variant | `09` | Not built — the mechanism vocabulary lands with the precedence tuple | H19 · M3 |
| Mermaid layer diagram generated from `xtask/layers.toml` | `01`, `02` | Not built — the diagram is hand-maintained until it is | H39 · M1 |
| `cargo xtask dir-audit <game-dir>`: vanilla directories the profile omits | `10` | Not built; renamed from `profile-audit`, which is the release-profile gate above | H39 · M1 |

The milestone is the one in which the gate's *subject* first exists: there is no shared
enum matched outside `smd-core` before M1's first rule, no parallel code before M2, and no
`weights.toml` before M5. A gate scheduled later than its subject is a gap a regression
can fall through, so moving one later is a register change, not an edit.

## Coverage stance

Coverage percentage is not a gate. Three specific requirements instead:

1. Every `Rule` impl supplies all five `RuleCase` kinds — enforced by the compiler for
   existence and by the generated harness for behaviour, never by counting files.
2. Every `[[dir]]` entry in a shipped profile is exercised by the golden classification
   list, including its `merge` and `additive_children`.
3. Every registered rule, parser, renderer, and profile appears in `registry.lock`, and
   the **release** binary's `--selftest` diffs its live registry against it. A dropped
   `inventory` registration produces zero findings for that rule, which for a conflict
   detector reads as a clean bill of health — the one failure this project cannot afford
   to discover from a user.

All three are structural, all three are enforceable, and none can be satisfied by writing
tests that assert nothing.
