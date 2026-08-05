# 08 — Rule engine

This is the extension seam that will be used most often, so it gets the strictest
contract. Adding a rule touches **four things and no central list**: the rule file, its
fixtures, one `[base]` line in `weights.toml`, and one entry per shipped locale in
`explain/`. Removing a rule removes the same four, plus any preset or suppression naming
its id. The contract below exists so that every one of those four is *checked* — a
stranded weight or a missing explanation is a compile or lint failure, not a rule that
quietly never fires (ADR-0002).

## The contract

```rust
pub trait Rule: Sync {
    /// Stable, namespaced, never reused. Appears in reports, config, suppressions.
    fn id(&self) -> RuleId;

    /// Which index structures must be built for this rule to run.
    fn needs(&self) -> Capabilities;

    /// Which semantic features the game profile must declare. Empty = any game.
    fn requires(&self) -> &'static [FeatureTag] { &[] }

    /// Every evidence key this rule may emit, and its type. Nothing else is permitted.
    fn evidence(&self) -> &'static [(&'static str, EvidenceType)];

    /// The fixtures this rule is defined by. No default: a rule with no cases is not a
    /// rule. Kinds and expectations are specified in § `13`.
    fn cases(&self) -> &[RuleCase];

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding>;
}
```

Six constraints, enforced by the type system and by lints:

| Constraint | Mechanism |
| --- | --- |
| A rule cannot mutate the index | `RuleCtx` holds `&ScanIndex` |
| A rule cannot score its findings | `Finding.score`/`severity` are written by `smd-policy`; a debug assert catches a rule that sets them |
| A rule cannot format text | `EvidenceBag` is structured; prose lives in the explanation catalogue |
| A rule cannot see other rules | No handle to the registry in `RuleCtx` |
| A rule cannot corrupt the engine by panicking | `run` returns `Vec<Finding>` by value, so the unwind boundary is a value boundary and `catch_unwind` is sound |
| A rule cannot name a game | `requires()` returns `FeatureTag`s from a closed vocabulary in `smd-core`; a game id anywhere in `smd-rules` fails the lint gate |

The fourth is what makes rules independently deletable. Cross-rule relationships
(suppress X when Y fired) are expressed *declaratively in policy data*, never as a call
between rules.

### Why `requires()` and not `applies_to()`

`applies_to() -> &["stellaris"]` would put a game id in a crate that `xtask lint` forbids
game names in — the design's own rule, broken by its own trait. It is also the wrong
predicate: a rule about duplicate event ids applies to any game whose profile declares it
has events, which is a property of the profile, not of the game's name.

```rust
pub enum FeatureTag { Definitions, Localisation, Events, Namespaces, Defines,
                      ReplacePath, Dependencies, VersionMetadata }
```

Profiles declare their tags in TOML. A rule with a tag no loaded profile declares is
skipped, silently and correctly. This also makes the second-game milestone mechanical:
the game abstraction holds if and only if adding a profile enables the right rules with
no code change.

### `RuleCtx`

```rust
pub struct RuleCtx<'a> {
    pub index:  &'a ScanIndex,
    pub cancel: &'a CancellationToken,
}
```

**A rule must check `cancel` once per outer-loop iteration.** A rule is otherwise an
indivisible unit of work, and the analyse stage is seconds long on a large playset — long
enough that a user's cancel or a superseding rescan appears to do nothing (§ `04`). The
check is one atomic load against an iteration that does map lookups; it is free. A rule
that returns early on cancellation returns whatever it has; the findings are discarded
with the job.

### Evidence keys are declared, not improvised

`evidence()` is the other half of a contract that is otherwise entirely unchecked. The
`EvidenceBag` a rule emits is read by `weights.toml` modifiers and by `{placeholder}`s in
`explain/*.toml` — three files, two languages, and no compiler between them. A typo'd key
does not fail; it silently never matches, forever.

`EvidenceType` mirrors `EvidenceValue` (§ `03`) one variant for one variant, and `Tag`
additionally names the enum whose `snake_case` variant names are the legal values — so a
modifier matching `eq = "file_replaced"` against a `MergeMode` fails the lint rather than
never firing.

```rust
pub enum EvidenceType { Int, Bool, Tag(&'static str), Sym, Path, Src }

fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
    &[("domain",     EvidenceType::Sym),
      ("agreement",  EvidenceType::Tag("Agreement")),
      ("merge",      EvidenceType::Tag("MergeMode")),
      ("mechanism",  EvidenceType::Tag("Mechanism")),
      ("confidence", EvidenceType::Tag("Confidence"))]
}
```

`cargo xtask lint` cross-checks every `when = { evidence = … }` in `weights.toml` and
every `{placeholder}` in every locale against the union of this rule's declared keys and
the closed derived set defined in `smd-core` (§ `09`). A debug assertion catches a rule
emitting a key it did not declare.

### Registration

```rust
register_rule!(SymbolCollision);
```

The macro does two things: it submits the rule to `inventory`, and it const-asserts that
the rule's fixture set covers every `CaseKind`.

