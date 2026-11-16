//! Precedence and contenders — the data types only.

use crate::ids::{PathId, SourceId};

/// Who a resolution is about: a file, or one definition inside one file.
///
/// A [`SourceId`] alone cannot say which of a mod's two trait files won.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Contender {
    /// The mod.
    pub source: SourceId,
    /// The file.
    pub path: PathId,
    /// Which definition within the file, when the subject is a key rather than a path.
    pub slot: Option<u32>,
}

/// Whether the last or the first ordinal filename wins in a directory.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum FileOrder {
    /// Last in ordinal filename order wins.
    Lios,
    /// First in ordinal filename order wins.
    Fios,
}

/// The filename axis of [`Precedence`]: `FileEntry::raw_name`, compared ordinally (byte
/// order, case-sensitive).
///
/// **Provisional.** Whether the key is the basename without extension, the basename, or
/// the directory-relative path is unestablished (`ResolutionGap::FilenameKeyForm`); this
/// type holds whichever form the index chose and will be refined with Stage D (M3).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct FileKey(pub Box<str>);

/// A reference into the source table's `rank`, rather than a copy of it.
///
/// **Provisional**, for the same reason as [`FileKey`]: the comparison that reads it lands
/// with Stage D.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct RankRef(pub SourceId);

/// The precedence tuple of one definition site (or one file, for Stage A). Compared
/// component by component, first difference decides. Built per site by `smd-index`; never
/// stored in a fact.
///
/// **Data only here.** `Precedence::compare` takes `smd-index`'s `DirRules`, which
/// `smd-core` cannot name, so it lands with Stage D (M3) in `smd-index`. It returns `None`
/// where the comparison reaches an unknown component, never a guess.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Precedence {
    /// Path-class tier; higher wins regardless of everything below.
    pub tier: u8,
    /// The filename axis. Direction is the directory's [`FileOrder`].
    pub file: FileKey,
    /// Load order: the source's `rank`. Reached only when `tier` and `file` tie.
    pub rank: RankRef,
    /// Position within the file, for the same key defined twice in one file.
    pub ordinal: u32,
}
