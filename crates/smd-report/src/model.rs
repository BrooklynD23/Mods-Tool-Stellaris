//! The renderer input model, and the options every renderer takes.
//!
//! Everything here is text or an `smd-core` enum: no interned id reaches this crate
//! (Invariant C1). The caller builds a [`ReportModel`] from the engine's output and sorts
//! `findings` with `smd-core`'s text sort before handing it over; renderers print them in
//! the order given.

use smd_core::{
    Activation, DiagSeverity, PlaysetProvenance, ResolvedFinding, ScanCompleteness, SourceRole,
};

use crate::redact::RedactionContext;

/// Everything a renderer prints.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReportModel {
    /// Printed first, by every format, whatever `findings` holds.
    pub header: ReportHeader,
    /// In the order the caller sorted them; renderers never reorder.
    pub findings: Vec<ResolvedFinding>,
    /// Scan diagnostics, as text.
    pub diagnostics: Vec<DiagnosticRow>,
}

/// The part of a report that is never optional: where the order came from, how much of the
/// tree was read, and which mods were seen.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReportHeader {
    /// Order source, order and activation confidence, snapshot method.
    pub provenance: PlaysetProvenance,
    /// What was not fully read.
    pub completeness: ScanCompleteness,
    /// One row per source, in discovery order.
    pub inventory: Vec<InventoryRow>,
}

/// One mod in the inventory, as text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InventoryRow {
    /// Stable key.
    pub mod_key: String,
    /// Display name, exactly as the descriptor gives it.
    pub name: String,
    /// Where it came from.
    pub role: SourceRole,
    /// Whether the playset has it switched on.
    pub activation: Activation,
    /// How the activation is known, e.g. `launcher database` or `not established`.
    pub activation_basis: String,
    /// Load position, or `None` when the order is unknown.
    pub position: Option<u32>,
    /// The mod's own version string.
    pub version: Option<String>,
    /// Where the mod lives on disk.
    pub path: String,
    /// Descriptor problems for this mod, as text.
    pub descriptor_diagnostics: Vec<String>,
}

/// One scan diagnostic, as text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiagnosticRow {
    /// Stable code, e.g. `clausewitz.unbalanced_brace`.
    pub code: String,
    /// How badly it went wrong.
    pub level: DiagSeverity,
    /// Mod-relative or absolute path text; empty when there is none.
    pub location: String,
    /// Human-readable specifics, possibly an OS error message that embeds a path.
    pub detail: String,
}

/// Caps on how much one report prints (roadmap M1, "Bounded evidence").
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Caps {
    /// Participants printed per finding.
    pub max_participants: usize,
    /// Findings printed in total.
    pub max_findings: usize,
}

impl Caps {
    /// Default participants per finding.
    pub const DEFAULT_MAX_PARTICIPANTS: usize = 10;
    /// Default findings per report.
    pub const DEFAULT_MAX_FINDINGS: usize = 200;
}

impl Default for Caps {
    fn default() -> Caps {
        Caps {
            max_participants: Caps::DEFAULT_MAX_PARTICIPANTS,
            max_findings: Caps::DEFAULT_MAX_FINDINGS,
        }
    }
}

/// How to render.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RenderOptions {
    /// Bounds on the output.
    pub caps: Caps,
    /// ANSI colour in the table. Off unless the caller says stdout is a TTY.
    pub color: bool,
    /// Redaction is on by default; `false` is the explicit opt-out (`--no-redact`).
    pub redact: bool,
    /// What to redact.
    pub redaction: RedactionContext,
}

impl RenderOptions {
    /// Defaults with the given redaction context: caps default, colour off, redaction on.
    #[must_use]
    pub fn new(redaction: RedactionContext) -> RenderOptions {
        RenderOptions { caps: Caps::default(), color: false, redact: true, redaction }
    }

    /// Colour on, for a caller whose stdout is a TTY.
    #[must_use]
    pub fn with_color(mut self, tty: bool) -> RenderOptions {
        self.color = tty;
        self
    }

    /// Replace the caps.
    #[must_use]
    pub fn with_caps(mut self, caps: Caps) -> RenderOptions {
        self.caps = caps;
        self
    }

    /// Switch redaction off (`--no-redact`).
    #[must_use]
    pub fn without_redaction(mut self) -> RenderOptions {
        self.redact = false;
        self
    }
}

impl Default for RenderOptions {
    /// Redaction on, with the context taken from the environment, so "on by default"
    /// actually redacts this machine's home directory and account name.
    fn default() -> RenderOptions {
        RenderOptions::new(RedactionContext::from_env())
    }
}
