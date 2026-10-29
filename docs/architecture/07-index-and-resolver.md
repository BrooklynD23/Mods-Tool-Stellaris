# 07 — Index and resolver

The index is the immutable, queryable model of "what the game will actually see". It is
built once per scan, then shared by `&` with every rule and every UI query. It contains
no judgement — no severities, no risk, no opinions. Those come later.

## Structures

```rust
pub struct ScanIndex {
    pub sources: SourceTable,                  // SourceId -> SourceMeta; holds `rank`
    pub provenance: PlaysetProvenance,         // order + activation confidence (§ `03`)
    pub paths:   Arc<Interner<NormPath>>,
    pub symbols: Arc<Interner<str>>,
    pub classes: Arc<PathClassTable>,
    pub dir_rules: Arc<[DirRules]>,           // DirRulesId -> DirRules; plus OrderRules (§ `10`)
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

### `SourceTable` — the only place load order is compared

```rust
pub struct SourceTable { metas: Vec<SourceMeta> }   // indexed by SourceId::index()

impl SourceTable {
    pub fn meta(&self, s: SourceId) -> &SourceMeta;
    /// `None` when either rank is empty — the order is genuinely unknown, not equal.
    pub fn precedes(&self, a: SourceId, b: SourceId) -> Option<bool>;
    pub fn by_rank(&self) -> impl Iterator<Item = SourceId> + '_;
}
```

`SourceId` is not `Ord` (§ `03`), so `precedes` is the only way to compare two mods'
load order, and every such question in the codebase is greppable. It answers one
component of the precedence tuple — `rank` — and **is not the question "who wins"**:
for differently-named files the filename axis decides before load order is reached
(§ Resolution pipeline). `Precedence::compare` is the only way to ask that.

### `PathClassTable` — merge behaviour belongs to the path

```rust
/// PathId -> the profile's classification. Assigned once, at ingest.
pub struct PathClassTable { classes: Box<[PathClass]> }   // indexed by PathId

#[derive(Copy, Clone)]
pub struct PathClass { pub domain: Option<DomainId>, pub rules: DirRulesId }

