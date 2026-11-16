//! Interning and identity.
//!
//! Raw `String`s are the memory-and-speed killer at this scale: a 300-mod playset
//! produces ~400k paths and ~1.3M symbols, since definition names and localisation keys
//! share one interner. Everything addressable is interned to a `u32` at ingest time and
//! resolved back to text only at render time.

/// Identity only: *which* mod. Assigned in discovery order.
///
/// **This type has no `Ord`, deliberately.** Precedence lives in a separate field
/// ([`SourceMeta::rank`](crate::SourceMeta::rank)), so `a < b` does not compile and every
/// precedence comparison has to route through the source table. Identity and precedence
/// are two different facts about a mod; fusing them means a reorder renumbers every id
/// and invalidates every structure keyed or sorted by one. (REVIEW-2026-08 § G2.)
///
/// > **Invariant P1:** `SourceId`s are dense from 0 in discovery order and do not change
/// > within a scan. Vanilla is always `SourceId(0)`, with `rank == [0]`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct SourceId(pub u32);

impl SourceId {
    /// Vanilla, by Invariant P1.
    pub const VANILLA: SourceId = SourceId(0);

    /// Discovery index, for deterministic ordering only. Carries no precedence.
    ///
    /// Sort keys that need a stable tiebreaker over sources use this; it is an arbitrary
    /// but stable total order over identity, and it means nothing else.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// A normalised, mod-root-relative path. Scan-local (Invariant C1).
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct PathId(pub u32);

/// A definition name or a localisation key; both share one interner. Scan-local.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct SymbolId(pub u32);

/// A profile-declared symbol domain (`trait`, `civic`, `building`, …).
///
/// Profile-derived and stable for a given `profile_hash`, which is part of the cache key,
/// so unlike [`SymbolId`] it may be cached.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DomainId(pub u16);

/// A localisation language. Profile-derived, and cacheable for the same reason as
/// [`DomainId`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LangId(pub u8);

/// A symbol qualified by its domain — the key of the symbol table.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct SymbolKey {
    /// Which kind of game object this name denotes.
    pub domain: DomainId,
    /// The interned name.
    pub symbol: SymbolId,
}

/// Stable across runs and machines.
///
/// The workshop id when present, else a hash of the descriptor's identifying fields.
/// Used for cache keys, suppressions, and saved config — and it is what a [`SourceId`]
/// turns into on the way out of the engine (Invariant C1).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModKey(pub Box<str>);

impl ModKey {
    /// Borrow the key as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ModKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SourceId` must not gain `Ord` by accident. If it ever does, this stops
    /// compiling — which is the point: precedence must route through `SourceMeta::rank`.
    #[test]
    fn source_id_is_identity_only() {
        assert_eq!(SourceId::VANILLA.index(), 0);
        assert_eq!(SourceId(7).index(), 7);
        // The compile-time half: `SourceId: Ord` must never become true.
        assert!(!implements!(SourceId: Ord));
        assert!(!implements!(SourceId: PartialOrd));
        // Its neighbours are ordered, so the probe is measuring something.
        assert!(implements!(PathId: Ord));
        assert!(implements!(SymbolId: Ord));
    }

    #[test]
    fn mod_key_displays_as_its_text() {
        let k = ModKey("281990_12345".into());
        assert_eq!(k.to_string(), "281990_12345");
        assert_eq!(k.as_str(), "281990_12345");
    }
}
