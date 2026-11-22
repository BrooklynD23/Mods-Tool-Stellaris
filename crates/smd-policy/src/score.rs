//! Two-phase scoring, banding and certainty caps (doc 09 § Pipeline position).

use std::collections::{BTreeMap, BTreeSet};

use smd_core::{ActivationCertainty, Completeness, ResolvedEvidence, ResolvedFinding, Severity};

use crate::load::{Modifier, Weights};

/// Scores are clamped into this range before banding.
const SCORE_RANGE: (i32, i32) = (0, 100);

/// The distinct mods taking part in a finding.
fn mods(f: &ResolvedFinding) -> BTreeSet<&str> {
    f.participants.iter().map(|p| p.mod_key.as_str()).collect()
}

/// Findings per mod over the aggregate population: every finding, before suppression and
/// filtering, except those whose activation is `Conditional`.
pub(crate) fn population(findings: &[ResolvedFinding]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for f in findings.iter().filter(|f| f.certainty.activation != ActivationCertainty::Conditional)
    {
        for m in mods(f) {
            *counts.entry(m.to_owned()).or_insert(0) += 1;
        }
    }
    counts
}

fn evidence_is(f: &ResolvedFinding, key: &str, want: &str) -> bool {
    f.facts.iter().any(|(k, v)| {
        *k == key
            && match v {
                ResolvedEvidence::Tag(t) => *t == want,
                ResolvedEvidence::Text(t) => &**t == want,
                ResolvedEvidence::Bool(b) => b.to_string() == want,
                ResolvedEvidence::Int(i) => i.to_string() == want,
                _ => false,
            }
    })
}

fn path_matches(f: &ResolvedFinding, globs: &[String]) -> bool {
    let paths = f.participants.iter().map(|p| p.path.to_lowercase());
    paths
        .chain([f.subject_text.to_lowercase()])
        .any(|p| globs.iter().any(|g| smd_profiles::glob_matches(&g.to_lowercase(), &p)))
}

fn matches(m: &Modifier, f: &ResolvedFinding, counts: &BTreeMap<String, usize>) -> bool {
    let w = &m.when;
    let ev = match (&w.evidence, &w.eq) {
        (Some(k), Some(v)) => evidence_is(f, k, v),
        _ => true,
    };
    let glob = w.path_glob.as_ref().is_none_or(|g| path_matches(f, g.list()));
    let gte = w.participants_gte.is_none_or(|n| f.participants.len() >= n);
    let rule = w.rule.as_deref().is_none_or(|r| r == f.rule.as_str());
    let agg = w
        .same_mod_findings_gte
        .is_none_or(|n| mods(f).iter().any(|m| counts.get(*m).copied().unwrap_or(0) >= n));
    ev && glob && gte && rule && agg
}

/// Base weight plus every matching modifier, phase 1 then phase 2, clamped. A negative
/// modifier applies only to a `Complete` finding (H18).
pub(crate) fn raw_score(w: &Weights, f: &ResolvedFinding, counts: &BTreeMap<String, usize>) -> i32 {
    let complete = f.certainty.completeness == Completeness::Complete;
    let base = w.base.get(f.rule.as_str()).copied().unwrap_or(0);
    let (phase1, phase2): (Vec<&Modifier>, Vec<&Modifier>) =
        w.modifiers.iter().partition(|m| m.when.same_mod_findings_gte.is_none());
    let total = phase1
        .into_iter()
        .chain(phase2)
        .filter(|m| (m.add >= 0 || complete) && matches(m, f, counts))
        .fold(base, |acc, m| acc.saturating_add(m.add));
    total.clamp(SCORE_RANGE.0, SCORE_RANGE.1)
}

/// The band, then clamped by the worst certainty axis. Caps are not modifiers: they cannot
/// be tuned away.
pub(crate) fn severity(w: &Weights, f: &ResolvedFinding, score: i32) -> Severity {
    let c = f.certainty;
    let axes = [
        ("completeness", c.completeness.as_tag()),
        ("activation", c.activation.as_tag()),
        ("order", c.order.as_tag()),
        ("semantics", c.semantics.as_tag()),
    ];
    axes.iter()
        .filter_map(|(a, v)| w.caps.get(&((*a).to_owned(), (*v).to_owned())))
        .fold(w.bands.band(score), |band, cap| band.min(*cap))
}
