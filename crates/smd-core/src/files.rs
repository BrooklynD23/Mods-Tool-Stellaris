//! Location and file identity.

use crate::ids::{PathId, SourceId};

/// Byte offsets into the **raw file as read from disk**: before BOM stripping, before any
/// encoding fallback, before newline normalisation.
///
/// Raw-file offsets let the UI seek to the exact bytes, and stay valid for a file that
/// had to be lossily decoded from Windows-1252 — the file most worth showing a user.
/// (REVIEW-2026-08 § G16: *all spans are raw-file byte offsets*.)
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Span {
    /// Byte offset of the first byte.
    pub offset: u32,
    /// Length in bytes.
    pub len: u32,
    /// 1-based, derived from the *decoded* text, and **advisory**: display only, never
    /// identity. Two spans that differ only in `line` still describe the same bytes.
    pub line: u32,
}

/// Where a fact or a finding can be verified: a mod, a file, and a byte range.
///
/// `Site` is what makes a finding verifiable, and a rule that emits a finding without at
/// least one `Site` fails a debug assertion.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Site {
    /// Which mod the bytes belong to.
    pub source: SourceId,
    /// Which file, mod-root-relative and normalised.
    pub path: PathId,
    /// Which bytes.
    pub span: Span,
}

impl Site {
    /// The sort projection for a site: `(source.index(), path, span.offset)`.
    ///
    /// Routed through [`SourceId::index`] because [`SourceId`] has no `Ord` — precedence
    /// is not identity, and a sort key must not pretend otherwise.
    #[must_use]
    pub const fn sort_key(&self) -> (u32, u32, u32) {
        (self.source.index(), self.path.0, self.span.offset)
    }
}

/// A content address: blake3 truncated to 128 bits, plus the length it was taken over.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Blob {
    /// blake3, truncated to 16 bytes.
    pub hash: [u8; 16],
    /// Length in bytes of the content that was hashed.
    pub len: u64,
}

/// What a file is, as far as dispatch is concerned. Assigned by the game profile.
///
/// Discriminants are explicit and part of the parse-cache key
/// ([`ExtractionClass::key_bytes`](crate::ExtractionClass::key_bytes)): a new kind takes
/// the next unused number and an existing number is never reused.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum FileKind {
    /// Clausewitz script.
    Script = 0,
    /// Paradox-flavoured YAML localisation.
    Localisation = 1,
    /// Binary or otherwise unparsed content: textures, models, sounds.
    Asset = 2,
    /// `descriptor.mod` or a launcher `*.mod`.
    Descriptor = 3,
    /// `.gui` interface definitions.
    Gui = 4,
    /// Not classified. The honest default; never a guess.
    #[default]
    Unknown = 5,
}

/// One file belonging to one mod, as the enumeration stage produces it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileEntry {
    /// Which mod ships it.
    pub source: SourceId,
    /// Normalised: lowercase, forward slashes, no leading slash, Unicode NFC,
    /// mod-root-relative. Normalisation lives in `smd-fs` and is non-negotiable —
    /// Windows and Linux mod archives disagree on case, and a case-sensitive comparison
    /// would report thousands of phantom non-conflicts on Linux.
    pub path: PathId,
    /// The as-shipped spelling, directory-relative, Unicode NFC only. **A sort key, never
    /// identity**: the FIOS/LIOS filename axis is an ordinal, case-sensitive comparison
    /// (`Z` sorts before `a`), and lowercasing into [`path`](FileEntry::path) destroys
    /// exactly the information it needs.
    pub raw_name: Box<str>,
    /// Content address.
    pub blob: Blob,
    /// Dispatch class.
    pub kind: FileKind,
}

/// How completely one file's facts were extracted. Cached beside the facts.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ParseCompleteness {
    /// Parsed end to end with no recovery.
    Complete,
    /// Parsed with recovery: the facts emitted are real, but facts may be missing.
    Partial,
    /// Contributes to the overlay but no facts were extracted.
    Unparsed(UnparsedReason),
    /// Hash-only by design (`parser = "none"`). Facts are not expected.
    NotApplicable,
}

/// Why a file contributes no facts.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum UnparsedReason {
    /// Larger than the size bound.
    Oversize,
    /// Could not be read.
    Unreadable,
    /// Its content changed while the scan ran.
    ChangedDuringScan,
    /// No parser accepts it.
    NoParser,
    /// The trust boundary declined to read it: a link or reparse point inside a root, a
    /// cloud placeholder that would need hydration, a non-regular file, a handle that
    /// resolved outside its root, or an unsafe archive entry (doc 05, Trust boundaries).
    Refused,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_sort_key_routes_through_index() {
        let s = Site {
            source: SourceId(3),
            path: PathId(9),
            span: Span { offset: 12, len: 4, line: 2 },
        };
        assert_eq!(s.sort_key(), (3, 9, 12));
    }

    #[test]
    fn file_kind_defaults_to_unknown() {
        assert_eq!(FileKind::default(), FileKind::Unknown);
    }
}
