# 03 — Data contracts (`smd-core`)

These types are the only thing crates share. Everything else is an implementation
detail. Changing a type here is the one edit that ripples, so this file is deliberately
small and the types are deliberately open to extension.

## Interning and identity

Raw `String`s are the memory-and-speed killer at this scale (a 300-mod playset produces
~400k paths and ~1.3M symbols — definition names and localisation keys share one
interner). Everything addressable is interned to a `u32` at ingest time and resolved back
to text only at render time.

```rust
/// Identity only: *which* mod. Assigned in discovery order.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]                  pub struct SourceId(pub u32);
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct PathId(pub u32);
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct SymbolId(pub u32);
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct DomainId(pub u16);
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct LangId(pub u8);

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolKey { pub domain: DomainId, pub symbol: SymbolId }

/// Stable across runs and machines. Workshop id when present, else a hash of the
/// descriptor's identifying fields. Used for cache keys, suppressions, and saved config.
pub struct ModKey(pub Box<str>);

impl SourceId {
    /// Discovery index, for deterministic ordering only. Carries no precedence.
    pub fn index(self) -> u32 { self.0 }
}
```

**`SourceId` has no `Ord`, deliberately.** Precedence lives in a separate field
(`SourceMeta::rank`), so `a < b` does not compile and every precedence comparison routes
through the source table. Identity and precedence are two different facts about a mod;
fusing them means a reorder renumbers every id and invalidates every structure keyed or
sorted by one.

> **Invariant P1:** `SourceId`s are dense from 0 in canonical discovery order: vanilla
> 0, then DLC, then mods, each group by `ModKey` bytes — never filesystem enumeration
> order. `PathId`s are assigned after the stage-3 barrier in ascending `NormPath` byte
> order; `SymbolId`s at the stage-6 merge in ascending text byte order. Ids do not change
> within a scan and remain scan-local (C1); the assignment is deterministic so that two
> scans of one input build identical tables (mechanism: § `04`). Vanilla is always
> `SourceId(0)`, with `rank == [0]`.

> **Invariant C1:** `SourceId`, `PathId`, and `SymbolId` are **scan-local** — never
> serialised, never written to the parse cache, never across the API boundary.
> `DomainId` and `LangId` are profile-derived and stable for a given `profile_hash`,
> which is part of the cache key, so they may be cached. What leaves the engine carries
> text (`ResolvedFinding`) or a stable key (`ModKey`).

## Sources

```rust
pub struct SourceMeta {
    pub key:              ModKey,
    pub name:             Box<str>,
    /// Load-order precedence, compared lexicographically; higher wins. Empty = order
    /// unknown. One component of the precedence tuple (§ Precedence), not the whole of it.
    pub rank:             Vec<u32>,
    /// Whether this source is in the enabled set, and how far that is trusted. Independent
    /// of the order's confidence: an order can be exact while activation is not, and vice
    /// versa.
    pub activation:       Activation,
    pub role:             SourceRole,
    pub deps:             Vec<ResolvedDependency>,
}

pub enum Activation { Active, Inactive, Unknown }

pub enum SourceRole { Vanilla, Dlc, Workshop, Local, Archive }

/// Scan-wide; one per scan, from the `LoadOrderSource` chain (§ `05`). The only home of
/// order and activation confidence — there is no per-source copy to disagree with it.
pub struct PlaysetProvenance {
    pub order_source: &'static str,
    pub order:        Confidence,   // how far `rank` is trusted
    pub activation:   Confidence,   // how far the enabled set is trusted
    pub snapshot:     SnapshotMethod,
}

pub enum SnapshotMethod { OnlineBackup, StableFileCopy, NotApplicable }

/// How far an input is to be trusted; supplied by the order source (§ `05`).
pub enum Confidence { Exact, Inferred, Fallback }

pub enum ResolvedDependency {
    Resolved { declared: Box<str>, source: SourceId },
    /// Declared by display name, so matching is fuzzy (§ `05`). The `u8` is a 0–100
    /// match score; candidates are shown, never auto-applied.
    Missing  { declared: Box<str>, candidates: SmallVec<[(SourceId, u8); 2]> },
}
```

`rank` is a vector so that tiers compare correctly without reserving numeric ranges
(vanilla `[0]`, DLC `[1, n]`, mods `[2, n]`), and so a reorder rewrites 300 short vectors
and nothing else. An empty `rank` is the honest representation of the `DirectoryScan`
fallback — not "loads first", but "no claim".

