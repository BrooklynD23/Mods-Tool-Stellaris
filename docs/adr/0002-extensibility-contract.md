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

Shared *traits* live there too, and for a sharper reason than tidiness. A trait that
returns implementations drags the implementor's crate down onto the declarer. Had
`GameProfile` returned `Vec<Box<dyn LoadOrderSource>>`, `smd-profiles` would have needed
`smd-ingest` — which already depends on `smd-profiles` for `[locations]`. That is not a
lint failure to argue about; Cargo refuses to build it. So `LoadOrderSource`,
`DescriptorReader`, and `VersionReader` are declared in `smd-core`, `GameProfile` returns
`&[SourceSpec]` — a name plus its configuration — and `smd-ingest` owns the name→impl
registry. The shipped TOML already reads `sources = ["launcher_v2_sqlite", …]`; the trait
signature is what has to agree with it.

*Prevents:* the dependency tangle where "just import the other module" is easier than
defining a type — and its sharpest form, the abstract factory whose return type inverts
the layer it lives in.

### 2. Self-registration at the definition site

Parsers, rules, renderers, and profiles register themselves where they are defined:

```rust
// smd-rules/src/registry.rs
pub struct RuleRegistration(pub fn() -> Arc<dyn Rule>);
inventory::collect!(RuleRegistration);

// smd-rules/src/rules/symbol_collision.rs — the whole of the registration
inventory::submit!(RuleRegistration(|| Arc::new(SymbolCollision)));
```

Three details in that shape are load-bearing:

- **The newtype is not decoration.** `inventory` collects a concrete type, so
  `inventory::submit!(&SymbolCollision as &dyn Rule)` does not typecheck. The submitted
  value has to be something `collect!` was invoked on.
- **The registry is sorted at startup.** `inventory::iter` yields link order, which is
  unspecified and varies with the linker, the feature set, and the object-file order.
  Sorting by `RuleId` once at construction makes every downstream ordering derive from a
  stable key rather than from a build artefact.
- **The element is `Arc<dyn Rule>`, not `&'static dyn Rule`.** A `'static` reference can
  only come from a `const`, which forecloses the PyO3 scripting surface and the plugin
  SDK — neither can produce one. `inventory` seeds the registry; `Registry::register`
  stays open at runtime. Inventory is the authoring ergonomic, not the storage.

There is **no central list for the three extension seams** — parsers, rules, and
profiles. Adding a rule never edits a list, and two contributors adding rules in parallel
never conflict.

Three lists *are* central, deliberately: the `Capabilities` bitflag, the
capability→index-builder map, and the `Command`/`Query`/`Event` surface in `smd-api`.
Each is small, each changes under contract review, and each is central because the
alternative is worse — a capability set nobody can enumerate cannot be gated off, and an
IPC surface assembled at link time cannot be exported as TypeScript. §7's gates exist to
keep those three from acquiring a fourth sibling.

*Prevents:* the central registry file that every change touches and every merge fights
over — the single strongest force pulling a codebase toward coupling.

### 2a. Verifying that the registration happened

Link-time collection fails silently, in three ways this workspace will actually hit: an
rlib that nothing references is never linked, so its submissions vanish; `smd-py` is a
`cdylib` and therefore a separate link unit with its own registry; and an integration
test binary links a different subset again, where an empty registry raises no error at
all. A dropped rule produces zero findings, which for a conflict detector reads as a
clean bill of health.

Counting files in `rules/` does not detect this. It cannot model the feature matrix —
`rules-paradox` off is a legitimate build with fewer rules — and a checked-in count is
the central list this mechanism exists to remove.

Instead the **release** binary carries `--selftest`, which prints every registered rule,
parser, renderer, and profile id together with the feature set that produced it, and CI
diffs that against a checked-in `registry.lock` for each shipped feature combination. All
four seams are covered; three of them are otherwise unchecked entirely. It runs against
the release binary because the release profile is where LTO, `codegen-units = 1`, and
section stripping can drop a submission that the debug build kept.

*Prevents:* the failure mode this whole mechanism trades for — no central list means
nothing to read to find out what is missing.

### 3. `#[non_exhaustive]` where it can act, and a construction lint where it cannot

New `FactPayload` variants, new `Severity` levels, new `FileKind`s cannot break
downstream `match` expressions, because downstream code is required to have a wildcard
arm from the first commit.

That applies to **every** shared enum, which means `Subject`, `Role`, `Outcome`,
`Mechanism`, `ShadowCause`, `Confidence`, `JobState`, `Op`, and `Value` carry the
attribute as well. All nine are matched across crate boundaries and all nine will grow —
`Subject` gains three variants at M0 alone.

`Agreement` is the deliberate exception. Its three states are the complete partition of
"do the losers get an outcome they would have wanted", and a fourth would not be a new
case so much as evidence the question was wrong. An enum that cannot grow should say so
by staying exhaustive, so that adding to it is a compile error everywhere at once — which
is what you want when the meaning of every discount in `weights.toml` depends on it.

Two costs the mechanism carries, both of which have to be managed rather than accepted:

