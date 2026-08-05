# 11 — GUI design

## Stance

The GUI is a **viewer over the engine**, with no domain logic of its own. Every number
it shows comes from a `Query`. If a computation is needed for display, it belongs in the
engine so the CLI and the HTML report get it too.

The user it is designed for is a player with 200 workshop subscriptions whose game
crashed on load, who does not know what a scripted trigger is. Everything below follows
from that.

> **Delivery is provisional.** Tauri is the current shell, re-decided at M7 against a
> browser-first alternative (CLI + a self-contained HTML report, escalating to
> `smd-cli serve` for the live features). Everything in this document is a description of
> the *interface*, not of Tauri: the screens, the query surface, and the ownership rules
> are the same either way. Only the transport differs. See ADR-0001 and § `15`.

## The query surface this UI requires

"The UI holds a window, the engine holds truth" is the right rule, and offset pagination
cannot implement it. Three things in the design below break it:

| Feature | Why offset pagination fails |
| --- | --- |
| Sorting by any column | An offset into a sorted set requires the set to be *materialised in that order*. Sorting 200k findings costs 20–40 ms, so re-sorting per page blows the 15 ms page budget an order of magnitude over. |
| Conditioned facet counts | "42 high, of which 9 in Gigas" is a whole-set aggregation per dimension. Counting over the loaded page is the exact bug the ownership rule below forbids. |
| Grouped trees | "Rows 4000–4200, where group 37 is expanded and 12 others are collapsed" is not an offset into anything. |

So the engine hands out a **result-set handle**, and the UI pages against it:

```rust
Query::OpenResultSet(FindingQuery) -> ResultSetId { len, epoch, facet_counts }
Query::Page(ResultSetId, offset, limit) -> Vec<ResolvedFinding>
Query::GroupedQuery { set: ResultSetId, group_by, expanded: Vec<GroupKey>, offset, limit }
```

The engine sorts and materialises once, on open, and owns the ordering. `facet_counts`
comes back with the handle because it is computed in the same pass. Handles are
LRU-evicted (a small cap — the UI holds one or two) and carry the `ScanEpoch`, so a
handle opened before a rescan fails with `Stale` rather than splicing two snapshots into
one scroll position. Changing a filter opens a new set; changing a sort opens a new set;
scrolling does not.

This is the whole of the ownership rule made mechanical: the UI can express *which* rows
it wants and *where it is*, and it can express nothing about their order or their totals,
because it does not have them.

