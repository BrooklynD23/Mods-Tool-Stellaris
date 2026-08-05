# 02 — Module and crate layout

The workspace is split so that **every unit is independently testable, independently
benchmarkable, and independently deletable**. If removing a feature requires touching
more than its own crate plus a data file, the boundary is in the wrong place.

## Workspace

```
stellaris-mod-doctor/
├── Cargo.toml                  # workspace root
├── xtask/                      # cargo xtask: lint gates, fixture gen, release
├── crates/
│   ├── smd-core/               # contracts only. zero third-party deps.
│   ├── smd-fs/                 # paths, Steam discovery, VFS, hashing
│   ├── smd-profiles/           # GameProfile trait + TOML loader + shipped profiles
│   ├── smd-ingest/             # mod discovery, descriptors, load order
│   ├── smd-syntax/             # shared lexer primitives
│   │   ├── smd-clausewitz/     # PDX script parser
│   │   └── smd-locyml/         # PDX localisation parser
│   ├── smd-parse/              # parser registry + fact emission
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

## Crate responsibilities and hard limits

| Crate | Owns | Must never |
| --- | --- | --- |
| `smd-core` | `ModId`, `SourceId`, `PathId`, `SymbolId`, `Fact`, `Finding`, `Severity`, `Diagnostic`, `Span` | Depend on anything; know what a mod directory looks like |
| `smd-fs` | OS path resolution, `libraryfolders.vdf`, path normalisation, `blake3` hashing, mmap | Know what Stellaris is |
| `smd-profiles` | `GameProfile` trait, TOML schema, `stellaris.toml` | Contain analysis logic |
| `smd-ingest` | `.mod`/`descriptor.mod` parsing, workshop scan, `LoadOrderSource` impls | Parse mod *content* |
| `smd-clausewitz` | Lexer + event-driven parser for PDX script | Know about traits, civics, or any game concept |
| `smd-locyml` | Lexer for `l_english:` files | Same |
| `smd-parse` | `FileParser` registry, dispatch by `FileKind`, fact emission | Store anything |
| `smd-index` | Overlay table, symbol table, reverse indices, resolver, on-disk cache | Interpret meaning or assign risk |
| `smd-rules` | `Rule` trait, engine, one rule per file | Format output or hardcode a score |
| `smd-policy` | weights, bands, explanation strings, suppressions | Detect anything |
| `smd-report` | `Renderer` trait + impls | Compute findings |
| `smd-engine` | Stage wiring, channels, cancellation, progress, error collection | Contain domain rules |
| `smd-api` | `Command`/`Query`/`Event` enums, serde + TS export | Be imported by anything below it |

The two most important lines in that table: **`smd-rules` never assigns a number**, and
**`smd-index` never assigns a meaning.** Detection, valuation, and presentation are three
separate crates so that retuning severity never risks changing detection.

## Size discipline

The brief asks for no files or functions running to hundreds of lines. This is enforced
mechanically by `cargo xtask lint`, run in CI:

| Gate | Limit | Rationale |
| --- | --- | --- |
| File length | **300 lines** (hard fail), 200 (warn) | A file past 300 lines is two concepts |
| Function length | **60 lines** (hard fail) | Anything longer has extractable stages |
| Function params | 5 (use a struct beyond that) | Keeps call sites readable |
| Cyclomatic complexity | 12 | `clippy::cognitive_complexity` |
| Public items per module | 10 | Signals a module doing two jobs |
| Crate dep direction | see `01-overview.md` graph | A cycle or an upward edge fails the build |

Exemptions require an inline `// xtask:allow(file-length) reason: …` with a reason
string. The lint counts exemptions and fails if the count grows — a ratchet, so debt
can be paid down but not added.

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
    ├── replace_path.rs     # ~110 lines
    └── patch_candidate.rs  # ~120 lines
```

Adding rule #12 creates one file. Deleting rule #7 deletes one file. Neither touches
`engine.rs`, `registry.rs`, or any other rule — see `07-rule-engine.md` for the
self-registration mechanism that makes that true.

## Feature flags

Each rule group and each renderer is a Cargo feature, all on by default:

```toml
[features]
default = ["rules-core", "rules-paradox", "report-html", "profile-stellaris"]
rules-core     = []   # file/symbol/loc collisions — game-agnostic
rules-paradox  = []   # namespaces, event ids, defines, replace_path
report-html    = ["dep:minijinja"]
profile-ck3    = []
py             = ["dep:pyo3"]
```

This gives a second, coarser removal mechanism: a downstream embedder that only wants
file-level analysis compiles out two thirds of the crate.

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
