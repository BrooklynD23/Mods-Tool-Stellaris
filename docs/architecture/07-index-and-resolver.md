# 07 — Index and resolver

The index is the immutable, queryable model of "what the game will actually see". It is
built once per scan, then shared by `&` with every rule and every UI query. It contains
no judgement — no severities, no risk, no opinions. Those come later.

## Structures

```rust
pub struct ScanIndex {
    pub sources: SourceTable,                  // SourceId -> SourceMeta; holds `rank`
    pub paths:   Arc<Interner<NormPath>>,
    pub symbols: Arc<Interner<str>>,
    pub classes: Arc<PathClassTable>,
    pub overlay: Arc<OverlayTable>,
    pub defs:    Arc<DefinitionTable>,
    pub loc:     Arc<LocTable>,
    pub events:  Arc<EventTable>,
    pub refs:    Option<Arc<ReferenceTable>>,  // never built in v1 (§ `03`)
    pub derived: Derived,                      // everything the load order decides
    pub diags:   Arc<Vec<Diagnostic>>,
}
```

Every table is `Arc`'d individually and every table except `derived` is built in
*discovery* order, which makes it independent of the load order. That is what the
identity/precedence split in § `03` buys: a reorder rewrites `sources` (300 short rank
vectors) and recomputes `derived`, and shares the rest untouched.

### `Interner<T>` — text stored exactly once

```rust
/// Append-only during the build, frozen for the life of the index. Ids are dense
/// from 0; id 0 is the empty string.
pub struct Interner<T: ?Sized> {
    text: Box<[u8]>,       // every string, concatenated
    ends: Box<[u32]>,      // id -> end offset into `text`
    rev:  HashTable<u32>,  // ids, hashed by the text they point at
}

impl<T: ?Sized> Interner<T> {
    pub fn get(&self, id: u32) -> &T;
    pub fn find(&self, s: &T) -> Option<u32>;
    pub fn len(&self) -> u32;
}
```

`rev` stores ids rather than strings and compares candidates against `text`, so the
reverse lookup costs 8 bytes an entry instead of a second copy of every symbol name.
Across ~1.3M symbols that difference is ~35 MB.

### `SourceTable` — the only place precedence is compared

```rust
pub struct SourceTable { metas: Vec<SourceMeta> }   // indexed by SourceId::index()

impl SourceTable {
    pub fn meta(&self, s: SourceId) -> &SourceMeta;
    /// `None` when either rank is empty — the order is genuinely unknown, not equal.
    pub fn precedes(&self, a: SourceId, b: SourceId) -> Option<bool>;
    pub fn by_rank(&self) -> impl Iterator<Item = SourceId> + '_;
}
```

`SourceId` is not `Ord` (§ `03`), so `precedes` is the only way to ask which of two mods
wins, and every such question in the codebase is greppable.

### `PathClassTable` — merge behaviour belongs to the path

```rust
/// PathId -> the profile's classification. Assigned once, at ingest.
pub struct PathClassTable { classes: Box<[PathClass]> }   // indexed by PathId

#[derive(Copy, Clone)]
pub struct PathClass { pub domain: Option<DomainId>, pub merge: MergeMode }
```

`MergeMode` is a property of where a file sits in the tree, so it is stored once per
path, not once per definition. A path the profile does not cover classifies as
`MergeMode::Unknown` and every order-dependent rule declines to state an outcome for it.

### `OverlayTable` — the file layer

```rust
/// PathId -> contributors, in discovery order. Precedence is decided at resolve time.
pub struct OverlayTable {
    contributors: HashMap<PathId, SmallVec<[Contribution; 2]>>,
}

pub struct Contribution { pub source: SourceId, pub blob: Blob }
```

`SmallVec<[_; 2]>` because the overwhelming majority of paths have exactly one
contributor; inline storage avoids ~400k heap allocations per scan.

### `DefinitionTable`, `LocTable`, `EventTable` — the symbol layer