`DirectoryScan` yields `activation == Unknown` for every discovered source — never
`Active`. "Found on disk" is not "enabled". An empty `rank` (order unknown) and `Unknown`
activation are separate claims and are reported separately. The resolver consumes
activation directly (§ `07`): a contender whose activation is `Unknown` is never a stated
winner, and a stated winner must be `Active` **and** outrank every `Unknown` contender,
because only then is the verdict the same whichever of them are really enabled.
`Inactive` sources are discovered and inventoried but contribute to no resolution.

## Location

```rust
/// Byte offsets into the **raw file as read from disk**: before BOM stripping, before
/// any encoding fallback, before newline normalisation.
pub struct Span { pub offset: u32, pub len: u32, pub line: u32 }

pub struct Site { pub source: SourceId, pub path: PathId, pub span: Span }
```

Raw-file offsets let the UI seek to the exact bytes, and stay valid for a file that had
to be lossily decoded from Windows-1252 — the file most worth showing a user. `line` is
1-based, derived from the decoded text, and advisory: display only, never identity.
`Site` is what makes a finding verifiable, and a rule that emits a finding without at
least one `Site` fails a debug assertion.

## Files

```rust
pub struct Blob { pub hash: [u8; 16], pub len: u64 }   // blake3, truncated

#[non_exhaustive]
pub enum FileKind { Script, Localisation, Asset, Descriptor, Gui, Unknown }

pub struct FileEntry {
    pub source:   SourceId,
    pub path:     PathId,      // identity: normalised, lowercase, `/`, mod-root-relative
    pub raw_name: Box<str>,    // as shipped, directory-relative, NFC only; sort key only
    pub blob:     Blob,
    pub kind:     FileKind,
}
```

Path normalisation is non-negotiable and lives in `smd-fs`: locale-independent Unicode
lowercase (no Turkish/Azeri dotted-i tailoring), `/` separators, no leading slash,
Unicode NFC. Windows and Linux mod archives disagree on case, and a
case-sensitive comparison would report thousands of phantom non-conflicts on Linux.
The key is identity only: the OS path used to open a file is the raw directory-entry name
carried beside it, never reconstructed from the key. Two entries of one source whose keys
collide are both kept and the path is marked ambiguous (`fs.identity_collision`, § `05`);
Stage A resolves such a path with `ResolutionGap::IdentityCollision` rather than picking
one.

**The normalised path is identity; it is not the filename sort key.** The FIOS/LIOS
filename axis (§ Precedence) is an *ordinal, case-sensitive* comparison — `Z` sorts
before `a` — so lowercasing destroys exactly the information it needs. `FileEntry`
therefore keeps the as-shipped spelling, `raw_name`, beside the identity. Two files whose
normalised paths are equal are the same path for the overlay (Stage A, § `07`) whatever
their spelling; whether the *game* agrees on case-insensitive identity is `VERIFY` V42
(single source: Irony groups by case-insensitive filename).

```rust
/// The effective extraction classification of one path: everything besides the bytes
/// that decides which facts a parse produces. Part of the parse-cache key (§ `06`).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ExtractionClass {
    pub kind:      FileKind,
    pub parser:    &'static str,       // FileParser::id()
    pub extractor: &'static str,       // "depth_key" | "event" | "none" | named
    pub level:     u8,
    pub domain:    Option<Box<str>>,   // domain *name*, not DomainId
    /// The profile's `additive_children` for this path, sorted. Non-empty only where the
    /// extractor emits `AdditiveSplit` facts, which depend on it (§ Facts).
    pub additive_children: Box<[Box<str>]>,
}
impl ExtractionClass { pub fn key_bytes(&self) -> Vec<u8>; }  // canonical, length-prefixed
```

Identical bytes under `common/traits/` and `common/civics/` are two cache entries, not
one: the cached `Definition` carries a domain, and the domain comes from the path, not the
bytes. The same holds for `additive_children`: identical on_action bytes under two
profiles with different child sets split differently, so the set is in the key.

## Facts

A fact is one atomic observation extracted from one file. Parsers emit a flat stream of
them; the index folds them. Parsers hold no state beyond the current file, which is what
makes the parse stage trivially parallel. Every fact has two forms, and the split is a
correctness boundary rather than an optimisation:

