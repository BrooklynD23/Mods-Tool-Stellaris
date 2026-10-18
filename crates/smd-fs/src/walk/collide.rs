//! Marking entries that share an identity key.

use smd_core::DiagSeverity;

use crate::record::{FsDiagnostic, WalkOutcome, codes};

pub(super) fn mark(out: &mut WalkOutcome) {
    let recs = &mut out.records;
    let mut i = 0;
    while i < recs.len() {
        let run = recs[i..].iter().take_while(|r| r.path == recs[i].path).count();
        if run > 1 {
            for r in &mut recs[i..i + run] {
                r.ambiguous = true;
            }
            let key = recs[i].path.as_str().into();
            let detail = format!("{run} entries share this identity key").into();
            out.diagnostics.push(FsDiagnostic {
                code: codes::IDENTITY_COLLISION,
                severity: DiagSeverity::Warn,
                path: key,
                detail,
            });
        }
        i += run;
    }
}
