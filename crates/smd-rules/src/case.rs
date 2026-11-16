//! Rule cases and the harness that runs them (doc 13 § Rule testing).

use smd_core::{Completeness, ResolutionGap, Role, Subject};
use smd_index::PlaysetBuilder;

use crate::rule::{CancellationToken, Rule, RuleCtx};
use crate::runner::run_one;

/// The five mandatory kinds.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CaseKind {
    /// The finding fires, with the expected subject and roles.
    Positive,
    /// It does not fire although the shape is close.
    NearMiss,
    /// With no order source the rule reports without a winner rather than guessing one.
    OrderUnknown,
    /// The 1 -> 0 case: the subject existed in one source and now exists nowhere live.
    Deletion,
    /// An input in scope is unparsed: no stated winner over it.
    Incomplete,
}

impl CaseKind {
    /// Every kind.
    pub const ALL: [CaseKind; 5] = [
        CaseKind::Positive,
        CaseKind::NearMiss,
        CaseKind::OrderUnknown,
        CaseKind::Deletion,
        CaseKind::Incomplete,
    ];

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// What the winner of a fired finding must look like.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum WinnerExpect {
    /// Not asserted.
    Any,
    /// The `Winner` participant is the mod with this display name.
    Mod(&'static str),
    /// No participant has `Role::Winner`.
    NoWinner,
}

/// What a case asserts, checked against the finding itself and not against a count.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Expectation {
    /// No finding at all.
    None,
    /// A finding on the path, with this winner and (if given) carrying this gap.
    Fires {
        /// Normalised path text of the subject.
        path: &'static str,
        /// The winner assertion.
        winner: WinnerExpect,
        /// A gap the finding must carry.
        gap: Option<ResolutionGap>,
    },
}

impl Expectation {
    /// Fires on `path` with the mod named `winner` winning.
    #[must_use]
    pub const fn fires_on_path(path: &'static str, winner: &'static str) -> Expectation {
        Expectation::Fires { path, winner: WinnerExpect::Mod(winner), gap: None }
    }

    /// Fires on `path` and names no winner, carrying `gap`.
    #[must_use]
    pub const fn fires_without_winner(path: &'static str, gap: ResolutionGap) -> Expectation {
        Expectation::Fires { path, winner: WinnerExpect::NoWinner, gap: Some(gap) }
    }
}

/// One fixture and what it must produce.
#[derive(Copy, Clone)]
pub struct RuleCase {
    /// Human-readable.
    pub name: &'static str,
    /// Which of the five kinds it is.
    pub kind: CaseKind,
    /// Builds the playset.
    pub build: fn(&mut PlaysetBuilder),
    /// What must come out.
    pub expect: Expectation,
}

impl RuleCase {
    /// Compile-time half of the coverage gate: panics (so, fails the build when used in a
    /// `const _`) unless every [`CaseKind`] appears.
    pub const fn assert_covers_all_kinds(cases: &[RuleCase]) {
        let mut seen = 0u8;
        let mut i = 0;
        while i < cases.len() {
            seen |= cases[i].kind.bit();
            i += 1;
        }
        let mut k = 0;
        while k < CaseKind::ALL.len() {
            assert!(seen & CaseKind::ALL[k].bit() != 0, "a rule's cases must cover every CaseKind");
            k += 1;
        }
    }
}

/// Build the case's playset, run `rule`, and check the expectation.
///
/// # Errors
/// A description of the first mismatch.
pub fn run_case(rule: &dyn Rule, case: &RuleCase) -> Result<(), String> {
    let mut b = PlaysetBuilder::new();
    (case.build)(&mut b);
    let index = b.build();
    let cancel = CancellationToken::new();
    let findings = run_one(rule, &RuleCtx { index: &index, cancel: &cancel })
        .map_err(|p| format!("{}: rule panicked: {}", case.name, p.message))?;
    let fail = |why: String| Err(format!("{} ({:?}): {why}", case.name, case.kind));
    let Expectation::Fires { path, winner, gap } = case.expect else {
        return if findings.is_empty() {
            Ok(())
        } else {
            fail(format!("expected none, got {}", findings.len()))
        };
    };
    let Some(f) = findings
        .iter()
        .find(|f| matches!(f.subject, Subject::Path(p) if index.path_text(p) == path))
    else {
        return fail(format!("no finding on {path}"));
    };
    let resolved = index.resolve_finding(f);
    let winners: Vec<&str> = resolved
        .participants
        .iter()
        .filter(|p| p.role == Role::Winner)
        .map(|p| &*p.mod_name)
        .collect();
    match winner {
        WinnerExpect::Any => {}
        WinnerExpect::NoWinner if winners.is_empty() => {}
        WinnerExpect::Mod(n) if winners == [n] => {}
        other => return fail(format!("winner {other:?} vs {winners:?}")),
    }
    if case.kind == CaseKind::Positive && matches!(winner, WinnerExpect::Mod(_)) {
        if let Some(p) = resolved
            .participants
            .iter()
            .find(|p| p.role != Role::Winner && p.role != Role::Shadowed)
        {
            return fail(format!("{} is {:?}, expected Shadowed", p.mod_name, p.role));
        }
    }
    // Doc 13: an Incomplete case must not claim full completeness. The engine-level check
    // that certainty is derived from the inputs is deferred; a rule leaves the worst value.
    if case.kind == CaseKind::Incomplete && f.certainty.completeness == Completeness::Complete {
        return fail("an Incomplete case reported Complete".into());
    }
    match gap {
        Some(g) if !f.gaps.contains(&g) => fail(format!("missing gap {g:?}, has {:?}", f.gaps)),
        _ => Ok(()),
    }
}
