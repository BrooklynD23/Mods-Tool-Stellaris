//! Reading descriptors off disk, through `smd-fs`, into discovery candidates.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use smd_core::{
    Descriptor, DescriptorReader, DescriptorSource, Diagnostic, PathId, Site, SourceId, SourceRole,
    Span,
};
use smd_fs::ReadOnlyRoot;

use super::listing::{self, Entry, Kind};
use super::refuse::Guard;
use super::{ContentState, UnreadReason};
use crate::codes::{self, IngestDiagnostic};
use crate::descriptor::{FILE_MAX, KeyValuesReader};
use smd_core::DiagSeverity;

/// A mod found on disk, before its key and `SourceId` are assigned.
pub(super) struct Raw {
    pub descriptor: Descriptor,
    pub diagnostics: Vec<Diagnostic>,
    pub dir_name: Box<str>,
    pub mod_file: Option<Box<str>>,
    pub role: SourceRole,
    pub content_root: Option<PathBuf>,
    pub content: ContentState,
}

/// Where a descriptor is resolved from, and what it must not swallow.
pub(super) struct Ctx<'a> {
    pub guard: &'a Guard,
    pub user_dir: &'a Path,
}

/// The `SourceId` descriptor diagnostics carry until ids are assigned.
pub(crate) const UNASSIGNED: SourceId = SourceId(u32::MAX);

fn site() -> Site {
    Site { source: UNASSIGNED, path: PathId(0), span: Span::default() }
}

pub(super) fn is_invalid(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.code == codes::DESCRIPTOR_INVALID)
}

/// What discovery noticed along the way: diagnostics, and whether anything could have been
/// missed.
#[derive(Default)]
pub(super) struct Sink {
    pub diags: Vec<IngestDiagnostic>,
    pub incomplete: bool,
}

/// Read a descriptor of `len` bytes (as listed). One over [`FILE_MAX`] is never read: it is
/// rejected whole as `ingest.descriptor_invalid`, exactly as the reader rejects one it was
/// handed.
pub(super) fn read_descriptor(
    root: &ReadOnlyRoot,
    os_path: &[OsString],
    len: u64,
) -> Result<(Descriptor, Vec<Diagnostic>), String> {
    if len > FILE_MAX as u64 {
        let why = format!("descriptor is {len} bytes, over the {FILE_MAX}-byte cap");
        let d = Diagnostic {
            site: site(),
            code: codes::DESCRIPTOR_INVALID,
            severity: DiagSeverity::Error,
            detail: why.into(),
        };
        return Ok((Descriptor::default(), vec![d]));
    }
    let bytes = root.read_file(os_path, FILE_MAX as u64).map_err(|e| e.to_string())?;
    Ok(KeyValuesReader.read(&bytes, site()))
}

pub(super) fn unread(dir_name: &str, mod_file: Option<&str>, why: UnreadReason) -> Raw {
    Raw {
        descriptor: Descriptor::default(),
        diagnostics: Vec::new(),
        dir_name: dir_name.into(),
        mod_file: mod_file.map(Into::into),
        role: SourceRole::Local,
        content_root: None,
        content: ContentState::Unread(why),
    }
}

fn stem(name: &str) -> &str {
    name.strip_suffix(".mod").or_else(|| name.strip_suffix(".MOD")).unwrap_or(name)
}

/// Every `*.mod` directly under `<user_dir>/mod`, in name order.
pub(super) fn user_mods(ctx: &Ctx<'_>, sink: &mut Sink) -> Vec<Raw> {
    let dir = ctx.user_dir.join(super::MODS_SUBDIR);
    let root = match ReadOnlyRoot::open(&dir) {
        Ok(r) => r,
        Err(e) => {
            open_failed(&dir, super::MODS_SUBDIR, &e, sink);
            return Vec::new();
        }
    };
    let listed =
        listing::list(root.path(), super::MODS_SUBDIR, |n| n.ends_with(".mod"), &mut sink.diags);
    sink.incomplete |= listed.incomplete;
    let mut out = Vec::new();
    for e in listed.entries.iter().filter(|e| e.kind == Kind::File) {
        let file = e.text();
        if file.ends_with(".mod") {
            out.push(one_mod_file(ctx, &root, e, &file, sink));
        }
    }
    out
}

