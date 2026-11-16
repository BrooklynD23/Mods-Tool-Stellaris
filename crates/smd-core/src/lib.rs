//! `smd-core` — the data contracts every other crate shares.
//!
//! **Owns:** `ModKey`, `SourceId`, `PathId`, `SymbolId`, `Fact`, `Finding`,
//! `ResolvedFinding`, `Severity`, `Diagnostic`, `Span`, `Capabilities`, `FeatureTag`, and
//! the `LoadOrderSource` / `DescriptorReader` / `VersionReader` trait *declarations*.
//!
//! **Must never:** depend on anything, or know what a mod directory looks like.
//!
//! These types are the only thing crates share; everything else is an implementation
//! detail. Changing a type here is the one edit that ripples, so the surface is
//! deliberately small and deliberately open to extension.
//!
//! # Why the traits live here
//!
//! `LoadOrderSource`, `DescriptorReader`, and `VersionReader` are declared in this crate
//! rather than in `smd-ingest`. A trait that *returns implementations* drags the
//! implementor's crate down onto the declarer: had `GameProfile` returned
//! `Vec<Box<dyn LoadOrderSource>>`, `smd-profiles` would need `smd-ingest`, which already
//! needs `smd-profiles` for `[locations]`. Cargo rejects that cycle outright — it is not
//! a lint failure to argue about. `GameProfile` therefore returns [`SourceSpec`] values —
//! a name plus its configuration — and `smd-ingest` owns the name→impl registry.
//! (ADR-0002 § 1, REVIEW-2026-08 § G40.)
//!
//! # Compatibility rules for this crate
//!
//! 1. **[`FactPayload`] variants are append-only.** A new variant is declared last and
//!    takes the next free discriminant. Inserting, reordering, or removing one requires a
//!    [`CONTRACT_VERSION`] bump. The enum is `#[repr(u8)]` with explicit discriminants,
//!    and [`FactPayload::discriminant`] plus its snapshot test pin the assignment — rkyv
//!    and postcard encode a variant by position, not by name, so an inserted variant does
//!    not fail to decode: it decodes every fact already in the cache as the wrong kind.
//! 2. Adding a variant to any other `#[non_exhaustive]` enum is allowed; none are
//!    serialised.
//! 3. Adding a struct field is allowed only if it has a `Default`, so that the named
//!    constructor plus functional-update idiom (`..Finding::unscored()`) absorbs it.
//! 4. [`Capabilities`] is a `u32`, of which seven bits are used. A bitflags set that must
//!    never be narrowed costs nothing to declare wide now and a contract bump later.
//! 5. Removing or repurposing anything requires bumping [`CONTRACT_VERSION`], which is
//!    part of every cache key — so an incompatible change safely invalidates caches
//!    rather than silently reading stale data with the wrong meaning.
//!
//! # Invariant C1 — interned ids are scan-local
//!
//! [`SourceId`], [`PathId`], and [`SymbolId`] are **never** serialised, never written to
//! the parse cache, and never cross the API boundary. [`DomainId`] and [`LangId`] are
//! profile-derived and stable for a given `profile_hash`, which is part of the cache key,
//! so they may be cached. What leaves the engine carries text ([`ResolvedFinding`]) or a
//! stable key ([`ModKey`]).

#![forbid(unsafe_code)]

#[cfg(test)]
#[macro_use]
mod testutil;

mod capability;
mod descriptor;
mod diagnostic;
mod evidence;
mod facts;
mod files;
mod finding;
mod hash;
mod ids;
mod locations;
mod placeholder;
mod resolution;
mod resolved;
mod sortkey;
mod sources;
mod spec;
mod version;

pub use capability::{Capabilities, FeatureTag};
pub use descriptor::{Descriptor, DescriptorSource};
pub use diagnostic::{DiagSeverity, Diagnostic};
pub use evidence::{EvidenceBag, EvidenceType, EvidenceValue};
pub use facts::{EventOrdinal, Fact, FactPayload, MergeMode, RawFact};
pub use files::{Blob, FileEntry, FileKind, Site, Span};
pub use finding::{Finding, Participant, Role, RuleId, Severity, Subject};
pub use hash::{ContentHash, Hasher128};
pub use ids::{DomainId, LangId, ModKey, PathId, SourceId, SymbolId, SymbolKey};
pub use locations::ResolvedLocations;
pub use placeholder::{DERIVED_PLACEHOLDERS, is_derived_placeholder};
pub use resolution::{Agreement, Confidence, Mechanism, Outcome, Resolution, ShadowCause};
pub use resolved::{ResolvedEvidence, ResolvedFinding, ResolvedParticipant};
pub use sortkey::{FindingSortKey, canonical_evidence_hash, sort_findings};
pub use sources::{ResolvedDependency, SourceMeta, SourceRole, SourceTable};
pub use spec::{
    DescriptorReader, LoadOrderSource, OrderError, Playset, PlaysetEntry, SourceSpec, VersionReader,
};
pub use version::{GameVersion, Match, VersionComponent, VersionPattern};

/// The version of every type in this crate, considered as one artefact.
///
/// It is part of every cache key. Bump it for anything the compatibility rules above
/// call a breaking change; an incompatible change then invalidates caches rather than
/// silently reading stale bytes with the wrong meaning.
pub const CONTRACT_VERSION: u32 = 1;
