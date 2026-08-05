# 02 — Module and crate layout

The workspace is split so that **every unit is independently testable, independently
benchmarkable, and independently deletable**. The ordinary unit of separation is a file;
a crate boundary is the expensive kind, and is drawn only where its specific benefits are
wanted.

## Workspace

```
stellaris-mod-doctor/
├── Cargo.toml                  # workspace root
├── xtask/                      # cargo xtask: lint gates, fixture gen, release
│   ├── layers.toml             # normative crate graph, checked against cargo metadata
│   └── allowances.toml         # per-file lint exemptions (the ratchet)
├── crates/
│   ├── smd-core/               # contracts only. zero third-party deps.
│   ├── smd-fs/                 # paths, Steam discovery, VFS, hashing
│   ├── smd-profiles/           # GameProfile trait + TOML loader + shipped profiles
│   ├── smd-ingest/             # mod discovery, descriptors, load-order sources
│   ├── smd-parse/              # lexers, parser registry, fact emission
│   ├── smd-index/              # overlay, symbol table, resolver, cache
│   ├── smd-rules/              # rule engine + one file per rule
│   ├── smd-policy/             # scoring, severity bands, explanation catalogue
│   ├── smd-report/             # renderers: json, markdown, html, table
│   ├── smd-engine/             # orchestration: the pipeline + job supervisor
│   ├── smd-api/                # stable command/query surface + TS type export
│   ├── smd-testkit/            # fixture builders, synthetic corpus generator
│   ├── smd-cli/                # [bin] thin adapter
│   ├── smd-gui/                # [bin] Tauri shell, thin adapter
│   └── smd-py/                 # optional PyO3 binding (feature-gated, not in default)
├── ui/                         # Svelte front-end, built into smd-gui
├── profiles/                   # shipped game profile data (TOML)
├── policy/                     # shipped scoring + explanation data (TOML)
└── docs/
```

Fifteen crates; fourteen in a default build, since `smd-py` is feature-gated out.

The count is deliberate in both directions, because "modular" and "many crates" are not
the same property and the brief asked for the first. The requirement is about files and
functions running to hundreds of lines, and the 300-line gate below is what satisfies it.
A crate boundary buys three further things — separate compilation, a dependency direction
a lint can enforce, and genuine deletability — and costs a public API, a manifest, and a
call the optimiser cannot inline without LTO.

Lexing and parsing does not earn four of them. `smd-syntax` + `smd-clausewitz` +
`smd-locyml` + `smd-parse` is one concern split four ways, and it is the hot path: a
`&mut dyn FactSink` virtual call per lexer token, crossing three crate boundaries where
nothing inlines. It is one crate, `smd-parse`, with a module per syntax — the same
separation, at the boundary that costs nothing.

## Crate responsibilities and hard limits

| Crate | Owns | Must never |
| --- | --- | --- |
| `smd-core` | `ModKey`, `SourceId`, `PathId`, `SymbolId`, `Fact`, `Finding`, `ResolvedFinding`, `Severity`, `Diagnostic`, `Span`, `Capabilities`, `FeatureTag`; the `LoadOrderSource` / `DescriptorReader` / `VersionReader` trait *declarations* | Depend on anything; know what a mod directory looks like |
| `smd-fs` | OS path resolution, `libraryfolders.vdf`, path normalisation, `blake3` hashing | Know what Stellaris is |
| `smd-profiles` | `GameProfile` trait, TOML schema, `stellaris.toml`, `SourceSpec` names + config, per-profile `FeatureTag` declarations | Contain analysis logic; name or construct a load-order *implementation* |
| `smd-ingest` | `.mod`/`descriptor.mod` parsing, workshop scan, the name→impl registry of `LoadOrderSource`s | Parse mod *content* |
| `smd-parse` | Lexers for PDX script and localisation, `FileParser` registry, dispatch by `FileKind`, fact emission | Know about traits, civics, or any game concept; store anything |
| `smd-index` | Overlay table, symbol table, reverse indices, resolver, interners, on-disk cache | Interpret meaning or assign risk |
| `smd-rules` | `Rule` trait, engine, one rule per file, per-rule `evidence()` and `cases()` | Format output, hardcode a score, or name a game |
| `smd-policy` | Id resolution into `ResolvedFinding`, weights, bands, explanation strings, suppressions | Detect anything |
| `smd-report` | `Renderer` trait + impls over `ResolvedFinding` | Compute findings; depend on anything but `smd-core` |
| `smd-engine` | Stage wiring, the thread pool, channels, cancellation, progress, error collection | Contain domain rules |
| `smd-api` | `Command`/`Query`/`Event` enums, serde + TS export | Be imported by anything below it |
| `smd-testkit` | Fixture builders, synthetic corpus generator | Appear in a non-dev dependency of a shipping crate |

