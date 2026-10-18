# ADR-0001: Technology stack

- **Status:** Accepted (design phase) — **except the GUI shell, which is Provisional.**
  Tauri is the working choice; § "The GUI delivery fork" records the alternative and the
  M7 checkpoint that settles it.
- **Date:** 2026-08-05
- **Supersedes:** the "Python + Typer first" sketch in the original brief

## Context

The application is a **local desktop tool** that:

1. Walks 50–400 mod directories. A 300-mod playset is on the order of 150k files and
   1.3 GB (§ `12-performance.md`; provisional until measured).
2. Parses ~400 MB of Clausewitz script and Paradox-flavoured YAML out of that 1.3 GB.
3. Builds cross-mod indices and runs dozens of analysis rules over them.
4. Presents the result in a **GUI** with dense, filterable, virtualised tables and
   side-by-side text diffs.
5. Must run on the machines Stellaris runs on: Windows (dominant), Linux, macOS.
6. Must be installable by a non-technical player — ideally one download, no runtime.
7. Must survive being extended to other Paradox titles and later other Steam games
   without a rewrite.

The two hard constraints pulling against each other are **memory and IO behaviour on a
machine that is simultaneously running a game** (items 1–3) and **shipping to
non-developers** (item 6). Raw parse throughput is *not* one of them — see below.

## Decision

**Rust core + Tauri 2 shell + TypeScript/Svelte front-end**, with the shell provisional.

| Concern | Choice |
| --- | --- |
| Core engine, parsers, rules | Rust **edition 2024**, Cargo workspace |
| Toolchain | Stable for build, test, clippy, and release. Nightly **for the `cargo-fuzz` job only** — no other job may require it |
| Release profile | `panic = "unwind"`, `lto = "thin"`, `codegen-units = 1` |
| Data parallelism | `rayon` (CPU stages), dedicated blocking pool (IO stage), both driven from one engine-owned pool |
| Directory traversal | `ignore` (`WalkBuilder::build_parallel`), `follow_links(false)`; every open goes through `smd-fs`'s root handle (§ `05-ingest.md`, Trust boundaries) |
| Archive reading | `zip`, read in place behind the bounded reader of § `05` (entry-path validation, decompressed-byte caps); never extracted to disk |
| File reads | `read()` into a pooled per-thread buffer; `FILE_FLAG_SEQUENTIAL_SCAN` on Windows. mmap only above a measured size threshold and only under the game install directory |
| Hashing | `blake3` (content addressing + cache keys) |
| Local cache | SQLite via `rusqlite` (`bundled`), WAL mode |
| Paradox launcher DB | `rusqlite` with `bundled` and the `backup` feature, isolated behind one trait; snapshotted with the online backup API, never read in place (§ `05`) |
| GUI shell | **Provisional:** Tauri 2 (system WebView) |
| GUI front-end | Svelte 5 + TypeScript + `tanstack-virtual` for large tables |
| IPC types | `ts-rs`, generated from the Rust types; no bare `u64` across the boundary |
| CLI | Same core crate, separate thin binary |
| Config / game knowledge | TOML data files, not code |
| Tests | `cargo test`, `insta` snapshots, `proptest`, `cargo-fuzz`, `criterion` |

The engine is a **library first**. The CLI and the GUI are both thin adapters over it.
Neither can contain domain logic.

Three rows above are load-bearing enough to be enforced rather than trusted:

- **`panic = "unwind"` is not a preference.** Rule isolation (§ `08-rule-engine.md`) is
  built on `catch_unwind`; under `panic = "abort"` that call is a no-op and a single
  panicking rule takes the whole scan down. Size pressure pushes toward `abort`, so
  `cargo xtask lint` audits the profile and CI runs a test that a deliberately panicking
  rule is contained. `catch_unwind` still cannot catch a stack overflow, which is why the
  parser is iterative with a depth cap (§ `06-parsing.md`).
- **`ignore` rather than `jwalk`.** `jwalk`'s own README declares it unmaintained (one
  release after a 3.5-year gap); it is rayon-based, which contradicts the IO-pool
  separation in § `04-process-architecture.md`; and it parallelises per *directory*,
  while mod trees are wide and flat. `ignore` is ripgrep's walker: its own thread pool,
  parallelism *within* a directory, the most Windows-hardened traversal in the ecosystem,
  and it supplies the glob matcher the profile system needs anyway — one dependency
  instead of two.
- **SQLite rather than a second embedded store.** A pure-Rust KV store buys "zero C
  dependencies" only if there are none already, and `rusqlite(bundled)` is two rows
  above: the launcher database is not optional. Beyond that, the cache's LRU eviction is
  a query SQLite answers directly (`DELETE … ORDER BY atime LIMIT n`) and that a
  single-writer MVCC KV store can only answer by writing on every read.