```rust
/// Parser output. File-local and string-based: the only form that may be serialised,
/// and therefore the only form the parse cache stores.
pub struct RawFact { pub span: Span, pub payload: FactPayload<Box<str>> }

/// Index input. Interned and sited. Never serialised (Invariant C1).
pub struct Fact { pub site: Site, pub payload: FactPayload<SymbolId> }
```

The cache is keyed by file content, so the same bytes vendored by five mods is one entry
and five hits. That only holds if the cached value says nothing about *which* mod it came
from — and a `SymbolId` is exactly that, a position in one scan's interner. A cache
written by one scan and read by the next attributes facts to whichever mod interned that
id first, making warm scans — the normal path — silently wrong. `RawFact` carries the
symbol's text and a span; `source` and `path` are supplied at rehydration, by the caller
that already knows them.

```rust
#[repr(u8)]
#[non_exhaustive]
pub enum FactPayload<S> {
    /// A top-level named game object: a trait, civic, building, tech…
    Definition { domain: DomainId, symbol: S, body_hash: [u8; 16] } = 0,
    /// A localisation entry. `version_ordinal` is the optional `:0` suffix.
    LocKey { lang: LangId, key: S, version_ordinal: Option<u16>, value_hash: [u8; 16] } = 1,
    /// A use of a symbol. **No producer in v1** — reserved, see below.
    Reference { domain: DomainId, symbol: S } = 2,
    /// `namespace = my_mod`
    Namespace { symbol: S } = 3,
    /// `id = my_mod.100`
    EventId { namespace: S, ordinal: EventOrdinal<S> } = 4,
    /// A single key inside `common/defines/`.
    DefineKey { group: S, key: S, value_hash: [u8; 16] } = 5,
    /// Emitted immediately after a `Definition` on a path whose classification names
    /// `additive_children`. `residual_hash` covers the body with every named additive
    /// child removed; `additive_count` is how many such children were removed.
    AdditiveSplit { domain: DomainId, symbol: S, residual_hash: [u8; 16],
                    additive_count: u16 } = 6,
}

/// Event ids need not be numeric: `id = my_mod.setup` is legal and shipped.
#[non_exhaustive]
pub enum EventOrdinal<S> { Num(u32), Name(S) }
```

`Definition` carries no `MergeMode`: merge behaviour is a property of the *path*,
classified once by the profile (§ `10`). A definition cannot merge differently from the
file it lives in, and a per-definition copy only creates something that can disagree.

### `AdditiveSplit` — why a whole-body hash is not enough

`body_hash` answers one question: are two definitions of a key byte-for-byte the same
body. Additive-body semantics ask a different one: *do they differ only in children that
accumulate?* Two on_action definitions that each add one distinct event differ in
`body_hash` and are harmless; two that add the same event and disagree on a
non-accumulating child also differ in `body_hash` and are a real override. A whole-body
hash cannot tell them apart, so a resolver fed only `body_hash` either reports every
additive overlap as a conflict or none of them.

`residual_hash` is the smallest fact that separates the cases: equal residuals mean the
participants differ only in additive children, which all reach the game; unequal
residuals mean the winner's non-additive children replace the losers'. It is one extra
fact per definition, emitted only where the profile names `additive_children`, which is
why `additive_children` is part of `ExtractionClass`.

> **Invariant A1:** `MergeMode::MergeByKeyAdditiveBody` is resolvable only for a path
> whose extractor emits `AdditiveSplit`. The profile loader rejects
> `merge = "merge_by_key_additive_body"` on any `[[dir]]` whose extractor does not
> declare it, and a definition reaching the resolver without its split resolves with
> `ResolutionGap::ChildDigestsUnavailable` — never as `Coexist`.

Until that extractor exists (gate H4), `common/on_actions` stays `merge = "unknown"`.
Per-child digests (which event, which child) are deliberately not in v1: the residual is
enough to decide the outcome, and naming the colliding child is a rendering refinement.

### `ParseCompleteness` — what a file's facts can be trusted to cover

