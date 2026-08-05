# 09 — Policy: scoring, severity, explanation

Detection is mechanical. Valuation is a judgement call that will be wrong at first and
will need tuning for years. Therefore **valuation is data, not code** — a separate crate
reading separate TOML, changeable without recompiling and without touching a rule.

## Pipeline position

```
rules → Vec<Finding> (score 0)
          ↓  smd-policy
        weight lookup → modifiers → clamp → band → explanation → suppression
          ↓
        Vec<Finding> (scored, sorted)
```

## Weights

```toml
# policy/weights.toml
[base]
"core.file_overwrite"      = 45
"core.symbol_collision"    = 65
"core.loc_collision"       = 20
"pdx.event_id_dup"         = 85
"pdx.namespace_share"      = 30
"pdx.defines_overlap"      = 75
"pdx.replace_path_shadow"  = 55
"dep.missing"              = 80
"dep.order"                = 60
"meta.version_drift"       = 15
"meta.descriptor_mismatch" = 10
```

## Modifiers

Modifiers read the `EvidenceBag` the rule attached. They are additive, applied in a
defined order, and each one is a line of TOML.

```toml
[[modifier]]
when = { evidence = "identical", eq = true }
add  = -50
why  = "byte-identical definitions cannot disagree"

[[modifier]]
when = { evidence = "merge", eq = "file_replace" }
add  = 25
why  = "whole-file replacement discards every unrelated entry too"

[[modifier]]
when = { evidence = "basis", eq = "unknown" }
add  = -15
why  = "load order unknown, so the outcome is uncertain"

[[modifier]]
when = { path_glob = "common/defines/**" }
add  = 20

[[modifier]]
when = { path_glob = ["gfx/**", "**/*.dds", "**/*.png"] }
add  = -35
why  = "art overwrites are cosmetic and usually intentional"

[[modifier]]
when = { evidence = "referenced_by_others", gte = 1 }
add  = 20
why  = "another mod depends on the definition being replaced"

[[modifier]]
when = { participants_gte = 4 }
add  = 10
why  = "a pile-up is harder for a user to reason about"

[[modifier]]
when = { rule = "meta.version_drift", same_mod_findings_gte = 3 }
add  = 25
why  = "version drift matters when the mod is already colliding"
```

The last one is how cross-rule interaction is expressed without any rule knowing about
another rule.

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

## Explanations

Prose never appears in rule code. It lives in a catalogue, keyed by rule id, with
placeholders filled from the `EvidenceBag`:

```toml
# policy/explain/en.toml
["core.symbol_collision"]
title  = "{n} mods define {domain} “{symbol}”"
body   = """
{winner} loads last, so its definition of `{symbol}` is the one the game uses. \
The definitions from {losers} are discarded entirely — not merged.
"""
why    = "Two mods defining the same object is not a merge. The later one replaces the earlier one wholesale, including fields the later mod never intended to change."
action = "Check whether a compatibility patch exists for these mods, or change the load order if you prefer {loser_first}'s version."

["core.symbol_collision".variant.identical]
title  = "{n} mods ship an identical {domain} “{symbol}”"
body   = "The definitions are byte-identical, so which one wins does not matter."
why    = "One mod has almost certainly vendored a copy of the other's file."
action = "No action needed."
```

Three payoffs: translation is a file, tone changes need no code review, and the same
strings serve the CLI, the GUI, and the HTML report.

### Language discipline

Enforced in review, and the reason the catalogue is centralised:

| Never | Instead |
| --- | --- |
| "Mod X is broken" | "Mod X's definition of Y is discarded" |
| "This will crash" | "This is the kind of conflict that commonly causes …" |
| "Incompatible" | "Overlapping — {n} shared definitions" |
| "Fix by removing Mod X" | "Options: patch, reorder, or disable one" |

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

## Calibration

Weights start as informed guesses and must be corrected against reality. The plan:

1. `smd-testkit` ships a corpus of **known-good** playsets (popular mods documented as
   working together) and **known-bad** ones (documented incompatibilities from mod pages
   and bug reports).
2. `cargo xtask calibrate` scores both and reports false-positive rate on the good set
   and false-negative rate on the bad set.
3. A weights change that worsens either is a failing PR.

This turns "is our severity model any good?" from an argument into a number, and it is
only possible because weights are data that a test can vary.
