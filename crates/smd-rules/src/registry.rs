//! The rule registry, as an explicit sorted function. See the crate docs for what replaces it.

use crate::Rule;
use crate::rules::{DepMissing, DepOrder, FileOverwrite, MetaDescriptorMismatch, MetaVersionDrift};

static FILE_OVERWRITE: FileOverwrite = FileOverwrite;
static DEP_MISSING: DepMissing = DepMissing;
static DEP_ORDER: DepOrder = DepOrder;
static META_DESCRIPTOR_MISMATCH: MetaDescriptorMismatch = MetaDescriptorMismatch;
static META_VERSION_DRIFT: MetaVersionDrift = MetaVersionDrift;

/// Every shipped rule, sorted by id.
#[must_use]
pub fn registry() -> Vec<&'static dyn Rule> {
    let mut v: Vec<&'static dyn Rule> = vec![&FILE_OVERWRITE];
    v.extend([
        &DEP_MISSING as &dyn Rule,
        &DEP_ORDER,
        &META_DESCRIPTOR_MISMATCH,
        &META_VERSION_DRIFT,
    ]);
    v.sort_by_key(|r| r.id());
    v
}
