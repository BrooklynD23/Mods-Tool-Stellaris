//! What a walk returns: per-file records, diagnostics, and completeness.

use std::ffi::OsString;

use smd_core::{
    Blob, ContentIncompleteReason, DiagSeverity, ModKey, ParseCompleteness, ScanCompleteness,
    UnparsedReason,
};

use crate::normpath::NormPath;

/// One enumerated file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WalkRecord {
    /// Identity key.
    pub path: NormPath,
    /// The final path component as shipped: NFC only, case preserved. A sort key, never
    /// identity. For a non-Unicode name this is the NFC of the lossy decoding.
    pub raw_name: Box<str>,
    /// The root-relative path as raw OS components. This, not `path`, is what opens the
    /// file ([`ReadOnlyRoot::read_file`](crate::ReadOnlyRoot::read_file)).
    pub os_path: Vec<OsString>,
    /// Content address; `None` unless the file was fully read and unchanged.
    pub blob: Option<Blob>,
    /// `NotApplicable` for a hashed file until a parser exists; `Unparsed(_)` otherwise.
    pub completeness: ParseCompleteness,
    /// Another entry of the same source has the same identity key.
    pub ambiguous: bool,
    /// The name is not valid Unicode (or holds a `\`), so `path` is a lossy key.
    pub unrepresentable: bool,
}

impl WalkRecord {
    /// The raw OS name of the final component.
    #[must_use]
    pub fn os_name(&self) -> &std::ffi::OsStr {
        self.os_path.last().map_or(std::ffi::OsStr::new(""), |s| s.as_os_str())
    }
}

/// A diagnostic in `smd_core::Diagnostic`-compatible form. `smd-fs` has no `SourceId` or
/// `PathId`, so it carries the path text; ingest turns this into a sited `Diagnostic`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FsDiagnostic {
    /// Stable code, e.g. `fs.link_skipped`.
    pub code: &'static str,
    /// How badly.
    pub severity: DiagSeverity,
    /// Root-relative path (lossy for non-Unicode names).
    pub path: Box<str>,
    /// Human-readable specifics.
    pub detail: Box<str>,
}

/// Diagnostic codes `smd-fs` emits.
pub mod codes {
    /// A link inside the root was not followed.
    pub const LINK_SKIPPED: &str = "fs.link_skipped";
    /// A FIFO, socket or device was not opened.
    pub const SPECIAL_SKIPPED: &str = "fs.special_skipped";
    /// A handle resolved outside the root (or onto another mount).
    pub const ESCAPED_ROOT: &str = "fs.escaped_root";
    /// Two entries of one source share an identity key.
    pub const IDENTITY_COLLISION: &str = "fs.identity_collision";
    /// A name is not valid Unicode and took a lossy key.
    pub const UNREPRESENTABLE_NAME: &str = "fs.unrepresentable_name";
    /// An enumeration or file-size bound tripped.
    pub const LIMIT_EXCEEDED: &str = "fs.limit_exceeded";
    /// A file could not be opened or read.
    pub const UNREADABLE: &str = "fs.unreadable";
    /// A file changed between the pre-read and post-read stat.
    pub const CHANGED_DURING_SCAN: &str = "fs.changed_during_scan";
    /// A cloud placeholder that would need hydration (Windows).
    pub const CLOUD_PLACEHOLDER: &str = "fs.cloud_placeholder";
}

/// A reason the source's tree was not fully enumerated, with where reading stopped.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Incomplete {
    /// `Truncated` or `LinkRefused`.
    pub reason: ContentIncompleteReason,
    /// Root-relative path of the refused link or tripped bound.
    pub path: Option<Box<str>>,
}

/// Everything one walk produced. Records are sorted by [`NormPath`] bytes (ties by raw
/// OS path); diagnostics and incompleteness are in deterministic order too.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WalkOutcome {
    /// One record per file-like entry.
    pub records: Vec<WalkRecord>,
    /// Diagnostics, sorted by `(path, code)`.
    pub diagnostics: Vec<FsDiagnostic>,
    /// Content-incomplete markers (doc 05; H50 fs half).
    pub content_incomplete: Vec<Incomplete>,
}

impl WalkOutcome {
    /// This walk's contribution to the scan-wide [`ScanCompleteness`] for source `key`.
    /// `partial` and `domains` are the parser's to fill.
    #[must_use]
    pub fn completeness(&self, key: &ModKey) -> ScanCompleteness {
        let mut c = ScanCompleteness {
            files_total: u32::try_from(self.records.len()).unwrap_or(u32::MAX),
            ..ScanCompleteness::default()
        };
        let mut counts: Vec<(UnparsedReason, u32)> = Vec::new();
        for r in &self.records {
            if let ParseCompleteness::Unparsed(why) = r.completeness {
                match counts.iter_mut().find(|(w, _)| *w == why) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((why, 1)),
                }
            }
        }
        counts.sort();
        c.unparsed.extend(counts);
        for i in &self.content_incomplete {
            c.content_incomplete.push((key.clone(), i.reason, i.path.clone()));
        }
        c
    }
}
