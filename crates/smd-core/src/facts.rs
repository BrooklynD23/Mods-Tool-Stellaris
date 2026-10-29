//! Facts — one atomic observation extracted from one file.
//!
//! Parsers emit a flat stream of them; the index folds them. Parsers hold no state beyond
//! the current file, which is what makes the parse stage trivially parallel.

use crate::files::{Site, Span};
use crate::ids::{DomainId, LangId, SymbolId};

mod class;
mod merge;

pub use class::ExtractionClass;
pub use merge::{MergeMode, SamePath};

/// Parser output. **File-local and string-based**: the only form that may be serialised,
/// and therefore the only form the parse cache stores.
///
/// The cache is keyed by file content, so the same bytes vendored by five mods is one
/// entry and five hits. That only holds if the cached value says nothing about *which*
/// mod it came from. `source` and `path` are supplied at rehydration, by the caller that
/// already knows them. (REVIEW-2026-08 § G1.)
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RawFact {
    /// Raw-file byte offsets within the file this fact came from.
    pub span: Span,
    /// The observation, carrying the symbol's *text*.
    pub payload: FactPayload<Box<str>>,
}

/// Index input. Interned and sited.
///
/// # Contract clause
///
/// **Interned ids are scan-local and must never be serialised, cached, or cross the API
/// boundary** (Invariant C1). A [`SymbolId`] is a position in one scan's interner; a
/// cache written by one scan and read by the next would attribute facts to whichever mod
/// interned that id first, making warm scans — the normal path — silently wrong.
///
/// This type therefore deliberately implements neither `serde::Serialize` nor
/// `serde::Deserialize`, under any feature, and a unit test asserts that it does not.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Fact {
    /// Which mod, which file, which bytes.
    pub site: Site,
    /// The observation, with its symbol interned.
    pub payload: FactPayload<SymbolId>,
}

/// What was observed. Generic over the symbol representation: `Box<str>` in [`RawFact`],
/// [`SymbolId`] in [`Fact`].
///
/// # Append-only
///
/// **Variants are append-only.** A new variant is declared last and takes the next free
/// discriminant. Inserting, reordering, or removing one requires a
/// [`CONTRACT_VERSION`](crate::CONTRACT_VERSION) bump, because rkyv and postcard encode a
/// variant by position rather than by name: an inserted variant does not fail to decode,
/// it decodes every fact already in the cache as the wrong kind.
/// [`FactPayload::discriminant`] and its snapshot test pin the assignment.
#[repr(u8)]
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum FactPayload<S> {
    /// A top-level named game object: a trait, civic, building, tech…
    ///
    /// Carries no `MergeMode`: merge behaviour is a property of the *path*, classified
    /// once by the profile. A definition cannot merge differently from the file it lives
    /// in, and a per-definition copy only creates something that can disagree.
    Definition {
        /// Which kind of object.
        domain: DomainId,
        /// Its name.
        symbol: S,
        /// blake3 over the normalised body, truncated — the identical-copy discount.
        body_hash: [u8; 16],
    } = 0,
    /// A localisation entry.
    LocKey {
        /// Which language file it came from.
        lang: LangId,
        /// The key.
        key: S,
        /// The optional `:0` suffix. `None` when the entry declared none — an absent
        /// ordinal is not ordinal zero. (REVIEW-2026-08 § G16.)
        version_ordinal: Option<u16>,
        /// blake3 over the value text, truncated.
        value_hash: [u8; 16],
    } = 1,
    /// A use of a symbol. **No producer in v1** — reserved.
    ///
    /// `Capabilities::REFERENCES` is off by default and a rule declaring it does not run.
    /// The variant and its discriminant stay in the contract so that turning extraction on
    /// later is a parser change, not a cache-invalidating contract bump. Untyped
    /// extraction is worse than none: with no symbol type map, `owner`, `root`, `value`,
    /// and `yes` all read as references.
    Reference {
        /// Which kind of object is being referred to.
        domain: DomainId,
        /// Its name.
        symbol: S,
    } = 2,
    /// `namespace = my_mod`
    Namespace {
        /// The declared namespace.
        symbol: S,
    } = 3,
    /// `id = my_mod.100`
    EventId {
        /// The namespace part.
        namespace: S,
        /// The ordinal part, which need not be numeric.
        ordinal: EventOrdinal<S>,
    } = 4,
    /// A single key inside `common/defines/`.
    DefineKey {
        /// The enclosing group, e.g. `NGameplay`.
        group: S,
        /// The key.
        key: S,
        /// blake3 over the value text, truncated.
        value_hash: [u8; 16],
    } = 5,
    /// Emitted immediately after a `Definition` on a path whose classification names
    /// `additive_children`.
    ///
    /// `residual_hash` covers the body with every named additive child removed;
    /// `additive_count` is how many such children were removed. Equal residuals mean the
    /// participants differ only in children that accumulate; unequal residuals mean the
    /// winner's non-additive children replace the losers'. (Invariant A1, doc 03.)
    AdditiveSplit {
        /// Which kind of object.
        domain: DomainId,
        /// Its name.
        symbol: S,
        /// blake3 over the body with every additive child removed, truncated.
        residual_hash: [u8; 16],
        /// How many additive children were removed.
        additive_count: u16,
    } = 6,
}

impl<S> FactPayload<S> {
    /// The wire discriminant. **Snapshot-pinned**; see the append-only rule above.
    #[must_use]
    pub const fn discriminant(&self) -> u8 {
        match self {
            FactPayload::Definition { .. } => 0,
            FactPayload::LocKey { .. } => 1,
            FactPayload::Reference { .. } => 2,
            FactPayload::Namespace { .. } => 3,
            FactPayload::EventId { .. } => 4,
            FactPayload::DefineKey { .. } => 5,
            FactPayload::AdditiveSplit { .. } => 6,
        }
    }
}

/// Event ids need not be numeric: `id = my_mod.setup` is legal and shipped.
///
/// Coercing a non-numeric ordinal to a number either drops the fact or collides unrelated
/// events, and `pdx.event_id_dup` carries the highest base weight in the catalogue — it
/// is the last rule that should be fed guesses. (REVIEW-2026-08 § G16.)
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum EventOrdinal<S> {
    /// `my_mod.100`
    Num(u32),
    /// `my_mod.setup`
    Name(S),
}

#[cfg(test)]
mod tests;