```rust
pub struct DefinitionTable { map: HashMap<SymbolKey, SmallVec<[DefSite; 2]>> }
pub struct LocTable        { map: HashMap<(LangId, SymbolId), SmallVec<[LocSite; 2]>> }
pub struct EventTable      { map: HashMap<EventKey, SmallVec<[EventSite; 2]>> }

pub struct EventKey { pub namespace: SymbolId, pub ordinal: EventOrdinal<SymbolId> }

pub struct DefSite   { pub site: Site, pub body_hash: [u8; 16], pub slot: u32 }
pub struct LocSite   { pub site: Site, pub value_hash: [u8; 16],
                       pub version_ordinal: Option<u16>, pub slot: u32 }
pub struct EventSite { pub site: Site, pub slot: u32 }
```

`EventKey` carries the ordinal as `EventOrdinal`, not `u32`, because `id = my_mod.setup`
is legal and shipped. Coercing a non-numeric ordinal to a number either drops the fact or
collides unrelated events, and `pdx.event_id_dup` carries the highest base weight in the
catalogue — it is the last rule that should be fed guesses.

### `Derived` — the load-order layer

```rust
/// Rebuilt on every reorder; nothing else is.
pub struct Derived {
    live:     BitBox,                                  // one bit per site slot
    shadowed: Vec<(PathId, SourceId, ShadowCause)>,
}

pub enum ShadowCause { ReplacePath(SourceId), FileReplace(SourceId) }
```

`slot` is allocated from one counter across the definition, loc, and event tables, so the
whole derived layer is a single bitset. `live` is the join between the two layers and is
easy to get wrong. A definition inside a file that was itself replaced (or dropped by
`replace_path`) never reaches the game at all — it is not a competitor for its symbol, it
is already gone. A tool that misses this reports "mod A and mod B both define X, B wins"
when in truth A's whole file was discarded by mod C, which is a different and more
serious story.

Because `live` is derived and stored apart from the sites it describes, it is also the
only thing a reorder invalidates.

## Reordering

```rust
impl ScanIndex {
    /// New ranks in, new index out. Allocates one bitset and one SourceTable.
    pub fn reorder(&self, ranks: &[Vec<u32>]) -> ScanIndex;
}
```

One pass over the overlay (~400k paths) recomputes the shadow list, then one bit per site
(~1.3M) is written. No interner is touched, no map is rebuilt, no key moves. That is the
100 ms reorder budget in § `12`.

Re-running the rules against the reordered index is a *separate* cost, and a much larger
one: a reorder changes which findings exist, not merely their scores, so the sandbox
re-derives rather than re-scores. § `12` budgets that as the analyse stage.

## Resolution

```rust
pub struct Resolution {
    pub outcome:    Outcome,
    pub winner:     Option<SourceId>,
    pub discarded:  SmallVec<[SourceId; 2]>,
    pub agreement:  Agreement,
    pub mechanism:  Mechanism,
    pub confidence: Confidence,
}

/// What actually happens to the participants. Not every collision has a loser.
#[non_exhaustive]
pub enum Outcome {
    /// One live participant reaches the game; the rest are discarded.
    SingleWinner,
    /// All live participants reach the game (`Additive`, `MergeByKeyAdditiveBody`).
    Coexist,
    /// The participants combine and none is discarded (`MergeByKey` across distinct
    /// keys, `Single`).
    MergedInternally,
    /// Participants exist on disk and none of them is live.
    Deleted,
    /// No order is known, or the path is `MergeMode::Unknown`.
    Unknown,
}

/// How far the bodies agree. Computed over live participants only.
pub enum Agreement { AllIdentical, LosersMatchWinner, Differs }

/// Which rule decided the outcome — the mechanism axis, separate from confidence.
#[non_exhaustive]
pub enum Mechanism { LoadOrder, ReplacePath, DirectoryMerge, Undetermined }

impl ScanIndex {
    pub fn resolve_path(&self, p: PathId) -> Resolution;
    pub fn resolve_symbol(&self, k: SymbolKey) -> Resolution;
    pub fn resolve_loc(&self, lang: LangId, key: SymbolId) -> Resolution;
    pub fn resolve_event(&self, k: &EventKey) -> Resolution;
}
```

Resolution rules, in order:

1. If any source declares `replace_path` covering this path and out-ranks a contributor,
   that contributor is dropped with `ShadowCause::ReplacePath` and
   `Mechanism::ReplacePath`.
