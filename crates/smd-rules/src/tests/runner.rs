use smd_core::{Capabilities, EvidenceType, EvidenceValue, Finding, PathId, RuleId, Subject};
use smd_index::PlaysetBuilder;

use super::seeded;
use crate::{CancellationToken, Rule, RuleCase, RuleCtx, check_finding, registry, run_all};

struct Panicker;

impl Rule for Panicker {
    fn id(&self) -> RuleId {
        RuleId("test.aaa_panics")
    }
    fn needs(&self) -> Capabilities {
        Capabilities::OVERLAY
    }
    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[]
    }
    fn cases(&self) -> &[RuleCase] {
        &[]
    }
    fn run(&self, _: &RuleCtx<'_>) -> Vec<Finding> {
        panic!("boom from a rule")
    }
}

#[test]
fn runner_isolates_panicking_rule() {
    let mut b = PlaysetBuilder::new();
    seeded(&mut b, 3);
    let ix = b.build();
    let cancel = CancellationToken::new();
    let mut rules = registry();
    rules.push(&Panicker);
    let out = run_all(&rules, &RuleCtx { index: &ix, cancel: &cancel });
    assert_eq!(out.findings.len(), 3, "core.file_overwrite still ran");
    assert_eq!(out.diagnostics.len(), 1);
    assert_eq!(out.diagnostics[0].code, crate::RULE_PANICKED);
    assert!(out.diagnostics[0].detail.contains("test.aaa_panics"));
    assert!(out.diagnostics[0].detail.contains("boom from a rule"));
}

#[test]
fn runner_runs_rules_sorted_by_id() {
    let mut b = PlaysetBuilder::new();
    seeded(&mut b, 1);
    let ix = b.build();
    let cancel = CancellationToken::new();
    let out = run_all(&[&Panicker, registry()[0]], &RuleCtx { index: &ix, cancel: &cancel });
    // The panicker sorts first but its diagnostic does not disturb the findings.
    assert_eq!((out.findings.len(), out.diagnostics.len()), (1, 1));
}

#[test]
fn contract_checks_catch_undeclared_keys_and_scoring() {
    let rule = &crate::rules::FileOverwrite;
    let ix = PlaysetBuilder::new().build();
    let good = Finding {
        rule: rule.id(),
        subject: Subject::Path(PathId(1)),
        participants: vec![smd_core::Participant {
            site: smd_core::Site {
                source: smd_core::SourceId(0),
                path: PathId(1),
                span: smd_core::Span::default(),
            },
            role: smd_core::Role::Contributor,
        }],
        ..Finding::unscored()
    };
    assert!(check_finding(rule, &ix, &good).is_ok());
    let mut bad = good.clone();
    bad.facts.push(("typo", EvidenceValue::Int(1)));
    assert!(check_finding(rule, &ix, &bad).unwrap_err().contains("undeclared"));
    let mut bad = good.clone();
    bad.facts.push(("agreement", EvidenceValue::Int(1)));
    assert!(check_finding(rule, &ix, &bad).unwrap_err().contains("wrong type"));
    let mut bad = good.clone();
    bad.score = 5;
    assert!(check_finding(rule, &ix, &bad).is_err());
    let mut bad = good;
    bad.participants.clear();
    assert!(check_finding(rule, &ix, &bad).is_err());
}

#[test]
fn cancellation_stops_the_rule() {
    let mut b = PlaysetBuilder::new();
    seeded(&mut b, 5);
    let ix = b.build();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let out = run_all(&registry(), &RuleCtx { index: &ix, cancel: &cancel });
    assert!(out.findings.is_empty());
    assert!(out.cancelled);
}

#[test]
fn uncancelled_run_is_not_flagged_cancelled() {
    let mut b = PlaysetBuilder::new();
    seeded(&mut b, 2);
    let ix = b.build();
    let cancel = CancellationToken::new();
    assert!(!run_all(&registry(), &RuleCtx { index: &ix, cancel: &cancel }).cancelled);
}

#[test]
fn r1_shadowed_without_a_winner_is_caught() {
    let mut b = PlaysetBuilder::new();
    seeded(&mut b, 1);
    b.same_path(smd_core::SamePath::Unknown);
    let ix = b.build();
    let p = ix.overlay().multi_contributor_paths().next().unwrap();
    let rule = &crate::rules::FileOverwrite;
    let site =
        smd_core::Site { source: smd_core::SourceId(0), path: p, span: smd_core::Span::default() };
    let f = Finding {
        rule: rule.id(),
        subject: Subject::Path(p),
        participants: vec![smd_core::Participant { site, role: smd_core::Role::Shadowed }],
        ..Finding::unscored()
    };
    assert!(check_finding(rule, &ix, &f).unwrap_err().contains("R1"));
}
