//! Reading and hashing one opened handle, with the pre/post-read stamp check.

use smd_core::{Blob, UnparsedReason};

use crate::hashing::hash_reader;
use crate::sys::{OpenedFile, restamp};

#[cfg(test)]
type Hook = Box<dyn Fn(&str)>;

#[cfg(test)]
thread_local! {
    /// Test-only hook run after the pre-read stat and before the read.
    pub(crate) static BEFORE_READ: std::cell::RefCell<Option<Hook>> =
        const { std::cell::RefCell::new(None) };
}

/// Read and hash an opened handle; `Err` is why it is `Unparsed`.
pub(super) fn hash_open(o: &OpenedFile, rel: &str) -> Result<Blob, UnparsedReason> {
    #[cfg(test)]
    BEFORE_READ.with(|h| {
        if let Some(f) = h.borrow().as_ref() {
            f(rel);
        }
    });
    #[cfg(not(test))]
    let _ = rel;
    let (blob, more) =
        hash_reader(&o.file, o.stamp.size).map_err(|_| UnparsedReason::Unreadable)?;
    let after = restamp(&o.file).map_err(|_| UnparsedReason::Unreadable)?;
    if more || blob.len != o.stamp.size || after != o.stamp {
        return Err(UnparsedReason::ChangedDuringScan);
    }
    Ok(blob)
}
