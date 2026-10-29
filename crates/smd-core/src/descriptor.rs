//! Descriptors — the parsed shape of a `descriptor.mod` or a launcher `*.mod`.
//!
//! The *reading* of those files is `smd-ingest`'s job; the shape is a contract because
//! [`DescriptorReader`](crate::DescriptorReader) is declared here (ADR-0002 § 1) and
//! because rules report on descriptor disagreements.

use std::path::PathBuf;

use crate::version::VersionPattern;

/// One mod's declared metadata.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Descriptor {
    /// Display name. Dependencies are matched against this, fuzzily.
    pub name: Box<str>,
    /// `path="mod/example"`.
    pub path: Option<PathBuf>,
    /// `archive="mod/example.zip"`.
    pub archive: Option<PathBuf>,
    /// The workshop id, when there is one.
    pub remote_file_id: Option<Box<str>>,
    /// The mod's own version string, uninterpreted.
    pub version: Option<Box<str>>,
    /// `supported_version="3.13.*"`.
    pub supported_version: Option<VersionPattern>,
    /// Free-form category tags.
    pub tags: Vec<Box<str>>,
    /// Declared by *display name*, not by id, so matching is inherently fuzzy.
    pub dependencies: Vec<Box<str>>,
    /// **`replace_path` repeats.** A mod may declare several; a reader must accumulate,
    /// not overwrite.
    pub replace_paths: Vec<Box<str>>,
    /// Which of the two copies this is, and where it came from.
    pub source: DescriptorSource,
}

/// Which copy of the descriptor this is.
///
/// `.mod` and `descriptor.mod` can disagree — the launcher's copy goes stale. The
/// launcher copy determines *whether and where* the mod loads; the inner copy is the
/// author's intent. Both are captured; disagreement is itself a low-severity finding.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum DescriptorSource {
    /// The launcher's `*.mod` for a workshop subscription.
    Workshop,
    /// A `descriptor.mod` inside a local mod directory.
    Local,
    /// A `descriptor.mod` inside a zipped mod.
    Archive,
    /// Provenance not established.
    #[default]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_paths_accumulate_rather_than_overwrite() {
        let mut d = Descriptor::default();
        d.replace_paths.push("common/governments/civics".into());
        d.replace_paths.push("common/traits".into());
        assert_eq!(d.replace_paths.len(), 2);
        assert_eq!(d.source, DescriptorSource::Unknown);
    }
}
