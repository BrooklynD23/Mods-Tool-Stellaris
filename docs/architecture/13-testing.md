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
| Golden | Profile classification, report output | `insta` snapshots | every commit |
| Integration | Whole pipeline over a fixture playset | `cargo test --test pipeline` | every commit |
| Determinism | Same input, varied thread counts, identical bytes out | feature-gated test | every commit |
| Concurrency stress | Randomised cancellation under load; thread-count sweep 1…2×cores | feature-gated test | every commit |
| Calibration | Regression against a committed symbolic corpus | `cargo xtask calibrate` | every PR touching rules or policy |
| Ground truth | The developer's real subscribed mods vs a checked-in manifest | `cargo xtask ground-truth` | locally, before a release |
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

## Rule testing

Cases are part of the `Rule` trait, not a convention about file layout:

```rust
pub trait Rule: Sync {
    // …
    fn cases(&self) -> &[RuleCase];
}

pub struct RuleCase {
    pub name:   &'static str,
    pub kind:   CaseKind,        // Positive | NearMiss | OrderUnknown | Deletion
    pub build:  fn(&mut PlaysetBuilder),
    pub expect: Expectation,     // finding with given subject/role, or explicitly none
}
```

There is no `cases` default, so **a rule with no cases does not compile**, and one
generated harness runs every case of every registered rule. This is the difference
between checking behaviour and checking file existence: a gate satisfied by
`#[test] fn positive_symbol_collision() {}` is not a gate, and an empty test module is
exactly what a hurried contributor writes.

Four kinds, all mandatory:

| Kind | Asserts |
| --- | --- |
| `Positive` | The finding fires, with the expected subject and participant roles |
| `NearMiss` | It does **not** fire — same symbol with identical bodies, a benign path, or a `MergeByKey` directory where only the file names differ |
| `OrderUnknown` | With no order source, the rule degrades (reports without a winner) rather than guessing one |
| `Deletion` | The 1→0 case: a symbol that existed in exactly one source and now exists in none |

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

### 3. What is still not measured

Neither of the above observes the running game. A finding confirmed by both is confirmed
as *"the tool's model says this, consistently, about real inputs"* — not as "this breaks
Stellaris". That gap closes only through M0.5's probe mods and user reports, and the
prose in `09` hedges precisely because of it.

## Cross-platform CI

| Job | Runner | Covers |
| --- | --- | --- |
| test-windows | `windows-latest` | Path casing, `\` separators, `%USERPROFILE%`, launcher DB, WAL copy |
| test-linux | `ubuntu-latest` | Case sensitivity, XDG paths, Proton prefix probing |
| test-macos | `macos-14` and `macos-latest` | Case-insensitive HFS+/APFS, `~/Documents`. **These are the macOS versions the project may claim to support** |
| lint | ubuntu | `clippy -D warnings`, `fmt --check`, `cargo xtask lint` (size gates, dep graph, profile audit, layer map) |
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

## Coverage stance

Coverage percentage is not a gate. Three specific requirements instead:

1. Every `Rule` impl supplies all four `RuleCase` kinds — enforced by the compiler for
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