```rust
inventory::collect!(&'static dyn Rule);

macro_rules! register_rule {
    ($ty:ty) => {
        // `cases()` has no default, so a rule with no fixtures cannot be written at all.
        // This rejects the other half: a set that is all positives, or misses a kind.
        const _: () = RuleCase::assert_covers_all_kinds(<$ty>::CASES);
        inventory::submit!(&$ty as &'static dyn Rule);
    };
}
```

Cases live in an inherent `const CASES` that `cases()` returns, because a const is what a
`const _: () = …` assertion can read, and an associated const would make `Rule` not
object-safe — which `inventory` requires. The generated harness runs every case of every
registered rule (§ `13`), so the coverage gate is held by the compiler and the runner
rather than by a reviewer noticing an empty test module.

Two properties the registry must have, neither of which `inventory` gives for free:

- **Sorted by id at startup.** `inventory`'s iteration order is unspecified and varies
  with link order, which would make rule output order a property of the linker.
- **Verified in the shipped binary.** `--selftest` on the *release* binary enumerates
  every registered rule, parser, renderer, and profile and diffs the list against a
  checked-in `registry.lock`. Counting source files cannot model feature flags, and a
  registration dropped by link-time collection produces zero findings for that rule —
  which, for a conflict detector, reads to the user as a clean bill of health.

## Capabilities

```rust
bitflags! {
    pub struct Capabilities: u32 {
        const OVERLAY     = 1 << 0;
        const DEFINITIONS = 1 << 1;
        const REFERENCES  = 1 << 2;   // off in v1
        const LOCALISATION= 1 << 3;
        const EVENTS      = 1 << 4;
        const DESCRIPTORS = 1 << 5;
        const FILE_BYTES  = 1 << 6;   // rule wants to read raw content
    }
}
```

`u32` rather than `u16` because the width is part of the serialised contract and running
out of bits later costs a `CONTRACT_VERSION` bump; 16 spare bits are cheaper.

The engine unions the capabilities of enabled rules and builds only those index
structures. `REFERENCES` is **off by default in v1**: untyped reference extraction matches
`owner`, `root`, `value`, and `yes` as readily as it matches a trait name, so impact
analysis would be noise and every score that reads `referenced_by_others` would inflate
uniformly. Turning it off removes the `ReferenceTable`'s ~120 MB *and* its parse-time
extraction, with no conditional anywhere in the pipeline — which is the clearest
demonstration of why capabilities are declared rather than assumed.

## Execution

`smd-rules` exposes one rule at a time and nothing parallel:

```rust
pub fn run_one(rule: &dyn Rule, ctx: &RuleCtx<'_>) -> Result<Vec<Finding>, RulePanic> {
    catch_unwind(|| rule.run(ctx)).map_err(|p| RulePanic::new(rule.id(), p))
}
```

The fan-out over rules belongs to `smd-engine`, which owns the process's only thread pool
(§ `04`). A `par_iter` here would target rayon's global pool — a third pool nobody
budgeted — so `xtask lint` bans rayon entry points outside `smd-engine` and this function
is deliberately sequential.

- Each rule is a pure function of `&ScanIndex` into `Vec<Finding>`, so the `catch_unwind`
  boundary is a value boundary and needs no `AssertUnwindSafe`. Rules hold no
  `std::sync::Mutex`, so a panic cannot poison state the next rule needs.
- Containment depends on `panic = "unwind"`, pinned in the release profile and asserted by
  a profile-audit lint. A test deliberately panics a rule and asserts the scan completes.
  What `catch_unwind` cannot catch — stack overflow, abort — is handled upstream by the
  parser's depth cap, not here (§ `04`).
- Each rule has a soft time budget (default 5 s); exceeding it logs a warning with the
  rule id, so a pathological rule is identifiable rather than mysterious. The rule is not
  killed: there is no safe way to interrupt one, which is the same reason `cancel` is
  cooperative.
- Results are concatenated in registry order — the registry is sorted by id at startup —
  and the total sort is applied by the policy layer.

## The v1 rule catalogue

| Rule id | Detects | Notes |
| --- | --- | --- |
| `core.file_overwrite` | ≥2 sources contribute the same path | Downgraded to `Info` when blobs are identical |
| `core.symbol_collision` | ≥2 live definitions of one `(domain, symbol)` | Downgraded when the losers agree with the winner |
| `core.symbol_deleted` | A `(domain, symbol)` with ≥1 definition and **no live one** | Base weight above `core.symbol_collision` — a symbol the game can no longer see is worse than one it resolves the wrong way. Backed by `ScanIndex::all_dead()` and `Outcome::Deleted`. |
| `core.loc_collision` | Same loc key in ≥2 sources | Split by value-equal vs value-differs |
| `pdx.event_id_dup` | Same `namespace.ordinal` in ≥2 sources | Near-certain breakage; high base weight |
| `pdx.namespace_share` | Unrelated mods sharing an event namespace | Signals future collisions even without one today |
| `pdx.defines_overlap` | Same define key from ≥2 sources | `common/defines` is `FileReplace`-shaped and unforgiving |
| `pdx.replace_path_shadow` | A `replace_path` silently drops another mod's files | Usually intentional by the declaring mod, usually a surprise to the user |
| `dep.missing` | Declared dependency not in the playset | Fuzzy name match; carries candidates |
| `dep.order` | Dependency loads *after* its dependent | Deterministic and highly actionable |
| `meta.version_drift` | `supported_version` excludes the installed version | Low alone; amplified in combination |
| `meta.descriptor_mismatch` | `.mod` disagrees with `descriptor.mod` | Stale launcher metadata |
| `meta.patch_candidate` | An installed mod looks like a patch for a detected conflict pair | Suppresses the conflict it covers |

