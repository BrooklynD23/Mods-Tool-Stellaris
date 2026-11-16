//! `ScanOutcome` to `ReportModel`: field-for-field, no logic.

use smd_api::{LocateOutput, LocatedMod, ModRow, ScanDiagnostic, ScanOutcome};
use smd_report::{
    DiagnosticRow, InventoryRow, LocateCandidate, LocateMod, LocateReport, ReportHeader,
    ReportModel,
};

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

fn text(p: &std::path::Path) -> String {
    p.display().to_string()
}

fn located_mod(m: LocatedMod) -> LocateMod {
    LocateMod {
        key: m.key,
        name: m.name,
        version: m.version,
        supported_version: m.supported_version,
        role: m.role.to_owned(),
        mod_file: m.mod_file,
        content_root: m.content_root.as_deref().map(text),
        readable: m.readable,
        remote_file_id: m.remote_file_id,
        origin: m.origin.to_owned(),
    }
}

/// The renderer input for `smd locate`: field-for-field, paths as text.
#[must_use]
pub fn locate_report(o: LocateOutput) -> LocateReport {
    let l = o.located;
    LocateReport {
        store: l.store.as_str().to_owned(),
        game_dir: l.game_dir.as_deref().map(text),
        user_dir: l.user_dir.as_deref().map(text),
        workshop_dirs: l.workshop_dirs.iter().map(|p| text(p)).collect(),
        steam_libraries: l.steam_libraries.iter().map(|p| text(p)).collect(),
        candidates: l
            .candidates
            .iter()
            .map(|c| LocateCandidate {
                key: c.key.to_owned(),
                path: text(&c.path),
                origin: c.origin.as_str().to_owned(),
                exists: c.exists,
            })
            .collect(),
        mods: o.mods.into_iter().map(located_mod).collect(),
        diagnostics: o.diagnostics.into_iter().map(diagnostic).collect(),
        incomplete: o.incomplete,
        game_version: o.game_version,
    }
}