/// One per `[[dir]]` entry (plus one for "no glob matched"); a few hundred in total.
pub struct DirRules {
    pub merge:     MergeMode,       // cross-file axis (Stage C)
    pub same_path: SamePath,        // Stage A: Replace | Unknown
    pub order:     Option<FileOrder>,  // Stage D filename direction; None = unknown
    pub tier:      u8,              // Stage D tier; 1 for localisation/replace/**
    pub scope:     MergeScopeId,    // which directories' keys compete with each other
    pub verified:  Option<GameVersion>,  // profile `verified`; None = Assumed (§ `03`)
}
```

Merge behaviour is a property of where a file sits in the tree, so it is stored once per
path, not once per definition. A path the profile does not cover classifies with
`merge = Unknown`, `same_path` inherited from the profile default, and `order = None`;
Stage A still runs for it, and Stages C and D decline to state a key-level outcome.

`scope` is what makes `localisation/replace/` a tier rather than a separate table:
`localisation/**` and `localisation/replace/**` are two `[[dir]]` entries with one
`MergeScopeId`, so their keys compete in one group, and the replace entry's `tier = 1`
decides that competition before filename or load order.

### `OverlayTable` — the file layer

```rust
/// PathId -> contributors, in discovery order. Precedence is decided at resolve time.
pub struct OverlayTable {
    contributors: HashMap<PathId, SmallVec<[Contribution; 2]>>,
}

pub struct Contribution {
    pub source:       SourceId,
    pub blob:         Blob,
    pub raw_name:     SymbolId,           // as-shipped spelling, the filename sort key
    pub completeness: ParseCompleteness,  // § `03`; read by Stage C and the Deleted rule
}
```

`SmallVec<[_; 2]>` because the overwhelming majority of paths have exactly one
contributor; inline storage avoids ~400k heap allocations per scan.

### `DefinitionTable`, `LocTable`, `EventTable` — the symbol layer

```rust
pub struct DefinitionTable { map: HashMap<SymbolKey, SmallVec<[DefSite; 2]>> }
pub struct LocTable        { map: HashMap<(LangId, SymbolId), SmallVec<[LocSite; 2]>> }
pub struct EventTable      { map: HashMap<EventKey, SmallVec<[EventSite; 2]>> }

pub struct EventKey { pub namespace: SymbolId, pub ordinal: EventOrdinal<SymbolId> }

pub struct DefSite   { pub site: Site, pub body_hash: [u8; 16],
                       pub residual_hash: Option<[u8; 16]>,   // from AdditiveSplit
                       pub slot: u32 }
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
    surviving: BitBox,                                 // Stage B: stated to reach the game
    possibly:  BitBox,                                 // Stage B: may reach the game (a Stage A gap)
    winning:   BitBox,                                 // Stage C: the definition is in effect
    shadowed:  Vec<(Contender, ShadowCause)>,
    path_gaps: HashMap<PathId, SmallVec<[ResolutionGap; 1]>>,  // Stage A gaps, by path
}

impl Derived {
    /// Stated or possible survivor: a site that takes part in Stage C as a contender.
    /// The predicate collision rules filter on — never `surviving` alone.
    pub fn contends(&self, slot: u32) -> bool;
}

pub enum ShadowCause {
    ReplacePath(SourceId),   // Stage A: a `replace_path` declaration dropped the file
    SamePath(SourceId),      // Stage A: the same root-relative path, higher `rank`
    KeyOverride(Contender),  // Stage C: a higher-precedence definition of the same key
    ReplaceTier(Contender),  // Stage C: a `localisation/replace/` candidate (tier 1)
}
```

`slot` is allocated from one counter across the definition, loc, and event tables, so
each derived layer is a single bitset. `surviving` is the join between the two layers and
is easy to get wrong; `winning ⊆ surviving` and `surviving ∩ possibly = ∅` always. A definition inside a file that was itself replaced (or dropped by
`replace_path`) never reaches the game at all — it is not a competitor for its symbol, it
is already gone. A tool that misses this reports "mod A and mod B both define X, B wins"
when in truth A's whole file was discarded by mod C, which is a different and more
serious story.

Because both bitsets are derived and stored apart from the sites they describe, they are
also the only thing a reorder invalidates.

## Reordering

```rust
impl ScanIndex {
    /// New ranks in, new index out. Allocates three bitsets and one SourceTable.
    pub fn reorder(&self, ranks: &[Vec<u32>]) -> ScanIndex;
}
```

One pass over the overlay (~400k paths) recomputes Stage A, the `surviving` and
`possibly` bits, and `path_gaps`; then one pass over the multi-contender key groups
recomputes Stage C and the `winning` bits.
No interner is touched, no map is rebuilt, no key moves. That is the 100 ms reorder budget
in § `12`. A reorder changes only the `rank` component of the precedence tuple, so a key
group whose contenders all differ in `tier` or filename key cannot change winner; the
pass may skip those groups, and on a FIOS-heavy playset most groups are of that kind.

Re-running the rules against the reordered index is a *separate* cost, and a much larger
one: a reorder changes which findings exist, not merely their scores, so the sandbox
re-derives rather than re-scores. § `12` budgets that as the analyse stage.

## Resolution pipeline

The previous algorithm let a directory's `MergeMode` choose *between* file replacement and
key merging. Stellaris does both in the same directory, selected per pair of files by
filename identity, and orders differently-named files by filename before load order
(`GROUND-TRUTH-PRIOR-ART.md` § 5.1, § 5.2). Resolution is therefore four stages, in a
fixed order, and each stage consumes only the output of the one before:

| Stage | Unit | Decides | Mechanism tags |
| --- | --- | --- | --- |
| **A — file overlay** | `PathId` | which contribution of each path reaches the game | `replace_path`, `same_path` |
| **B — surviving files** | the Stage A winners, plus every possible survivor of a path Stage A left undetermined | which definitions are candidates at all | — |
| **C — key resolution** | key group within one merge scope | how surviving definitions of one key combine | `directory_merge`, `replace_tier` |
| **D — precedence tuple** | two contenders in one group | which one wins | `filename_order`, `load_order` |

### Stage A — file overlay

For each path, over contributions from sources whose `activation` is not `Inactive`:

1. **`replace_path`.** A contribution under a prefix some source declares in
   `replace_path` is dropped with `ShadowCause::ReplacePath` if the declarer outranks it
   by `rank`. Vanilla is always dropped. Earlier **mods** are dropped only if
   `[load_order].replace_path_scope = "vanilla_and_earlier"`; while it is `unknown` the
   resolver enumerates both readings (below), so a replace_path shadowing of vanilla is
   stated and one of an earlier mod carries `ResolutionGap::ReplacePathScope`. A drop is
   stated only if the declarer is `Active` (rule 1 below): a declaration from a source
   whose activation is `Unknown` leaves the contributions under its prefix possibly
   surviving with `ActivationUnknown`, because if the declarer is not enabled nothing is
   dropped.
2. **Same path.** Where the path's `DirRules::same_path` is `Replace`, among the
   remaining contributions the highest `rank` wins the **whole file**; every other is
   dropped with `ShadowCause::SamePath`, and keys present only in a loser's copy are lost
   with it. This runs in every directory, whatever its `merge` — including `Unknown`,
   because file identity does not depend on the cross-file axis. Where `same_path` is
   `Unknown` the path still reports its contributors, with
   `ResolutionGap::SamePathUnknown` and no winner. Path identity is the normalised (lowercased) path;
   that the game agrees is `VERIFY` V42, single source.

Same-path identity against vanilla is **game-version-specific**. Stellaris 4.5.2 renamed
vanilla policy files (`VERIFY` § 4.5 refresh); a mod that whole-file-overrode the old
name now replaces nothing, and its keys collide under Stage C instead. The vanilla file
list therefore always comes from the installed game's tree, never from profile data, and
a vanilla path no installed file matches is not a Stage A participant.

`resolve_path` returns after Stage A. It is complete on its own, which is what lets the
first release report same-path conflicts (§ `15`, M1) before any key-level resolution
exists.

### Stage B — surviving files

Two sets, because Stage A does not always name a winner:

- **surviving** — on a path whose Stage A outcome is stated, the one winning
  contribution. Its sites get `surviving` bits.
- **possibly surviving** — on a path whose Stage A outcome is `Unknown` (any gap:
  `OrderUnknown`, `ActivationUnknown`, `SamePathUnknown`, `ReplacePathScope`,
  `IdentityCollision`), **every** non-`Inactive` contribution. Its sites get `possibly`
  bits, and the path's gaps are kept in `path_gaps`. A single-contribution path from a
  source whose activation is `Unknown` is possibly surviving too: it reaches the game if
  and only if that source is enabled.

A definition in a file Stage A *stated* dropped never reaches the game: it is not a
competitor for its key, it is already gone, and Stage C never sees it. A definition in a
possibly-surviving file may reach the game, and treating it as gone is the error this
split exists to prevent. Under `DirectoryScan` every source's activation is `Unknown` and
every `rank` is empty, so almost every contribution is possibly surviving: collisions are
still detected, as conditional findings (§ `05`), and nothing is ever `Deleted`.

### Stage C — key resolution

Contending definitions — surviving and possibly surviving (`Derived::contends`) — are
grouped by key — `SymbolKey`, `(LangId, SymbolId)`, `EventKey`, or define `(group, key)` —
**within one merge scope** (`DirRules::scope`). A possibly-surviving contender can
**block** a winner but is **never the stated winner**: a winner is stated only if it is
surviving and outranks every possibly-surviving contender in its group under Stage D;
otherwise the outcome is `Unknown` and the resolution inherits the blocking contenders'
`path_gaps`. A group of one surviving site is trivially `winning`; a group of one
possibly-surviving site has no winner and no conflict. A possibly-surviving participant
in a finding is always `Role::Contributor` — never `Winner`, never `Shadowed` (R1) — and
the finding's `certainty` is conditional on the gaps it carries. For a larger group the
directory's `merge` decides:

| `merge` | Stage C behaviour | `Outcome` |
| --- | --- | --- |
| `path_only` | No key layer; groups are not formed | — (Stage A only) |
| `merge_by_key` | Stage D picks one winner; the rest are `KeyOverride` / `ReplaceTier` shadowed. Other keys from the losing files are untouched | `SingleWinner` |
| `merge_by_key_additive_body` | Stage D picks the winner of the **residual** (body minus additive children). Equal `residual_hash` across the group → nothing is lost. Unequal → the winner's non-additive children replace the losers' while every participant's additive children still apply | `Coexist` / `PartiallyMerged` |
| `additive` | Every participant applies | `Coexist` |
| `single` | The scope is one logical file; Stage D orders the group as for `merge_by_key` | `MergedInternally` |
| `unknown` | Participants are reported; no outcome | `Unknown` + `MergeModeUnknown` |

A `merge_by_key_additive_body` group containing a site with `residual_hash = None`
resolves `Unknown` with `ChildDigestsUnavailable` — whole-body hashes cannot distinguish an
additive difference from an override (§ `03`, Invariant A1), and guessing either way
produces exactly the finding this mode exists to prevent or suppress.

### Stage D — precedence tuple

Two contenders in one group are ordered by `Precedence::compare` (§ `03`): `tier`, then
the filename key in the directory's `FileOrder` direction, then `rank`, then the in-file
ordinal. The mechanism is the component that decided: `replace_tier`, `filename_order`, or
`load_order`. The same comparison orders a single mod's own files, so `Resolution`
names the winning **file and definition**, not just the mod.

The ordinal compares positions within **one** file only. Two contenders at **different
paths** whose `tier`, file key, and `rank` all tie — one source shipping
`common/traits/a/x.txt` and `common/traits/b/x.txt` under a basename key — have no
defined order: `compare` returns `None`. While `filename_key` is `unknown` this surfaces
as `FilenameKeyForm` (the `dir_relative` reading separates them, the basename readings do
not, so the readings disagree); once `filename_key` is settled to a reading under which
they still tie, as `FileKeyTie`. Neither is ever broken by `PathId` or discovery order.

### Activation, unknown parameters, and incompleteness

Three rules wrap every stage. Each records a `ResolutionGap` rather than defaulting:

1. **Activation.** A contender from a source with `Activation::Unknown` is never the
   stated winner. A winner is stated only if it is `Active` and outranks every `Unknown`
   contender, because only then is the verdict the same whichever of them are really
   enabled; otherwise `ActivationUnknown`.
2. **Unknown parameters are enumerated.** `replace_path_scope`, `filename_key`
   (basename without extension — Irony's reading, `VERIFY` V37 — vs basename vs
   directory-relative path), `vanilla_in_filename_axis`, `in_file_duplicate`,
   and a directory's `order` or `same_path` may each be `unknown` in the profile (§ `10`).
   The resolver evaluates the subject under every reading of the parameters it actually
   reaches; if the winner agrees, the verdict stands with no gap, otherwise the outcome is
   `Unknown`, `mechanism` is `Undetermined`, and the gap names the parameter. An empty
   `rank` reached by a comparison is `OrderUnknown`.
3. **Incompleteness.** A key group's *scope* is every surviving or possibly-surviving file in its merge scope
   (any filename can define the key); a path's scope is that path's contributions. If any
   file in scope is not `ParseCompleteness::Complete` (or `NotApplicable`), the
   resolution carries `ParseIncomplete`. An unseen definition can only add a contender,
   but an added contender can win, so a winner among the definitions that *were*
   extracted is stated only if it outranks, by `Precedence`, every incomplete file in
   scope — the same test as for activation, and the one `CaseKind::Incomplete` asserts
   (§ `13`: no stated winner over an unparsed contribution). Otherwise there is no winner.
   The `Deleted` rule below admits no such exception.

```rust
pub struct Resolution {
    pub outcome:   Outcome,
    pub winner:    Option<Contender>,
    pub discarded: SmallVec<[Contender; 2]>,
    pub agreement: Agreement,
    pub mechanism: Mechanism,
    pub gaps:      SmallVec<[ResolutionGap; 1]>,
}

/// What actually happens to the participants. Not every collision has a loser.
#[non_exhaustive]
pub enum Outcome {
    /// One live participant reaches the game; the rest are discarded.
    SingleWinner,
    /// All live participants reach the game (`Additive`, or `MergeByKeyAdditiveBody`
    /// with equal residuals).
    Coexist,
    /// Every participant's additive children reach the game; non-additive children come
    /// from the winner only. Nobody is wholly discarded, so nobody is `Shadowed`.
    PartiallyMerged,
    /// The participants combine and none is discarded (`Single`).
    MergedInternally,
    /// Participants exist on disk and none of them survives.
    Deleted,
    /// A gap prevents a stated outcome; `gaps` says which.
    Unknown,
}

/// How far the bodies agree. Computed over live participants only; under
/// `MergeByKeyAdditiveBody` over `residual_hash` rather than `body_hash`.
pub enum Agreement { AllIdentical, LosersMatchWinner, Differs }

/// Which stage and component decided the outcome — separate from certainty.
#[non_exhaustive]
pub enum Mechanism {
    ReplacePath, SamePath,          // Stage A
    ReplaceTier, FilenameOrder,     // Stage D: tier, filename key
    LoadOrder,                      // Stage D: `rank`
    DirectoryMerge,                 // Stage C combined without a winner
    Undetermined,
}

impl ScanIndex {
    pub fn dir_rules_of(&self, p: PathId) -> &DirRules;               // via PathClassTable
    pub fn resolve_path(&self, p: PathId) -> Resolution;              // Stage A only
    pub fn resolve_symbol(&self, k: SymbolKey) -> Resolution;         // A–D
    pub fn resolve_loc(&self, lang: LangId, key: SymbolId) -> Resolution;
    pub fn resolve_event(&self, k: &EventKey) -> Resolution;
}
```

`confidence` is gone from `Resolution`: order and activation confidence are scan-wide
(`PlaysetProvenance`, § `03`), and what one resolution needs to say about them is whether
they *mattered* — which `gaps` says exactly. `Outcome` and certainty stay separate axes:
"these two mods coexist" and "I could not read your playset" are different statements,
and a single field collapses them into one that means neither. `Finding::certainty` is
derived from `gaps` plus the scan's provenance (§ `03`, mapping in § `08`).

### Worked cases

| Case | Old verdict | New verdict |
| --- | --- | --- |
| Mods A < B both ship `common/traits/00_traits.txt` | `merge_by_key`: per-trait collisions only | Stage A: B's file wins wholesale, `same_path`; A's traits absent from B's copy are gone |
| A ships `common/traits/aaa.txt`, B (later) ships `zzz.txt`, both define `trait_x` | B wins, `load_order` | A wins, `filename_order` — `common/traits` is FIOS |
| Same pair in `common/buildings` (LIOS) | B wins | B wins, `filename_order` — and still B under a reversed playset |
| One mod ships `00_a.txt` and `zz_b.txt` both defining `building_x` | Unresolvable (one `SourceId`) | `zz_b.txt`'s definition wins; `Contender` names it |
| A (later) has `localisation/english/x_l_english.yml`, B (earlier) has `localisation/replace/english/y_l_english.yml`, same key | A wins | B wins, `replace_tier`, regardless of order |
| Two mods hook `on_game_start` in differently-named files | `Coexist` from whole-body hashes (unsound) | `Unknown` + `child_digests_unavailable` until H4 lands; then `Coexist` or `PartiallyMerged` |
| Mod in a FIOS dir redefines a vanilla key from `zzz_x.txt` | Mod wins (vanilla lowest) | `Unknown` + `vanilla_filename_axis` until probed: vanilla's `00_` file sorts first |

The last row is deliberately uncomfortable. "Vanilla is always lowest" is corroborated
only for the same-path case (CWTools' override unit is the logical path). For
differently-named files Irony **contradicts** it outside localisation: vanilla
definitions are removed only when they share a filename with a mod's, and otherwise take
part in the ordinal filename sort (`VERIFY` V09). That is one source against an
inference, so the profile carries `vanilla_in_filename_axis = "unknown"` and the
resolver enumerates it; for the most common shape — a mod overriding a vanilla key from
a `zzz_` file in a LIOS directory — both readings agree and the verdict stands.

> **Invariant R1:** a `Finding` carrying any `Role::Shadowed` participant must have a
> resolution whose outcome is `SingleWinner` or `Deleted`. Debug-asserted at the point
> a finding is pushed.

R1 is worth an assertion because the failure it catches is not a crash but a confident
lie: under `Additive` the participants all survive, and a tool that prints "mod B's
version is discarded" there states the exact opposite of the truth, at High severity, in
bold. `PartiallyMerged` is deliberately outside R1: a loser there keeps its additive
children, so it is a `Contributor` carrying `residual_lost = true` in evidence, not
`Shadowed`.

> **Invariant R2:** a `Resolution` with non-empty `gaps` that block its outcome has
> `outcome == Unknown`, `winner == None`, and `mechanism == Undetermined`. A gap that was
> enumerated and found not to change the winner is not recorded at all.

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
    /// Defined somewhere on disk; no site surviving or possibly surviving; every site's
    /// path stated-dropped by Stage A; every file in the key's merge scope `Complete`.
    /// Keys failing any test but the first resolve `Unknown` with the blocking gaps
    /// instead of being yielded.
    pub fn all_dead(&self, derived: &Derived) -> impl Iterator<Item = SymbolKey> + '_;
}
```

**`Deleted` is the one outcome an incomplete parse — or an undetermined overlay — can
never support.** Every other outcome is a claim about definitions that were seen;
`Deleted` is a claim that a definition exists *nowhere* live. Two things can make that
claim false without the index noticing: a file that was only partially parsed, or not
parsed at all, may be exactly where the key lives; and a file whose Stage A outcome is
`Unknown` may reach the game. So outcome `Deleted` requires all three of:

1. no site of the key is `surviving` **or** `possibly` surviving;
2. every path holding a site of the key has a **stated** Stage A outcome that drops it —
   no entry in `path_gaps` for any of them;
3. no `ParseIncomplete` gap in the key's scope, including content-incomplete sources
   (§ `06`).

Failing (1) or (2), the outcome is `Unknown` with the blocking paths' gaps; failing (3),
`Unknown` with `ParseIncomplete`. Either way the finding says the deletion could not be
confirmed. (1) already implies (2) by construction of the `possibly` set; (2) is stated
separately so that the check does not depend on that construction staying right.
The same applies to the overlay: a path whose only surviving contribution is `Unparsed`
still survives — Stage A depends on bytes and paths, not on facts.

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
per path, ~30k events, three bitsets of 1.3M bits, and the Stage A gap map — and sit inside the rounding of the rows
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