`core.symbol_deleted` is the one rule in the set that is not "≥2 of something". Every
other v1 rule reports a *contest*; this one reports an *absence* — a file replaced
wholesale, or dropped by a `replace_path`, taking with it definitions nothing else
supplies. The result is a symbol referenced by name and defined nowhere the game will
look, which is both structurally unreachable and exactly the class of bug that makes a
player open the tool. A catalogue made only of contests cannot see it.

Post-v1, each still one file: `pdx.on_action_overlap`,
`pdx.scripted_effect_signature_change`, `meta.workshop_delisted`,
`core.gui_element_collision`, `pdx.tech_tree_orphan`, and `core.dangling_reference` —
the last blocked on `Capabilities::REFERENCES`, which stays off until reference
extraction is typed rather than textual.

## Anatomy of a rule

`rules/symbol_collision.rs`, complete in about 150 lines:

```rust
pub struct SymbolCollision;

impl SymbolCollision {
    const CASES: &'static [RuleCase] = &[
        RuleCase { name: "two overhauls, one trait", kind: CaseKind::Positive,
                   build: fixtures::two_trait_overhauls,
                   expect: Expectation::fires_on_symbol("trait_super_adaptive") },
        RuleCase { name: "vendored identical copy", kind: CaseKind::NearMiss,
                   build: fixtures::identical_trait, expect: Expectation::None },
        RuleCase { name: "no order source", kind: CaseKind::OrderUnknown,
                   build: fixtures::directory_scan_only,
                   expect: Expectation::fires_without_winner() },
        RuleCase { name: "sole definer replaced away", kind: CaseKind::Deletion,
                   build: fixtures::last_definer_shadowed, expect: Expectation::None },
    ];
}

impl Rule for SymbolCollision {
    fn id(&self) -> RuleId { RuleId("core.symbol_collision") }
    fn needs(&self) -> Capabilities { Capabilities::DEFINITIONS | Capabilities::OVERLAY }
    fn requires(&self) -> &'static [FeatureTag] { &[FeatureTag::Definitions] }
    fn cases(&self) -> &[RuleCase] { Self::CASES }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[("domain",     EvidenceType::Sym),
          ("agreement",  EvidenceType::Tag("Agreement")),
          ("merge",      EvidenceType::Tag("MergeMode")),
          ("mechanism",  EvidenceType::Tag("Mechanism")),
          ("confidence", EvidenceType::Tag("Confidence"))]
    }

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding> {
        let mut out = Vec::new();
        for (key, sites) in ctx.index.defs.multi_defined() {
            if ctx.cancel.is_cancelled() { return out; }

            let live: Vec<_> = sites.iter().filter(|d| d.live).collect();
            if live.len() < 2 { continue; }

            let res = ctx.index.resolve_symbol(key);
            out.push(Finding {
                rule: self.id(),
                subject: Subject::Symbol { domain: key.domain, symbol: key.symbol },
                participants: participants_from(&live, &res),
                facts: evidence! {
                    "domain"     => ctx.domain_name(key.domain),
                    "agreement"  => res.agreement,
                    "merge"      => live[0].merge,
                    "mechanism"  => res.mechanism,
                    "confidence" => res.confidence,
                },
                ..Finding::unscored()
            });
        }
        out
    }
}

register_rule!(SymbolCollision);
```

Note what is absent: no numbers, no severity, no user-facing sentence, no knowledge of
which game it is running against. Those are three other people's jobs. Note also what is
*present* and could not be omitted: the cancellation check, the declared evidence keys,
and all four case kinds — things the compiler or a lint demands before this file builds.

The `Deletion` case is the instructive one. This rule's honest answer to "the only mod
defining X had its file replaced" is *I do not fire* — the finding belongs to
`core.symbol_deleted`. Requiring every rule to state that answer is what stops the 1→0
case from falling between two rules, each assuming the other covers it.

## Suppressions

Three layers, all data:

1. **Policy suppressions** (shipped, in `policy/suppress.toml`) — e.g. `core.file_overwrite`
   is suppressed for paths where `pdx.replace_path_shadow` already fired on the same
   subject, because reporting both is telling the user the same thing twice.
2. **Profile suppressions** — known-benign paths per game (`thumbnail.png`,
   `descriptor.mod`, `.git/**`, `readme*`).
3. **User suppressions** (`~/.config/smd/suppress.toml`) — by rule id, by mod, by
   subject, or by mod pair, with an optional note. Written by the UI's "ignore this"
   action, human-editable, and diffable so a playset owner can share theirs.

Suppression is applied *after* scoring and is reversible in the UI ("show suppressed"),
so nothing is ever silently invisible.