- **A wildcard arm converts a compile error into silent data loss.** `_ => {}` in a
  renderer means a new `Subject` variant renders as nothing, in a tool whose entire value
  is not omitting things. The rule is therefore that a wildcard over a shared enum emits
  `Diagnostic::unhandled { type, discriminant }` and never `{}` or `None`; `xtask lint`
  fails the empty forms. Loud degradation is the point of the attribute; quiet
  degradation is worse than the compile error it replaced.
- **It does nothing at the TypeScript boundary.** `ts-rs` emits a closed union, so adding
  a variant is a breaking change for the front-end however the Rust is marked. The GUI's
  protection is `CONTRACT_VERSION` and the generated-types drift check, not this
  attribute.

For **structs** the attribute is the wrong tool outright. `#[non_exhaustive]` forbids
functional-update syntax from another crate, so marking `Finding` would break
`..Finding::unscored()` — the idiom that keeps every rule from re-listing `score` and
`severity`, and the thing that actually absorbs a new field. `Finding` therefore can
never be `non_exhaustive`, and the anti-ripple mechanism for shared structs is the named
constructor plus FRU. `xtask lint` bans direct struct-literal construction of a shared
`smd-core` struct outside `smd-core`, which makes adding a field one line in one crate.

*Prevents:* the ripple where adding one variant breaks compilation in eleven crates and
the resulting reluctance to add variants at all — and its inverse, an attribute that
reads as protection while a `_ => {}` two crates away eats the new variant.

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

Applicability is data too. A rule declares `requires() -> &[FeatureTag]` — "I need event
ids to exist", "I need a descriptor" — against a closed tag vocabulary in `smd-core`,
and each profile declares which tags it supplies. It never names a game. Naming games
was the old `applies_to() -> &[&str]`, which put the string `"stellaris"` in `smd-rules`
and so failed §7's own lint. Feature tags also make the second-game milestone mechanical:
the set of rules that should run is computed, not guessed.

*Prevents:* game specifics leaking into engine code, which is the thing that makes
multi-game support a rewrite rather than a file.

### 6. Three-way split of detect / value / present

`smd-rules` detects and may not score. `smd-policy` scores and may not detect.
`smd-report` presents and may not do either. Each is separately testable and separately
changeable.

The split is only real if the crates' *dependencies* respect it, and the obvious shape
does not. `path_glob` modifiers need the path interner; `{symbol}` placeholders need the
symbol interner; both live in `smd-index`. Taken literally, that hands `&ScanIndex` —
the whole detection layer — to the two crates forbidden from detecting. Instead
`smd-policy` takes `&ScanIndex` for exactly one purpose, resolution, and does it first:
interned ids become strings, participants flatten, evidence values arrive typed. Every
stage after that consumes `ResolvedFinding`, and `smd-report` depends on `smd-core`
alone. The same boundary keeps scan-local interner ids off the API surface, where they
would be meaningless to a caller holding a different scan.

*Prevents:* the most common coupling in tools of this kind — a hardcoded severity
constant inside a detection function, which makes retuning a code change and a
regression risk — and the quieter version, a separation that exists in the prose and not
in `Cargo.toml`.

### 7. Mechanical enforcement

`cargo xtask lint` runs in CI and fails the build on:

**Structure**

- file > 300 code lines, function > 60 code lines, > 5 parameters;
- a dependency edge that contradicts `xtask/layers.toml`, which names every crate in the
  workspace and is validated against `cargo metadata`;
- a game name appearing in a crate above `smd-profiles`;
- a `Rule` impl with no `cases()`;
- two parsers claiming the same `(kind, glob)` at the same priority;
- `_ => {}` or `_ => None` over a shared enum; a struct literal of a shared `smd-core`
  struct outside `smd-core`.

**Referential integrity** — the block the first draft of this ADR was missing, and the
one that decides whether the table below is a claim or a fact:

- every registered `RuleId` resolves to a `[base]` entry in `weights.toml` and to an
  entry in *every* shipped `explain/*.toml`, and every id in those files resolves back to
  a registered rule;
- every rule id named in `suppress.toml` — on either side of a cross-rule condition — and
  in any preset's `disabled_rules` resolves to a registered rule;
- every `when.evidence` key resolves to a key the named rule's `evidence()` declares;
- every `{placeholder}` in `explain/*` is a declared evidence key or a member of the
  closed derived set in `smd-core`;
- `--selftest` output matches `registry.lock`, per shipped feature combination;
- every `[[dir]]` in a shipped profile appears in that profile's golden classification
  list.

The first of those is the one that pays for the rest. A missing weight is not a load
error — it is a rule that scores 0 on every finding forever, which is indistinguishable
from a correct rule finding nothing.

**Counting and exemptions**

- **Code** lines, not raw lines. Blank lines, doc comments, and `#[cfg(test)]` blocks do
  not count. Raw counting taxes documentation and creates an incentive to delete the most
  valuable text in the file.
