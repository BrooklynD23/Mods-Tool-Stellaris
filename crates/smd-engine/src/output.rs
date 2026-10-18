//! What a scan returns: text and `smd-core` enums only. No interned id leaves the engine
//! (Invariant C1); a finding is already a [`ResolvedFinding`].

use smd_core::{
    Activation, DiagSeverity, PlaysetProvenance, ResolvedFinding, ScanCompleteness, SourceRole,
};

/// One source in the inventory, as text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ModRow {
    /// Stable key.
    pub mod_key: String,
    /// Display name, exactly as the descriptor gives it.
    pub name: String,
    /// Where it came from.
    pub role: SourceRole,
    /// Whether the playset has it switched on.
    pub activation: Activation,
    /// How the activation is known.
    pub activation_basis: String,
    /// Load position, or `None` when the order is unknown.
    pub position: Option<u32>,
    /// The mod's own version string.
    pub version: Option<String>,
    /// Where it lives on disk; empty when it has no readable content root.
    pub path: String,
    /// Descriptor problems for this mod.
    pub descriptor_diagnostics: Vec<String>,
}

/// One scan diagnostic, as text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScanDiagnostic {
    /// Stable code, e.g. `fs.unreadable`.
    pub code: String,
    /// How badly it went wrong.
    pub level: DiagSeverity,
    /// A path or key; empty when the diagnostic is scan-wide.
    pub location: String,
    /// Human-readable specifics.
    pub detail: String,
}

/// A finished scan.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScanOutput {
    /// Where the order came from and how far it and the activation are trusted.
    pub provenance: PlaysetProvenance,
    /// What was not fully read.
    pub completeness: ScanCompleteness,
    /// One row per source, vanilla first, then mods in `ModKey` byte order.
    pub inventory: Vec<ModRow>,
    /// Sorted with `smd-core`'s text sort.
    pub findings: Vec<ResolvedFinding>,
    /// Sorted by location, code, detail.
    pub diagnostics: Vec<ScanDiagnostic>,
}
