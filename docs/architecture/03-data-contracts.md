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

> **Invariant P1:** `SourceId`s are dense from 0 in discovery order and do not change
> within a scan. Vanilla is always `SourceId(0)`, with `rank == [0]`.

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
    /// Precedence, compared lexicographically; higher wins. Empty = order unknown.
    pub rank:             Vec<u32>,
    pub enabled:          bool,
    pub role:             SourceRole,
    pub order_confidence: Confidence,
    pub deps:             Vec<ResolvedDependency>,
}

pub enum SourceRole { Vanilla, Dlc, Workshop, Local, Archive }

/// How far the load order is to be trusted; supplied by the order source (§ `05`).
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
    pub source: SourceId,
    pub path:   PathId,      // normalised, lowercase, forward slashes, mod-root-relative
    pub blob:   Blob,
    pub kind:   FileKind,
}
```

Path normalisation is non-negotiable and lives in `smd-fs`: lowercase, `/` separators,
no leading slash, Unicode NFC. Windows and Linux mod archives disagree on case, and a
case-sensitive comparison would report thousands of phantom non-conflicts on Linux.

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
}

/// Event ids need not be numeric: `id = my_mod.setup` is legal and shipped.
#[non_exhaustive]
pub enum EventOrdinal<S> { Num(u32), Name(S) }
```

`Definition` carries no `MergeMode`: merge behaviour is a property of the *path*,
classified once by the profile (§ `10`). A definition cannot merge differently from the
file it lives in, and a per-definition copy only creates something that can disagree.

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

### `MergeMode` — the concept that makes risk assessment possible

Paradox directories do not all behave alike, and treating them alike is why naive
conflict tools drown users in noise.

```rust
#[non_exhaustive]
pub enum MergeMode {
    /// Same filename anywhere later in load order replaces the whole file.
    /// The vanilla file is gone even if the mod only wanted to change one entry.
    FileReplace,
    /// Entries across all files in the directory are merged; the last definition
    /// of a given key wins, other keys from the losing file survive.
    MergeByKey,
    /// Merged by key, but same-key *bodies* accumulate rather than replace.
    MergeByKeyAdditiveBody,
    /// Entries accumulate; duplicates coexist rather than override.
    Additive,
    /// The directory is read as exactly one logical file.
    Single,
    /// The profile does not classify this path. No merge claim can be made, and no
    /// rule may assert an outcome for it.
    Unknown,
}
```

Two mods each shipping `common/traits/00_species_traits.txt` under `FileReplace` is a
**hard** conflict — one mod's entire trait file is discarded. Two mods shipping
*differently named* files in a `MergeByKey` directory is **usually fine** and only
matters where symbol names actually collide. Same raw observation, opposite verdicts.
The distinction comes from the game profile (§ `10`), not from rule code.

`MergeByKeyAdditiveBody` is selected by `additive_children = true` on a `merge_by_key`
directory. `common/on_actions/` is what it exists for: every mod hooking `on_game_start`
contributes effects and all of them run. Without the mode that is one 200-participant
collision — the loudest finding in the report, and entirely false.

`Unknown` makes an unclassified path a stated gap rather than a default guess. Defaulting
it to `MergeByKey` would produce confident verdicts about directories nobody has
modelled.

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
    pub participants: Vec<Participant>,   // ascending by rank, ties by SourceId::index()
    pub facts:        EvidenceBag,        // structured k/v for the renderer
    pub score:        i32,                // filled by smd-policy, 0 from the rule
    pub severity:     Severity,           // ditto
}

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

### Ordering must be total

`Severity` derives `Ord` — declaration order is severity order. `Subject` implements it
by hand over `(variant discriminant, then each field as its raw u32)`, because deriving
would demand `Ord` on `SourceId` and reintroduce the identity/precedence confusion that
split exists to remove. The result is an arbitrary but stable total order over identity,
and it means nothing else. Findings sort by:

| # | Key | Direction |
| --- | --- | --- |
| 1 | `severity` | descending |
| 2 | `score` | descending |
| 3 | `rule` | lexicographic |
| 4 | `subject` | ascending |
| 5 | participant vector, each as `(source.index(), path, span.offset)` | lexicographic |
| 6 | canonical evidence hash — blake3 over the `EvidenceBag` sorted by key | ascending |

Keys 1–4 are what a reader wants. Keys 5 and 6 make the order **total**: both are
functions of the finding's entire content, so two findings that compare equal are the
same finding. A partial key over values drawn from `HashMap` iteration is consistent
within a run and different between runs, which makes the determinism gate (§ `13`) pass
most of the time and fail at random — the worst behaviour available to a guard.

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