2. Among survivors, precedence is the highest `rank`, compared lexicographically through
   `SourceTable::precedes`.
3. The path's `MergeMode` selects the granularity and the `Outcome`: `FileReplace`
   resolves the whole file as a unit; `MergeByKey` resolves each symbol independently and
   a "losing" file still contributes its non-colliding keys; `Additive` and
   `MergeByKeyAdditiveBody` produce `Coexist`, because nothing is discarded.
4. If every live participant is shadowed, the outcome is `Deleted`.
5. If the order source was `DirectoryScan`, or any participant's `rank` is empty, or the
   path is `MergeMode::Unknown`, then `winner` is `None`, the outcome is `Unknown`,
   `mechanism` is `Undetermined`, and rules that depend on a winner skip rather than
   guess. `confidence` carries the order source's own verdict (§ `05`) alongside.

`Outcome` and `Confidence` are separate axes and must stay separate: "these two mods
coexist" and "I could not read your playset" are different statements, and a single
`Basis` field collapses them into one that means neither.

> **Invariant R1:** a `Finding` carrying any `Role::Shadowed` participant must have a
> resolution whose outcome is `SingleWinner` or `Deleted`. Debug-asserted at the point
> a finding is pushed.

R1 is worth an assertion because the failure it catches is not a crash but a confident
lie: under `Additive` the participants all survive, and a tool that prints "mod B's
version is discarded" there states the exact opposite of the truth, at High severity, in
bold.

### Agreement

`AllIdentical` means every live participant, vanilla included, hashes equal.
`LosersMatchWinner` is the weaker and far more common claim: every discarded **mod**
matches the winner. Vanilla's body is excluded from that comparison, because a mod that
overwrites a vanilla file differs from it by construction — including vanilla makes the
predicate almost always false and retires the discount that exists for exactly this case,
which is one mod vendoring a copy of another's file.

Policy consumes `agreement` as a tag, not a boolean, so the two cases can be scored and
worded differently: identical everywhere means the collision is cosmetic, losers matching
the winner means nothing is lost but the mods are still shipping each other's content.

### Deleted symbols

```rust
impl DefinitionTable {
    /// Defined somewhere on disk, live nowhere.
    pub fn all_dead(&self, derived: &Derived) -> impl Iterator<Item = SymbolKey> + '_;
}
```

This backs `core.symbol_deleted`. Every other v1 rule is a variation on "≥2 of
something"; this one is the opposite, and it is the more serious class. A symbol that
existed and now exists nowhere live is structurally unreachable — a tech with no entry, a
civic the game cannot instantiate — and it happens through a combination of mods no
single-mod validator can see. Its base weight sits above `symbol_collision` for that
reason.

## Reverse indices

Built only when a registered rule declares it needs them (§ `08`), so a run without
localisation rules never pays to build `loc_by_key`.

| Index | Key → value | Consumer |
| --- | --- | --- |
| `by_source` | `SourceId → [PathId]` | mod inspector, per-mod reports |
| `by_domain` | `DomainId → [SymbolKey]` | domain-grouped browsing |
| `loc_by_key` | `(LangId, SymbolId) → [LocSite]` | localisation rules |
| `events` | `EventKey → [EventSite]` | duplicate event id rule |
| `refs` | `SymbolKey → [Site]` | **not built in v1** — impact analysis, see below |

## Resolving findings for the boundary

```rust
impl ScanIndex {
    pub fn resolve_finding(&self, f: &Finding) -> ResolvedFinding;
}
```

`smd-index` owns the interners, so it is the crate that turns ids into text (§ `03`).
Findings are resolved a page at a time — at most a few hundred rows, whatever the UI or
the report writer is about to emit — so `ResolvedFinding` is a bounded transient rather
than a second copy of the finding set. Nothing downstream of this call holds a
`&ScanIndex`.

## Impact analysis — deferred to v2

```rust
pub struct ReferenceTable { map: HashMap<SymbolKey, Vec<Site>> }

pub struct Impact { pub by_source: Vec<(SourceId, u32)>, pub sites: Vec<Site> }

pub fn impact(&self, k: SymbolKey) -> Impact;   // requires Capabilities::REFERENCES
```