```rust
/// How completely one file's facts were extracted. Cached beside the facts.
#[non_exhaustive]
pub enum ParseCompleteness {
    /// Parsed end to end with no recovery.
    Complete,
    /// Parsed with recovery (unbalanced braces, depth cap, a replacement char inside a
    /// key token): the facts emitted are real, but facts may be missing.
    Partial,
    /// Contributes to the overlay but no facts were extracted: oversize, unreadable,
    /// changed during the scan, or no parser accepts it.
    Unparsed(UnparsedReason),
    /// Hash-only by design (`parser = "none"`). Facts are not expected.
    NotApplicable,
}
pub enum UnparsedReason {
    Oversize, Unreadable, ChangedDuringScan, NoParser,
    /// The trust boundary declined to read it: a link/junction/reparse point inside a
    /// root, a cloud placeholder (OneDrive) that would need hydration, a non-regular
    /// file, a handle that resolved outside its root, or an unsafe archive entry
    /// (§ `05` Trust boundaries).
    Refused,
}
```

The resolver reads it: a key whose surviving files include one that is not `Complete`
cannot be stated `Deleted` — an absent definition in a partial file is absence of
evidence (§ `06`, § `07`).

`#[non_exhaustive]` on every payload enum is the load-bearing detail: a new fact kind can
be added without breaking any downstream `match`, because downstream code is forced to
have a `_ =>` arm from day one. The explicit discriminants are the other half — see the
compatibility rules.

### `FactPayload::Reference` is reserved in v1

No parser emits it, `Capabilities::REFERENCES` (§ `08`) is off by default, and a rule
declaring that capability does not run. The variant and its discriminant stay in the
contract so that turning extraction on later is a parser change, not a cache-invalidating
contract bump. Untyped extraction is worse than none: with no symbol type map, `owner`,
`root`, `value`, and `yes` all read as references, impact analysis becomes noise, and
every score consulting it inflates uniformly. That type map is the prerequisite and a v2
decision (§ `07`).

### `MergeMode` — the cross-file axis, and only that

Paradox directories do not all behave alike, and treating them alike is why naive
conflict tools drown users in noise. But a directory's behaviour has **two** axes, and an
earlier version of this enum tried to express both as one choice:

- **same path** — two sources ship the identical root-relative path. Corroborated by
  three independent sources (`VERIFY.md`): the winner's file replaces the loser's
  *wholesale*, and keys present only in the loser's copy are lost. This is a property of
  file identity, not of a directory, and it is decided by the overlay (§ `07`, Stage A)
  before any key is looked at.
- **cross file** — differently-named files in the same directory. Here the directory
  matters, and this is all `MergeMode` now describes.

Modelling them as alternatives (`FileReplace` *or* `MergeByKey`) made `common/traits`
unclassifiable: `merge_by_key` missed two mods both shipping `00_traits.txt`;
`file_replace` inflated every ordinary trait overlap (REVIEW-2026-10, H1).

```rust
#[non_exhaustive]
pub enum MergeMode {
    /// No key layer. Files resolve by path identity alone (Stage A); differently-named
    /// files never interact. Binary assets. Formerly `FileReplace`, renamed because
    /// same-path replacement is no longer a mode — it happens in every directory.
    PathOnly,
    /// Across surviving files, definitions are grouped by key; the highest-precedence
    /// definition of a key wins (§ Precedence) and other keys from the losing file survive.
    MergeByKey,
    /// Merged by key, but the children named in `additive_children` accumulate from every
    /// participant. Gated by Invariant A1.
    MergeByKeyAdditiveBody,
    /// Entries accumulate; duplicates coexist rather than override.
    Additive,
    /// The directory is read as exactly one logical file.
    Single,
    /// The profile does not classify this path's cross-file behaviour. Stage A still
    /// applies; no key-level outcome may be asserted.
    Unknown,
}

/// Whether Stage A's whole-file replacement applies on this path. Per directory, defaulted
/// from `[load_order].same_filename_replaces_whole_file`.
pub enum SamePath { Replace, Unknown }
```

Two mods each shipping `common/buildings/00_buildings.txt` is a **hard** conflict in any
directory — one mod's entire file is discarded — and it is reported by Stage A with
`Mechanism::SamePath` whatever the directory's `MergeMode`, including `Unknown`. Two mods
shipping *differently named* files in a `MergeByKey` directory is **usually fine** and
only matters where keys actually collide. Same directory, two axes, two verdicts; both
come from the game profile (§ `10`), not from rule code.

`MergeByKeyAdditiveBody` is selected by `additive_children` on a `merge_by_key`
directory, and only where the extractor emits `AdditiveSplit` (Invariant A1).
`common/on_actions/` is what it exists for: every mod hooking `on_game_start` contributes
events and all of them run. Without the mode that is one 200-participant collision — the
loudest finding in the report, and entirely false. Without `AdditiveSplit` the mode
cannot tell that case from a real override, which is why it is gated rather than shipped.

