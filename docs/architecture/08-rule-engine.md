# 08 — Rule engine

This is the extension seam that will be used most often, so it gets the strictest
contract. **Adding a rule must be: create one file. Removing a rule must be: delete one
file.** Nothing else.

## The contract

```rust
pub trait Rule: Sync {
    /// Stable, namespaced, never reused. Appears in reports, config, suppressions.
    fn id(&self) -> RuleId;

    /// Which index structures must be built for this rule to run.
    fn needs(&self) -> Capabilities;

    /// Which games this applies to. Empty = all.
    fn applies_to(&self) -> &'static [&'static str] { &[] }

    fn run(&self, ctx: &RuleCtx<'_>, out: &mut FindingSink);
}

inventory::submit!(&SymbolCollision as &dyn Rule);
```

Four constraints, enforced by the type system and by lints:

| Constraint | Mechanism |
| --- | --- |
| A rule cannot mutate the index | `RuleCtx` holds `&ScanIndex` |
| A rule cannot score its findings | `Finding.score`/`severity` are written by `smd-policy`; a debug assert catches a rule that sets them |
| A rule cannot format text | `EvidenceBag` is structured; prose lives in the explanation catalogue |
| A rule cannot see other rules | No handle to the registry in `RuleCtx` |

The last one is what makes rules independently deletable. Cross-rule relationships
(suppress X when Y fired) are expressed *declaratively in policy data*, never as a call
between rules.

## Capabilities

```rust
bitflags! {
    pub struct Capabilities: u16 {
        const OVERLAY     = 1 << 0;
        const DEFINITIONS = 1 << 1;
        const REFERENCES  = 1 << 2;
        const LOCALISATION= 1 << 3;
        const EVENTS      = 1 << 4;
        const DESCRIPTORS = 1 << 5;
        const FILE_BYTES  = 1 << 6;   // rule wants to read raw content
    }
}
```

The engine unions the capabilities of enabled rules and builds only those index
structures. Disabling the reference-based rules automatically skips building
`ReferenceTable` and its ~120 MB and its parse-time reference extraction — a removed
feature stops costing anything, without a single `if` anywhere in the pipeline.

## Execution

```rust
pub fn run_all(index: &ScanIndex, cfg: &RuleConfig) -> Vec<Finding> {
    enabled_rules(cfg)
        .par_iter()
        .flat_map(|rule| run_one_isolated(rule, index))
        .collect()
}
```

- Rules run in parallel; each is a pure function of `&ScanIndex` into `Vec<Finding>`.
- Each is wrapped in `catch_unwind`: a panicking rule loses only its own findings and
  produces a `Diagnostic` naming its id.
- Each has a soft time budget (default 5 s); exceeding it logs a warning with the rule
  id, so a pathological rule is identifiable rather than mysterious.
- Results are concatenated then sorted deterministically by the policy layer.

## The v1 rule catalogue

| Rule id | Detects | Notes |
| --- | --- | --- |
| `core.file_overwrite` | ≥2 sources contribute the same path | Downgraded to `Info` when blobs are identical |
| `core.symbol_collision` | ≥2 live definitions of one `(domain, symbol)` | Downgraded when `body_hash` matches |
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

Post-v1, each still one file: `core.dangling_reference`, `pdx.on_action_overlap`,
`pdx.scripted_effect_signature_change`, `meta.workshop_delisted`,
`core.gui_element_collision`, `pdx.tech_tree_orphan`.

## Anatomy of a rule

`rules/symbol_collision.rs`, complete in about 130 lines:

```rust
pub struct SymbolCollision;

impl Rule for SymbolCollision {
    fn id(&self) -> RuleId { RuleId("core.symbol_collision") }
    fn needs(&self) -> Capabilities { Capabilities::DEFINITIONS | Capabilities::OVERLAY }

    fn run(&self, ctx: &RuleCtx<'_>, out: &mut FindingSink) {
        for (key, sites) in ctx.index.defs.multi_defined() {
            let live: Vec<_> = sites.iter().filter(|d| d.live).collect();
            if live.len() < 2 { continue; }

            let res = ctx.index.resolve_symbol(key);
            out.push(Finding {
                rule: self.id(),
                subject: Subject::Symbol { domain: key.domain, symbol: key.symbol },
                participants: participants_from(&live, &res),
                facts: evidence! {
                    "domain"    => ctx.domain_name(key.domain),
                    "identical" => res.identical,
                    "merge"     => live[0].merge,
                    "basis"     => res.basis,
                },
                ..Finding::unscored()
            });
        }
    }
}
```

Note what is absent: no numbers, no severity, no user-facing sentence, no knowledge of
which game it is running against. Those are three other people's jobs.

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
