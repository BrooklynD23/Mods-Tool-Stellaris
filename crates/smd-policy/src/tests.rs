//! One smoke check of the shipped data and the pipeline. Broad suites are deferred (owner).

use smd_core::{
    ActivationCertainty, Certainty, Completeness, Confidence, ModKey, PathId, ResolvedEvidence,
    ResolvedFinding, ResolvedParticipant, Role, RuleId, SemanticCertainty, Severity, Span, Subject,
};

use super::*;

fn finding(path: &str, agreement: &'static str) -> ResolvedFinding {
    let p = |key: &str, role| ResolvedParticipant {
        mod_key: ModKey(key.into()),
        mod_name: key.into(),
        path: path.into(),
        span: Span { offset: 0, len: 0, line: 0 },
        role,
    };
    ResolvedFinding {
        rule: RuleId("core.file_overwrite"),
        subject: Subject::Path(PathId(1)),
        subject_text: path.into(),
        subject_parts: Vec::new(),
        participants: vec![p("a", Role::Shadowed), p("b", Role::Winner)],
        facts: [
            ("agreement", ResolvedEvidence::Tag(agreement)),
            ("mechanism", ResolvedEvidence::Tag("same_path")),
        ]
        .into_iter()
        .collect(),
        certainty: Certainty {
            completeness: Completeness::Complete,
            activation: ActivationCertainty::Certain,
            order: Confidence::Exact,
            semantics: SemanticCertainty::Assumed,
        },
        gaps: Default::default(),
        score: 0,
        severity: Severity::Info,
    }
}

#[test]
fn shipped_policy_scores_caps_explains_and_suppresses() {
    let policy = Policy::shipped().expect("shipped policy loads");
    let differs = finding("common/x.txt", "differs");
    let same = finding("gfx/a.dds", "all_identical");
    let id = finding_id(&same);
    let supp = Suppressions::default().with(&id, None).unwrap();
    let out = policy.score(vec![same, differs], &supp);
    // 45 + 25 (same_path) = 70: high, and `semantics = assumed` caps at high.
    assert_eq!((out.findings[0].score, out.findings[0].severity), (70, Severity::High));
    // 45 - 50 + 25 - 35 clamps to 0.
    assert_eq!((out.findings[1].score, out.findings[1].severity), (0, Severity::Info));
    assert!(out.annotations[1].suppressed && !out.annotations[0].suppressed);
    let e = out.annotations[0].explanation.as_ref().unwrap();
    assert!(e.title.contains("b's copy replaces"), "{}", e.title);
    assert_eq!(e.qualifiers.len(), 1, "one non-best axis, one qualifier");
    assert!(is_finding_id(&id));
}

#[test]
fn suppressions_round_trip_through_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub").join("suppressions.toml");
    let s = Suppressions::default().with("0123456789abcdef", Some("mine".into())).unwrap();
    s.save(&path).unwrap();
    assert_eq!(Suppressions::load(&path).unwrap(), s);
    assert!(Suppressions::default().with("nope", None).is_err());
}
