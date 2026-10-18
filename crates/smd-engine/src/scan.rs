//! The scan pipeline: ingest, walk, index, rules, certainty, text.
//!
//! No domain rule lives here. The engine wires stages together; what a conflict is, is the
//! rules'; what a directory means, is the profile's.

use std::collections::BTreeSet;

use smd_core::{DiagSeverity, ModKey, ResolvedFinding, ResolvedLocations};
use smd_index::{IndexBuilder, OverlayRules, ReplacePathScope, ScanIndex};
use smd_ingest::{
    DiscoveryOptions, Inventory, KEY_INSTALL, KEY_USER_DIR, KEY_WORKSHOP, OrderChain,
    SourceOptions, inventory,
};
use smd_profiles::Profile;
use smd_rules::{Rule, RuleCtx, run_all};

use crate::certainty::certainty_of;
use crate::output::{ScanDiagnostic, ScanOutput};
use crate::request::{ScanError, ScanRequest};
use crate::rows;
use crate::walk::{absorb, walk_sources};

/// Run one scan.
///
/// # Errors
/// [`ScanError`] when a root cannot be read, the request asks for something unsupported,
/// or the cancellation token fires. A scan that ran is always `Ok`, however incomplete;
/// its completeness says how much.
pub fn scan(req: &ScanRequest) -> Result<ScanOutput, ScanError> {
    let profile = Profile::shipped_default().map_err(ScanError::Profile)?;
    let loc = locations(req)?;
    let disc = DiscoveryOptions { home: req.home.clone() };
    let opts = SourceOptions { discovery: disc.clone(), ..SourceOptions::default() };
    let (chain, mut diags) = from_profile(&profile, &opts);
    let inv = inventory(&loc, &chain, &disc);
    diags.extend(inv.diagnostics.iter().map(rows::from_ingest));
    cancelled(req)?;
    let walked = walk_sources(&inv, &profile, req)?;
    diags.extend(walked.diagnostics);
    let mut completeness = inv.completeness.clone();
    absorb(&mut completeness, walked.completeness);
    if inv.discovery_incomplete {
        // Not a mod: smd-core has no discovery-incomplete field or Unreadable reason yet,
        // so this rides content_incomplete under a synthetic key and says so in the text.
        let key = ModKey("(discovery)".into());
        let why = Some(
            "not a mod: mod discovery was incomplete, so mods may be missing from this scan".into(),
        );
        completeness.content_incomplete.push((
            key,
            smd_core::ContentIncompleteReason::Truncated,
            why,
        ));
    }
    let index = build_index(&inv, &profile, walked.inputs);
    let incomplete: BTreeSet<ModKey> =
        completeness.content_incomplete.iter().map(|(k, _, _)| k.clone()).collect();
    let findings = run_rules(&index, req, &incomplete, &mut diags)?;
    diags.sort_by(|a, b| (&a.location, &a.code, &a.detail).cmp(&(&b.location, &b.code, &b.detail)));
    Ok(ScanOutput {
        provenance: inv.provenance.clone(),
        completeness,
        inventory: rows::inventory_rows(&inv),
        findings,
        diagnostics: diags,
    })
}

fn cancelled(req: &ScanRequest) -> Result<(), ScanError> {
    if req.cancel.is_cancelled() { Err(ScanError::Cancelled) } else { Ok(()) }
}

fn locations(req: &ScanRequest) -> Result<ResolvedLocations, ScanError> {
    if req.workshop_dirs.len() > 1 {
        return Err(ScanError::Unsupported("more than one workshop directory"));
    }
    let check = |role: &'static str, path: &std::path::Path| {
        smd_fs::ReadOnlyRoot::open(path).map(|_| ()).map_err(|e| ScanError::RootUnreadable {
            role,
            path: path.to_path_buf(),
            reason: e.to_string(),
        })
    };
    check("game", &req.game_dir)?;
    check("user", &req.user_dir)?;
    let mut loc = ResolvedLocations::new();
    loc.insert(KEY_INSTALL, req.game_dir.clone());
    loc.insert(KEY_USER_DIR, req.user_dir.clone());
    for ws in &req.workshop_dirs {
        check("workshop", ws)?;
        loc.insert(KEY_WORKSHOP, ws.clone());
    }
    Ok(loc)
}

fn from_profile(profile: &Profile, opts: &SourceOptions) -> (OrderChain, Vec<ScanDiagnostic>) {
    let (chain, diags) = OrderChain::from_specs(&profile.load_order_sources(), opts);
    (chain, diags.iter().map(rows::from_ingest).collect())
}

fn build_index(
    inv: &Inventory,
    profile: &Profile,
    inputs: Vec<smd_index::SourceInput>,
) -> ScanIndex {
    let scope = match profile.replace_path_scope() {
        smd_profiles::ReplacePathScope::VanillaOnly => ReplacePathScope::VanillaOnly,
        smd_profiles::ReplacePathScope::VanillaAndEarlier => ReplacePathScope::VanillaAndEarlier,
        _ => ReplacePathScope::Unknown,
    };
    let shared = profile.clone();
    let rules = OverlayRules::new(scope, move |path| shared.classify(path));
    let mut builder = IndexBuilder::new(inv.table.clone(), inv.provenance.clone(), rules);
    for input in inputs {
        builder.add_source(input);
    }
    builder.build()
}

fn run_rules(
    index: &ScanIndex,
    req: &ScanRequest,
    incomplete: &BTreeSet<ModKey>,
    diags: &mut Vec<ScanDiagnostic>,
) -> Result<Vec<ResolvedFinding>, ScanError> {
    let all = smd_rules::registry();
    let (runnable, skipped): (Vec<&dyn Rule>, Vec<&dyn Rule>) =
        all.into_iter().partition(|r| r.requires().is_empty());
    diags.extend(skipped.iter().map(|r| ScanDiagnostic {
        code: "engine.rule_skipped".to_owned(),
        level: DiagSeverity::Warn,
        location: r.id().to_string(),
        detail: "the profile declares no semantic features yet, so this rule did not run".into(),
    }));
    let out = run_all(&runnable, &RuleCtx { index, cancel: &req.cancel });
    if out.cancelled {
        return Err(ScanError::Cancelled);
    }
    diags.extend(out.diagnostics.iter().map(rows::from_core));
    let mut found = out.findings;
    for f in &mut found {
        f.certainty = certainty_of(index, f, incomplete);
    }
    let mut resolved: Vec<ResolvedFinding> =
        found.iter().map(|f| index.resolve_finding(f)).collect();
    smd_core::sort_resolved_findings(&mut resolved);
    Ok(resolved)
}
