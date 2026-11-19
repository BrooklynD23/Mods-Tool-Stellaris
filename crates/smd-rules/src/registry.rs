//! The rule registry, as an explicit sorted function. See the crate docs for what replaces it.

use crate::Rule;
use crate::rules::{FileOverwrite, SymbolCollision, SymbolDeleted};

static FILE_OVERWRITE: FileOverwrite = FileOverwrite;
static SYMBOL_COLLISION: SymbolCollision = SymbolCollision;
static SYMBOL_DELETED: SymbolDeleted = SymbolDeleted;

/// Every shipped rule, sorted by id.
#[must_use]
pub fn registry() -> Vec<&'static dyn Rule> {
    let mut v: Vec<&'static dyn Rule> = vec![&FILE_OVERWRITE, &SYMBOL_COLLISION, &SYMBOL_DELETED];
    v.sort_by_key(|r| r.id());
    v
}
