//! The trait declarations that break the `GameProfile` → `smd-ingest` cycle.
//!
//! A trait that returns implementations drags the implementor's crate down onto the
//! declarer. Had `GameProfile` returned `Vec<Box<dyn LoadOrderSource>>`, `smd-profiles`
//! would need `smd-ingest`, which already needs `smd-profiles` for `[locations]`. **Cargo
//! rejects that outright — it is a build failure, not a lint failure to argue about.**
//!
//! So the traits are declared here, `GameProfile` returns `&[SourceSpec]` — a name plus
//! its configuration — and `smd-ingest` owns the name→impl registry. The shipped profile
//! already reads `sources = ["launcher_v2_sqlite", …]`; the trait signature is what has to
//! agree with it. (ADR-0002 § 1, REVIEW-2026-08 § G40.)

use std::collections::BTreeMap;

use crate::descriptor::Descriptor;
use crate::diagnostic::Diagnostic;
use crate::files::Site;
use crate::ids::ModKey;
use crate::locations::ResolvedLocations;
use crate::resolution::Confidence;
use crate::version::{GameVersion, VersionPattern};

/// A load-order source **named**, with its configuration — never constructed.
///
/// This is what a profile returns. `smd-ingest` resolves [`name`](SourceSpec::name)
/// against its registry and tries the resulting implementations in declaration order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SourceSpec {
    /// The registry key, e.g. `"launcher_v2_sqlite"`.
    pub name: Box<str>,
    /// Free-form configuration, passed through to the implementation.
    ///
    /// `BTreeMap` so that a profile hash is stable across runs.
    pub config: BTreeMap<Box<str>, Box<str>>,
}

impl SourceSpec {
    /// A spec with no configuration.
    #[must_use]
    pub fn named(name: &str) -> SourceSpec {
        SourceSpec { name: name.into(), config: BTreeMap::new() }
    }
}

/// One entry of a playset, as an order source reports it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlaysetEntry {
    /// Stable identity of the mod.
    pub key: ModKey,
    /// Position within the playset, ascending. Turned into
    /// [`SourceMeta::rank`](crate::SourceMeta::rank) by `smd-ingest`.
    pub position: u32,
    /// Whether the playset has it switched on.
    pub enabled: bool,
}

/// A load order as read from one source.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Playset {
    /// The playset's display name, when the source has one.
    pub name: Box<str>,
    /// Entries, in the order the source gave them.
    pub entries: Vec<PlaysetEntry>,
    /// How far this source's answer is to be trusted.
    pub confidence: Confidence,
}

/// Why an order source could not answer.
///
/// **Failures are loud.** A launcher update that quietly demotes every user to the
/// directory-scan fallback while the UI still looks fine is a bug that would take months
/// to notice; each variant here becomes a named diagnostic in the report.
#[derive(Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum OrderError {
    /// The source's input file does not exist. The ordinary case; try the next source.
    NotPresent,
    /// It exists but could not be read.
    Unreadable {
        /// What went wrong.
        detail: Box<str>,
    },
    /// It was read but its shape is not what this reader expects.
    SchemaUnknown {
        /// Whatever build or version string was recoverable.
        found: Box<str>,
    },
    /// It was read and failed an integrity check. Refusing is the only honest answer; a
    /// corrupt page still yields *some* rows.
    Corrupt {
        /// What the check reported.
        detail: Box<str>,
    },
}

/// Reads a load order. Implementations live in `smd-ingest` and are reached by name.
pub trait LoadOrderSource: Sync {
    /// The registry key this implementation answers to.
    fn id(&self) -> &'static str;

    /// How far this source's answers are to be trusted, before it is even tried.
    fn confidence(&self) -> Confidence;

    /// Try to read a playset. `Err` means "try the next source", loudly.
    fn try_load(&self, locations: &ResolvedLocations) -> Result<Playset, OrderError>;
}

/// Reads a `descriptor.mod` or launcher `*.mod`. Implementations live in `smd-ingest`.
pub trait DescriptorReader: Sync {
    /// The registry key this implementation answers to.
    fn id(&self) -> &'static str;

    /// Parse one descriptor. Never fails outright: what could not be read comes back as
    /// diagnostics alongside whatever fields were recovered.
    fn read(&self, bytes: &[u8], site: Site) -> (Descriptor, Vec<Diagnostic>);
}

/// Reads the installed game version and interprets `supported_version` patterns.
/// Implementations live in `smd-ingest`.
pub trait VersionReader: Sync {
    /// The registry key this implementation answers to.
    fn id(&self) -> &'static str;

    /// The installed version, if it could be established.
    fn installed(&self, locations: &ResolvedLocations) -> Option<GameVersion>;

    /// Parse a descriptor's `supported_version`. `None` for an absent or unparseable
    /// field — which becomes [`Match::Unknown`](crate::Match), never a `No`.
    fn parse_pattern(&self, raw: &str) -> Option<VersionPattern>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DirectoryScan;

    impl LoadOrderSource for DirectoryScan {
        fn id(&self) -> &'static str {
            "directory_scan"
        }
        fn confidence(&self) -> Confidence {
            Confidence::Fallback
        }
        fn try_load(&self, _locations: &ResolvedLocations) -> Result<Playset, OrderError> {
            Err(OrderError::NotPresent)
        }
    }

    /// The point of declaring the traits here: they are object-safe, so `smd-ingest` can
    /// hold a fallback chain of them, and nothing in `smd-profiles` had to be named.
    #[test]
    fn load_order_sources_are_usable_as_trait_objects() {
        let chain: Vec<Box<dyn LoadOrderSource>> = vec![Box::new(DirectoryScan)];
        let locations = ResolvedLocations::new();
        for source in &chain {
            assert_eq!(source.id(), "directory_scan");
            assert_eq!(source.confidence(), Confidence::Fallback);
            assert_eq!(source.try_load(&locations), Err(OrderError::NotPresent));
        }
    }

    #[test]
    fn a_spec_names_an_implementation_rather_than_constructing_one() {
        let spec = SourceSpec::named("launcher_v2_sqlite");
        assert_eq!(&*spec.name, "launcher_v2_sqlite");
        assert!(spec.config.is_empty());
    }
}
