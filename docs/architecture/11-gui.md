# 11 — GUI design

## Stance

The GUI is a **viewer over the engine**, with no domain logic of its own. Every number
it shows comes from a `Query`. If a computation is needed for display, it belongs in the
engine so the CLI and the HTML report get it too.

The user it is designed for is a player with 200 workshop subscriptions whose game
crashed on load, who does not know what a scripted trigger is. Everything below follows
from that.

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

- **Grouping toggle**: by severity / by mod / by file / by domain / by mod-pair.
  By mod-pair is the one that maps to how users actually act ("do I need a Gigas ×
  Planetary Diversity patch?").
- **Virtualised table** (`tanstack-virtual`), pages of 200 fetched on demand. Scrolling
  100k rows must stay smooth; the UI holds a window, never the whole set.
- **Facets**: severity, rule, domain, mod, "hide suppressed", "hide identical".
- **Detail pane** on selection:
  - the plain-language explanation from the catalogue,
  - the participant list in load order with winner/shadowed badges,
  - a **side-by-side diff** of the conflicting blocks — this is the feature that turns
    "trust me" into "see for yourself", and it is what makes the tool credible,
  - impact: who else references this symbol,
  - actions: suppress, copy citation, open containing file, search the Workshop for a
    patch.

### Load order — a sandbox

Drag to reorder, and the **winner column recomputes live without a rescan**. This is
cheap because resolution is a pure function over the immutable index: reassign
`SourceId`s, re-resolve, re-score. Sub-100 ms for a large playset.

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

## Rendering rules

| Rule | Reason |
| --- | --- |
| Never render more than ~300 rows of real DOM | WebView layout cost dominates past that |
| Never send >1 MB in a single query response | IPC serialisation stalls the main thread |
| Progress events throttled to 20 Hz | Higher rates make the scan slower, not the UI smoother |
| No spinner without a stage name and a count | "Loading…" for 20 s reads as a hang |
| Every severity is text + icon, never colour alone | Accessibility; ~8% of the male player base is colour-blind |
| Diff pane loads byte ranges on demand | Some conflicting files are megabytes |

## State ownership

```
engine (Rust)          truth: index, findings, scores
    ↓ Query / Event
ui stores (Svelte)     view state: filters, selection, scroll, pagination window
    ↓ props
components             pure rendering
```

No derived domain state in the front-end. If the UI needs "count of high findings for
mod X", that is a `Query`, not a `.filter().length` over a partial page — a computation
over a paginated window is simply wrong, and this rule prevents that whole bug class.

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
