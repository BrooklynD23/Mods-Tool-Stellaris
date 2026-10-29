//! The `Rule` contract (doc 08 § The contract).

use std::panic::RefUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use smd_core::{Capabilities, EvidenceType, FeatureTag, Finding, RuleId};
use smd_index::ScanIndex;

use crate::case::RuleCase;

/// Cooperative cancellation. Cloning shares the flag.
///
/// Declared here because no lower crate owns one yet; `smd-engine` may substitute its own
/// when the process architecture (doc 04) lands.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// A token that has not been cancelled.
    #[must_use]
    pub fn new() -> CancellationToken {
        CancellationToken::default()
    }

    /// Request cancellation.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether cancellation was requested. One atomic load.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// What a rule can see: the index by shared reference and a cancellation flag. No handle
/// to the registry, so a rule cannot see another rule.
pub struct RuleCtx<'a> {
    /// The immutable index.
    pub index: &'a ScanIndex,
    /// Check once per outer-loop iteration.
    pub cancel: &'a CancellationToken,
}

/// A rule is a pure function of `&ScanIndex` into `Vec<Finding>`.
///
/// `RefUnwindSafe` is a supertrait so that the unwind boundary in [`run_one`](crate::run_one)
/// needs no `AssertUnwindSafe`: the contract is that a rule holds no state a panic could
/// leave half-written.
pub trait Rule: Sync + RefUnwindSafe {
    /// Stable, namespaced, never reused.
    fn id(&self) -> RuleId;

    /// Which index structures must be built for this rule to run.
    fn needs(&self) -> Capabilities;

    /// Which semantic features the game profile must declare. Empty = any game.
    fn requires(&self) -> &'static [FeatureTag] {
        &[]
    }

    /// Every evidence key this rule may emit, and its type. Nothing else is permitted.
    fn evidence(&self) -> &'static [(&'static str, EvidenceType)];

    /// The fixtures this rule is defined by. No default: a rule with no cases is not a rule.
    fn cases(&self) -> &[RuleCase];

    /// Detect. Must not set `score` or `severity`.
    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding>;
}
