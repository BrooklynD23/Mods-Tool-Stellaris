//! Resolution — what actually happens to the participants in a collision.
//!
//! These types are defined in `docs/architecture/07-index-and-resolver.md` but live here
//! because they are shared contracts: `smd-index` produces them, `smd-rules` reads them,
//! `smd-policy` scores on them, and `smd-report` renders them as tags.

use smallvec::SmallVec;

use crate::ids::SourceId;

/// The outcome of resolving one subject across its participants.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Resolution {
    /// What happened. Not every collision has a loser.
    pub outcome: Outcome,
    /// The single live participant, when there is exactly one.
    pub winner: Option<SourceId>,
    /// Participants that exist on disk but do not reach the game.
    pub discarded: SmallVec<[SourceId; 2]>,
    /// How far the bodies agree, over live participants only.
    pub agreement: Agreement,
    /// Which rule decided the outcome.
    pub mechanism: Mechanism,
    /// How far the load order behind the decision is to be trusted.
    pub confidence: Confidence,
}

/// What actually happens to the participants. Not every collision has a loser.
///
/// Replaces the earlier `Resolution.identical: bool` plus an implicit "someone lost"
/// assumption. Under [`Additive`](crate::MergeMode::Additive) the tool would otherwise
/// state the exact opposite of the truth, at High severity, in bold.
/// (REVIEW-2026-08 § G3.)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum Outcome {
    /// One live participant reaches the game; the rest are discarded.
    SingleWinner,
    /// All live participants reach the game (`Additive`, `MergeByKeyAdditiveBody`).
    Coexist,
    /// The participants combine and none is discarded (`MergeByKey` across distinct keys,
    /// `Single`).
    MergedInternally,
    /// Participants exist on disk and none of them is live.
    Deleted,
    /// No order is known, or the path is [`MergeMode::Unknown`](crate::MergeMode::Unknown).
    #[default]
    Unknown,
}

impl Outcome {
    /// The `snake_case` tag a `weights.toml` modifier compares against.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Outcome::SingleWinner => "single_winner",
            Outcome::Coexist => "coexist",
            Outcome::MergedInternally => "merged_internally",
            Outcome::Deleted => "deleted",
            Outcome::Unknown => "unknown",
        }
    }

    /// Whether this outcome is one in which a participant can be
    /// [`Role::Shadowed`](crate::Role::Shadowed).
    ///
    /// > **Invariant R1:** a `Finding` carrying any `Role::Shadowed` participant must
    /// > have a resolution whose outcome is `SingleWinner` or `Deleted`.
    #[must_use]
    pub const fn permits_shadowed_participants(self) -> bool {
        matches!(self, Outcome::SingleWinner | Outcome::Deleted)
    }
}

/// How far the bodies agree. Computed over live participants only.
///
/// **Deliberately exhaustive — the one shared enum in this crate that is.** Its three
/// states are the complete partition of "do the losers get an outcome they would have
/// wanted", and a fourth would not be a new case so much as evidence the question was
/// wrong. An enum that cannot grow should say so by staying exhaustive, so that adding to
/// it is a compile error everywhere at once — which is what you want when the meaning of
/// every discount in `weights.toml` depends on it. (ADR-0002 § 3.)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Agreement {
    /// Every live participant, vanilla included, hashes equal.
    AllIdentical,
    /// Every discarded **mod** matches the winner. Vanilla's body is excluded, because a
    /// mod that overwrites a vanilla file differs from it by construction — including
    /// vanilla makes the predicate almost always false and retires the discount that
    /// exists for exactly this case. (REVIEW-2026-08 § G10.)
    LosersMatchWinner,
    /// Something is actually lost.
    #[default]
    Differs,
}

impl Agreement {
    /// The `snake_case` tag a `weights.toml` modifier compares against.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Agreement::AllIdentical => "all_identical",
            Agreement::LosersMatchWinner => "losers_match_winner",
            Agreement::Differs => "differs",
        }
    }
}

/// Which rule decided the outcome — the mechanism axis, separate from confidence.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum Mechanism {
    /// Precedence by rank.
    LoadOrder,
    /// A `replace_path` declaration dropped a contributor.
    ReplacePath,
    /// The directory's merge mode combined the participants.
    DirectoryMerge,
    /// No mechanism could be established.
    #[default]
    Undetermined,
}

impl Mechanism {
    /// The `snake_case` tag a `weights.toml` modifier compares against.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Mechanism::LoadOrder => "load_order",
            Mechanism::ReplacePath => "replace_path",
            Mechanism::DirectoryMerge => "directory_merge",
            Mechanism::Undetermined => "undetermined",
        }
    }
}

/// How far the load order is to be trusted; supplied by the order source.
///
/// A separate axis from [`Outcome`] and it must stay separate: "these two mods coexist"
/// and "I could not read your playset" are different statements, and a single `Basis`
/// field collapses them into one that means neither. (REVIEW-2026-08 § G13.)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum Confidence {
    /// Read from an authoritative source with explicit positions.
    Exact,
    /// Derived from a secondary source.
    Inferred,
    /// No order source worked; order-dependent rules are suppressed.
    #[default]
    Fallback,
}

impl Confidence {
    /// The `snake_case` tag a `weights.toml` modifier compares against.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Confidence::Exact => "exact",
            Confidence::Inferred => "inferred",
            Confidence::Fallback => "fallback",
        }
    }
}

/// Why a site is not live even though it exists on disk.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum ShadowCause {
    /// The named source declared a `replace_path` covering it.
    ReplacePath(SourceId),
    /// The named source shipped the same path under `FileReplace`.
    FileReplace(SourceId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invariant_r1_only_permits_shadowed_where_something_is_lost() {
        assert!(Outcome::SingleWinner.permits_shadowed_participants());
        assert!(Outcome::Deleted.permits_shadowed_participants());
        assert!(!Outcome::Coexist.permits_shadowed_participants());
        assert!(!Outcome::MergedInternally.permits_shadowed_participants());
        assert!(!Outcome::Unknown.permits_shadowed_participants());
    }

    #[test]
    fn defaults_are_the_no_claim_variants() {
        assert_eq!(Outcome::default(), Outcome::Unknown);
        assert_eq!(Mechanism::default(), Mechanism::Undetermined);
        assert_eq!(Confidence::default(), Confidence::Fallback);
        assert_eq!(Agreement::default(), Agreement::Differs);
    }

    #[test]
    fn agreement_is_the_deliberate_exhaustive_exception() {
        // Exhaustive without a wildcard arm: this match is the assertion. If `Agreement`
        // ever gains `#[non_exhaustive]` or a fourth variant, this stops compiling —
        // which is exactly the property ADR-0002 § 3 asks for.
        let tag = match Agreement::LosersMatchWinner {
            Agreement::AllIdentical => "all_identical",
            Agreement::LosersMatchWinner => "losers_match_winner",
            Agreement::Differs => "differs",
        };
        assert_eq!(tag, Agreement::LosersMatchWinner.as_tag());
    }
}
