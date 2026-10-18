//! The rule registry, as an explicit sorted function. See the crate docs for what replaces it.

use crate::Rule;
use crate::rules::FileOverwrite;

static FILE_OVERWRITE: FileOverwrite = FileOverwrite;

/// Every shipped rule, sorted by id.
#[must_use]
pub fn registry() -> Vec<&'static dyn Rule> {
    let mut v: Vec<&'static dyn Rule> = vec![&FILE_OVERWRITE];
    v.sort_by_key(|r| r.id());
    v
}
