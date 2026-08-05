# ADR-0001: Technology stack

- **Status:** Accepted (design phase)
- **Date:** 2026-08-05
- **Supersedes:** the "Python + Typer first" sketch in the original brief

## Context

The application is a **local desktop tool** that:

1. Walks 50–400 mod directories containing 100k–500k files.
2. Parses hundreds of MB of Clausewitz script and Paradox-flavoured YAML.
3. Builds cross-mod indices and runs dozens of analysis rules over them.
4. Presents the result in a **GUI** with dense, filterable, virtualised tables and
   side-by-side text diffs.
5. Must run on the machines Stellaris runs on: Windows (dominant), Linux, macOS.
6. Must be installable by a non-technical player — ideally one download, no runtime.
7. Must survive being extended to other Paradox titles and later other Steam games
   without a rewrite.

The two hard constraints pulling against each other are **parse throughput** (item 2)
and **shipping to non-developers** (item 6).

## Decision

**Rust core + Tauri 2 shell + TypeScript/Svelte front-end.**

| Concern | Choice |
| --- | --- |
| Core engine, parsers, rules | Rust 2021, stable toolchain, Cargo workspace |
| Data parallelism | `rayon` (CPU stages), dedicated blocking pool (IO stage) |
| File IO | `memmap2` for text, `jwalk` for parallel directory traversal |
| Hashing | `blake3` (content addressing + cache keys) |
| Local cache | `redb` (pure-Rust embedded KV, zero C deps) |
| Paradox launcher DB | `rusqlite` with `bundled`, isolated behind one trait |
| GUI shell | Tauri 2 (system WebView, ~8–15 MB installer) |
| GUI front-end | Svelte 5 + TypeScript + `tanstack-virtual` for large tables |
| CLI | Same core crate, separate thin binary |
| Config / game knowledge | TOML data files, not code |
| Tests | `cargo test`, `insta` snapshots, `proptest`, `cargo-fuzz`, `criterion` |

The engine is a **library first**. The CLI and the GUI are both thin adapters over it.
Neither can contain domain logic.

## Why Rust for the core

- The hot path is a byte-level lexer over mmap'd text. A hand-written Rust lexer
  sustains 200–500 MB/s per core; the same work in CPython is 30–80× slower and forces
  either a C extension or a multiprocessing architecture that reintroduces IPC cost.
- `rayon` makes the parse and analyse stages embarrassingly parallel with no GIL, no
  process pool, no pickling of results across process boundaries.
- Arena/interning strategies (§ `03-data-contracts.md`) need precise control of
  allocation and lifetimes to keep a 500k-symbol index under a sane memory budget.
- One statically-linked binary per platform. No interpreter, no `PATH`, no venv, no
  "Python not found" support tickets.
- Enum + trait modelling maps cleanly onto the plugin contracts this design depends on,
  and the compiler enforces the module boundaries rather than a style guide doing it.

## Why Tauri rather than an immediate-mode or native toolkit

The UI is a **data inspector**: 10k-row virtualised tables, tree views, faceted filters,
two-pane syntax-highlighted diffs, exportable HTML reports. Web layout and the existing
virtualisation/diff libraries are far ahead of anything in the native-Rust GUI space for
exactly this shape of UI, and the HTML report renderer and the GUI can then share one
set of components.

Tauri keeps the cost of that low: it uses the OS's own WebView instead of bundling a
browser, so installers land around 8–15 MB rather than Electron's 120 MB+, and idle RSS
is roughly a tenth of Electron's.

## Compatibility cost of Tauri, and the mitigation

| OS | Runtime used | Risk | Mitigation |
| --- | --- | --- | --- |
| Windows 10 1803+ / 11 | WebView2 (Evergreen) | Pre-installed since 2021 via Edge; a few stale machines lack it | Ship the WebView2 bootstrapper in the NSIS installer (Tauri supports this natively) |
| Linux | WebKitGTK 4.1 | Distro version skew is the real weak point | `.deb`/`.rpm` declare the dep; AppImage for everyone else; **the CLI is a static musl binary with no GUI deps** |
| macOS 11+ | WKWebView | None meaningful | Universal2 build, notarised |

The escape hatch is deliberate: because the GUI is a thin adapter, replacing the Tauri
shell with an `egui` shell later touches only `smd-gui` and costs nothing in the engine.
If Linux WebKitGTK support proves too painful in practice, that swap is the answer,
not a rearchitecture.

## Alternatives considered

**C# / .NET 8 + Avalonia.** The strongest runner-up, and the stack Irony Mod Manager
actually uses, which is evidence it works for this exact problem. Rejected on two
counts: self-contained deployment is ~70 MB versus ~12 MB, and the parse hot path
would need `Span<T>`-heavy, allocation-free code that fights the language's grain to
approach what Rust gives by default. If the team's existing skill is C#, this is a
defensible substitute and the architecture below transfers essentially unchanged —
interfaces replace traits, `[Export]`/DI replaces `inventory`.

**Python + PySide6.** Fastest to a prototype, and the right answer if this were a
one-off script. Rejected as the product stack: parse throughput needs a native
extension anyway (so you end up maintaining Rust *and* Python), PyInstaller bundles are
80–200 MB and routinely trip antivirus heuristics, and the GIL forces multiprocessing
for the parallel stages. **Python still has a role** — see § "Python is not gone" below.

**Go + Wails.** Good packaging story, weaker parser ecosystem, and GC pauses are
awkward when a scan allocates a few hundred MB of AST in a burst.

**Electron / Node.** Rejected: 120 MB+ installers and a JS parse path that would be the
slowest component in the system.

## Python is not gone

The core exposes a PyO3 binding (`smd-py`, built by `maturin`) as a **secondary
deliverable**, not a dependency. Modders get `pip install stellaris-mod-doctor` for
scripting and CI without the desktop app, and the tool gets a scripting surface for
custom checks. The GUI never depends on it.

## Consequences

- A Rust toolchain becomes a contributor prerequisite; the project must compensate with
  a strong `README`, `just`/`cargo xtask` task runner, and CI that produces artefacts so
  casual contributors can test without building the GUI.
- Front-end and back-end are different languages, so the command boundary
  (§ `04-process-architecture.md`) must be generated from Rust types (`ts-rs` or
  `specta`) rather than hand-maintained, or it will drift.
- Code signing is an unsolved cost: unsigned Windows binaries hit SmartScreen. Ship
  unsigned for v0.x with a documented checksum, and revisit once the project has users.