## Why Rust for the core

**Not throughput.** The parseable text is ~400 MB; at a conservative 200 MB/s per core
across 8 cores the lexing is ~0.26 s, about 1% of the 20 s cold budget and a small
fraction even of the 4 s parse stage that contains it. Any compiled language wins that
race and the race is not close enough to decide anything. The reasons that do hold:

- **Precise memory control against a hard ceiling.** The tool runs while the game or
  launcher is open, on a 16 GB machine, against a budget measured in hundreds of MB
  (§ `12-performance.md`). Arena allocation, `u32` interning, `SmallVec` inline storage,
  and dropping every AST at the end of its file are the difference between 500 MB and
  2 GB — and they are ordinary code in Rust rather than a fight with a runtime.
- **No GC pause during the index merge.** Indexing is burst-allocate/burst-free: each
  shard builds a private map, then all shards merge and the shards die at once. That is
  the worst shape for a generational collector and the exact moment a pause would be
  visible, because it sits between the progress bar finishing and the table appearing.
- **Deterministic output.** Snapshot testing (§ `13-testing.md`) is the primary
  correctness gate, and it requires that the same corpus produces byte-identical output
  across runs and thread counts. Rust gives no reordering the code did not ask for, no
  hash-seed randomisation unless chosen, no finalisation order to design around.
- **A genuinely static single-file CLI.** `x86_64-unknown-linux-musl` produces a binary
  with no interpreter, no runtime, no `PATH`, no venv. That binary is the compatibility
  floor of the whole product (§ `14-packaging.md`) and the fallback whenever a WebView
  runtime misbehaves.
- **The libraries this workload needs are mature here.** `ignore` (traversal + globs),
  `rayon` (structured data parallelism with a controllable pool), `blake3` (content
  addressing at memory bandwidth) are all first-class and battle-tested at exactly this
  scale.
- **Prior art on this exact workload.** `amtep/tiger` validates CK3, Victoria 3, and
  Imperator mod trees — Clausewitz parsing, cross-file symbol resolution, diagnostics
  with spans — in Rust. It is the strongest available evidence that the language and the
  problem fit, and a useful reference for lexer edge cases (§ `06-parsing.md`).

## Why a web front-end rather than an immediate-mode or native toolkit

The UI is a **data inspector**: 10k-row virtualised tables, tree views, faceted filters,
two-pane syntax-highlighted diffs, exportable HTML reports. Web layout and the existing
virtualisation and diff libraries are far ahead of anything in the native-Rust GUI space
for exactly this shape of UI, and the HTML report renderer and the GUI then share one set
of components rather than two implementations of the same table.

That argument is about **HTML**, not about Tauri. Tauri is one way to get an HTML
front-end onto a desktop; § "The GUI delivery fork" covers the other.

## Compatibility cost of Tauri, and the mitigation

| OS | Runtime used | Risk | Mitigation |
| --- | --- | --- | --- |
| Windows 10 1803+ / 11 | WebView2 (Evergreen) | Pre-installed since 2021 via Edge, but the machines that lack it force a trilemma: the ~2 MB bootstrapper needs network at install time; the fixed-version runtime adds ~130 MB to the download; shipping neither means a blank window and no error | Bundle the bootstrapper in the NSIS installer, detect its failure explicitly, and fall back to telling the user to run the CLI |
| Linux | WebKitGTK 4.1 | Distro version skew, and — on NVIDIA drivers — a blank window from the DMA-BUF renderer, still live in 2026 and disproportionately common in a *gaming* audience | Set `WEBKIT_DISABLE_DMABUF_RENDERER=1` in `main()` **before** the webview is initialised; `.deb`/`.rpm` declare the dependency; AppImage for the rest, at a realistic 76–100 MB once WebKitGTK and its GTK stack are bundled; **the CLI is a static musl binary with no GUI deps** |
| macOS 11+ | WKWebView | Runtime is fine; distribution is not. Notarisation is mandatory in practice and presumes a $99/yr Apple Developer account before the first user exists | Universal2 build, notarised; start the account and certificate paperwork at M5, not at release |

**The escape hatch is the browser, not `egui`.** If a WebView runtime proves
untenable, the answer is to render the same Svelte bundle into a file or serve it on
localhost — the components, the virtualiser, and the diff view all survive unchanged.
`egui` is the wrong direction for this UI specifically: immediate-mode redraws fight
virtualised tables of 10k rows, text selection and copy in a diff pane are the weakest
part of the ecosystem, and accessibility is effectively absent. Swapping Tauri for `egui`
would discard the entire front-end and land somewhere worse.

## The GUI delivery fork