## Information architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Playset: "Gigas Run"   ·  247 mods  ·  Stellaris 4.0.3  ·  Rescan│
├──────────────┬──────────────────────────────────────────────────┤
│  Overview    │                                                  │
│  Conflicts ● │              main content region                 │
│  Load order  │                                                  │
│  Mods        │                                                  │
│  Report      │                                                  │
│  Settings    │                                                  │
├──────────────┴──────────────────────────────────────────────────┤
│  ▸ 3 critical · 41 high · 388 medium · 1.2k low   ⚠ 12 diagnostics│
└─────────────────────────────────────────────────────────────────┘
```

### Overview — the first screen after a scan

Not a conflict dump. A verdict and a shortlist:

- One headline: *"3 issues likely to affect your game."*
- The three, expanded, each with mods involved and a suggested action.
- Playset health strip: mods scanned, outdated, unresolved dependencies, order source
  used and its confidence.
- "Everything else" as a single collapsed row leading to Conflicts.

A 300-mod playset produces thousands of findings, and a wall of them is
indistinguishable from noise. The overview's entire job is to be short.

### Conflicts — the working surface

- **Grouping toggle**: by severity / by mod / by file / by domain / by mod-pair, served by
  `GroupedQuery`. By mod-pair is the one that maps to how users actually act ("do I need a
  Gigas × Planetary Diversity patch?").
- **Virtualised table** (`tanstack-virtual`), pages of 200 fetched against an open
  `ResultSetId`. Scrolling 100k rows must stay smooth; the UI holds a window, never the
  whole set.
- **Facets**: severity, rule, domain, mod, "hide suppressed", "hide identical". Counts
  come from `facet_counts` on the handle and are conditioned on the *other* active
  filters, which is the only version of a facet count that is not misleading.
- **Detail pane** on selection:
  - the plain-language explanation from the catalogue,
  - the participant list in load order with winner/shadowed badges,
  - a **side-by-side diff** of the conflicting blocks — this is the feature that turns
    "trust me" into "see for yourself", and it is what makes the tool credible, which is
    why it ships in M7 with the rest of this screen rather than after it,
  - impact: who else references this symbol — **post-v1**, gated with reference
    extraction (§ `08`); v1's detail pane ends at the diff,
  - actions: suppress, copy citation, open containing file, search the Workshop for a
    patch.

### Load order — a sandbox

Drag to reorder, and the **winner column recomputes without a rescan**. Resolution is a
pure function over the immutable index — nothing is re-read, re-hashed, or re-parsed — so
a reorder is a re-run of the resolve and analyse stages against new `rank` values.

Two things about that are easy to state wrongly, and both are user-visible:

**A reorder re-*derives* findings, it does not re-score them.** Changing the order flips
`live` flags, and rules are "≥2 live definitions of X" predicates, so findings *appear and
disappear* — the row you were looking at may not exist afterwards. This is the same work
as the analyse stage, which the budget puts at ≤2 s (§ `12`), not at 100 ms. The 100 ms
figure covers the **winner column on the load-order screen only**: re-resolving who wins
each contended subject, which is a comparison over already-built tables.

So the screen behaves accordingly: the winner column updates live during the drag; the
findings table greys and shows a stage label while the derivation runs, then swaps in
whole. Half-updated findings are worse than visibly-stale ones, because a user cannot see
which half. If that trade proves unconvincing in testing, the honest fallback is to drop
the sandbox until the identity/`rank` split (§ `03`) makes a scoped re-derivation cheap
enough to be imperceptible — the feature is worth having, not worth faking.

**A sandbox is not global state.** Reordering is a *query parameter*, never a command:

```rust
Query::ResolveWith { order: OrderOverride, .. }   // yes
Command::SetLoadOrder(..)                          // no
```

A `SetLoadOrder` command sitting next to a pure query surface makes every subsequent
query's answer depend on invisible process state — two identical queries returning
different results is exactly what the ownership model exists to prevent, and it breaks the
CLI and the report, which have no sandbox. The override travels with the query (or, for
several queries against one hypothetical, behind a `SandboxId` handle obtained and
released explicitly, scoped like a `ResultSetId`).

It is a *sandbox*: changes are simulated. Writing back to the launcher is an explicit,
confirmed, backed-up action, kept behind a clearly-labelled button, and out of scope for
v1 (§ read-only principle in `01-overview.md`).

### Mods — per-mod inspector

Descriptor fields, file count and size, what this mod overrides, what overrides it,
dependency status, version match, and a "conflict neighbours" list ranked by shared
symbol count. The neighbours list is the fastest route to "which two mods actually fight".

### Report — export

Markdown / JSON / HTML / clipboard summary. The clipboard summary is deliberately shaped
for pasting into a Discord support channel or a Workshop comment: ten lines, mod names,
no paths.

**Redaction is on by default for every export format**, not just the clipboard. HTML and
JSON are what get attached to a bug report, and a file path on Windows carries the user's
real name. The export dialog shows the toggle in the off-by-default direction it will
actually be used in, and `--no-redact` is the local-debugging escape (§ `14`).

## Rendering rules

| Rule | Reason |
| --- | --- |
| Never render more than ~300 rows of real DOM | WebView layout cost dominates past that |
| Never send >1 MB in a single query response | IPC serialisation stalls the main thread |
| Progress events throttled to 20 Hz | Higher rates make the scan slower, not the UI smoother |
| No spinner without a stage name and a count | "Loading…" for 20 s reads as a hang |
| Every severity is text + icon, never colour alone | Accessibility; ~8% of the male player base is colour-blind |
| Diff pane loads byte ranges on demand | Some conflicting files are megabytes |
| A response carrying a stale `ScanEpoch` is discarded, never rendered | An in-flight query can land after a rescan; splicing two snapshots into one scroll position shows rows that never coexisted |

## State ownership

```
engine (Rust)          truth: index, findings, scores, result sets, counts, ordering
    ↓ Query / Event
ui stores (Svelte)     view state: filters, selection, scroll offset, open ResultSetId
    ↓ props
components             pure rendering
```

No derived domain state in the front-end. If the UI needs "count of high findings for
mod X", that is `facet_counts` on the handle, not a `.filter().length` over a partial
page — a computation over a paginated window is simply wrong, and the query surface above
is what makes the wrong version unavailable rather than merely discouraged.

## Cold start

The window must be interactive in under a second, before any scan. Sequence: paint shell
→ show detected locations → offer "Scan" → run. Auto-scanning on launch is a setting,
off by default; a tool that hammers the disk for 20 seconds the moment it opens gets
uninstalled.

## Accessibility and platform fit

- Full keyboard navigation; `/` focuses search, `j`/`k` move rows, `Enter` opens detail.
- Respects the OS light/dark preference; no forced theme.
- Respects `prefers-reduced-motion`.
- Text scales with the OS setting; no fixed-pixel type ramp.
- Window state, column widths, and facet selections persist per playset.
