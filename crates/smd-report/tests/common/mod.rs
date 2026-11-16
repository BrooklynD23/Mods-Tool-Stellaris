//! Shared fixtures. No test here reads a game install; every string is synthetic.
// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use smd_core::{
    Activation, Certainty, Confidence, ContentIncompleteReason, DiagSeverity, ModKey, PathId,
    PlaysetProvenance, ResolutionGap, ResolvedEvidence, ResolvedFinding, ResolvedParticipant, Role,
    RuleId, ScanCompleteness, SnapshotMethod, SourceRole, Span, Subject, UnparsedReason,
};
use smd_report::{
    DiagnosticRow, InventoryRow, Json, Markdown, RenderOptions, Renderer, ReportHeader,
    ReportModel, Table,
};

pub fn participant(key: &str, name: &str, path: &str, role: Role) -> ResolvedParticipant {
    ResolvedParticipant {
        mod_key: ModKey(key.into()),
        mod_name: name.into(),
        path: path.into(),
        span: Span { offset: 12, len: 40, line: 3 },
        role,
    }
}

pub fn finding(subject: &str, participants: Vec<ResolvedParticipant>) -> ResolvedFinding {
    ResolvedFinding {
        rule: RuleId("core.file_overwrite"),
        subject: Subject::Path(PathId(1)),
        subject_text: subject.into(),
        subject_parts: Vec::new(),
        participants,
        facts: [("files", ResolvedEvidence::Int(2)), ("identical", ResolvedEvidence::Bool(false))]
            .into_iter()
            .collect(),
        certainty: Certainty::default(),
        gaps: [ResolutionGap::OrderUnknown].into_iter().collect(),
        score: 0,
        severity: Default::default(),
    }
}

pub fn pair(subject: &str) -> ResolvedFinding {
    finding(
        subject,
        vec![
            participant("281990_1001", "Alpha", subject, Role::Winner),
            participant("281990_1002", "Beta", subject, Role::Shadowed),
        ],
    )
}

pub fn row(key: &str, name: &str, path: &str) -> InventoryRow {
    InventoryRow {
        mod_key: key.into(),
        name: name.into(),
        role: SourceRole::Workshop,
        activation: Activation::Unknown,
        activation_basis: "not established".into(),
        position: None,
        version: Some("1.2".into()),
        path: path.into(),
        descriptor_diagnostics: Vec::new(),
    }
}

pub fn complete() -> ScanCompleteness {
    ScanCompleteness { files_total: 120, ..ScanCompleteness::default() }
}

pub fn incomplete() -> ScanCompleteness {
    ScanCompleteness {
        files_total: 120,
        unparsed: [(UnparsedReason::Unreadable, 2), (UnparsedReason::Oversize, 1)]
            .into_iter()
            .collect(),
        domains: vec!["trait".into()],
        content_incomplete: vec![(
            ModKey("281990_1002".into()),
            ContentIncompleteReason::Truncated,
            Some("common/deep".into()),
        )],
        ..ScanCompleteness::default()
    }
}

pub fn provenance(order: Confidence, activation: Confidence) -> PlaysetProvenance {
    PlaysetProvenance {
        order_source: "directory_scan",
        order,
        activation,
        snapshot: SnapshotMethod::NotApplicable,
    }
}

pub fn model(
    completeness: ScanCompleteness,
    inventory: Vec<InventoryRow>,
    findings: Vec<ResolvedFinding>,
) -> ReportModel {
    ReportModel {
        header: ReportHeader {
            provenance: provenance(Confidence::Fallback, Confidence::Fallback),
            completeness,
            inventory,
        },
        findings,
        diagnostics: Vec::new(),
    }
}

pub fn diag(code: &str, location: &str, detail: &str) -> DiagnosticRow {
    DiagnosticRow {
        code: code.into(),
        level: DiagSeverity::Warn,
        location: location.into(),
        detail: detail.into(),
    }
}

/// `(format name, rendered output)` for the three report formats.
pub fn render_all(m: &ReportModel, opts: &RenderOptions) -> Vec<(&'static str, String)> {
    vec![
        ("table", Table.render(m, opts)),
        ("json", Json.render(m, opts)),
        ("markdown", Markdown.render(m, opts)),
    ]
}
