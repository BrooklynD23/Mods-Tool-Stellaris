//! `Certainty` — four independent axes, none of them the score.

use crate::resolution::Confidence;

/// How far a finding's claim can be trusted, on four axes that must not be folded into
/// one. Derived `Ord` is **best-first** (lexicographic over the axes in declaration
/// order), which is what sort key 5 uses.
///
/// Computed by `smd-engine` as the meet of what the rule supplies and what the
/// participants' sources, files, and resolution support: a rule can lower it, never raise
/// it. `Default` is the **least** certain value, so a forgotten field under-claims.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Certainty {
    /// Were all inputs the claim depends on fully parsed?
    pub completeness: Completeness,
    /// Are all participants known to be enabled?
    pub activation: ActivationCertainty,
    /// Min over participants; `Exact < Inferred < Fallback`.
    pub order: Confidence,
    /// Is the merge behaviour modelled and verified for this game build?
    pub semantics: SemanticCertainty,
}

/// Were all inputs the claim depends on fully parsed? Ordered best-first.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum Completeness {
    /// Every file in scope is `Complete` or `NotApplicable`.
    Complete,
    /// A file in scope is `Partial`.
    Partial,
    /// A file in scope is `Unparsed`.
    #[default]
    Unparsed,
}

/// Are all participants known to be enabled? Ordered best-first.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum ActivationCertainty {
    /// Every participant is known `Active` from an exact source.
    Certain,
    /// Activation was inferred from a secondary source.
    Inferred,
    /// Some participant's activation is unknown.
    #[default]
    Conditional,
}

/// Is the merge behaviour modelled and verified? Ordered best-first.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum SemanticCertainty {
    /// Verified against the game (the directory's profile `verified` key).
    Verified,
    /// Assumed from prior art, not verified.
    Assumed,
    /// Not modelled.
    #[default]
    Unmodelled,
}

impl Completeness {
    /// The `snake_case` tag.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Completeness::Complete => "complete",
            Completeness::Partial => "partial",
            Completeness::Unparsed => "unparsed",
        }
    }
}

impl ActivationCertainty {
    /// The `snake_case` tag.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            ActivationCertainty::Certain => "certain",
            ActivationCertainty::Inferred => "inferred",
            ActivationCertainty::Conditional => "conditional",
        }
    }
}

impl SemanticCertainty {
    /// The `snake_case` tag.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            SemanticCertainty::Verified => "verified",
            SemanticCertainty::Assumed => "assumed",
            SemanticCertainty::Unmodelled => "unmodelled",
        }
    }
}
