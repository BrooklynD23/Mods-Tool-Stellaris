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
| Calibration | FP/FN rate against known-good and known-bad playsets | `cargo xtask calibrate` | every PR touching rules or policy |
| Bench | Throughput and memory | `criterion` | every PR, compared to baseline |
| E2E | GUI flows | Playwright against a built Tauri binary | pre-release |

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

**No copyrighted content in the repo.** Fixtures are hand-written minimal files. The
performance corpus is *generated* from a seed by `smd-testkit`, not vendored.

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

Every rule ships with, at minimum:

- one fixture that **must** produce the finding,
- one near-miss fixture that **must not** (usually: same symbol, identical bodies, or a
  benign path),
- one fixture where the load order is unknown, asserting the rule degrades rather than
  guesses.

The near-miss test is the one that matters. It is the difference between a rule and a
false-positive generator, and it is a required review item for any new rule.

## Calibration harness

Described in `09-policy-and-scoring.md`. Concretely:

```
tests/corpora/
├── known_good/          # combinations documented as working
│   ├── gigas_acot.toml
│   └── ...
└── known_bad/           # documented incompatibilities, with a citation each
    ├── two_trait_overhauls.toml
    └── ...
```

Each `.toml` describes mods and files symbolically (via `smd-testkit`) and cites its
source — a mod page note, a bug report, a forum thread. `cargo xtask calibrate` reports:

```
known_good: 14 playsets, 2 false positives at >=high  (target: 0)
known_bad:  9 playsets, 9 detected at >=high          (target: 9)
```

A PR that regresses either number fails. This is how a subjective severity model gets
held to an objective standard.

## Cross-platform CI

| Job | Runner | Covers |
| --- | --- | --- |
| test-windows | `windows-latest` | Path casing, `\` separators, `%USERPROFILE%`, launcher DB |
| test-linux | `ubuntu-latest` | Case sensitivity, XDG paths, Proton prefix probing |
| test-macos | `macos-latest` | HFS case-insensitivity, `~/Documents` |
| lint | ubuntu | `clippy -D warnings`, `fmt --check`, `cargo xtask lint` (size gates, dep graph, profile audit) |
| bench | ubuntu | criterion vs baseline |
| ui | ubuntu | `vitest`, `tsc --noEmit`, generated-types drift check |
| fuzz | ubuntu, nightly | 30-minute `cargo-fuzz` run |

Windows is the primary correctness target — it is where the users are, and path handling
is where a cross-platform file tool actually breaks.

## Coverage stance

Coverage percentage is not a gate. Two specific requirements instead:

1. Every `Rule` impl has a positive and a negative test (checked by `xtask`, which
   enumerates registered rules and asserts a matching test module exists).
2. Every `[[dir]]` entry in a shipped profile is exercised by the golden classification
   list.

Both are structural, both are enforceable, and neither can be satisfied by writing tests
that assert nothing.
