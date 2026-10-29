//! Inventory rows and diagnostics as text.

use smd_core::{Activation, Diagnostic, PlaysetProvenance, SourceRole};
use smd_fs::FsDiagnostic;
use smd_ingest::{IngestDiagnostic, Inventory, ModRecord};

use crate::output::{ModRow, ScanDiagnostic};

fn basis(role: SourceRole, activation: Activation, prov: &PlaysetProvenance) -> String {
    match (role, activation) {
        (SourceRole::Vanilla, _) => "always active".to_owned(),
        (_, Activation::Active) => format!("enabled per {}", prov.order_source),
        (_, Activation::Inactive) => format!("not enabled per {}", prov.order_source),
        _ => "not established".to_owned(),
    }
}

fn row(inv: &Inventory, rec: &ModRecord) -> Option<ModRow> {
    let meta = inv.table.get(rec.source)?;
    Some(ModRow {
        mod_key: rec.key.as_str().to_owned(),
        name: meta.name.to_string(),
        role: meta.role,
        activation: meta.activation,
        activation_basis: basis(meta.role, meta.activation, &inv.provenance),
        position: meta.rank.last().copied(),
        version: rec.descriptor.version.as_deref().map(str::to_owned),
        path: rec.content_root.as_ref().map_or_else(String::new, |p| p.display().to_string()),
        descriptor_diagnostics: rec.diagnostics.iter().map(descriptor_line).collect(),
    })
}

fn descriptor_line(d: &Diagnostic) -> String {
    format!("{}: {}", d.code, d.detail)
}

/// The inventory table as rows, in `SourceId` order.
pub(crate) fn inventory_rows(inv: &Inventory) -> Vec<ModRow> {
    inv.records.iter().filter_map(|r| row(inv, r)).collect()
}

pub(crate) fn from_ingest(d: &IngestDiagnostic) -> ScanDiagnostic {
    ScanDiagnostic {
        code: d.code.to_owned(),
        level: d.severity,
        location: d.subject.to_string(),
        detail: d.detail.to_string(),
    }
}

pub(crate) fn from_fs(mod_key: &str, d: &FsDiagnostic) -> ScanDiagnostic {
    ScanDiagnostic {
        code: d.code.to_owned(),
        level: d.severity,
        location: d.path.to_string(),
        detail: format!("mod {mod_key}: {}", d.detail),
    }
}

pub(crate) fn from_core(d: &Diagnostic) -> ScanDiagnostic {
    ScanDiagnostic {
        code: d.code.to_owned(),
        level: d.severity,
        location: String::new(),
        detail: d.detail.to_string(),
    }
}