/// A location that could not be opened: absent is a named diagnostic, anything else is
/// also an incomplete scan.
pub(super) fn open_failed(dir: &Path, subject: &str, e: &std::io::Error, sink: &mut Sink) {
    if e.kind() == std::io::ErrorKind::NotFound {
        let why = format!("{} does not exist", dir.display());
        sink.diags.push(IngestDiagnostic::warn(codes::LOCATION_MISSING, subject, why));
    } else {
        let why = format!("{} cannot be opened: {e}", dir.display());
        sink.diags.push(IngestDiagnostic::warn(codes::DISCOVERY_INCOMPLETE, subject, why));
        sink.incomplete = true;
    }
}

fn one_mod_file(
    ctx: &Ctx<'_>,
    root: &ReadOnlyRoot,
    entry: &Entry,
    file: &str,
    sink: &mut Sink,
) -> Raw {
    let name = stem(file);
    let os_path = [entry.name.clone()];
    let (desc, dd) = match read_descriptor(root, &os_path, entry.len) {
        Ok(x) => x,
        Err(e) => {
            sink.diags.push(IngestDiagnostic::warn(codes::DESCRIPTOR_UNREADABLE, file, e));
            return unread(name, Some(file), UnreadReason::DescriptorUnreadable);
        }
    };
    if is_invalid(&dd) {
        let mut raw = unread(name, Some(file), UnreadReason::DescriptorInvalid);
        raw.diagnostics = dd;
        return raw;
    }
    let (role, content_root, content) = locate(ctx, &desc, file, sink);
    let source = match role {
        SourceRole::Archive => DescriptorSource::Archive,
        SourceRole::Workshop => DescriptorSource::Workshop,
        _ => DescriptorSource::Local,
    };
    let descriptor = Descriptor { source, ..desc };
    Raw {
        descriptor,
        diagnostics: dd,
        dir_name: name.into(),
        mod_file: Some(file.into()),
        role,
        content_root,
        content,
    }
}

/// Resolve `path=` (or `archive=`) once and decide whether the content may be walked.
fn locate(
    ctx: &Ctx<'_>,
    desc: &Descriptor,
    subject: &str,
    sink: &mut Sink,
) -> (SourceRole, Option<PathBuf>, ContentState) {
    let archive_only = desc.path.is_none() && desc.archive.is_some();
    let role = if archive_only {
        SourceRole::Archive
    } else if desc.remote_file_id.is_some() {
        SourceRole::Workshop
    } else {
        SourceRole::Local
    };
    let Some(declared) = desc.path.as_ref().or(desc.archive.as_ref()) else {
        let d =
            IngestDiagnostic::warn(codes::MOD_PATH_MISSING, subject, "names no path or archive");
        sink.diags.push(d);
        return (role, None, ContentState::Unread(UnreadReason::NoPath));
    };
    let resolved = match ctx.guard.resolve(ctx.user_dir, declared) {
        Ok(p) => p,
        Err(why) => {
            sink.diags.push(IngestDiagnostic::error(codes::ROOT_REFUSED, subject, why));
            return (role, None, ContentState::Unread(UnreadReason::RootRefused));
        }
    };
    if archive_only {
        return (role, None, ContentState::Unread(UnreadReason::Archive));
    }
    if resolved.is_dir() {
        return (role, Some(resolved), ContentState::Readable);
    }
    let why = format!("{} is not a directory", resolved.display());
    sink.diags.push(IngestDiagnostic::warn(codes::MOD_PATH_MISSING, subject, why));
    (role, None, ContentState::Unread(UnreadReason::PathMissing))
}
