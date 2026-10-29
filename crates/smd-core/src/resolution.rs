//! Resolution — what actually happens to the participants in a collision.
//!
//! These types are defined in `docs/architecture/07-index-and-resolver.md` but live here
//! because they are shared contracts: `smd-index` produces them, `smd-rules` reads them,
//! `smd-policy` scores on them, and `smd-report` renders them as tags.

use smallvec::SmallVec;

use crate::gap::ResolutionGap;
use crate::ids::SourceId;
use crate::precedence::Contender;

/// The outcome of resolving one subject across its participants.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Resolution {
    /// What happened. Not every collision has a loser.
    pub outcome: Outcome,
    /// The single live participant, when there is exactly one. A [`Contender`] rather than
    /// a source: a source alone cannot say which of a mod's two trait files won.
    pub winner: Option<Contender>,
    /// Participants that exist on disk but do not reach the game.
    pub discarded: SmallVec<[Contender; 2]>,
    /// How far the bodies agree, over live participants only.
    pub agreement: Agreement,
    /// Which rule decided the outcome.
    pub mechanism: Mechanism,
    /// Why no outcome could be stated, one entry per input the verdict depended on and
    /// could not establish. Empty means the outcome is stated.
    ///
    /// There is no `confidence` here: order and activation confidence are scan-wide
    /// ([`PlaysetProvenance`](crate::PlaysetProvenance)), and what one resolution needs to
    /// say is whether they *mattered*, which this field says exactly.
    pub gaps: SmallVec<[ResolutionGap; 1]>,
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
    /// All live participants reach the game (`Additive`, or `MergeByKeyAdditiveBody` with
    /// equal residuals). Not every additive-body overlap is `Coexist`: unequal residuals
    /// are `PartiallyMerged`.
    Coexist,
    /// Every participant's additive children reach the game; non-additive children come
    /// from the winner only. Nobody is wholly discarded, so nobody is `Shadowed`.
    PartiallyMerged,
    /// The participants combine and none is discarded (`Single`).
    MergedInternally,
    /// Participants exist on disk and none of them is live.
    Deleted,
    /// A gap prevents a stated outcome; [`Resolution::gaps`] says which (for example no
    /// known order, or a path whose [`MergeMode`](crate::MergeMode) is `Unknown`).
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
            Outcome::PartiallyMerged => "partially_merged",
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

/// How far the bodies agree. Computed over live participants only; under
/// `MergeByKeyAdditiveBody` over `residual_hash` rather than `body_hash`.
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

/// Which stage and component decided the outcome — the mechanism axis, separate from
/// certainty.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum Mechanism {
    /// Stage A: a `replace_path` declaration dropped a contributor.
    ReplacePath,
    /// Stage A: the same path shipped by a higher-precedence source replaced the file.
    SamePath,
    /// Stage D: the path-class tier decided (`localisation/replace/**`).
    ReplaceTier,
    /// Stage D: the filename key decided.
    FilenameOrder,
    /// Stage D: load order (`rank`) decided.
    LoadOrder,
    /// Stage C combined the participants without a winner.
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
            Mechanism::ReplacePath => "replace_path",
            Mechanism::SamePath => "same_path",
            Mechanism::ReplaceTier => "replace_tier",
            Mechanism::FilenameOrder => "filename_order",
            Mechanism::LoadOrder => "load_order",
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
    /// The named source shipped the same path (Stage A). Was `FileReplace` in the M0
    /// skeleton.
    SamePath(SourceId),
    /// A higher-precedence definition of the same key overrode it.
    KeyOverride(Contender),
    /// A candidate under a higher path-class tier beat it.
    ReplaceTier(Contender),
}

#[cfg(test)]
mod tests;
