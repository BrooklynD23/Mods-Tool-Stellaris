//! Windows entry classification as a pure function of attribute bits (H22), compiled and
//! unit-tested on every platform. Only the Windows backend feeds it real metadata.
//!
//! `surrogate` is std's `FileType::is_symlink()`, which on Windows is the reparse-tag
//! name-surrogate test: true for symlinks and junctions (they name another path) and false
//! for cloud placeholders, dedup and WOF files (they are the file itself). That is the
//! tag check, without `unsafe`.
// ponytail: the exact reparse tag is unreadable without unsafe, so a non-surrogate reparse
// point (dedup, WOF, hydrated placeholder, AppExecLink) is read as a regular file and an
// unknown tag that cannot be read fails loudly as `fs.unreadable`. Upgrade: read the tag.
#![cfg_attr(unix, allow(dead_code))]

use crate::sys::Kind;

pub(crate) const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
pub(crate) const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x4_0000;
pub(crate) const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

/// The facts about one entry, taken without following it.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Facts {
    pub attrs: u32,
    pub surrogate: bool,
}

/// Content lives in the cloud: reading it would download it.
pub(crate) fn is_offline(attrs: u32) -> bool {
    let bits = FILE_ATTRIBUTE_OFFLINE
        | FILE_ATTRIBUTE_RECALL_ON_OPEN
        | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS;
    attrs & bits != 0
}

pub(crate) fn kind_of(f: Facts) -> Kind {
    if f.surrogate {
        Kind::Link
    } else if f.attrs & FILE_ATTRIBUTE_DIRECTORY != 0 {
        Kind::Dir
    } else if is_offline(f.attrs) {
        Kind::Offline
    } else {
        Kind::File
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPARSE: u32 = 0x400;
    const SPARSE: u32 = 0x200;
    const fn f(attrs: u32, surrogate: bool) -> Facts {
        Facts { attrs, surrogate }
    }

    #[test]
    fn symlinks_and_junctions_are_links() {
        assert_eq!(kind_of(f(REPARSE, true)), Kind::Link);
        assert_eq!(kind_of(f(REPARSE | FILE_ATTRIBUTE_DIRECTORY, true)), Kind::Link);
    }

    #[test]
    fn dedup_and_hydrated_placeholder_files_are_regular() {
        assert_eq!(kind_of(f(REPARSE | SPARSE, false)), Kind::File);
        assert_eq!(kind_of(f(REPARSE | 0x8_0000, false)), Kind::File);
        assert_eq!(kind_of(f(0x20, false)), Kind::File);
    }

    #[test]
    fn offline_placeholders_are_never_read() {
        for bit in [
            FILE_ATTRIBUTE_OFFLINE,
            FILE_ATTRIBUTE_RECALL_ON_OPEN,
            FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
        ] {
            assert_eq!(kind_of(f(REPARSE | bit, false)), Kind::Offline);
        }
    }

    #[test]
    fn non_surrogate_directories_are_directories() {
        assert_eq!(kind_of(f(REPARSE | FILE_ATTRIBUTE_DIRECTORY, false)), Kind::Dir);
    }
}
