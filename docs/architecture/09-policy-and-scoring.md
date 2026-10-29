# 09 — Policy: scoring, severity, explanation

Detection is mechanical. Valuation is a judgement call that will be wrong at first and
will need tuning for years. Therefore **valuation is data, not code** — a separate crate
reading separate TOML, changeable without recompiling and without touching a rule.

## Pipeline position

```
rules → Vec<Finding> (score 0, interned ids)
          ↓  smd-policy: resolve interners, once
        Vec<ResolvedFinding>
          ↓  phase 1, per finding:   base weight → per-finding modifiers → clamp
          ↓  phase 2, per population: aggregates → aggregate modifiers → clamp → band
          ↓  certainty caps (not configurable away)
          ↓  explanation → suppression → sort (total, over resolved text — § `03`)
        Vec<ResolvedFinding> (scored, explained, ordered)
```

**Resolution happens first, and only here.** `path_glob` modifiers need the path interner
and `{symbol}` placeholders need the symbol interner, both of which live in `smd-index`.
Handing `&ScanIndex` to scoring *and* rendering would give the two crates forbidden from
detecting a full view of the detection layer. Instead `smd-policy` takes `&ScanIndex` for
exactly one purpose — turning ids into strings — and everything downstream, `smd-report`
included, consumes `ResolvedFinding` and depends on `smd-core` alone. The same boundary
keeps scan-local interner ids off the API surface, where they would be meaningless to a
caller holding a different scan.

**Scoring is two phases**, because some modifiers read a finding and some read the finding
*set*. `same_mod_findings_gte` cannot be evaluated while findings are still arriving. The
split has to be explicit: evaluating aggregates lazily, as each finding is scored, makes
the score depend on the order rules happened to finish in, and determinism is gone.

The **aggregate population** is stated rather than left to fall out, because every
plausible choice gives a different number: it is *every finding every enabled rule
emitted*, before suppression and before any `min_severity` filter. Counting after
suppression would let one user's ignore list change the scores of findings they did not
ignore; counting after filtering would make `--preset ci` and `--preset modder` disagree
about the same playset. A score is a property of the playset, not of the reader's
settings. Aggregates are keyed by `SourceId`, and a finding increments the count of every
participant source, so a two-mod collision counts for both. A finding whose `activation`
is `Conditional` is excluded from every aggregate: "would collide if both were enabled"
about a mod that may not be enabled is not evidence that the mod is already colliding.

## Weights

```toml
# policy/weights.toml
[base]
"core.file_overwrite"      = 45
"core.symbol_collision"    = 65
"core.symbol_deleted"      = 75
"core.loc_collision"       = 20
"pdx.event_id_dup"         = 85
"pdx.namespace_share"      = 30
"pdx.defines_overlap"      = 75
"pdx.replace_path_shadow"  = 55
"dep.missing"              = 80
"dep.order"                = 60
"meta.version_drift"       = 15
"meta.descriptor_mismatch" = 10
"meta.patch_candidate"     = 5    # registered at M9 (H13); the row lands with the rule
```

Every registered rule appears here and every id here resolves to a registered rule;
`xtask lint` checks both directions (not yet built; gated H38, M5 — § `13` lint ledger),
and the first direction is the one that pays for the gate. A missing weight is not a load error — it is a rule that scores 0 on every finding
forever, which from the outside is indistinguishable from a correct rule finding nothing.
`meta.patch_candidate` is the row most easily forgotten, because its findings are
reassuring rather than alarming. Its weight is deliberately low and it quietens nothing
(§ Suppression predicates).

### Scale discipline

The base weight is the prior; the modifiers are the evidence. That only works if evidence
can move a finding across a band, and against the bands below it frequently cannot.
`pdx.event_id_dup` starts at 85 with `critical` at 90, so `+25` for `same_path` and
`+20` for `common/defines/**` land on the same verdict as `+5`, and `−50` for
byte-identical bodies is the only modifier in the set that changes anything at all. Most
of the modifier space is inert for the three highest-weighted rules — which is to say the
tool stops distinguishing exactly where distinguishing matters most.

The constraint, stated so that calibration can be held to it: **no base weight may sit
closer to the top band than the largest single positive modifier.** Satisfying it is one
of two edits — compress the bases into roughly 10–60 and let the evidence discriminate,
or widen the bands — and which is right is a question for the corpus, not for this
document. `cargo xtask calibrate` reports the fraction of findings whose band is
determined by the base weight alone; that number is the measurement, and it should fall.

