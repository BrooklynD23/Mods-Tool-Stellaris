//! Dependency resolution by display name, and the inner `descriptor.mod` copy.

use std::ffi::OsString;

use smallvec::SmallVec;
use smd_core::{
    Descriptor, DescriptorReader, PathId, ResolvedDependency, Site, SourceId, SourceMeta, Span,
};
use smd_fs::ReadOnlyRoot;

use crate::descriptor::{FILE_MAX, KeyValuesReader};
use crate::{ContentState, DiscoveredMod};

/// The file name of the in-content descriptor.
const INNER: &str = "descriptor.mod";

/// The content directory's own `descriptor.mod`, when the mod was found through a launcher
/// `*.mod` (so this is a second copy) and its content root is readable. Read-only and
/// capped; any read or parse problem is `None` — no claim, never a mismatch.
pub(super) fn inner_descriptor(m: &DiscoveredMod, id: SourceId) -> Option<Descriptor> {
    if m.mod_file.is_none() || m.content != ContentState::Readable {
        return None;
    }
    let root = ReadOnlyRoot::open(m.content_root.as_ref()?).ok()?;
    let bytes = root.read_file(&[OsString::from(INNER)], FILE_MAX as u64).ok()?;
    let site = Site { source: id, path: PathId(0), span: Span::default() };
    let (desc, diags) = KeyValuesReader.read(&bytes, site);
    diags.is_empty().then_some(desc)
}

/// Every source's declared dependencies, matched against the other sources by display
/// name: exact first, then trimmed and case-insensitive. One match resolves; several are
/// `Missing` with every match as a candidate (ambiguous, not absent); none is `Missing`
/// with no candidates. Vanilla is never a match and never declares any.
///
/// ponytail: no fuzzy scoring; candidates are all-or-nothing at 100. Add edit-distance
/// candidates when a corpus shows renamed dependencies matter.
pub(super) fn resolve_deps(
    metas: &[SourceMeta],
    declared: &[&[Box<str>]],
) -> Vec<Vec<ResolvedDependency>> {
    (0u32..)
        .zip(declared)
        .map(|(i, names)| names.iter().map(|n| resolve_one(metas, SourceId(i), n)).collect())
        .collect()
}

fn resolve_one(metas: &[SourceMeta], me: SourceId, name: &str) -> ResolvedDependency {
    let others = || {
        (0u32..)
            .zip(metas)
            .skip(1)
            .filter(move |(i, _)| *i != me.index())
            .map(|(i, m)| (SourceId(i), &*m.name))
    };
    let mut hits: Vec<SourceId> = others().filter(|(_, n)| *n == name).map(|(s, _)| s).collect();
    if hits.is_empty() {
        let want = name.trim().to_lowercase();
        hits = others().filter(|(_, n)| n.trim().to_lowercase() == want).map(|(s, _)| s).collect();
    }
    let declared: Box<str> = name.into();
    match hits.as_slice() {
        [one] => ResolvedDependency::Resolved { declared, source: *one },
        _ => {
            let candidates: SmallVec<[(SourceId, u8); 2]> =
                hits.iter().map(|s| (*s, 100)).collect();
            ResolvedDependency::Missing { declared, candidates }
        }
    }
}