- Exempt by construction: `**/generated/**`, and any file whose top-level items are all
  `const`/`static`. The byte-class tables, the Windows-1252 map, and the 200-path golden
  classification list are data that happens to be written in Rust.
- Exemptions otherwise require an inline `// xtask:allow(file-length) reason: …`, and the
  allowance is recorded **per file** in `xtask/allowances.toml`. A single workspace-wide
  integer would be exactly the pathology §2 exists to remove: one mutable line every
  branch increments and every merge fights over. Per-file allowances conflict only when
  two branches touch the same file, which is when a conflict is information.

Two gates that look right are absent. `clippy::cognitive_complexity` is a nursery lint:
it misfires, and its output moves between toolchain releases, so a routine `rustup
update` turns CI red on code nobody touched. "Public items per module ≤ 10" has no lint
behind it and is wrong for `smd-core`, which is nothing but public items. A gate that has
to be switched off once is switched off permanently, and the 300-line gate already
reaches what both were aimed at.

*Prevents:* every rule above from being a style guide nobody reads. A constraint that is
not enforced by CI is a preference.

## What "insert or remove without affecting others" concretely means

The unit that matters is not the file, it is the *edit*: small, single-purpose, and in a
place whose name says what it is. Four such edits across four files are a better answer
to the requirement than one edit in one file that a reviewer has to read the whole system
to understand. What the design needs is not for every count to be 1 — it is for no count
to be unknown, and for a missed edit to be a red build rather than a rule that quietly
scores zero.

| Change | Edits | What catches a missed one |
| --- | --- | --- |
| Add a conflict rule | **4** — the rule file (`id`, `needs`, `requires`, `evidence`, `cases`, `run`), its test module, one `[base]` line in `policy/weights.toml`, one entry in `policy/explain/<lang>.toml` **per shipped locale** | rule-id resolution across weights and every locale; evidence/placeholder cross-check; `--selftest` vs `registry.lock` |
| Remove a conflict rule | **4 + N** — those four, plus every `disabled_rules` preset and every `suppress.toml` condition naming the id | the same gates run in reverse: a stranded weight, explain entry, preset, or suppression names an unregistered rule |
| Retune a severity | 1 line in `policy/weights.toml` | calibration FP/FN gate |
| Reword an explanation | 1 entry in `policy/explain/en.toml`; other locales keep the old wording until translated | placeholder cross-check |
| Add a translation | 1 new file `policy/explain/<lang>.toml`, complete for every registered rule | rule-id resolution — a partial locale fails |
| Support a Stellaris patch's new folder | **2** — one `[[dir]]` in `profiles/stellaris.toml`, one line in that profile's golden classification list | `[[dir]]`-coverage gate; `xtask profile-audit` is what finds the folder in the first place |
| Add a Paradox game | **4**, none of them Rust — `profiles/<game>.toml` with its feature tags declared inline, a golden classification list, a fixture tree for the ≥95% coverage gate, one `registry.lock` line | profile round-trip, coverage, and golden gates; `--selftest` diff |
| Add a report format | **2** — one file in `smd-report/src/renderers/`, one `registry.lock` line | `--selftest` diff |
| Add a GUI view | 1 Svelte file + 1 line in `routes.ts` | route-table test |
| Add a new fact kind | 1 **appended** variant, the parser that emits it, the rules that want it, and the discriminant snapshot | append-only snapshot test; the `_ => {}` lint surfaces every match that would now drop it |
| Replace the GUI toolkit | `smd-gui` and `ui/` only; engine untouched | layer graph |

Only "add a new fact kind" touches more than one crate, and it is a genuine cross-cutting
change where that is correct. The rows that cost more than one file cost it because a
rule's *data* lives with the other data of its kind rather than beside its code — which
is mechanism 5 working as intended, not a leak.

## Consequences

- `inventory` collects at link time, and link time is where registrations disappear:
  unreferenced rlibs, the `smd-py` cdylib as a separate link unit, and test binaries
  where an empty registry is not an error. Mitigation is §2a — `--selftest` against
  `registry.lock` on the release binary, plus a startup assertion that the registry is
  non-empty in any binary that intends to run rules.
- `Arc<dyn Rule>` over `&'static dyn Rule` costs an atomic refcount per rule per scan —
  a dozen of them — and buys the scripting and plugin seams. Trivially the right trade.
- `#[non_exhaustive]` costs a wildcard arm in some matches, and the wildcard arm is
  precisely where the mechanism turns into silent data loss. The `Diagnostic::unhandled`
  rule and its lint are not trimmings; without them the attribute converts a loud failure
  into a quiet one.
- Data-driven policy means some errors move from compile time to load time. Mitigated by
  validating all TOML at startup with precise errors, by a CI test that loads every
  shipped data file, and by the referential-integrity gates, which move most of that class
  back to CI time.
- Three lists stay central by design, and that is a real cost: a new capability is a
  reviewed contract change, not a data PR. Paid deliberately, for the reasons in §2.
- The size limits will occasionally be annoying. That is the point; the exemption
  mechanism exists with a required reason, and the per-file ratchet keeps it honest.
