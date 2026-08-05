# 07 — Index and resolver

The index is the immutable, queryable model of "what the game will actually see". It is
built once per scan, then shared by `&` with every rule and every UI query. It contains
no judgement — no severities, no risk, no opinions. Those come later.

## Structures

```rust
pub struct ScanIndex {
    pub sources:  SourceTable,     // SourceId -> mod metadata, in load order
    pub paths:    Interner<NormPath>,
    pub symbols:  Interner<str>,
    pub overlay:  OverlayTable,
    pub defs:     DefinitionTable,
    pub refs:     ReferenceTable,
    pub loc:      LocTable,
    pub events:   EventTable,
    pub diags:    Vec<Diagnostic>,
}
```

### `OverlayTable` — the file layer

```rust
/// PathId -> contributors, ascending by SourceId. Last entry wins under FileReplace.
pub struct OverlayTable {
    contributors: HashMap<PathId, SmallVec<[Contribution; 2]>>,
    shadowed:     Vec<(PathId, SourceId, ShadowCause)>,
}

pub struct Contribution { pub source: SourceId, pub blob: Blob }

pub enum ShadowCause { ReplacePath(SourceId), FileReplace(SourceId) }
```

`SmallVec<[_; 2]>` because the overwhelming majority of paths have exactly one
contributor; inline storage avoids ~400k heap allocations per scan.

### `DefinitionTable` — the symbol layer

```rust
/// (DomainId, SymbolId) -> every definition, ascending by SourceId.
pub struct DefinitionTable {
    map: HashMap<SymbolKey, SmallVec<[DefSite; 2]>>,
}

pub struct DefSite {
    pub site:      Site,
    pub body_hash: [u8; 16],
    pub merge:     MergeMode,
    pub live:      bool,   // false when the containing file was shadowed
}
```

The `live` flag is the join between the two layers and is easy to get wrong. A
definition inside a file that was itself replaced (or dropped by `replace_path`) never
reaches the game at all — it is not a competitor for its symbol, it is already gone. A
tool that misses this reports "mod A and mod B both define X, B wins" when in truth A's
whole file was discarded by mod C, which is a different and more serious story.

## Resolution

```rust
pub struct Resolution {
    pub winner:   Option<SourceId>,
    pub shadowed: SmallVec<[SourceId; 2]>,
    pub identical: bool,        // all bodies hash-equal → cosmetic collision
    pub basis:    Basis,        // LoadOrder | ReplacePath | Unknown
}

impl ScanIndex {
    pub fn resolve_path(&self, p: PathId) -> Resolution;
    pub fn resolve_symbol(&self, k: SymbolKey) -> Resolution;
}
```

Resolution rules, in order:

1. If any source declares `replace_path` covering this path and loads later than a
   contributor, that contributor is dropped with `ShadowCause::ReplacePath`.
2. Among survivors, precedence is the highest `SourceId` (later loads win).
3. `MergeMode` selects the granularity: under `FileReplace` the whole file resolves as a
   unit; under `MergeByKey` each symbol resolves independently and a "losing" file still
   contributes its non-colliding keys.
4. If the load-order source was `DirectoryScan` (no real order available), `winner` is
   `None` and `basis` is `Unknown`. Rules that depend on a winner skip rather than guess.

Point 4 is the reason `Basis` exists at all: the UI must be able to say "these three
mods collide, but I cannot tell you which wins because I could not read your playset"
instead of inventing an answer.

## Reverse indices

Built only when a registered rule declares it needs them (§ `08`), so an analysis run
with references disabled never pays to build `ReferenceTable`.

| Index | Key → value | Consumer |
| --- | --- | --- |
| `by_source` | `SourceId → [PathId]` | mod inspector, per-mod reports |
| `by_domain` | `DomainId → [SymbolKey]` | domain-grouped browsing |
| `refs` | `SymbolKey → [Site]` | orphan-reference and impact analysis |
| `loc_by_key` | `(LangId, SymbolId) → [DefSite]` | localisation rules |
| `events` | `(namespace, ordinal) → [Site]` | duplicate event id rule |

## Impact analysis (what makes the diagnosis actionable)

Knowing that mod B overwrote mod A's `trait_super_adaptive` is half a diagnosis. The
other half is: **who was relying on it?**

```rust
pub fn impact(&self, k: SymbolKey) -> Impact {
    // every Reference to k, grouped by the source that made it
}
```

This turns a flat conflict list into a causal story:

> `Mod B` replaces `Mod A`'s definition of `trait_super_adaptive`.
> `Mod A` references that trait from 4 other places (2 events, 1 civic, 1 loc key).
> Mod B's definition does not define the fields Mod A's references expect.

That is the sentence a player can act on, and it is only reachable because references
are indexed alongside definitions. It is also the foundation for the highest-value
post-v1 rule: a symbol that is *referenced* but, after resolution, *defined nowhere
live* — a dangling reference created by the combination of mods, which no single-mod
validator can ever detect.

## Memory budget

For a large playset (300 mods, ~400k files, ~500k definitions, ~800k loc keys):

| Component | Estimate |
| --- | --- |
| Path interner | ~30 MB |
| Symbol interner | ~45 MB |
| OverlayTable | ~40 MB |
| DefinitionTable | ~70 MB |
| LocTable | ~90 MB |
| ReferenceTable (optional) | ~120 MB |
| Transient parse arenas | ~cores × 8 MB |
| **Steady-state target** | **< 500 MB** with references, < 380 MB without |

Interning, `SmallVec` inlining, and dropping ASTs immediately are all in service of this
number. It matters because the tool runs on a gaming machine while the user may still
have the game or launcher open.

## Immutability and sharing

`ScanIndex` is built by the pipeline, then wrapped in `Arc` and never mutated. Rules and
UI queries take `&ScanIndex`. A new scan builds a new index and atomically swaps the
`Arc`; in-flight queries against the old one finish safely and the old index drops when
the last reader releases it. No locks in the query path.