The two most important lines in that table: **`smd-rules` never assigns a number**, and
**`smd-index` never assigns a meaning.** Detection, valuation, and presentation are three
separate crates so that retuning severity never risks changing detection.

The `smd-profiles` / `smd-ingest` pair deserves spelling out, because its obvious shape
does not compile. `smd-ingest` depends on `smd-profiles` for `[locations]` — where the
user directory and the workshop root live. Had `GameProfile` also returned its load-order
sources as trait objects (`fn order_sources(&self) -> Vec<Box<dyn LoadOrderSource>>`),
`smd-profiles` would need the crate that implements them, and Cargo rejects the cycle
outright. So the traits are declared in `smd-core`, `GameProfile` returns
`&[SourceSpec]` — a name plus its configuration — and `smd-ingest` resolves names to
impls. The shipped profile already reads `sources = ["launcher_v2_sqlite", …]`; the trait
signature is what has to agree with it. Generally: **a trait that returns implementations
drags the implementor's crate down onto the declarer**, and in a layered workspace that
is usually a cycle.

## Size discipline

The brief asks for no files or functions running to hundreds of lines. This is enforced
mechanically by `cargo xtask lint`, run in CI:

| Gate | Limit | Rationale |
| --- | --- | --- |
| File length | **300 code lines** (hard fail), 200 (warn) | A file past 300 lines is two concepts |
| Function length | **60 code lines** (hard fail) | Anything longer has extractable stages |
| Function params | 5 (use a struct beyond that) | Keeps call sites readable |
| Crate dep direction | `xtask/layers.toml`, checked against `cargo metadata` | A cycle or an upward edge fails the build |

**Code lines, not raw lines.** Blank lines, `//!` and `///` comments, and `#[cfg(test)]`
blocks do not count. Raw counting taxes documentation, and a gate that rewards deleting
the explanation of a subtle function is a gate working against the codebase.

**Exempt by construction:** `**/generated/**`, and any file whose top-level items are all
`const`/`static`. The lexer's byte-class tables, the Windows-1252 mapping, and the
200-path golden classification list are data that happens to be written in Rust;
splitting them across four files makes them harder to review, not easier.

**Exemptions otherwise** require an inline `// xtask:allow(file-length) reason: …`, and
the allowance is recorded **per file** in `xtask/allowances.toml`. A single
workspace-wide integer would be a merge-conflict magnet — one mutable line every branch
increments — which is exactly the central-list pathology this layout exists to avoid.
Per-file allowances conflict only when two branches touch the same file, and that is a
conflict worth having.

**Two gates that look right are deliberately absent.** `clippy::cognitive_complexity` is
a nursery lint: it misfires, and its output shifts between toolchain releases, so a
routine `rustup update` turns CI red on code nobody touched. "Public items per module ≤
10" has no lint behind it and is wrong for `smd-core`, which is nothing but public items.
A gate that has to be switched off once stays off; the 300-line gate reaches what both
were aimed at.

The layer graph in `01-overview.md` is rendered *from* `xtask/layers.toml`, not
maintained alongside it. A graph that omits a crate is worse than no graph, because the
lint reading it permits every edge it cannot see.

