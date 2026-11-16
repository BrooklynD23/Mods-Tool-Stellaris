//! Runner, registry, case harness, and `core.file_overwrite` acceptance tests.

use smd_core::{Activation, Finding, Role, SamePath, Subject};
use smd_index::{PlaysetBuilder, ScanIndex};

use crate::{CancellationToken, Rule, RuleCtx, registry};

mod runner;

fn run(index: &ScanIndex) -> Vec<Finding> {
    let cancel = CancellationToken::new();
    crate::run_all(&registry(), &RuleCtx { index, cancel: &cancel }).findings
}

fn seeded(b: &mut PlaysetBuilder, k: usize) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    for i in 0..40 {
        b.file(a, &format!("common/solo/{i:02}.txt"), "s");
    }
    for i in 0..k {
        let p = format!("common/clash/{i:02}.txt");
        b.file(a, &p, "from a").file(m, &p, "from b");
    }
}

#[test]
fn file_overwrite_k_seeded_collisions() {
    let mut b = PlaysetBuilder::new();
    seeded(&mut b, 7);
    let findings = run(&b.build());
    assert_eq!(findings.len(), 7);
    let mut b = PlaysetBuilder::new();
    seeded(&mut b, 7);
    let ix = b.build();
    for f in run(&ix) {
        let r = ix.resolve_finding(&f);
        assert!(r.subject_text.starts_with("common/clash/"));
        let winners: Vec<_> = r.participants.iter().filter(|p| p.role == Role::Winner).collect();
        assert_eq!(winners.len(), 1);
        assert_eq!(&*winners[0].mod_name, "B");
        assert_eq!(r.participants.iter().filter(|p| p.role == Role::Shadowed).count(), 1);
        assert!(f.is_unscored());
    }
    // Same layout, DirectoryScan: still exactly 7, none with a Winner.
    let mut b = PlaysetBuilder::new();
    seeded(&mut b, 7);
    b.directory_scan();
    let findings = run(&b.build());
    assert_eq!(findings.len(), 7);
    for f in &findings {
        assert!(f.participants.iter().all(|p| p.role == Role::Contributor));
        assert!(!f.gaps.is_empty());
    }
}

#[test]
fn file_overwrite_same_path_unknown_reports_contributors_only() {
    let mut b = PlaysetBuilder::new();
    seeded(&mut b, 2);
    b.same_path(SamePath::Unknown);
    let findings = run(&b.build());
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| f.participants.iter().all(|p| p.role == Role::Contributor)));
}

#[test]
fn file_overwrite_cases_cover_all_kinds() {
    let rule = crate::rules::FileOverwrite;
    let kinds: Vec<_> = rule.cases().iter().map(|c| c.kind).collect();
    for k in crate::CaseKind::ALL {
        assert!(kinds.contains(&k), "{k:?}");
    }
    for case in rule.cases() {
        crate::run_case(&rule, case).unwrap_or_else(|e| panic!("{e}"));
    }
}

#[test]
fn resolve_finding_is_text_only() {
    let mut b = PlaysetBuilder::new();
    seeded(&mut b, 1);
    let ix = b.build();
    let f = run(&ix).remove(0);
    let r = ix.resolve_finding(&f);
    assert_eq!(&*r.subject_text, "common/clash/00.txt");
    assert!(matches!(r.subject, Subject::Path(_)));
    assert_eq!(r.participants.len(), 2);
    for p in &r.participants {
        assert_eq!(&*p.path, "common/clash/00.txt");
        assert!(p.mod_key.as_str().starts_with("mod:"));
        assert!(["A", "B"].contains(&&*p.mod_name));
        assert!(p.span.len > 0);
    }
    let tag = |k: &str| r.facts.iter().find(|(key, _)| *key == k).map(|(_, v)| v.clone());
    assert_eq!(tag("mechanism"), Some(smd_core::ResolvedEvidence::Tag("same_path")));
    assert_eq!(tag("same_path"), Some(smd_core::ResolvedEvidence::Tag("replace")));
    assert_eq!(tag("agreement"), Some(smd_core::ResolvedEvidence::Tag("differs")));
}
