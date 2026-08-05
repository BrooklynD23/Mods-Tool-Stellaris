# 03 — Data contracts (`smd-core`)

These types are the only thing crates share. Everything else is an implementation
detail. Changing a type here is the one edit that ripples, so this file is deliberately
small and the types are deliberately open to extension.

## Interning and identity

Raw `String`s are the memory-and-speed killer at this scale (a 300-mod playset produces
~400k paths and ~500k symbols). Everything addressable is interned to a `u32` at ingest
time and resolved back to text only at render time.

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct SourceId(pub u32);
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct PathId(pub u32);
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct SymbolId(pub u32);
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct DomainId(pub u16);
#[derive(Copy, Clone, PartialEq, Eq, Hash)]                  pub struct LangId(pub u8);

/// Stable across runs and machines. Workshop id when present, else a hash of the
/// descriptor's identifying fields. Used for cache keys, suppressions, and saved config.
pub struct ModKey(pub Box<str>);
```

`SourceId` doubles as **load-order precedence**: it is assigned in load order, so
`a < b` means `b` wins. This removes an entire class of bug — there is no separate
`load_order_index` field that can disagree with the ordering.

> **Invariant P1:** `SourceId` values are assigned densely from 0 in ascending load
> order. Vanilla game files are always `SourceId(0)`.

## Location

```rust
pub struct Span { pub offset: u32, pub len: u32, pub line: u32 }

pub struct Site { pub source: SourceId, pub path: PathId, pub span: Span }
```

`Site` is what makes a finding verifiable: the UI can open the exact bytes that produced
it. A rule that emits a finding without at least one `Site` fails a debug assertion.

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

A `Fact` is one atomic observation extracted from one file. Parsers emit a flat stream
of them; the index folds them. Parsers hold no state beyond the current file, which is
what makes the parse stage trivially parallel.

```rust
pub struct Fact { pub site: Site, pub payload: FactPayload }

#[non_exhaustive]
pub enum FactPayload {
    /// A top-level named game object: a trait, civic, building, tech…
    Definition {
        domain:    DomainId,
        symbol:    SymbolId,
        body_hash: [u8; 16],   // hash of the normalised block — detects identical copies
        merge:     MergeMode,
    },
    /// A localisation entry.
    LocKey { lang: LangId, key: SymbolId, value_hash: [u8; 16] },
    /// A use of a symbol. Powers orphan-reference detection.
    Reference { domain: DomainId, symbol: SymbolId },
    /// `namespace = my_mod`
    Namespace { symbol: SymbolId },
    /// `id = my_mod.100`
    EventId { namespace: SymbolId, ordinal: u32 },
    /// A single key inside `common/defines/`.
    DefineKey { group: SymbolId, key: SymbolId, value_hash: [u8; 16] },
}
```

`#[non_exhaustive]` on every payload enum is the load-bearing detail: a new fact kind
can be added without breaking any downstream `match`, because downstream code is forced
to have a `_ =>` arm from day one.

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
    /// Entries accumulate; duplicates coexist rather than override.
    Additive,
    /// The directory is read as exactly one logical file.
    Single,
}
```

Two mods each shipping `common/traits/00_species_traits.txt` under `FileReplace` is a
**hard** conflict — one mod's entire trait file is discarded. Two mods shipping
*differently named* files in a `MergeByKey` directory is **usually fine** and only
matters where symbol names actually collide. Same raw observation, opposite verdicts.
The distinction comes from the game profile (§ `09`), not from rule code.

## Findings

```rust
pub struct RuleId(pub &'static str);        // "core.symbol_collision"

pub enum Subject {
    Path(PathId),
    Symbol { domain: DomainId, symbol: SymbolId },
    Mod(SourceId),
    ModPair(SourceId, SourceId),
}

pub enum Role { Winner, Shadowed, Identical, Contributor, Missing }

pub struct Participant { pub site: Site, pub role: Role }

pub struct Finding {
    pub rule:         RuleId,
    pub subject:      Subject,
    pub participants: Vec<Participant>,   // ordered by SourceId, i.e. by load order
    pub facts:        EvidenceBag,        // structured k/v for the renderer
    pub score:        i32,                // filled by smd-policy, 0 from the rule
    pub severity:     Severity,           // ditto
}

#[non_exhaustive]
pub enum Severity { Info, Low, Medium, High, Critical }
```

Rules emit findings with `score: 0` and `severity: Info`. **The policy layer is the only
code allowed to write those two fields.** This is what lets severity be retuned from a
TOML file without recompiling or re-testing detection logic.

`EvidenceBag` is a small typed map (`SmallVec<[(&'static str, EvidenceValue); 4]>`)
rather than a formatted string, so the same finding can render as a CLI table row, a
JSON object, and a GUI card without any renderer re-parsing prose.

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

1. Adding an enum variant is allowed (all are `#[non_exhaustive]`).
2. Adding a struct field is allowed only if it has a `Default`.
3. Removing or repurposing anything requires bumping `CONTRACT_VERSION`, which is part
   of every cache key — so an incompatible change safely invalidates caches rather than
   silently reading stale data with the wrong meaning.

```rust
pub const CONTRACT_VERSION: u32 = 1;
```
