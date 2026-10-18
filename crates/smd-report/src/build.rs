//! Model to view: redact, bound, and turn enums into tags. No sink knowledge lives here.

use smd_core::{
    ContentIncompleteReason, DiagSeverity, ResolvedEvidence, ResolvedFinding, ResolvedParticipant,
    ScanCompleteness,
};

use crate::model::{DiagnosticRow, InventoryRow, RenderOptions, ReportHeader, ReportModel};
use crate::redact::Redactor;
use crate::view::{
    CertaintyView, ContentIncompleteView, DiagView, FindingView, HeaderView, INCOMPLETE_NOTE,
    InventoryView, NO_CONFLICTS_SENTENCE, NOTHING_SCANNED_SENTENCE, ParticipantView, UnparsedView,
    View,
};

/// Build the view for a full report, then redact every string of it in one pass.
pub(crate) fn view(model: &ReportModel, opts: &RenderOptions) -> View {
    let r = Redactor::new(&opts.redaction, opts.redact);
    let c = &model.header.completeness;
    let scan_complete = is_complete(c);
    let shown = model.findings.len().min(opts.caps.max_findings);
    let mut v = View {
        header: header(&model.header),
        scan_complete,
        summary: (scan_complete && model.findings.is_empty()).then(|| summary(c.files_total)),
        incomplete_note: (!scan_complete).then_some(INCOMPLETE_NOTE),
        findings: model.findings[..shown].iter().map(|f| finding(f, opts)).collect(),
        findings_hidden: model.findings.len() - shown,
        findings_more: more(model.findings.len() - shown, "findings"),
        diagnostics: diagnostics(&model.diagnostics, &Redactor::new(&opts.redaction, false)),
    };
    r.view(&mut v);
    v
}

/// The zero-finding sentence, qualified by how much was read. A scan that read nothing
/// never calls itself clean.
fn summary(files_total: u32) -> String {
    if files_total == 0 {
        NOTHING_SCANNED_SENTENCE.to_owned()
    } else {
        format!("{NO_CONFLICTS_SENTENCE} among the {files_total} files read")
    }
}

/// Build just the diagnostics rows, redacted by the one redaction pass.
pub(crate) fn diagnostics(rows: &[DiagnosticRow], r: &Redactor<'_>) -> Vec<DiagView> {
    let mut views: Vec<DiagView> = rows
        .iter()
        .map(|d| DiagView {
            level: level_tag(d.level),
            code: d.code.clone(),
            location: d.location.clone(),
            detail: d.detail.clone(),
        })
        .collect();
    r.diagnostics(&mut views);
    views
}

/// The marker line a binding cap prints, in every format.
fn more(hidden: usize, what: &str) -> Option<String> {
    (hidden > 0).then(|| format!("... and {hidden} more {what}"))
}

fn is_complete(c: &ScanCompleteness) -> bool {
    c.partial == 0 && c.unparsed.iter().all(|(_, n)| *n == 0) && c.content_incomplete.is_empty()
}

fn header(h: &ReportHeader) -> HeaderView {
    let c = &h.completeness;
    HeaderView {
        order_source: h.provenance.order_source.to_owned(),
        order_certainty: h.provenance.order.as_tag(),
        activation_certainty: h.provenance.activation.as_tag(),
        snapshot_method: snake(&format!("{:?}", h.provenance.snapshot)),
        files_total: c.files_total,
        partial: c.partial,
        unparsed_total: c.unparsed.iter().map(|(_, n)| *n).sum(),
        unparsed: c
            .unparsed
            .iter()
            .map(|(why, n)| UnparsedView { reason: snake(&format!("{why:?}")), count: *n })
            .collect(),
        domains: c.domains.iter().map(ToString::to_string).collect(),
        content_incomplete: c
            .content_incomplete
            .iter()
            .map(|(key, why, at)| content_incomplete(key.as_str(), *why, at.as_deref()))
            .collect(),
        inventory: h.inventory.iter().map(inventory).collect(),
    }
}

fn content_incomplete(
    key: &str,
    why: ContentIncompleteReason,
    at: Option<&str>,
) -> ContentIncompleteView {
    ContentIncompleteView {
        mod_key: key.to_owned(),
        reason: snake(&format!("{why:?}")),
        detail: at.map(str::to_owned),
    }
}

fn inventory(row: &InventoryRow) -> InventoryView {
    InventoryView {
        mod_key: row.mod_key.clone(),
        name: row.name.clone(),
        role: snake(&format!("{:?}", row.role)),
        activation: snake(&format!("{:?}", row.activation)),
        activation_basis: row.activation_basis.clone(),
        position: row.position.map_or_else(|| "unknown".to_owned(), |p| p.to_string()),
        version: row.version.clone(),
        path: row.path.clone(),
        descriptor_diagnostics: row.descriptor_diagnostics.clone(),
    }
}

fn finding(f: &ResolvedFinding, opts: &RenderOptions) -> FindingView {
    let shown = f.participants.len().min(opts.caps.max_participants);
    FindingView {
        rule: f.rule.as_str().to_owned(),
        subject: f.subject_text.to_string(),
        certainty: CertaintyView {
            completeness: f.certainty.completeness.as_tag(),
            activation: f.certainty.activation.as_tag(),
            order: f.certainty.order.as_tag(),
            semantics: f.certainty.semantics.as_tag(),
        },
        gaps: f.gaps.iter().map(|g| g.as_tag()).collect(),
        participants: f.participants[..shown].iter().map(participant).collect(),
        participants_hidden: f.participants.len() - shown,
        participants_more: more(f.participants.len() - shown, "participants"),
        facts: f.facts.iter().map(|(k, v)| ((*k).to_owned(), evidence(v))).collect(),
    }
}

fn participant(p: &ResolvedParticipant) -> ParticipantView {
    ParticipantView {
        role: p.role.as_tag(),
        mod_key: p.mod_key.as_str().to_owned(),
        mod_name: p.mod_name.to_string(),
        path: p.path.to_string(),
        offset: p.span.offset,
        len: p.span.len,
        line: p.span.line,
    }
}

fn evidence(v: &ResolvedEvidence) -> String {
    match v {
        ResolvedEvidence::Int(i) => i.to_string(),
        ResolvedEvidence::Bool(b) => b.to_string(),
        ResolvedEvidence::Tag(t) => (*t).to_owned(),
        ResolvedEvidence::Text(t) => t.to_string(),
        // `ResolvedEvidence` is `#[non_exhaustive]`: degrade loudly, never to nothing.
        _ => "<unhandled evidence variant>".to_owned(),
    }
}

fn level_tag(level: DiagSeverity) -> String {
    snake(&format!("{level:?}"))
}

/// `NotApplicable` to `not_applicable`: the tag form of a `Debug` variant name.
fn snake(camel: &str) -> String {
    let mut out = String::with_capacity(camel.len() + 4);
    for (i, c) in camel.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::snake;

    #[test]
    fn debug_names_become_tags() {
        assert_eq!(snake("NotApplicable"), "not_applicable");
        assert_eq!(snake("Oversize"), "oversize");
        assert_eq!(snake("ChangedDuringScan"), "changed_during_scan");
    }
}
