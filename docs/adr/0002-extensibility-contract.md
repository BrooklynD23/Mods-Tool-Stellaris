# ADR-0002: The extensibility contract

- **Status:** Accepted (design phase)
- **Date:** 2026-08-05

## Context

The stated requirement: *future updates can be inserted or removed without affecting
other components*, and no file or function should run to hundreds of lines.

That requirement is easy to agree with and hard to keep. Six months in, the usual
outcome is a 2,000-line `analysis.rs` with a `match` over every conflict type, because
each individual addition was "just one more arm". This ADR records the specific
mechanisms that make the good structure the path of least resistance, so the outcome
does not depend on discipline alone.

## Decision

Seven mechanisms, each addressing a specific way modularity decays.

### 1. Contracts crate with zero dependencies

`smd-core` holds every shared type and depends on nothing. Every other crate depends on
`smd-core` and, as far as possible, not on each other. Components communicate through
data, not through calls into each other's internals.

*Prevents:* the dependency tangle where "just import the other module" is easier than
defining a type.

### 2. Compile-time self-registration (`inventory`)

Parsers, rules, renderers, and profiles register themselves at their definition site:

```rust
inventory::submit!(&SymbolCollision as &dyn Rule);
```

There is **no central list to edit**. Adding a rule is `git add` of one file. Deleting a
rule is `git rm` of one file. Two contributors adding rules in parallel never conflict.

*Prevents:* the central registry file that every change touches and every merge fights
over — the single strongest force pulling a codebase toward coupling.

### 3. `#[non_exhaustive]` on every shared enum

New `FactPayload` variants, new `Severity` levels, new `FileKind`s cannot break
downstream `match` expressions, because downstream code is required to have a wildcard
arm from the first commit.

*Prevents:* the ripple where adding one variant breaks compilation in eleven crates, and
the resulting reluctance to add variants at all.

### 4. Capability declaration instead of implicit availability

A rule declares `needs() -> Capabilities`. The engine builds only what the enabled rules
require. Removing a feature therefore removes its cost automatically, with no
conditional anywhere in the pipeline.

*Prevents:* the dead weight of a removed feature — indices still built, fields still
populated, "just in case something needs it".

### 5. Domain knowledge as data

Folder semantics, merge modes, severity weights, explanation prose, benign-path lists,
and OS path layouts are TOML. Rules are generic; the game-specific knowledge is
configuration. A Stellaris patch adding `common/new_thing/` is a data PR reviewable by
someone who has never written Rust.

*Prevents:* game specifics leaking into engine code, which is the thing that makes
multi-game support a rewrite rather than a file.

### 6. Three-way split of detect / value / present

`smd-rules` detects and may not score. `smd-policy` scores and may not detect.
`smd-report` presents and may not do either. Each is separately testable and separately
changeable.

*Prevents:* the most common coupling in tools of this kind — a hardcoded severity
constant inside a detection function, which makes retuning a code change and a
regression risk.

### 7. Mechanical enforcement

`cargo xtask lint` runs in CI and fails the build on:

- file > 300 lines, function > 60 lines, > 5 parameters, complexity > 12;
- a dependency edge that points upward in the layer graph;
- a game name appearing in a crate above `smd-profiles`;
- a `Rule` impl without both a positive and a negative test;
- two parsers claiming the same `(kind, glob)` at the same priority;
- an increase in the count of `// xtask:allow(...)` exemptions (a ratchet).

*Prevents:* every rule above from being a style guide nobody reads. A constraint that is
not enforced by CI is a preference.

## What "insert or remove without affecting others" concretely means

| Change | Files touched |
| --- | --- |
| Add a conflict rule | 1 new file in `smd-rules/src/rules/` + 1 test module |
| Remove a conflict rule | delete those 2 files |
| Retune a severity | 1 line in `policy/weights.toml` |
| Reword an explanation | 1 entry in `policy/explain/en.toml` |
| Add a translation | 1 new file `policy/explain/<lang>.toml` |
| Support a Stellaris patch's new folder | 1 entry in `profiles/stellaris.toml` |
| Add a Paradox game | 1 new file in `profiles/` |
| Add a report format | 1 new file in `smd-report/src/renderers/` |
| Add a GUI view | 1 Svelte file + 1 line in `routes.ts` |
| Add a new fact kind | 1 variant + the parser that emits it + the rules that want it |
| Replace the GUI toolkit | `smd-gui` and `ui/` only; engine untouched |

Only the last two rows touch more than one place, and both are genuine cross-cutting
changes where that is correct.

## Consequences

- `inventory` uses link-time collection, which needs care with `dylib` builds and can be
  surprising when a crate is not otherwise referenced. Mitigation: rule crates are always
  linked into the binary, and `xtask` asserts the registered rule count matches the
  number of files in `rules/`, so a silently-dropped registration fails CI.
- `#[non_exhaustive]` costs an unreachable `_` arm in some matches. Accepted.
- Data-driven policy means some errors move from compile time to load time. Mitigated by
  validating all TOML at startup with precise errors, and by a CI test that loads every
  shipped data file.
- The size limits will occasionally be annoying. That is the point; the exemption
  mechanism exists with a required reason, and the ratchet keeps it honest.