## Modifiers

Modifiers read the `EvidenceBag` the rule attached. They are additive, applied in a
defined order, and each one is a line of TOML.

```toml
# phase 1 — per finding
[[modifier]]
when = { evidence = "agreement", eq = "all_identical" }
add  = -50
why  = "byte-identical definitions cannot disagree"

[[modifier]]
when = { evidence = "mechanism", eq = "same_path" }
add  = 25
why  = "whole-file replacement discards every unrelated entry too"

[[modifier]]
when = { path_glob = "common/defines/**" }
add  = 20

[[modifier]]
when = { path_glob = ["gfx/**", "**/*.dds", "**/*.png"] }
add  = -35
why  = "art overwrites are cosmetic and usually intentional"

[[modifier]]
when = { participants_gte = 4 }
add  = 10
why  = "a pile-up is harder for a user to reason about"

# phase 2 — reads the finding set
[[modifier]]
when = { rule = "meta.version_drift", same_mod_findings_gte = 3 }
add  = 25
why  = "version drift matters when the mod is already colliding"
```

The last one is how cross-rule interaction is expressed without any rule knowing about
another rule.

There is no modifier for an unknown load order, or for any other uncertainty. An earlier
draft subtracted 15 for `confidence = unknown`; uncertainty is not a property of how bad a
conflict is, and a tunable discount for it can be tuned to zero. It is handled by the
certainty caps and the qualifier below, which cannot.

### The evidence contract

`when = { evidence = "mechanism", eq = "same_path" }` is three languages agreeing on two
strings by convention: a Rust `evidence!` key, a TOML key, and — through `ts-rs` — a
TypeScript field. Nothing about that agreement is checked by the compiler, and the
failure is silent in the worst direction: a typo'd key matches nothing, the modifier never
fires, and the finding scores as though the evidence did not exist. Four mechanisms close
it.

**Rules declare their evidence.** `Rule::evidence() -> &[(&'static str, EvidenceType)]`
sits in the rule's own file beside `run`, so the one-file property survives. A debug
assertion catches a rule emitting an undeclared key. `xtask lint` (not yet built; gated
H38, M5 — § `13` lint ledger) fails on a
`when.evidence` key the named rule does not declare — and on a declared key that no
modifier and no explanation ever reads, because dead evidence is usually half of a rename
and the other half is the bug.

**Values are named, not derived.** `eq = "same_path"` compares against a serialised
enum name. `MergeMode`, `Role`, `Outcome`, `Agreement`, `Mechanism`, `Confidence`, `Completeness`,
`ActivationCertainty`, `SemanticCertainty`, and `ResolutionGap` therefore carry an explicit
`#[serde(rename = "…")]` on every variant, plus a snapshot test over the serialised names,
so a Rust-side rename shows up as a failing snapshot rather than as a modifier that
quietly stops matching.

**Every modifier must fire.** `cargo xtask calibrate` asserts each `[[modifier]]` matches
at least once across the calibration corpus. A modifier that never fires is either dead
policy or a typo, and from the outside those are the same thing.

**No modifier sees the index.** `path_glob` matches the resolved path string produced at
the boundary above, not a `PathId`. Scoring never holds a handle to the detection layer.

## Bands

```toml
[bands]
info     = 0
low      = 20
medium   = 40
high     = 65
critical = 90
```

Configurable, because "how loud should this tool be" is a user preference, and because
different games will need different calibration.

## Certainty: caps and the qualifier

Bands say how bad a finding would be if true; `Finding.certainty` (§ `08`) says how far it
is known to be true. Two mechanisms connect them, and neither is a modifier, because a
modifier can be tuned to zero and these must not be.

**No discount on uncertain evidence (H18).** A modifier with a negative `add` is
reassuring — byte-identical bodies, art paths, an identical blob. It is applied only when
`completeness` is `Complete`. A body hash over a recovered, truncated block can agree with
another truncation where the real bodies differ, and the discount would turn "we could not
read this properly" into "this is harmless". Positive modifiers still apply: uncertainty
may hide a problem, it never proves one away.

**Caps.** After banding, the band is clamped by the worst axis:

```toml
[caps]                       # every non-best value of every axis has a row (H38, M5)
completeness.partial    = "medium"
completeness.unparsed   = "medium"
activation.inferred     = "high"
activation.conditional  = "low"
order.fallback          = "medium"
semantics.assumed       = "high"
semantics.unmodelled    = "medium"
```

