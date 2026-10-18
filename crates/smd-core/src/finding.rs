//! Findings — what a rule produces and what everything downstream consumes.

use smallvec::SmallVec;

use crate::evidence::EvidenceBag;
use crate::facts::EventOrdinal;
use crate::files::Site;
use crate::gap::ResolutionGap;
use crate::ids::{LangId, PathId, SourceId, SymbolId, SymbolKey};

mod certainty;

pub use certainty::{ActivationCertainty, Certainty, Completeness, SemanticCertainty};

/// A stable, namespaced rule id. Never reused. Appears in reports, config, suppressions.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct RuleId(pub &'static str);

impl RuleId {
    /// The placeholder id [`Finding::unscored`] carries. Any real finding overwrites it.
    pub const UNSET: RuleId = RuleId("");

    /// The id as text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// What a finding is *about*.
///
/// Covers every shape a v1 rule reports on. Without the [`LocKey`](Subject::LocKey),
/// [`Event`](Subject::Event), and [`Directory`](Subject::Directory) variants a loc
/// collision, a duplicate event id, and a `replace_path` shadow all collapse onto
/// [`Path`](Subject::Path), and three distinct findings about one file become
/// indistinguishable to sorting, to suppression by subject, and to policy's same-subject
/// predicates. (REVIEW-2026-08 § G5.)
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum Subject {
    /// One file path.
    Path(PathId),
    /// One named game object.
    Symbol(SymbolKey),
    /// One localisation key in one language.
    LocKey {
        /// Which language.
        lang: LangId,
        /// The key.
        key: SymbolId,
    },
    /// One event id.
    Event {
        /// The namespace part.
        namespace: SymbolId,
        /// The ordinal part, which need not be numeric.
        ordinal: EventOrdinal<SymbolId>,
    },
    /// A `replace_path` declaration: which mod declared it, over what prefix.
    Directory {
        /// The mod that declared it.
        declarer: SourceId,
        /// The prefix it covers.
        prefix: PathId,
    },
    /// One mod.
    Mod(SourceId),
    /// An ordered pair of mods.
    ModPair(SourceId, SourceId),
}

/// What a participant's part in a finding is.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum Role {
    /// Reaches the game.
    Winner,
    /// Exists on disk but does not reach the game.
    Shadowed,
    /// Reaches the game and is byte-identical to the winner.
    Identical,
    /// Combines with the others rather than competing (`Additive`, additive bodies).
    Contributor,
    /// Expected and absent — a declared dependency with no match.
    Missing,
}

impl Role {
    /// The `snake_case` tag a `weights.toml` modifier compares against.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Role::Winner => "winner",
            Role::Shadowed => "shadowed",
            Role::Identical => "identical",
            Role::Contributor => "contributor",
            Role::Missing => "missing",
        }
    }
}

/// One mod's part in one finding, with the bytes that prove it.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Participant {
    /// Where to look.
    pub site: Site,
    /// What part it plays.
    pub role: Role,
}

/// Severity band. Declaration order is severity order, so the derived `Ord` is the
/// contract.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum Severity {
    /// Worth knowing, not worth acting on.
    #[default]
    Info,
    /// Minor.
    Low,
    /// Worth a look.
    Medium,
    /// Something is lost.
    High,
    /// Something is lost and the user will notice.
    Critical,
}

/// One thing a rule noticed, with the evidence that produced it.
///
/// Never `#[non_exhaustive]`: that attribute forbids functional-update syntax from another
/// crate and would break `..Finding::unscored()`, the idiom that keeps every rule from
/// re-listing `score` and `severity` and the thing that actually absorbs a new field.
/// The anti-ripple mechanism for shared structs is the named constructor plus FRU.
/// (ADR-0002 § 3.)
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Finding {
    /// Which rule emitted it.
    pub rule: RuleId,
    /// What it is about.
    pub subject: Subject,
    /// Ascending by rank, ties by `SourceId::index()`.
    pub participants: Vec<Participant>,
    /// Structured k/v for the renderer. Every key must be declared by the rule's
    /// `evidence()`.
    pub facts: EvidenceBag,
    /// The meet of rule and inputs; never raised. Defaults to the least certain value.
    pub certainty: Certainty,
    /// Copied from the `Resolution`.
    pub gaps: SmallVec<[ResolutionGap; 1]>,
    /// Filled by `smd-policy`; **0 from the rule**.
    pub score: i32,
    /// Filled by `smd-policy`; **`Info` from the rule**.
    pub severity: Severity,
}

impl Finding {
    /// The functional-update base for every rule-emitted finding.
    ///
    /// ```
    /// # use smd_core::{EvidenceBag, Finding, PathId, RuleId, Subject};
    /// let f = Finding {
    ///     rule: RuleId("core.file_overwrite"),
    ///     subject: Subject::Path(PathId(7)),
    ///     participants: Vec::new(),
    ///     facts: EvidenceBag::new(),
    ///     ..Finding::unscored()
    /// };
    /// assert_eq!(f.score, 0);
    /// ```
    ///
    /// `score` and `severity` are the only fields a caller is meant to keep. **The policy
    /// layer is the only code allowed to write those two**, which is what lets severity be
    /// retuned from a TOML file without recompiling or re-testing detection logic; a debug
    /// assertion in the engine catches a rule that wrote them. Every other field here is a
    /// placeholder the functional-update expression overwrites.
    #[must_use]
    pub fn unscored() -> Finding {
        Finding {
            rule: RuleId::UNSET,
            subject: Subject::Path(PathId(0)),
            participants: Vec::new(),
            facts: EvidenceBag::new(),
            certainty: Certainty::default(),
            gaps: SmallVec::new(),
            score: 0,
            severity: Severity::Info,
        }
    }

    /// The four detection fields, with `score` and `severity` left at their unscored
    /// values. Equivalent to the functional-update form and easier to call.
    #[must_use]
    pub fn detected(
        rule: RuleId,
        subject: Subject,
        participants: Vec<Participant>,
        facts: EvidenceBag,
    ) -> Finding {
        Finding { rule, subject, participants, facts, ..Finding::unscored() }
    }

    /// Whether the policy layer has written to this finding yet.
    #[must_use]
    pub fn is_unscored(&self) -> bool {
        self.score == 0 && self.severity == Severity::Info
    }
}

#[cfg(test)]
mod tests;
