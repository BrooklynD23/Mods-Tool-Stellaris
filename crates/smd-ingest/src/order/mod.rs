//! Load-order sources and the fallback chain (doc 05, "Load order").
//!
//! A source answers two questions that fail independently: in what order do the mods
//! load, and which of them load at all. Each source therefore reports a ceiling for both,
//! and the chain reports which source answered with both certainties.

use std::collections::BTreeSet;

use smd_core::{
    Confidence, LoadOrderSource, ModKey, OrderError, Playset, ResolvedLocations, SnapshotMethod,
    SourceSpec,
};

use crate::codes::{self, IngestDiagnostic};
use crate::{Discovery, DiscoveryOptions};

mod directory_scan;
mod dlc_load;
mod json_caps;
pub mod launcher_v2;

pub use directory_scan::DirectoryScan;
pub use dlc_load::DlcLoadJson;
pub use launcher_v2::{LauncherV2Options, LauncherV2Sqlite};

/// What one source attempt produced: a playset or a reason, plus named diagnostics.
#[derive(Debug)]
pub struct SourceAttempt {
    /// The playset, or why the source could not answer.
    pub result: Result<Playset, OrderError>,
    /// Diagnostics the attempt raised, whether or not it answered.
    pub diagnostics: Vec<IngestDiagnostic>,
}

/// A [`LoadOrderSource`] that can also report its diagnostics. The trait method alone
/// cannot, and a failure that is not reported is exactly the silent demotion the design
/// forbids.
pub trait ChainSource: LoadOrderSource {
    /// Try to read a playset, keeping the diagnostics. Runs its own discovery.
    fn load(&self, loc: &ResolvedLocations) -> SourceAttempt;

    /// As [`load`](ChainSource::load), over a discovery the caller already ran, so one
    /// inventory sees one discovery however many sources consult it.
    fn load_with(&self, loc: &ResolvedLocations, found: &Discovery) -> SourceAttempt;
}

/// What the chain concluded.
#[derive(Debug)]
pub struct ChainOutcome {
    /// The playset of the source that answered (empty and `Fallback` when none did).
    pub playset: Playset,
    /// The registry key of the source that answered.
    pub order_source: &'static str,
    /// Every diagnostic raised on the way, in chain order.
    pub diagnostics: Vec<IngestDiagnostic>,
}

/// Sources tried in declaration order.
pub struct OrderChain {
    sources: Vec<Box<dyn ChainSource>>,
}

/// Everything the registry needs to construct sources.
#[derive(Clone, Debug, Default)]
pub struct SourceOptions {
    /// Discovery knobs, shared by every source.
    pub discovery: DiscoveryOptions,
    /// Launcher-database knobs.
    pub launcher: LauncherV2Options,
}

/// The registry: a source named, constructed.
#[must_use]
pub fn source_by_name(name: &str, opts: &SourceOptions) -> Option<Box<dyn ChainSource>> {
    match name {
        launcher_v2::ID => Some(Box::new(LauncherV2Sqlite::new(opts.clone()))),
        dlc_load::ID => Some(Box::new(DlcLoadJson::new(opts.discovery.clone()))),
        directory_scan::ID => Some(Box::new(DirectoryScan::new(opts.discovery.clone()))),
        _ => None,
    }
}

impl OrderChain {
    /// A chain of exactly these sources.
    #[must_use]
    pub fn new(sources: Vec<Box<dyn ChainSource>>) -> OrderChain {
        OrderChain { sources }
    }

    /// The default chain: launcher database, `dlc_load.json`, directory scan.
    #[must_use]
    pub fn standard(opts: &SourceOptions) -> OrderChain {
        let names = [launcher_v2::ID, dlc_load::ID, directory_scan::ID];
        let sources = names.iter().filter_map(|n| source_by_name(n, opts)).collect();
        OrderChain { sources }
    }

