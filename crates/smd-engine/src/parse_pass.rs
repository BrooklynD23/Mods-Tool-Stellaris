//! The parse stage (doc 06): after a source's walk and hash pass, read and parse every
//! file whose profile class is script, record its `ParseCompleteness` on the walk record,
//! and keep its `RawFact`s for the index.
//!
//! Sequential, like the rest of the engine. The cache is in memory for one scan, so its
//! value today is the vendored-copy case: identical bytes under one classification parse
//! once.

use std::io::ErrorKind;
use std::sync::Arc;

use smd_core::{
    FileKind, ModKey, ParseCompleteness, RawDiagnostic, RawFact, SourceId, UnparsedReason,
};
use smd_fs::{NormPath, ReadOnlyRoot, WalkOutcome, WalkRecord, codes};
use smd_parse::{ParseCache, ParseCacheKey, ParseInput, ParseOutput, parse_file, parser_for};
use smd_profiles::{DirClass, Profile};

use crate::output::ScanDiagnostic;
use crate::request::{ScanError, ScanRequest};

/// One parsed file's facts, unsited.
pub(crate) struct Parsed {
    pub source: SourceId,
    pub path: NormPath,
    pub facts: Vec<RawFact>,
}

/// What parsing one source produced.
#[derive(Default)]
pub(crate) struct SourceParse {
    pub parsed: Vec<Parsed>,
    pub diagnostics: Vec<ScanDiagnostic>,
    /// Files left `Partial`.
    pub partial: u32,
    /// Domain names holding a `Partial` or `Unparsed` script file.
    pub domains: Vec<Box<str>>,
}

/// Scan-wide inputs of the parse stage.
pub(crate) struct ParseCtx<'a> {
    pub profile: &'a Profile,
    pub cache: &'a ParseCache,
    pub req: &'a ScanRequest,
}

fn diag(key: &ModKey, path: &str, code: &str, detail: String) -> ScanDiagnostic {
    ScanDiagnostic {
        code: code.to_owned(),
        level: smd_core::DiagSeverity::Warn,
        location: path.to_owned(),
        detail: format!("mod {}: {detail}", key.as_str()),
    }
}

fn from_raw(key: &ModKey, path: &str, d: &RawDiagnostic) -> ScanDiagnostic {
    let at = if d.span.line > 0 { format!("{path}:{}", d.span.line) } else { path.to_owned() };
    ScanDiagnostic {
        code: d.code.to_string(),
        level: d.severity,
        location: at,
        detail: format!("mod {}: {}", key.as_str(), d.detail),
    }
}

/// Read one record's bytes, or the reason it stays unparsed.
fn read(
    root: &ReadOnlyRoot,
    rec: &WalkRecord,
    cap: u64,
) -> Result<Vec<u8>, (UnparsedReason, &'static str)> {
    if rec.size > cap {
        return Err((UnparsedReason::Oversize, codes::LIMIT_EXCEEDED));
    }
    let bytes = root.read_file(&rec.os_path, cap).map_err(|e| match e.kind() {
        ErrorKind::InvalidData => (UnparsedReason::ChangedDuringScan, codes::CHANGED_DURING_SCAN),
        ErrorKind::PermissionDenied => (UnparsedReason::Refused, codes::UNREADABLE),
        _ => (UnparsedReason::Unreadable, codes::UNREADABLE),
    })?;
    // The hash pass established these bytes' address; a different hash now means the file
    // changed in between, and its facts would not describe the bytes the overlay compared.
    let hash = smd_parse::extract::hash16(&bytes);
    if rec.blob.is_some_and(|b| b.hash != hash) {
        return Err((UnparsedReason::ChangedDuringScan, codes::CHANGED_DURING_SCAN));
    }
    Ok(bytes)
}

/// Parse `bytes` through the cache.
fn parse_cached(cx: &ParseCtx<'_>, bytes: &[u8], class: &DirClass) -> Arc<ParseOutput> {
    let cancelled = || cx.req.cancel.is_cancelled();
    let input = ParseInput { bytes, class, cancelled: &cancelled };
    let Some(parser) = parser_for(class.kind) else {
        return Arc::new(parse_file(&input));
    };
    let key = ParseCacheKey::new(
        smd_parse::extract::hash16(bytes),
        parser,
        cx.profile.content_hash(),
        &class.extraction_class(),
    );
    if let Some(hit) = cx.cache.get(&key) {
        return hit;
    }
    let out = Arc::new(parse_file(&input));
    if !cancelled() {
        cx.cache.insert(key, Arc::clone(&out));
    }
    out
}

/// Parse every script file of one walked source, in record order, updating each record's
/// completeness. Checks cancellation before every file.
pub(crate) fn parse_source(
    cx: &ParseCtx<'_>,
    root: &ReadOnlyRoot,
    outcome: &mut WalkOutcome,
    (source, key): (SourceId, &ModKey),
) -> Result<SourceParse, ScanError> {
    let cap = cx.req.limits.file_cap.min(smd_parse::FILE_CAP);
    let mut sp = SourceParse::default();
    for rec in &mut outcome.records {
        if cx.req.cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        let path = rec.path.as_str();
        // Refused, unreadable or changing files already carry their reason; benign files
        // never join the overlay.
        if rec.completeness != ParseCompleteness::NotApplicable || cx.profile.is_benign(path) {
            continue;
        }
        let class = cx.profile.dir_class(path);
        if class.kind != FileKind::Script {
            continue;
        }
        let out = match read(root, rec, cap) {
            Ok(bytes) => parse_cached(cx, &bytes, &class),
            Err((why, code)) => {
                sp.diagnostics.push(diag(key, path, code, format!("not parsed: {why:?}")));
                Arc::new(ParseOutput::unparsed(why))
            }
        };
        if cx.req.cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        record(&mut sp, rec, &class, &out, (source, key));
    }
    Ok(sp)
}

fn record(
    sp: &mut SourceParse,
    rec: &mut WalkRecord,
    class: &DirClass,
    out: &ParseOutput,
    (source, key): (SourceId, &ModKey),
) {
    rec.completeness = out.completeness;
    let path = rec.path.as_str();
    sp.diagnostics.extend(out.diags.iter().map(|d| from_raw(key, path, d)));
    if out.completeness == ParseCompleteness::Partial {
        sp.partial += 1;
    }
    if out.completeness != ParseCompleteness::Complete {
        sp.domains.extend(class.domain_name.clone());
    }
    if !out.facts.is_empty() {
        sp.parsed.push(Parsed { source, path: rec.path.clone(), facts: out.facts.clone() });
    }
}
