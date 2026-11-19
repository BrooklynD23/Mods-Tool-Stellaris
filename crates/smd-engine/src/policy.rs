//! Wiring `smd-policy` into a scan (opt-in) and the suppression command.

use std::path::{Path, PathBuf};

use smd_core::{DiagSeverity, ResolvedFinding};
use smd_policy::{Annotation, Policy, Suppressions};

use crate::output::ScanDiagnostic;
use crate::request::{ScanError, ScanRequest};

fn diag(code: &str, level: DiagSeverity, location: String, detail: String) -> ScanDiagnostic {
    ScanDiagnostic { code: code.to_owned(), level, location, detail }
}

/// The suppressions file a request names, or the OS default.
fn suppressions_path(given: Option<&Path>) -> Option<PathBuf> {
    given.map(Path::to_path_buf).or_else(smd_policy::default_suppressions_path)
}

/// Score `findings` when the request asks for it; otherwise return them untouched with no
/// annotations, so unscored output is exactly what it was before policy existed.
pub(crate) fn apply(
    req: &ScanRequest,
    findings: Vec<ResolvedFinding>,
    diags: &mut Vec<ScanDiagnostic>,
) -> Result<(Vec<ResolvedFinding>, Vec<Annotation>), ScanError> {
    if !req.score {
        return Ok((findings, Vec::new()));
    }
    let policy = Policy::shipped().map_err(|e| ScanError::Policy(e.to_string()))?;
    let path = suppressions_path(req.suppressions.as_deref());
    // A bad suppressions file hides nothing: warn and score with none, never fail the scan.
    let supp = match path.as_deref().map(Suppressions::load).transpose() {
        Ok(s) => s.unwrap_or_default(),
        Err(e) => {
            let at = path.map(|p| p.display().to_string()).unwrap_or_default();
            diags.push(diag(
                "policy.suppressions_unreadable",
                DiagSeverity::Warn,
                at,
                e.to_string(),
            ));
            Suppressions::default()
        }
    };
    let scored = policy.score(findings, &supp);
    diags.extend(scored.unweighted.iter().map(|r| {
        let why = "the rule has no [base] weight in policy/weights.toml, so it scores 0";
        diag("policy.no_weight", DiagSeverity::Warn, r.clone(), why.to_owned())
    }));
    Ok((scored.findings, scored.annotations))
}

/// Add `id` to the suppressions file (`path`, or the OS default) and return the path
/// written.
///
/// # Errors
/// [`ScanError::Policy`] when the id is not a finding id, no config directory can be
/// named, or the file cannot be read or written.
pub fn suppress(id: &str, note: Option<String>, path: Option<&Path>) -> Result<PathBuf, ScanError> {
    let err = |e: smd_policy::PolicyError| ScanError::Policy(e.to_string());
    let path = suppressions_path(path).ok_or_else(|| {
        ScanError::Policy("no config directory (APPDATA / XDG_CONFIG_HOME / HOME unset)".into())
    })?;
    let next = Suppressions::load(&path).map_err(err)?.with(id, note).map_err(err)?;
    next.save(&path).map_err(err)?;
    Ok(path)
}