    /// Resolve a profile's named sources through the registry. An unknown name is a
    /// diagnostic, never silently skipped.
    #[must_use]
    pub fn from_specs(
        specs: &[SourceSpec],
        opts: &SourceOptions,
    ) -> (OrderChain, Vec<IngestDiagnostic>) {
        let mut sources = Vec::new();
        let mut diags = Vec::new();
        for spec in specs {
            match source_by_name(&spec.name, opts) {
                Some(s) => sources.push(s),
                None => diags.push(IngestDiagnostic::warn(
                    codes::UNKNOWN_ORDER_SOURCE,
                    &spec.name,
                    "no load-order source of this name is registered",
                )),
            }
        }
        (OrderChain { sources }, diags)
    }

    /// Try each source in order; the first to answer wins. Each source discovers for
    /// itself; prefer [`run_with`](OrderChain::run_with).
    #[must_use]
    pub fn run(&self, loc: &ResolvedLocations) -> ChainOutcome {
        self.run_over(loc, None)
    }

    /// As [`run`](OrderChain::run), every source reading the one `found` discovery.
    #[must_use]
    pub fn run_with(&self, loc: &ResolvedLocations, found: &Discovery) -> ChainOutcome {
        self.run_over(loc, Some(found))
    }

    fn run_over(&self, loc: &ResolvedLocations, found: Option<&Discovery>) -> ChainOutcome {
        let attempt_of = |s: &dyn ChainSource| match found {
            Some(f) => s.load_with(loc, f),
            None => s.load(loc),
        };
        let mut diagnostics = Vec::new();
        for (i, source) in self.sources.iter().enumerate() {
            let attempt = attempt_of(source.as_ref());
            diagnostics.extend(attempt.diagnostics);
            match attempt.result {
                Ok(playset) => {
                    Self::compare_activation(
                        source.id(),
                        &playset,
                        &self.sources[i + 1..],
                        &attempt_of,
                        &mut diagnostics,
                    );
                    return ChainOutcome { playset, order_source: source.id(), diagnostics };
                }
                Err(OrderError::NotPresent) => {}
                Err(e) => diagnostics.push(IngestDiagnostic::warn(
                    codes::ORDER_SOURCE_FAILED,
                    source.id(),
                    format!("{e:?}"),
                )),
            }
        }
        diagnostics.push(IngestDiagnostic::error(
            codes::NO_ORDER_SOURCE,
            "",
            "no load-order source answered",
        ));
        let playset = Playset {
            name: "".into(),
            entries: Vec::new(),
            order: Confidence::Fallback,
            activation: Confidence::Fallback,
            snapshot: SnapshotMethod::NotApplicable,
        };
        ChainOutcome { playset, order_source: "none", diagnostics }
    }

    /// If a later source also reads, and its enabled set differs, say so: the launcher
    /// database is what launches next, `dlc_load.json` what loaded last (VERIFY V08).
    /// The answering source's activation stands.
    fn compare_activation(
        winner: &'static str,
        won: &Playset,
        rest: &[Box<dyn ChainSource>],
        attempt_of: &dyn Fn(&dyn ChainSource) -> SourceAttempt,
        diags: &mut Vec<IngestDiagnostic>,
    ) {
        if won.activation == Confidence::Fallback {
            return;
        }
        for other in rest.iter().filter(|s| s.ceiling().1 != Confidence::Fallback) {
            let attempt = attempt_of(other.as_ref());
            diags.extend(attempt.diagnostics);
            let Ok(theirs) = attempt.result else { continue };
            let (a, b) = (enabled(won), enabled(&theirs));
            if a == b {
                continue;
            }
            let list =
                |s: &BTreeSet<&ModKey>| s.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ");
            let detail = format!(
                "enabled only per {winner}: [{}]; enabled only per {}: [{}]; {winner} is used",
                list(&a.difference(&b).copied().collect()),
                other.id(),
                list(&b.difference(&a).copied().collect()),
            );
            diags.push(IngestDiagnostic::warn(codes::ACTIVATION_DISAGREEMENT, "", detail));
        }
    }
}

fn enabled(p: &Playset) -> BTreeSet<&ModKey> {
    p.entries.iter().filter(|e| e.enabled).map(|e| &e.key).collect()
}

#[cfg(test)]
mod tests;