`Unknown` makes an unclassified path a stated gap rather than a default guess. Defaulting
it to `MergeByKey` would produce confident verdicts about directories nobody has
modelled.

## Precedence

Precedence in Stellaris is not "later mod wins". It is a tuple, and load order is its
third component (`VERIFY.md` § Load order semantics; `GROUND-TRUTH-PRIOR-ART.md` § 5.2):

```rust
/// Compared component by component, first difference decides. Built per definition site
/// (or per file, for Stage A) by `smd-index`; never stored in a fact.
pub struct Precedence {
    /// Path-class tier; higher wins regardless of everything below.
    /// `localisation/replace/**` is 1, everything else 0 (profile: `tier`).
    pub tier:    u8,
    /// The filename axis: `FileEntry::raw_name`, compared ordinally (byte order,
    /// case-sensitive). Direction is the directory's `FileOrder`.
    pub file:    FileKey,
    /// Load order: the source's `rank`. Reached only when `tier` and `file` tie, which
    /// for two sources means the same file key at different paths (subdirectories).
    pub rank:    RankRef,
    /// Position within the file, for the same key defined twice in one file.
    pub ordinal: u32,
}

pub enum FileOrder { Lios, Fios }   // last / first ordinal filename wins

impl Precedence {
    /// `None` when the comparison reaches a component that is unknown: an empty `rank`,
    /// an unknown filename-key form, an unknown in-file order. Never a guess.
    pub fn compare(&self, other: &Self, dir: &DirRules) -> Option<Ordering>;
}

/// Who a resolution is about — a file, or one definition inside one file. A `SourceId`
/// alone cannot say which of a mod's two trait files won.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Contender { pub source: SourceId, pub path: PathId, pub slot: Option<u32> }
```

Three consequences follow, and each was a wrong verdict in the previous model:

1. **For differently-named files, the filename decides and load order is irrelevant.** A
   mod placed *later* loses a `common/traits` conflict to an earlier mod whose filename
   sorts lower, because `common/traits` is FIOS. An empty `rank` therefore does not by
   itself stop a winner being named between differently-named files — though a
   `DirectoryScan` fallback still names none, because its activation is `Unknown`
   (§ Sources).
2. **Within one mod, the same tuple orders the mod's own files.** `00_a.txt` and
   `zz_b.txt` in one mod both defining a key resolve exactly as if two mods shipped them,
   and `Contender` names the winning file.
3. **`localisation/replace/` is a tier, not a special case in the resolver.** Any
   candidate under a `replace` directory beats every candidate that is not, before the
   filename or the load order is consulted.

Whether **vanilla** takes part in the filename axis for differently-named files is
unestablished ("vanilla is always lowest" is corroborated only for the same-path case),
and so is whether `file` is the basename without extension (Irony's reading, single
source), the basename, or the directory-relative path. Irony, the one source, says vanilla
*does* take part outside localisation (`VERIFY` V09, V37). Neither is defaulted; see
`ResolutionGap`.

### `ResolutionGap` — why a resolution could not name a winner

```rust
/// One per input the verdict depended on and could not be established. Empty = the
/// outcome is stated. Copied onto the `Finding` for explanation text.
#[non_exhaustive]
pub enum ResolutionGap {
    OrderUnknown,            // a needed `rank` comparison hit an empty rank
    ActivationUnknown,       // an `Activation::Unknown` contender could outrank the winner
    MergeModeUnknown,        // the directory's cross-file behaviour is `Unknown`
    SamePathUnknown,         // the directory's Stage A behaviour is `SamePath::Unknown`
    FilenameKeyForm,         // basename vs dir-relative path changes the winner
    VanillaFilenameAxis,     // whether vanilla joins the filename axis changes the winner
    ReplacePathScope,        // vanilla-only vs vanilla-and-earlier changes the outcome
    InFileOrder,             // first vs last duplicate in one file changes the winner
    ChildDigestsUnavailable, // additive-body path without `AdditiveSplit` (Invariant A1)
    ParseIncomplete,         // a file in the subject's scope is not `Complete`
    IdentityCollision,       // one source ships two entries normalising to one path
    FileKeyTie,              // two paths tie on tier, settled file key, and rank
}
```

**Unknown profile parameters are enumerated, not defaulted.** Where a parameter is
unknown, the resolver evaluates the subject under each candidate reading; if every
reading names the same winner, the outcome is stated and no gap is recorded, and only if
they disagree does the gap appear. Most subjects never reach the uncertain component —
two differently-named files in a LIOS directory need neither the filename-key form nor
the in-file order when no subdirectory is involved — so enumeration costs a hedge only
where the hedge is true. Six parameters are open, none with more than three readings, and
a subject is re-evaluated only for the parameters its comparison actually reaches — a
small constant, paid only by subjects with two or more contenders.

`ResolutionGap` is the specific reason; `Certainty` (§ Findings) is its coarse summary
for policy and sorting, derived from it by the mapping in § `08`. They are one source of
truth in two resolutions, not two.

**Stage A gaps propagate.** A path whose overlay outcome is `Unknown` does not lose all
its contributions — every non-`Inactive` one is *possibly surviving* (§ `07`, Stage B):
it contends in Stage C, can block a winner, is never the stated winner, and forbids
`Deleted` for every key it defines. The key-level resolution inherits the blocking
paths' gaps. Without this, a scan whose overlay is undetermined everywhere — every
`DirectoryScan` scan — would report every symbol deleted.

## Findings

```rust
pub struct RuleId(pub &'static str);        // "core.symbol_collision"

#[non_exhaustive]
pub enum Subject {
    Path(PathId),
    Symbol(SymbolKey),
    LocKey { lang: LangId, key: SymbolId },
    Event { namespace: SymbolId, ordinal: EventOrdinal<SymbolId> },
    /// A `replace_path` declaration: which mod declared it, over what prefix.
    Directory { declarer: SourceId, prefix: PathId },
    Mod(SourceId),
    ModPair(SourceId, SourceId),
}

pub enum Role { Winner, Shadowed, Identical, Contributor, Missing }

pub struct Participant { pub site: Site, pub role: Role }

pub struct Finding {
    pub rule:         RuleId,
    pub subject:      Subject,
    /// Ascending by `Precedence` (§ Precedence); where `compare` is `None`, and for ties,
    /// by `(SourceId::index(), path, span.offset, span.len, role)`. P1 assigns `SourceId`
    /// in `ModKey` order, so the fallback is deterministic across runs.
    pub participants: Vec<Participant>,
    pub facts:        EvidenceBag,        // structured k/v for the renderer
    pub certainty:    Certainty,          // the meet of rule and inputs; never raised
    pub gaps:         SmallVec<[ResolutionGap; 1]>,   // copied from the Resolution
    pub score:        i32,                // filled by smd-policy, 0 from the rule
    pub severity:     Severity,           // ditto
}

/// Four independent axes. None of them is the score, and none may be folded into one.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Certainty {
    pub completeness: Completeness,
    pub activation:   ActivationCertainty,
    pub order:        Confidence,          // min over participants; Exact < Inferred < Fallback
    pub semantics:    SemanticCertainty,
}
/// Were all inputs the claim depends on fully parsed? Ordered best-first.
pub enum Completeness { Complete, Partial, Unparsed }
/// Are all participants known to be enabled?
pub enum ActivationCertainty { Certain, Inferred, Conditional }
/// Is the merge behaviour for this subject modelled and verified for this game build?
pub enum SemanticCertainty { Verified, Assumed, Unmodelled }

#[derive(PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Severity { Info, Low, Medium, High, Critical }

pub type EvidenceBag = SmallVec<[(&'static str, EvidenceValue); 4]>;

#[non_exhaustive]
pub enum EvidenceValue {
    Int(i64), Bool(bool),
    /// A closed-vocabulary tag — an enum rendered as its snake_case name, never prose.
    Tag(&'static str),
    Sym(SymbolId), Path(PathId), Src(SourceId),
}

/// The declaration side of the same contract. One variant per `EvidenceValue` variant;
/// `Tag` additionally names the enum whose serialised variants are the legal vocabulary.
#[non_exhaustive]
pub enum EvidenceType {
    Int, Bool, Tag(&'static str), Sym, Path, Src,
}
```

`EvidenceType` lives here rather than beside the `Rule` trait because it is read from both
ends: `smd-rules` declares with it and `smd-policy` validates against it, and a type owned
by either would make the other depend on its neighbour. It is the schema half of the one
contract in this design that no compiler otherwise checks (§ `08`, § `09`).

`Subject` covers every shape a v1 rule reports on. Without the `LocKey`, `Event`, and
`Directory` variants, a loc collision, a duplicate event id, and a `replace_path` shadow
all collapse onto `Subject::Path`, and three distinct findings about one file become
indistinguishable to sorting, to suppression by subject, and to policy's same-subject
predicates.

Rules emit findings with `score: 0` and `severity: Info`. **The policy layer is the only
code allowed to write those two fields.** This is what lets severity be retuned from a
TOML file without recompiling or re-testing detection logic.

`EvidenceBag` is a small typed map rather than a formatted string, so the same finding
renders as a CLI table row, a JSON object, and a GUI card without any renderer re-parsing
prose. Values carrying ids are resolved at the policy boundary, never by a renderer.

`Certainty` is computed by `smd-engine` as the meet of what the rule supplies and what the
participants' sources, files, and resolution support — a rule can lower it, never raise
it. Its `Default` is the **least** certain value, `(Unparsed, Conditional, Fallback,
Unmodelled)`, so a forgotten field under-claims rather than over-claims. `semantics` is
derived from `gaps` and from the directory's profile `verified` key (§ `10`); the mapping
is § `08`'s. `Confidence` derives `Ord` best-first for this reason.

### Ordering must be total

`Severity` derives `Ord` — declaration order is severity order. `Subject` implements `Ord`
by hand over `(variant discriminant, then each field as its raw u32)`; that order is for
in-engine maps and equality only and **must never order output**, because interned ids
are minted during parallel stages and their numeric order is a property of one run. The
output order is defined over `ResolvedFinding`, on text:

| # | Key | Direction |
| --- | --- | --- |
| 1 | `severity` | descending |
| 2 | `score` | descending |
| 3 | `rule` | byte-lexicographic |
| 4 | `subject` — variant discriminant, then each field's resolved text in declaration order (`EventOrdinal`: `Num` before `Name`, `Num` numerically) | ascending |
| 5 | `certainty` (the four axes, best-first) | ascending |
| 6 | participant vector, each as `(mod_key, path, span.offset, span.len, role discriminant)`, compared lexicographically, shorter vector first on a common prefix | ascending |
| 7 | canonical evidence hash — blake3 over the *resolved* evidence sorted by key, each value tagged by variant and length-prefixed | ascending |

No interned id's numeric value is a sort key anywhere downstream of detection. Keys 1–5
are what a reader wants. Keys 6 and 7 make the order **total**: both are functions of the
finding's entire content, so two findings that compare equal are the same finding. A
partial key over values drawn from `HashMap` iteration is consistent within a run and
different between runs, which makes the determinism gate (§ `13`) pass most of the time
and fail at random — the worst behaviour available to a guard.

### `ResolvedFinding` — the policy and presentation boundary

Interned ids stop at the engine's edge (Invariant C1); everything downstream of detection
receives text.

```rust
pub struct ResolvedFinding {
    pub rule:         RuleId,
    pub subject:      Subject,      // retained for identity, sorting, and suppression
    pub subject_text: Box<str>,
    pub participants: Vec<ResolvedParticipant>,
    pub facts:        SmallVec<[(&'static str, ResolvedEvidence); 4]>,
    pub certainty:    Certainty,
    pub gaps:         SmallVec<[ResolutionGap; 1]>,
    pub score:        i32,
    pub severity:     Severity,
}

pub struct ResolvedParticipant {
    pub mod_key:  ModKey,
    pub mod_name: Box<str>,
    pub path:     Box<str>,        // normalised, mod-root-relative
    pub span:     Span,
    pub role:     Role,
}

#[non_exhaustive]
pub enum ResolvedEvidence { Int(i64), Bool(bool), Tag(&'static str), Text(Box<str>) }
```

`smd-index` produces these (§ `07`), being the crate that owns the interners.
`smd-policy` needs path text for its `path_glob` modifiers and `smd-report` needs symbol
text for `{symbol}`; given `ResolvedFinding`, neither takes `&ScanIndex`, and the
separation the layer map exists to enforce stays real rather than nominal.

## Diagnostics

Parse failures are data, not control flow.

```rust
pub struct Diagnostic {
    pub site:     Site,
    pub code:     &'static str,   // "clausewitz.unbalanced_brace"
    pub severity: DiagSeverity,   // Warn | Error
    pub detail:   Box<str>,
}
```

```rust
/// File-local form, cacheable like `RawFact`; sited at rehydration. Cached beside the
/// facts and the file's `ParseCompleteness`, so a warm scan replays the cold scan's
/// recovery diagnostics instead of reading a `Partial` file as clean.
pub struct RawDiagnostic {
    pub span:     Span,
    pub code:     Box<str>,       // resolved against the code registry on decode
    pub severity: DiagSeverity,
    pub detail:   Box<str>,
}
```

```rust
/// Scan-wide summary of what was not fully read. Printed by every report surface, even
/// with zero findings (§ `08`). Text and stable keys only, so it crosses the API
/// boundary as-is (Invariant C1).
pub struct ScanCompleteness {
    pub files_total:   u32,
    pub partial:       u32,                                 // ParseCompleteness::Partial
    pub unparsed:      SmallVec<[(UnparsedReason, u32); 5]>, // by reason, zero counts omitted
    /// Domain names holding at least one `Partial` or `Unparsed` file, sorted.
    pub domains:       Vec<Box<str>>,
    /// Sources whose tree exceeded an enumeration bound, or whose archive was refused
    /// or not read (§ `05`, § `06`). Content-incomplete: any key, any directory.
    /// The `Box<str>` is the mod-root-relative path where reading stopped (the refused
    /// link, the bound that was hit), when there is one.
    pub content_incomplete: Vec<(ModKey, ContentIncompleteReason, Option<Box<str>>)>,
}
/// `LinkRefused`: a directory link refused at the trust boundary (§ `05`), so its subtree
/// was never walked.
pub enum ContentIncompleteReason { Truncated, ArchiveRefused, ArchiveUnread, LinkRefused }
```

A scan returns `(Report, Vec<Diagnostic>)`. The GUI surfaces the diagnostic count in a
corner badge; the CLI prints a summary line and the full list under `-v`. A scan with
diagnostics is still a successful scan.

## Compatibility rules for this file

1. **`FactPayload` variants are append-only.** A new variant is declared last and takes
   the next free discriminant. Inserting, reordering, or removing one requires a
   `CONTRACT_VERSION` bump. The enum is `#[repr(u8)]` with explicit discriminants
   declared in order, and a snapshot test pins the assignment — rkyv and postcard encode
   a variant by position, not by name, so an inserted variant does not fail to decode: it
   decodes every fact already in the cache as the wrong kind.
2. Adding a variant to any other `#[non_exhaustive]` enum is allowed; none are serialised.
3. Adding a struct field is allowed only if it has a `Default`.
4. `Capabilities` (§ `08`) is a `u32`, of which seven bits are used. A bitflags set that
   must never be narrowed costs nothing to declare wide now and a contract bump later.
5. Removing or repurposing anything requires bumping `CONTRACT_VERSION`, which is part of
   every cache key — so an incompatible change safely invalidates caches rather than
   silently reading stale data with the wrong meaning.

```rust
pub const CONTRACT_VERSION: u32 = 1;
```

**The October 2026 amendments land in v1, not v2.** `FactPayload::AdditiveSplit` (6),
`ExtractionClass`, `ParseCompleteness`, `ScanCompleteness`, `RawDiagnostic`,
`Precedence`/`Contender`, `ResolutionGap`, `Certainty`, `Activation`/`PlaysetProvenance`,
`FileEntry::raw_name`, and the renames `MergeMode::FileReplace → PathOnly` and `ShadowCause::FileReplace → SamePath` are all
pre-freeze: no parse cache has ever been written, so there is nothing to invalidate and
`CONTRACT_VERSION` stays 1. The M0 skeleton in `crates/smd-core` predates them and must
be brought in line before M1 (REVIEW-2026-10, H7). From the first release on, the
rules above apply without exception — `AdditiveSplit` was appended rather than inserted
so that the discriminant snapshot only grows.

New `snake_case` tags, which `weights.toml` modifiers compare against (§ `09`):
`MergeMode` gains `path_only` (replacing `file_replace`); `Mechanism` gains `same_path`,
`filename_order`, `replace_tier`; `Outcome` gains `partially_merged`; `ResolutionGap`
serialises as `order_unknown`, `activation_unknown`, `merge_mode_unknown`,
`same_path_unknown`, `filename_key_form`, `vanilla_filename_axis`, `replace_path_scope`,
`in_file_order`, `child_digests_unavailable`, `parse_incomplete`, `identity_collision`,
`file_key_tie`.