Knowing that mod B overwrote mod A's `trait_super_adaptive` is half a diagnosis; the
other half is **who was relying on it?** That turns a flat conflict list into a causal
story — "mod A references that trait from 4 other places, and mod B's definition does not
define the fields those references expect" — and it is the foundation for the
highest-value post-v1 rule, `core.dangling_reference`: a symbol referenced but, after
resolution, defined nowhere live.

It is not in v1 because it is only as good as its inputs, and its input is
`FactPayload::Reference`, which has no producer until a symbol type map exists (§ `03`).
The seam stays: `ReferenceTable` is an `Option` field, `Capabilities::REFERENCES` gates
it, and enabling it later adds a parser and a rule file rather than touching this
document.

## Memory budget

Per structure, for the provisional playset in § `12` (300 mods, ~154k files, ~500k
definitions, ~800k loc keys, therefore ~1.3M interned symbols). The budget scales with
those counts, not with bytes on disk. § `12` carries the aggregate of record — peak,
process-tree RSS, and the targets — and the rows below must sum to it.

| Structure | v1 (references off) | With references |
| --- | ---: | ---: |
| Path interner (~154k paths) | 20 MB | 20 MB |
| Symbol interner (~1.3M: 500k definition names **+ 800k loc keys**) | 115 MB | 115 MB |
| `OverlayTable` | 40 MB | 40 MB |
| `DefinitionTable` | 70 MB | 70 MB |
| `LocTable` | 90 MB | 90 MB |
| `ReferenceTable` | — | 120 MB |
| Finding vector (~200k findings) | 40 MB | 50 MB |
| **Steady engine heap** | **375 MB** | **505 MB** |

Localisation keys are symbols and share one interner with definition names, which is why
it holds ~1.3M entries rather than the ~500k a definitions-only count suggests. It is the
largest single structure in the index, and correcting it moved the whole budget.

`PathClassTable`, `EventTable`, and `Derived` together are under 5 MB — one `PathClass`
per path, ~30k events, and 1.3M live bits — and sit inside the rounding of the rows
above. The resolved-finding page is bounded by the page size (≤ 500 rows, ~1 MB) rather
than by the finding count, which is the point of resolving at the boundary instead of
materialising a second copy of the finding set.

Peak adds what exists only while a scan runs, and it lands on the index stage — where the
shard maps, the merged tables, and the parse arenas are all alive at once — not on the
analyse stage, which holds the tables and the findings but neither shards nor arenas:

| | v1 (references off) | With references |
| --- | ---: | ---: |
| Steady engine heap | 375 MB | 505 MB |
| Merge transient — shard maps live until the merge completes | 150 MB | 200 MB |
| Parse arenas and pooled read buffers (8 × 8 MB) | 64 MB | 64 MB |
| **Peak engine heap** | **589 MB** | **769 MB** |

Engine heap is not the number the user's machine feels. § `12` adds allocator overhead,
the binary and thread stacks, and the WebView process tree to reach a process-tree RSS
around **1.0 GB for the GUI without references and ~1.2 GB with them**. Two numbers,
because they need two mechanisms: an allocator hook cannot see SQLite's page cache or any
of the WebView's processes, so process-tree RSS must come from the OS, walked over
children. Conflating them is how the budget under-counted before.

That ~200 MB gap is the practical argument for keeping references off in v1. Interning,
`SmallVec` inlining, dropping ASTs immediately, and storing text once in the interner are
all in service of these numbers, and they matter because the tool runs on a gaming
machine while the user may still have the game or launcher open.

## Immutability and sharing

`ScanIndex` is built by the pipeline, then wrapped in `Arc` and never mutated. Rules and
UI queries take `&ScanIndex`. A new scan builds a new index and atomically swaps the
`Arc`; in-flight queries against the old one finish safely and the old index drops when
the last reader releases it. No locks in the query path.

A reorder produces a new `ScanIndex` the same way. Because the tables inside it are
themselves `Arc`'d and order-independent, the new index shares them with the old one, and
two indices differing only in load order cost one extra bitset between them.