The values are calibration, and change with the corpus; the rows are not optional. A
user may lower a cap, never remove one — a missing row is a load error, because an absent
cap is an uncertain finding at full volume.

**The qualifier.** Every finding below `Certain`/`Complete`/`Exact`/`Verified` on any axis
renders one qualifier sentence per non-best axis from `[certainty.*]` in the explanation
catalogue — "*Load order unknown: which mod wins is not stated.*", "*Would collide only if
both mods are enabled; the launcher could not be read.*", "*1 file in this directory could
not be fully read; this definition may still exist there.*" Rule entries cannot override
it and the renderer cannot omit it. For `core.symbol_deleted` with a `ParseIncomplete`
gap, the catalogue must also carry a `variant.unverified` entry whose title does not
contain the word "deleted"; the lint checks every locale for both (not yet built; gated
H38, M5 — § `13` lint ledger).

## Explanations

Prose never appears in rule code. It lives in a catalogue, keyed by rule id, with
placeholders filled from the `EvidenceBag`:

```toml
# policy/explain/en.toml
["core.symbol_collision".variant.mechanism.load_order]
title  = "{n} mods define {domain} “{symbol}”"
body   = """
{winner} loads last, so its definition of `{symbol}` is the one the game uses. \
The definitions from {losers} are discarded entirely — not merged.
"""
why    = "Two mods defining the same object is not a merge. The later one replaces the earlier one wholesale, including fields the later mod never intended to change."
action = "Check whether a compatibility patch exists for these mods, or change the load order if you prefer {loser_first}'s version."

["core.symbol_collision".variant.mechanism.filename_order]
title  = "{n} mods define {domain} “{symbol}”"
body   = """
{winner}'s file sorts where this directory's filename rule makes it win, so its \
definition of `{symbol}` is the one the game uses, whatever the load order. \
The definitions from {losers} are discarded entirely — not merged.
"""
action = "Check whether a compatibility patch exists for these mods. Reordering the mods will not change the winner here; only the file names decide it."

["core.symbol_collision".variant.identical]
title  = "{n} mods ship an identical {domain} “{symbol}”"
body   = "The definitions are byte-identical, so which one wins does not matter."
why    = "One mod has almost certainly vendored a copy of the other's file."
action = "No action needed."
```

Three payoffs: translation is a file, tone changes need no code review, and the same
strings serve the CLI, the GUI, and the HTML report.

**Advice is keyed to the axis that decided.** In a filename-ordered directory (the FIOS
and LIOS directories of § `07`), "loads last" is false and "change the load order" is
advice that does nothing, delivered in the sentence the user is most likely to act on.
Every rule whose explanation names a winner or suggests a reorder carries one variant per
`Mechanism` that can decide it — `load_order`, `filename_order`, `replace_tier`,
`replace_path`, `same_path` — and a reorder suggestion appears only in the `load_order`
variant. The lint fails a winner-naming entry that lacks a variant for a mechanism the
rule can emit (not yet built; gated H19, M3 — § `13` lint ledger; the vocabulary exists
once the precedence tuple lands).

### Placeholders are a closed set

| Source | Members |
| --- | --- |
| Declared evidence | whatever the rule's `evidence()` returns — `{domain}`, `{agreement}`, `{merge}`, `{mechanism}`, … |
| Derived, defined once in `smd-core` | `{n}`, `{subject}`, `{winner}`, `{losers}`, `{loser_first}`, `{completeness}`, `{activation}`, `{order}`, `{semantics}` |

`xtask lint` parses every `explain/*.toml` and fails on a `{placeholder}` that is neither
(not yet built; gated H36, M1 — § `13` lint ledger).
The derived set is closed and small on purpose: it is the only text in the catalogue that
no rule can be asked to supply, so every member needs one definition in one place.

Those definitions are owed, because none of `{winner}`, `{losers}`, or `{loser_first}`
appears in any rule's evidence bag. All three are derived from `Finding.participants` by
`Role`:

- `{winner}` — the single `Role::Winner` participant. It exists only when the finding's
  outcome is a single winner; a coexisting or internally-merged outcome has none, and the
  catalogue entry that uses it must be a variant guarded on that outcome.
- `{losers}` — every `Role::Shadowed` participant, in load order, joined with the locale's
  list separator and truncated at three with "and N others". Truncation is a locale
  concern, which is why it is defined here rather than in a renderer.
