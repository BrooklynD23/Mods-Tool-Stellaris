//! The Markdown renderer. Every data string goes through [`crate::escape::markdown`];
//! the only text written directly is this file's own headings and labels.

use std::fmt::Write as _;

use crate::escape::markdown::{code_span, name, tag, text};
use crate::model::{RenderOptions, ReportModel};
use crate::view::{DiagView, FindingView, HeaderView, View};
use crate::{Renderer, build};

/// GitHub-flavoured Markdown. Data is escaped text; no link or HTML is ever generated.
#[derive(Copy, Clone, Debug, Default)]
pub struct Markdown;

impl Renderer for Markdown {
    fn render(&self, model: &ReportModel, opts: &RenderOptions) -> String {
        let v = build::view(model, opts);
        let mut out = String::from("# smd scan report\n");
        completeness(&mut out, &v.header);
        if let Some(note) = v.incomplete_note {
            let _ = writeln!(out, "\n**{note}**");
        }
        if let Some(sentence) = &v.summary {
            let _ = writeln!(out, "\n{sentence}");
        }
        inventory(&mut out, &v.header);
        findings(&mut out, &v);
        if !v.diagnostics.is_empty() {
            let _ = writeln!(out, "\n## Diagnostics ({})\n", v.diagnostics.len());
            out.push_str(&diagnostics_stream(&v.diagnostics));
        }
        out
    }
}

/// The diagnostics stream as a Markdown list. Used by [`crate::render_diagnostics`].
pub(crate) fn diagnostics_stream(rows: &[DiagView]) -> String {
    let mut out = String::new();
    for d in rows {
        let at = if d.location.is_empty() {
            String::new()
        } else {
            format!(" {}", code_span(&d.location))
        };
        let _ =
            writeln!(out, "- {} {}{at}: {}", tag(&d.level), code_span(&d.code), text(&d.detail));
    }
    out
}

fn completeness(out: &mut String, h: &HeaderView) {
    out.push_str("\n## Scan completeness\n\n");
    let _ = writeln!(out, "- files_total: {}", h.files_total);
    let _ = writeln!(out, "- partial: {}", h.partial);
    let _ = writeln!(out, "- unparsed: {}", h.unparsed_total);
    for u in &h.unparsed {
        let _ = writeln!(out, "  - {}: {}", tag(&u.reason), u.count);
    }
    let domains: Vec<String> = h.domains.iter().map(|d| text(d)).collect();
    let _ = writeln!(
        out,
        "- domains: {}",
        if domains.is_empty() { "(none)".into() } else { domains.join(", ") }
    );
    if h.content_incomplete.is_empty() {
        out.push_str("- content_incomplete: (none)\n");
    }
    for c in &h.content_incomplete {
        let at = c.detail.as_deref().map_or_else(String::new, |d| format!(" at {}", code_span(d)));
        let _ = writeln!(out, "- content_incomplete: {} {}{at}", text(&c.mod_key), tag(&c.reason));
    }
    let _ = writeln!(
        out,
        "- order certainty: {} (source: {})",
        h.order_certainty,
        tag(&h.order_source)
    );
    let _ = writeln!(out, "- activation certainty: {}", h.activation_certainty);
    let _ = writeln!(out, "- snapshot method: {}", tag(&h.snapshot_method));
}

fn inventory(out: &mut String, h: &HeaderView) {
    let _ = writeln!(out, "\n## Mods ({})\n", h.inventory.len());
    out.push_str("| key | name | role | activation | known by | position | version | path |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for m in &h.inventory {
        let version = m.version.as_deref().map_or_else(|| "-".to_owned(), text);
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            code_span(&m.mod_key),
            name(&m.name),
            tag(&m.role),
            tag(&m.activation),
            text(&m.activation_basis),
            tag(&m.position),
            version,
            code_span(&m.path),
        );
    }
    for m in &h.inventory {
        for d in &m.descriptor_diagnostics {
            let _ = writeln!(out, "\n- descriptor ({}): {}", code_span(&m.mod_key), text(d));
        }
    }
}

fn findings(out: &mut String, v: &View) {
    let _ = writeln!(out, "\n## Findings ({})", v.findings.len() + v.findings_hidden);
    for (i, f) in v.findings.iter().enumerate() {
        finding(out, i + 1, f);
    }
    if let Some(more) = &v.findings_more {
        let _ = writeln!(out, "\n{more}");
    }
}

fn finding(out: &mut String, n: usize, f: &FindingView) {
    let _ = writeln!(out, "\n### {n}. {} {}\n", code_span(&f.rule), code_span(&f.subject));
    let c = &f.certainty;
    let _ = writeln!(
        out,
        "- certainty: completeness={} activation={} order={} semantics={}",
        c.completeness, c.activation, c.order, c.semantics
    );
    if !f.gaps.is_empty() {
        let _ = writeln!(
            out,
            "- gaps: {}",
            f.gaps.iter().map(|g| tag(g)).collect::<Vec<_>>().join(", ")
        );
    }
    for (k, v) in &f.facts {
        let _ = writeln!(out, "- {}: {}", text(k), text(v));
    }
    let _ = writeln!(out, "\nParticipants ({}):\n", f.participants.len() + f.participants_hidden);
    out.push_str("| role | key | mod | path | span |\n| --- | --- | --- | --- | --- |\n");
    for p in &f.participants {
        let span = format!("@{}+{} line {}", p.offset, p.len, p.line);
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            p.role,
            code_span(&p.mod_key),
            name(&p.mod_name),
            code_span(&p.path),
            text(&span),
        );
    }
    if let Some(more) = &f.participants_more {
        let _ = writeln!(out, "\n{more}");
    }
}
