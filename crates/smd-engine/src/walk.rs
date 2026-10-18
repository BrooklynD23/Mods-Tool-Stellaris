//! Walking every source that may contribute, and folding what the walks saw into one
//! [`ScanCompleteness`].

use smd_core::{Activation, ContentIncompleteReason, ModKey, ScanCompleteness};
use smd_fs::ReadOnlyRoot;
use smd_index::{FileRecord, SourceInput};
use smd_ingest::{ContentState, Inventory, ModRecord};
use smd_profiles::Profile;

use crate::output::ScanDiagnostic;
use crate::request::{ScanError, ScanRequest};
use crate::rows;

/// Everything the walks produced.
pub(crate) struct Walked {
    pub inputs: Vec<SourceInput>,
    pub completeness: ScanCompleteness,
    pub diagnostics: Vec<ScanDiagnostic>,
}

/// Add `part` into `total`: counts add, incomplete sources concatenate.
pub(crate) fn absorb(total: &mut ScanCompleteness, part: ScanCompleteness) {
    total.files_total = total.files_total.saturating_add(part.files_total);
    total.partial = total.partial.saturating_add(part.partial);
    for (why, n) in part.unparsed {
        match total.unparsed.iter_mut().find(|(w, _)| *w == why) {
            Some((_, have)) => *have = have.saturating_add(n),
            None => total.unparsed.push((why, n)),
        }
    }
    total.unparsed.sort();
    total.domains.extend(part.domains);
    total.content_incomplete.extend(part.content_incomplete);
}

/// Walk each source that is not `Inactive` and has a readable content root, in `SourceId`
/// order, checking the cancellation token before each. Vanilla is walked from the
/// installed tree like any other source; no profile data stands in for it.
pub(crate) fn walk_sources(
    inv: &Inventory,
    profile: &Profile,
    req: &ScanRequest,
) -> Result<Walked, ScanError> {
    let mut w = Walked {
        inputs: Vec::new(),
        completeness: ScanCompleteness::default(),
        diagnostics: Vec::new(),
    };
    for rec in &inv.records {
        if req.cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        let inactive =
            inv.table.get(rec.source).is_none_or(|m| m.activation == Activation::Inactive);
        let (ContentState::Readable, Some(root), false) =
            (rec.content, rec.content_root.as_deref(), inactive)
        else {
            continue;
        };
        match ReadOnlyRoot::open(root) {
            Ok(handle) => walk_one(&mut w, rec, &handle, profile, req),
            Err(e) => unopened(&mut w, &rec.key, &e),
        }
    }
    Ok(w)
}

fn unopened(w: &mut Walked, key: &ModKey, e: &std::io::Error) {
    let detail = format!("content root could not be opened: {e}");
    let entry = (key.clone(), ContentIncompleteReason::Truncated, Some(detail.clone().into()));
    w.completeness.content_incomplete.push(entry);
    w.diagnostics.push(ScanDiagnostic {
        code: smd_fs::codes::UNREADABLE.to_owned(),
        level: smd_core::DiagSeverity::Error,
        location: key.as_str().to_owned(),
        detail,
    });
}

fn walk_one(w: &mut Walked, rec: &ModRecord, root: &ReadOnlyRoot, p: &Profile, r: &ScanRequest) {
    let outcome = root.walk(&r.limits);
    absorb(&mut w.completeness, outcome.completeness(&rec.key));
    w.diagnostics.extend(outcome.diagnostics.iter().map(|d| rows::from_fs(rec.key.as_str(), d)));
    // Benign paths (descriptors, thumbnails, readmes) are counted above but never become
    // overlay contributions: a shared `descriptor.mod` is not a conflict.
    let records = outcome
        .records
        .iter()
        .filter(|f| !p.is_benign(f.path.as_str()))
        .map(FileRecord::from)
        .collect();
    let replace_paths = rec.descriptor.replace_paths.iter().map(ToString::to_string).collect();
    w.inputs.push(SourceInput { source: rec.source, records, replace_paths });
}
