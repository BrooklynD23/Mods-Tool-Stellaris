//! `smd-report` — renderers over `ResolvedFinding`: table, JSON, and Markdown.
//!
//! **Owns:** the [`Renderer`] trait, the renderer input model ([`ReportModel`]), the
//! per-sink escaping, and the redaction pass.
//!
//! **Must never:** compute findings; depend on anything but `smd-core`; generate a link
//! from data; render HTML before gate H27 closes.
//!
//! # What every format prints
//!
//! The header comes first and is not optional: [`ScanCompleteness`](smd_core::ScanCompleteness),
//! the order and activation certainty badges, and the snapshot method, even with zero
//! findings (roadmap M1, gate H11). A scan that was not complete never prints a sentence
//! saying nothing was found; a complete scan with zero findings prints
//! "no same-path conflicts found" together with the header.
//!
//! M1 prints no severity and no score: findings are mechanism, participants, and outcome,
//! in the order the caller gives them. The caller sorts with `smd-core`'s text sort.
//!
//! # Safety and privacy
//!
//! Every data string is escaped for its sink in exactly one module per sink
//! (`escape/`, doc 11 "Output safety", gate H25), and redaction (doc 14 "Privacy",
//! gate H26) runs over the whole view before any sink sees it. Output is bounded by
//! [`Caps`], with an explicit "... and N more" line when a cap binds.

#![forbid(unsafe_code)]

mod build;
mod escape;
mod json;
mod markdown;
mod model;
mod redact;
mod table;
mod view;

pub use json::Json;
pub use markdown::Markdown;
pub use model::{Caps, DiagnosticRow, InventoryRow, RenderOptions, ReportHeader, ReportModel};
pub use redact::RedactionContext;
pub use table::Table;

/// A report format.
pub trait Renderer {
    /// Render the whole report. The result is the complete document for the format.
    fn render(&self, model: &ReportModel, opts: &RenderOptions) -> String;
}

/// The diagnostics stream alone (for `-v`, or stderr), in the same format and with the same
/// redaction as the report. `format` is any of [`Table`], [`Markdown`], [`Json`].
#[must_use]
pub fn render_diagnostics(
    format: DiagnosticsFormat,
    rows: &[DiagnosticRow],
    opts: &RenderOptions,
) -> String {
    let r = redact::Redactor::new(&opts.redaction, opts.redact);
    let view = build::diagnostics(rows, &r);
    match format {
        DiagnosticsFormat::Table => table::diagnostics_stream(&view),
        DiagnosticsFormat::Markdown => markdown::diagnostics_stream(&view),
        DiagnosticsFormat::Json => json::diagnostics_stream(&view),
    }
}

/// Which format [`render_diagnostics`] writes.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DiagnosticsFormat {
    /// Terminal lines.
    Table,
    /// A Markdown list.
    Markdown,
    /// A JSON array.
    Json,
}
