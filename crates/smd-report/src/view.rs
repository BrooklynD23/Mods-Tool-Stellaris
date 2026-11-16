//! The view model: the report as plain text, after redaction and bounding, before any sink
//! escaping. Every renderer walks this and nothing else, so the three formats cannot
//! disagree about what a report says. It is also the JSON schema, via `Serialize`.

use serde::Serialize;

/// The sentence a complete, zero-finding scan prints. It is only ever printed with the
/// header.
pub(crate) const NO_CONFLICTS_SENTENCE: &str = "no same-path conflicts found";
/// Printed instead of the above when the scan read no files at all.
pub(crate) const NOTHING_SCANNED_SENTENCE: &str = "no files were read, so nothing was scanned";
/// Printed whenever the scan was not complete. Deliberately never says that nothing was
/// found: a partial scan proves nothing about the files it did not read.
pub(crate) const INCOMPLETE_NOTE: &str = "scan incomplete: some files or sources were not fully \
     read, so the findings below may be missing conflicts";

#[derive(Serialize, Debug)]
pub(crate) struct View {
    pub header: HeaderView,
    /// `true` when nothing was partial, unparsed, or content-incomplete.
    pub scan_complete: bool,
    /// Set only for a complete scan with zero findings.
    pub summary: Option<String>,
    /// Set whenever the scan was not complete.
    pub incomplete_note: Option<&'static str>,
    pub findings: Vec<FindingView>,
    /// How many findings the total cap left out.
    pub findings_hidden: usize,
    /// The explicit "... and N more findings" line, when the total cap binds.
    pub findings_more: Option<String>,
    pub diagnostics: Vec<DiagView>,
}

#[derive(Serialize, Debug)]
pub(crate) struct HeaderView {
    pub order_source: String,
    pub order_certainty: &'static str,
    pub activation_certainty: &'static str,
    pub snapshot_method: String,
    pub files_total: u32,
    pub partial: u32,
    pub unparsed_total: u32,
    pub unparsed: Vec<UnparsedView>,
    pub domains: Vec<String>,
    pub content_incomplete: Vec<ContentIncompleteView>,
    pub inventory: Vec<InventoryView>,
}

#[derive(Serialize, Debug)]
pub(crate) struct UnparsedView {
    pub reason: String,
    pub count: u32,
}

#[derive(Serialize, Debug)]
pub(crate) struct ContentIncompleteView {
    pub mod_key: String,
    pub reason: String,
    pub detail: Option<String>,
}

#[derive(Serialize, Debug)]
pub(crate) struct InventoryView {
    pub mod_key: String,
    pub name: String,
    pub role: String,
    pub activation: String,
    pub activation_basis: String,
    /// The load position as text, or `unknown`.
    pub position: String,
    pub version: Option<String>,
    pub path: String,
    pub descriptor_diagnostics: Vec<String>,
}

#[derive(Serialize, Debug)]
pub(crate) struct FindingView {
    pub rule: String,
    pub subject: String,
    pub certainty: CertaintyView,
    pub gaps: Vec<&'static str>,
    pub participants: Vec<ParticipantView>,
    /// How many participants the per-finding cap left out.
    pub participants_hidden: usize,
    /// The explicit "... and N more participants" line, when the cap binds.
    pub participants_more: Option<String>,
    pub facts: Vec<(String, String)>,
}

#[derive(Serialize, Debug)]
pub(crate) struct CertaintyView {
    pub completeness: &'static str,
    pub activation: &'static str,
    pub order: &'static str,
    pub semantics: &'static str,
}

#[derive(Serialize, Debug)]
pub(crate) struct ParticipantView {
    pub role: &'static str,
    pub mod_key: String,
    pub mod_name: String,
    pub path: String,
    pub offset: u32,
    pub len: u32,
    pub line: u32,
}

#[derive(Serialize, Debug)]
pub(crate) struct DiagView {
    pub level: String,
    pub code: String,
    pub location: String,
    pub detail: String,
}
