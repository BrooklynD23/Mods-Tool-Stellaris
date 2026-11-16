//! The terminal table renderer.

use std::fmt::Write as _;

use crate::escape::terminal::{BOLD, YELLOW, styled, text};
use crate::model::{RenderOptions, ReportModel};
use crate::view::{DiagView, FindingView, HeaderView, ParticipantView, View};
use crate::{Renderer, build};

/// Plain-text output for a terminal. Colour only when [`RenderOptions::color`] is set.
#[derive(Copy, Clone, Debug, Default)]
pub struct Table;

impl Renderer for Table {
    fn render(&self, model: &ReportModel, opts: &RenderOptions) -> String {
        let v = build::view(model, opts);
        let mut out = String::new();
        title(&mut out, "smd scan report", opts.color);
        completeness(&mut out, &v.header, opts.color);
        status(&mut out, &v, opts.color);
        inventory(&mut out, &v.header, opts.color);
        findings(&mut out, &v, opts.color);
        if !v.diagnostics.is_empty() {
            title(&mut out, &format!("Diagnostics ({})", v.diagnostics.len()), opts.color);
            diagnostic_lines(&mut out, &v.diagnostics);
        }
        out
    }
}

/// The diagnostics stream as terminal lines, escaped. Used by [`crate::render_diagnostics`].
pub(crate) fn diagnostics_stream(rows: &[DiagView]) -> String {
    let mut out = String::new();
    diagnostic_lines(&mut out, rows);
    out
}

fn diagnostic_lines(out: &mut String, rows: &[DiagView]) {
    for d in rows {
        let at =
            if d.location.is_empty() { String::new() } else { format!(" {}", text(&d.location)) };
        let _ = writeln!(out, "{}: {}{}: {}", text(&d.level), text(&d.code), at, text(&d.detail));
    }
}

fn title(out: &mut String, s: &str, color: bool) {
    let _ = writeln!(out, "\n{}", styled(s, BOLD, color));
}

fn completeness(out: &mut String, h: &HeaderView, color: bool) {
    title(out, "Scan completeness", color);
    let _ = writeln!(out, "  files_total: {}", h.files_total);
    let _ = writeln!(out, "  partial: {}", h.partial);
    let _ = writeln!(out, "  unparsed: {}", h.unparsed_total);
    for u in &h.unparsed {
        let _ = writeln!(out, "    {}: {}", text(&u.reason), u.count);
    }
    let _ = writeln!(out, "  domains: {}", list_or_none(&h.domains));
    if h.content_incomplete.is_empty() {
        let _ = writeln!(out, "  content_incomplete: (none)");
    }
    for c in &h.content_incomplete {
        let at = c.detail.as_deref().map_or_else(String::new, |d| format!(" at {}", text(d)));
        let _ = writeln!(out, "  content_incomplete: {} {}{at}", text(&c.mod_key), text(&c.reason));
    }
    let _ = writeln!(
        out,
        "  order certainty: {} (source: {})",
        h.order_certainty,
        text(&h.order_source)
    );
    let _ = writeln!(out, "  activation certainty: {}", h.activation_certainty);
    let _ = writeln!(out, "  snapshot method: {}", text(&h.snapshot_method));
}

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        return "(none)".to_owned();
    }
    items.iter().map(|s| text(s)).collect::<Vec<_>>().join(", ")
}

fn status(out: &mut String, v: &View, color: bool) {
    if let Some(note) = v.incomplete_note {
        let _ = writeln!(out, "\n{}", styled(note, YELLOW, color));
    }
    if let Some(sentence) = &v.summary {
        let _ = writeln!(out, "\n{sentence}");
    }
}

fn inventory(out: &mut String, h: &HeaderView, color: bool) {
    title(out, &format!("Mods ({})", h.inventory.len()), color);
    let head = ["key", "name", "role", "activation", "known by", "position", "version", "path"];
    let mut rows: Vec<Vec<String>> = Vec::new();
    for m in &h.inventory {
        rows.push(
            [
                &m.mod_key,
                &m.name,
                &m.role,
                &m.activation,
                &m.activation_basis,
                &m.position,
                m.version.as_deref().unwrap_or("-"),
                &m.path,
            ]
            .iter()
            .map(|s| text(s))
            .collect(),
        );
    }
    grid(out, &head, &rows);
    for m in &h.inventory {
        for d in &m.descriptor_diagnostics {
            let _ = writeln!(out, "  descriptor ({}): {}", text(&m.mod_key), text(d));
        }
    }
}

/// Left-aligned columns, padded to the widest cell.
fn grid(out: &mut String, head: &[&str], rows: &[Vec<String>]) {
    let widths: Vec<usize> = (0..head.len())
        .map(|i| {
            rows.iter().map(|r| r[i].chars().count()).chain([head[i].len()]).max().unwrap_or(0)
        })
        .collect();
    let line = |cells: Vec<&str>| {
        let padded: Vec<String> =
            cells.iter().zip(&widths).map(|(c, w)| format!("{c:<w$}", w = *w)).collect();
        format!("  {}\n", padded.join("  ").trim_end())
    };
    out.push_str(&line(head.to_vec()));
    for r in rows {
        out.push_str(&line(r.iter().map(String::as_str).collect()));
    }
}

fn findings(out: &mut String, v: &View, color: bool) {
    let total = v.findings.len() + v.findings_hidden;
    title(out, &format!("Findings ({total})"), color);
    for (i, f) in v.findings.iter().enumerate() {
        finding(out, i + 1, f);
    }
    if let Some(more) = &v.findings_more {
        let _ = writeln!(out, "{more}");
    }
}

fn finding(out: &mut String, n: usize, f: &FindingView) {
    let _ = writeln!(out, "[{n}] {}  {}", text(&f.rule), text(&f.subject));
    let c = &f.certainty;
    let _ = writeln!(
        out,
        "    certainty: completeness={} activation={} order={} semantics={}",
        c.completeness, c.activation, c.order, c.semantics
    );
    if !f.gaps.is_empty() {
        let _ = writeln!(out, "    gaps: {}", f.gaps.join(", "));
    }
    let shown = f.participants.len();
    let _ = writeln!(out, "    participants ({}):", shown + f.participants_hidden);
    for p in &f.participants {
        let _ = writeln!(out, "      {}", participant(p));
    }
    if let Some(more) = &f.participants_more {
        let _ = writeln!(out, "      {more}");
    }
    for (k, v) in &f.facts {
        let _ = writeln!(out, "    {}: {}", text(k), text(v));
    }
}

fn participant(p: &ParticipantView) -> String {
    format!(
        "{:<11} {}  {}  {}  @{}+{} line {}",
        p.role,
        text(&p.mod_key),
        text(&p.mod_name),
        text(&p.path),
        p.offset,
        p.len,
        p.line
    )
}
