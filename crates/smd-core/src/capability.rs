//! Capabilities and feature tags — the two closed vocabularies a rule declares against.
//!
//! Both are **deliberately central lists**, and ADR-0002 § 2 says so out loud: a
//! capability set nobody can enumerate cannot be gated off, and a rule that names a game
//! makes the second game a rewrite. They are small, they change under contract review,
//! and § 7's lints exist to stop them acquiring a fourth sibling.

use bitflags::bitflags;

bitflags! {
    /// Which index structures a rule needs built.
    ///
    /// The engine unions the capabilities of enabled rules and builds only those
    /// structures, so removing a feature removes its cost automatically with no
    /// conditional anywhere in the pipeline.
    ///
    /// **`u32`, not `u16`**: the width is part of the serialised contract and running out
    /// of bits later costs a [`CONTRACT_VERSION`](crate::CONTRACT_VERSION) bump. Sixteen
    /// spare bits are cheaper. (REVIEW-2026-08 § G15.)
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Capabilities: u32 {
        /// The per-path overlay table.
        const OVERLAY = 1 << 0;
        /// The definition symbol table.
        const DEFINITIONS = 1 << 1;
        /// The reference table. **Off in v1** — untyped extraction matches `owner`,
        /// `root`, `value`, and `yes` as readily as a trait name, so impact analysis
        /// would be noise and every score reading it would inflate uniformly.
        const REFERENCES = 1 << 2;
        /// The localisation key table.
        const LOCALISATION = 1 << 3;
        /// The event id table.
        const EVENTS = 1 << 4;
        /// Parsed descriptors and dependency resolution.
        const DESCRIPTORS = 1 << 5;
        /// The rule wants to read raw file content.
        const FILE_BYTES = 1 << 6;
    }
}

impl Capabilities {
    /// The capabilities a v1 build may satisfy. [`REFERENCES`](Capabilities::REFERENCES)
    /// is excluded; a rule declaring it does not run.
    pub const V1_AVAILABLE: Capabilities = Capabilities::OVERLAY
        .union(Capabilities::DEFINITIONS)
        .union(Capabilities::LOCALISATION)
        .union(Capabilities::EVENTS)
        .union(Capabilities::DESCRIPTORS)
        .union(Capabilities::FILE_BYTES);

    /// Whether a v1 build can run a rule needing `self`.
    #[must_use]
    pub const fn available_in_v1(self) -> bool {
        Capabilities::V1_AVAILABLE.contains(self)
    }
}

/// Which semantic features a game profile must declare for a rule to be applicable.
///
/// A closed vocabulary, and the replacement for `applies_to() -> &[&str]`. A game id in
/// `smd-rules` would fail the design's own lint gate, and it is the wrong predicate
/// anyway: a rule about duplicate event ids applies to any game whose profile declares it
/// has events, which is a property of the profile and not of the game's name.
/// (REVIEW-2026-08 § G8, ADR-0002 § 5.)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum FeatureTag {
    /// The game has named top-level definitions.
    Definitions,
    /// The game has localisation files.
    Localisation,
    /// The game has events with namespaced ids.
    Events,
    /// The game has `namespace = …` declarations.
    Namespaces,
    /// The game has a defines mechanism.
    Defines,
    /// The game supports `replace_path` directory shadowing.
    ReplacePath,
    /// Descriptors can declare dependencies.
    Dependencies,
    /// Descriptors carry a supported-version field.
    VersionMetadata,
}

impl FeatureTag {
    /// The `snake_case` name a profile TOML declares it under.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            FeatureTag::Definitions => "definitions",
            FeatureTag::Localisation => "localisation",
            FeatureTag::Events => "events",
            FeatureTag::Namespaces => "namespaces",
            FeatureTag::Defines => "defines",
            FeatureTag::ReplacePath => "replace_path",
            FeatureTag::Dependencies => "dependencies",
            FeatureTag::VersionMetadata => "version_metadata",
        }
    }

    /// Parse a profile-declared tag. `None` is a load error with a precise message, never
    /// a silently ignored line.
    #[must_use]
    pub fn from_tag(s: &str) -> Option<FeatureTag> {
        FeatureTag::ALL.iter().copied().find(|t| t.as_tag() == s)
    }

    /// Every tag, in declaration order. The closed vocabulary, enumerable.
    pub const ALL: [FeatureTag; 8] = [
        FeatureTag::Definitions,
        FeatureTag::Localisation,
        FeatureTag::Events,
        FeatureTag::Namespaces,
        FeatureTag::Defines,
        FeatureTag::ReplacePath,
        FeatureTag::Dependencies,
        FeatureTag::VersionMetadata,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_are_a_u32_with_seven_bits_used() {
        assert_eq!(std::mem::size_of::<Capabilities>(), 4);
        assert_eq!(Capabilities::all().bits(), 0b111_1111);
        assert_eq!(Capabilities::FILE_BYTES.bits(), 1 << 6);
    }

    #[test]
    fn references_is_the_one_capability_v1_cannot_satisfy() {
        assert!(!Capabilities::REFERENCES.available_in_v1());
        assert!(Capabilities::OVERLAY.union(Capabilities::DEFINITIONS).available_in_v1());
        assert!(!Capabilities::OVERLAY.union(Capabilities::REFERENCES).available_in_v1());
    }

    #[test]
    fn feature_tags_round_trip_through_their_profile_names() {
        for tag in FeatureTag::ALL {
            assert_eq!(FeatureTag::from_tag(tag.as_tag()), Some(tag));
        }
        assert_eq!(FeatureTag::from_tag("no_such_feature"), None);
        let mut names: Vec<&str> = FeatureTag::ALL.iter().map(|t| t.as_tag()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), FeatureTag::ALL.len());
    }
}
