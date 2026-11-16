//! `ScanOutcome` to `ReportModel`: field-for-field, no logic.

use smd_api::{ModRow, ScanDiagnostic, ScanOutcome};
use smd_report::{DiagnosticRow, InventoryRow, ReportHeader, ReportModel};

fn inventory(m: ModRow) -> InventoryRow {
    InventoryRow {
        mod_key: m.mod_key,
        name: m.name,
        role: m.role,
        activation: m.activation,
        activation_basis: m.activation_basis,
        position: m.position,
        version: m.version,
        path: m.path,
        descriptor_diagnostics: m.descriptor_diagnostics,
    }
}

fn diagnostic(d: ScanDiagnostic) -> DiagnosticRow {
    DiagnosticRow { code: d.code, level: d.level, location: d.location, detail: d.detail }
}

/// The renderer input for a finished scan. The findings are already in `smd-core`'s text
/// sort order and are passed through untouched.
#[must_use]
pub fn report_model(o: ScanOutcome) -> ReportModel {
    ReportModel {
        header: ReportHeader {
            provenance: o.provenance,
            completeness: o.completeness,
            inventory: o.inventory.into_iter().map(inventory).collect(),
        },
        findings: o.findings,
        diagnostics: o.diagnostics.into_iter().map(diagnostic).collect(),
    }
}