### What this looks like in practice

`smd-rules` is the crate most at risk of turning into one giant `match`. Instead:

```
crates/smd-rules/src/
├── lib.rs                  # re-exports, ~40 lines
├── engine.rs               # runs registered rules, ~120 lines
├── registry.rs             # inventory collection + capability resolution, ~80 lines
├── context.rs              # RuleCtx: immutable views into the index, ~90 lines
└── rules/
    ├── file_overwrite.rs   # ~110 lines, one rule
    ├── symbol_collision.rs # ~130 lines
    ├── loc_collision.rs    # ~100 lines
    ├── event_id_dup.rs     # ~90 lines
    ├── namespace_share.rs  # ~70 lines
    ├── defines_overlap.rs  # ~80 lines
    ├── dep_missing.rs      # ~70 lines
    ├── dep_order.rs        # ~80 lines
    ├── version_drift.rs    # ~90 lines
    ├── descriptor_mismatch.rs # ~60 lines
    ├── replace_path.rs     # ~110 lines
    └── patch_candidate.rs  # ~120 lines
```

Adding rule #13 creates one file here; deleting rule #7 deletes one file here. Neither
touches `engine.rs`, `registry.rs`, or any other rule — see `08-rule-engine.md` for the
self-registration mechanism that makes that true. Each also touches the rule's *data*: a
base weight and an explanation entry per shipped locale, which live with the other
weights and the other prose rather than beside the code. ADR-0002 enumerates those edits
and the gates that fail when one is missed.

## Feature flags

Each rule group, each renderer, and each shipped profile is a Cargo feature — but they
are features of **four different crates**, and writing them as one table is how a
`--no-default-features` build ends up with no way to turn anything back on.

| Feature | Defined on | Default | Effect |
| --- | --- | --- | --- |
| `rules-core` | `smd-rules` | on | file/symbol/loc collisions — game-agnostic |
| `rules-paradox` | `smd-rules` | on | namespaces, event ids, defines, `replace_path` |
| `report-html` | `smd-report` | on | pulls `dep:minijinja` |
| `profile-stellaris` | `smd-profiles` | on | embeds `profiles/stellaris.toml` |
| `profile-ck3` | `smd-profiles` | off | embeds `profiles/ck3.toml` |
| `py` | workspace | off | builds `smd-py` as a cdylib, pulls `dep:pyo3` |

`smd-engine` is the single place the sets are composed, and it composes by forwarding:

```toml
# smd-engine/Cargo.toml
[features]
default           = ["rules-core", "rules-paradox", "report-html", "profile-stellaris"]
rules-core        = ["smd-rules/rules-core"]
rules-paradox     = ["smd-rules/rules-paradox"]
report-html       = ["smd-report/report-html"]
profile-stellaris = ["smd-profiles/profile-stellaris"]
profile-ck3       = ["smd-profiles/profile-ck3"]
```

This gives a second, coarser removal mechanism: a downstream embedder that only wants
file-level analysis compiles out two thirds of `smd-rules`.

Feature combinations are what `registry.lock` has to account for, since a smaller feature
set legitimately registers fewer rules. `--selftest` prints the active feature set
alongside the registered ids, and CI diffs the default build and
`--no-default-features --features rules-core` against their own recorded lines.

## Front-end layout

```
ui/src/
├── lib/
│   ├── api/            # generated types + typed command wrappers
│   ├── stores/         # scan state, filters, selection
│   └── components/     # VirtualTable, DiffPane, SeverityChip, ModChip …
├── views/
│   ├── Playset.svelte
│   ├── Conflicts.svelte
│   ├── LoadOrder.svelte
│   ├── ModInspector.svelte
│   └── Settings.svelte
└── routes.ts           # the single registry the views appear in
```

Same rule: a new view is one file plus one line in `routes.ts`. Components stay under
150 lines; anything larger splits into a container plus presentational children.