- `{loser_first}` — the *earliest* shadowed participant in load order: the one a reorder
  would promote. It is a different mod from the last, and getting it backwards produces
  advice that makes the conflict worse, in the sentence the user is most likely to act on.

## Suppression predicates

Suppression is applied after scoring and is reversible in the UI, so nothing is ever
silently invisible (§ `08-rule-engine.md`). What belongs here is the predicate: every
shipped suppression must name the same subject as the finding it hides, because a
predicate over anything coarser — mods, pairs, directories — hides findings it was never
evidence about.

```toml
# policy/suppress.toml
[[suppress]]
rule = "core.file_overwrite"
when = { other_rule = "pdx.replace_path_shadow", same_subject = true }
why  = "reporting both tells the user the same thing twice"
```

`same_subject` is correct here: both findings are about the same `Subject::Path`.

### Patch candidates suppress nothing (H13)

An earlier draft shipped a second entry — `rule = "*"`, suppressed when a
`meta.patch_candidate` finding's participant sources were a superset of F's. It is
removed. Participant coverage is a claim about *mods*; a conflict is about a *subject*. A
patch for mods A and B typically touches a handful of the objects they share, and the
predicate silenced every finding involving A and B — including the trait collision and
the deleted tech the patch never mentions — on the strength of a filename and a
dependency list. That is a reassurance with no evidence, and it is the most dangerous
kind of output a conflict checker can produce, because a suppressed finding is a finding
the user is told not to look at.

What would justify quietening F is **subject-specific coverage**: the patch is itself a
participant in F, contributes a surviving site for F's exact subject, and is F's stated
`Winner` with `certainty` best on every axis. That is already visible to the finding's own
resolution, which makes it a candidate *modifier* (`winner_is_patch_candidate`), not a
suppression — and even that stays out of `weights.toml` until three things exist: a
`NearMiss` fixture where the patch shares both mods but not the subject and the finding
keeps its band; a positive fixture where it covers the subject; and a calibration run
showing the false-negative rate on the known-bad corpus does not rise. Until then
`meta.patch_candidate` is an informational finding that names the possible patch beside
the pair, and the user decides.

Every rule id in `suppress.toml`, on either side of a condition, must resolve to a
registered rule. An id left behind by a deleted rule makes its condition permanently
unsatisfiable, which looks exactly like a suppression that simply has not been needed yet.

### Language discipline

Enforced in review, and the reason the catalogue is centralised:

| Never | Instead |
| --- | --- |
| "Mod X is broken" | "Mod X's definition of Y is discarded" |
| "This will crash" | "This is the kind of conflict that commonly causes …" |
| "Incompatible" | "Overlapping — {n} shared definitions" |
| "Fix by removing Mod X" | "Options: patch, reorder, or disable one" |
| "Y was deleted" (from a partial scope) | "Y could not be found; 1 file that may define it could not be read" |
| "No conflicts found" (with unread files) | "No conflicts found among the files read; 3 could not be read" |
| "A patch covers this" | "Mod P may be a compatibility patch for these mods" |

The tool infers from static files; it never observes the running game. Overstating
confidence is the fastest way to lose a user's trust, and one confidently wrong
"incompatible" verdict costs more than ten hedged-but-correct ones.

## Presets

```toml
[preset.player]      # default: actionable only
min_severity = "medium"
disabled_rules = ["meta.descriptor_mismatch", "pdx.namespace_share"]

[preset.modder]      # everything, including the pedantic
min_severity = "info"

[preset.ci]          # for a mod author's GitHub Action
min_severity = "high"
fail_on = "critical"
format = "json"
```

`disabled_rules` holds rule ids and is checked like every other id. A preset naming a
rule that no longer exists silently stops disabling anything, and the user starts seeing
findings they had turned off — a change in behaviour with no change they can point at.

## Calibration

Weights start as informed guesses and must be corrected against reality. The plan:

1. `smd-testkit` ships a corpus of **known-good** playsets (popular mods documented as
   working together) and **known-bad** ones (documented incompatibilities from mod pages
   and bug reports).
2. `cargo xtask calibrate` scores both and reports false-positive rate on the good set
   and false-negative rate on the bad set.
3. A weights change that worsens either is a failing PR.
4. The same run makes the two checks only a corpus can make: every `[[modifier]]` fires at
   least once, and the fraction of findings whose band is set by the base weight alone is
   reported as a trend.

This turns "is our severity model any good?" from an argument into a number, and it is
only possible because weights are data that a test can vary.