The GUI remains the goal. What is unsettled is whether it arrives as a packaged desktop
application or as a document plus an optional local server. **Decision D1 in
`REVIEW-2026-08.md`; checkpoint at M7.**

| | Tauri shell | CLI + self-contained HTML report |
| --- | --- | --- |
| Delivery | Signed installer per OS | One static binary; the report is a single `.html` the OS browser opens |
| Front-end | Svelte bundle in a WebView | The same Svelte bundle, findings JSON inlined |
| Live engine | Always | Only under `smd-cli serve` (binds `127.0.0.1`, random port, per-launch token, `Host`/`Origin` checks — § `11-gui.md`, Output safety; opens a browser, Jupyter-style) |
| Removed if chosen | — | WebView2's trilemma, WebKitGTK skew, the NVIDIA/DMA-BUF blank window, 76–100 MB AppImages, macOS notarisation and its $99/yr prerequisite, the generated-type IPC seam, the Playwright job |
| Cost if chosen | — | The reorder sandbox and on-demand diff fetch need `serve`; a static report can only carry a bounded slice of findings inline |

The second option was never considered in the original decision, and it removes more
risk than any mitigation in the table above. It is not free: an inlined report is a
snapshot, so anything interactive against live state — reordering the playset,
re-resolving, fetching a diff for an arbitrary pair — requires `serve`. And inlining is
bounded: at ~300 bytes of JSON per finding, a 200k-finding playset is ~60 MB, so the
static report carries the top *N* (default 2000) plus full facet counts, with the
remainder reachable through `serve` or a JSON export.

**Criteria at the M7 checkpoint**, in order:

1. Do the M7 trust-test participants actually use the live-engine features (reorder
   sandbox, on-demand diff), or do they read the report and edit their playset in the
   launcher? If the latter, ship the report.
2. Does `smd-cli serve` satisfy the live cases? It is a fraction of Tauri's packaging,
   signing, and updater cost, and it reuses the same bundle.
3. What did the first months of CLI users cost in WebView support load, on Linux
   especially? If it is material, that is the answer.
4. Only if 1 and 2 both favour the desktop shell does Tauri stay, and only then are
   `tauri-specta`, the updater manifest, and the notarisation pipeline worth building.

Nothing before M7 may assume the answer. The engine, the CLI, the query API, and the
Svelte components are shared under either outcome; only the transport differs, which is
why deferring this decision is cheap and making it early is not.

## Alternatives considered

**C# / .NET 8 + Avalonia.** The strongest runner-up, and the stack Irony Mod Manager
actually uses — direct evidence that it works for this exact problem. **A close call,
and not decided on speed:** with NativeAOT and trimming, a self-contained Avalonia app
is ~28 MB on disk and ~12 MB compressed, against ~8 MB compressed for the Rust +
WebView build. That is a difference worth noting, not a difference worth choosing on.
The decision was made on the two things that do diverge: memory control under a hard RSS
ceiling with a burst-allocate/burst-free indexing stage, and the static single-file CLI
that is this product's compatibility floor. If the team's existing skill is C#, this
remains a defensible substitute and the architecture transfers essentially unchanged —
interfaces replace traits, `[Export]`/DI replaces `inventory`.

**Python + PySide6.** Fastest to a prototype, and the right answer if this were a
one-off script. Rejected as the product stack: parse throughput needs a native
extension anyway (so you end up maintaining Rust *and* Python), PyInstaller bundles are
80–200 MB and routinely trip antivirus heuristics, and the GIL forces multiprocessing
for the parallel stages. **Python still has a role** — see § "Python is not gone" below.

**Go + Wails.** Good packaging story, weaker parser ecosystem, and the index merge is
precisely the allocation shape that makes GC pauses visible.

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
  (§ `04-process-architecture.md`) must be generated from Rust types rather than
  hand-maintained, or it will drift. **`ts-rs` only.** `specta` is not a peer: 2.x has
  been release-candidate since 2023, and `tauri-specta` is evaluated only if D1 keeps
  Tauri. Two constraints hold on that boundary regardless of who generates it: no bare
  `u64` crosses it, because JSON loses precision above 2^53 — ids and sizes are strings
  or split into two `u32`s — and every query handler is `async` over `spawn_blocking`,
  because a synchronous handler runs on the thread that must never block for more than
  1 ms (§ `11-gui.md`).
- Nightly exists in CI for exactly one job. Any second use of it is a defect: the fuzz
  targets must build and run under stable `cargo test` as ordinary corpus replays, with
  only the fuzzing driver nightly-gated.
- Code signing is an unsolved cost: unsigned Windows binaries hit SmartScreen. Ship
  unsigned for v0.x with a documented checksum, and revisit once the project has users.
  The Apple side has a lead time and a fee and must start at M5 regardless of D1, unless
  D1 lands on the report and macOS ships as a CLI only.
