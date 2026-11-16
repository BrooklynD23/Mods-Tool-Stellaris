//! Certainty is computed here, never by a rule (doc 08 § Completeness and certainty).
//!
//! Each axis is the worst value over what the finding's participants support, then met
//! with what the rule supplied. `Certainty` orders best-first, so "worst" is `max`.

use std::collections::BTreeSet;

use smd_core::{
    Activation, ActivationCertainty, Certainty, Completeness, Confidence, Finding, ModKey,
    ParseCompleteness, ResolutionGap, RuleId, SemanticCertainty,
};
use smd_index::ScanIndex;

/// Gaps that mean the merge behaviour is not modelled at all.
fn unmodelled(g: ResolutionGap) -> bool {
    matches!(
        g,
        ResolutionGap::MergeModeUnknown
            | ResolutionGap::SamePathUnknown
            | ResolutionGap::ChildDigestsUnavailable
            | ResolutionGap::IdentityCollision
            | ResolutionGap::FileKeyTie
    )
}

fn completeness(index: &ScanIndex, f: &Finding, incomplete: &BTreeSet<ModKey>) -> Completeness {
    let mut worst = Completeness::Complete;
    for p in &f.participants {
        let key = index.sources().get(p.site.source).map(|m| &m.key);
        if key.is_some_and(|k| incomplete.contains(k)) {
            worst = worst.max(Completeness::Partial);
        }
        let contribs = index.overlay().contributions(p.site.path);
        for c in contribs.iter().filter(|c| c.source == p.site.source) {
            worst = worst.max(match c.completeness {
                ParseCompleteness::Partial => Completeness::Partial,
                ParseCompleteness::Unparsed(_) => Completeness::Unparsed,
                _ => Completeness::Complete,
            });
        }
    }
    if f.gaps.contains(&ResolutionGap::ParseIncomplete) {
        worst = worst.max(Completeness::Unparsed);
    }
    worst
}

fn activation(index: &ScanIndex, f: &Finding) -> ActivationCertainty {
    let mut worst = match index.provenance().activation {
        Confidence::Exact => ActivationCertainty::Certain,
        Confidence::Inferred => ActivationCertainty::Inferred,
        _ => ActivationCertainty::Conditional,
    };
    let any_not_active = f.participants.iter().any(|p| {
        index.sources().get(p.site.source).is_none_or(|m| m.activation != Activation::Active)
    });
    if any_not_active || f.gaps.contains(&ResolutionGap::ActivationUnknown) {
        worst = ActivationCertainty::Conditional;
    }
    worst
}

fn order(index: &ScanIndex, f: &Finding) -> Confidence {
    if f.gaps.contains(&ResolutionGap::OrderUnknown) {
        return Confidence::Fallback;
    }
    index.provenance().order
}

/// `Verified` is earned per `[[dir]]` entry by a `verified` key, and no profile entry
/// carries one yet, so the best a finding can be is `Assumed`.
fn semantics(f: &Finding) -> SemanticCertainty {
    if f.gaps.iter().any(|g| unmodelled(*g)) {
        SemanticCertainty::Unmodelled
    } else {
        SemanticCertainty::Assumed
    }
}

/// The best value on every axis: a ceiling that constrains nothing.
const UNCONSTRAINED: Certainty = Certainty {
    completeness: Completeness::Complete,
    activation: ActivationCertainty::Certain,
    order: Confidence::Exact,
    semantics: SemanticCertainty::Verified,
};

/// The ceiling a rule imposes, stated explicitly.
///
/// `Certainty::default()` is the worst value on every axis, so a rule that leaves the
/// field alone under-claims, and a rule that lowers an axis to its worst value is
/// honoured: the meet is literal. A rule that makes no claim of its own must be named
/// here. `core.file_overwrite` only reports that a path was supplied by several sources,
/// so its ceiling is whatever the participants support. Every rule not listed gets
/// exactly what it emitted.
fn rule_ceiling(rule: RuleId, emitted: Certainty) -> Certainty {
    match rule.0 {
        "core.file_overwrite" => UNCONSTRAINED,
        _ => emitted,
    }
}

fn meet(a: Certainty, b: Certainty) -> Certainty {
    Certainty {
        completeness: a.completeness.max(b.completeness),
        activation: a.activation.max(b.activation),
        order: a.order.max(b.order),
        semantics: a.semantics.max(b.semantics),
    }
}

/// The certainty of `f`: participants' support, met with the rule's own ceiling.
pub(crate) fn certainty_of(
    index: &ScanIndex,
    f: &Finding,
    incomplete: &BTreeSet<ModKey>,
) -> Certainty {
    let supported = Certainty {
        completeness: completeness(index, f, incomplete),
        activation: activation(index, f),
        order: order(index, f),
        semantics: semantics(f),
    };
    meet(supported, rule_ceiling(f.rule, f.certainty))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEST: Certainty = UNCONSTRAINED;

    #[test]
    fn a_rule_that_lowers_an_axis_to_its_worst_value_keeps_it() {
        let lowered = Certainty { semantics: SemanticCertainty::Unmodelled, ..BEST };
        let ceiling = rule_ceiling(RuleId("test.lowering"), lowered);
        assert_eq!(meet(BEST, ceiling).semantics, SemanticCertainty::Unmodelled);
        let lowered = Certainty {
            completeness: Completeness::Unparsed,
            activation: ActivationCertainty::Conditional,
            order: Confidence::Fallback,
            ..BEST
        };
        let got = meet(BEST, rule_ceiling(RuleId("test.lowering"), lowered));
        assert_eq!(
            (got.completeness, got.activation, got.order),
            (Completeness::Unparsed, ActivationCertainty::Conditional, Confidence::Fallback)
        );
    }

    #[test]
    fn a_rule_that_leaves_the_default_under_claims() {
        let got = meet(BEST, rule_ceiling(RuleId("test.silent"), Certainty::default()));
        assert_eq!(got, Certainty::default());
    }

    #[test]
    fn file_overwrite_states_no_ceiling_of_its_own() {
        let got = rule_ceiling(RuleId("core.file_overwrite"), Certainty::default());
        assert_eq!(got, BEST);
    }
}
