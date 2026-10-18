//! The playset inventory: discovery plus the load-order chain, as a `SourceTable`.
//!
//! `SourceId`s are canonical (Invariant P1, REVIEW-2026-10 H16): vanilla is `0` with rank
//! `[0]`, and every mod follows in `ModKey` byte order, never enumeration order, so the
//! same mods on disk yield the same table whatever order they were created or listed in.

use std::path::PathBuf;

use smd_core::{
    Activation, Confidence, ContentIncompleteReason, Descriptor, Diagnostic, ModKey, Playset,
    PlaysetProvenance, ResolvedLocations, ScanCompleteness, SourceId, SourceMeta, SourceRole,
    SourceTable,
};

use crate::codes::IngestDiagnostic;
use crate::discovery::{UNASSIGNED, VANILLA_KEY};
use crate::order::OrderChain;
use crate::{ContentState, DiscoveredMod, Discovery, DiscoveryOptions, UnreadReason, discover};

/// What ingest knows about one source beyond its [`SourceMeta`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ModRecord {
    /// The id the same row has in the [`SourceTable`].
    pub source: SourceId,
    /// Stable identity.
    pub key: ModKey,
    /// The launcher file stem or workshop directory name; empty for vanilla.
    pub dir_name: Box<str>,
    /// The launcher `*.mod` file name, when there is one.
    pub mod_file: Option<Box<str>>,
    /// The descriptor as read.
    pub descriptor: Descriptor,
    /// Descriptor diagnostics, sited at this source.
    pub diagnostics: Vec<Diagnostic>,
    /// The canonical content directory; `Some` only when `content` is `Readable`.
    pub content_root: Option<PathBuf>,
    /// Whether the content may be walked.
    pub content: ContentState,
}

/// The result of an ingest pass.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Inventory {
    /// Every source, vanilla first, then mods in `ModKey` byte order.
    pub table: SourceTable,
    /// Per-source records, index-parallel to `table`.
    pub records: Vec<ModRecord>,
    /// Which order source answered and how far it is trusted, order and activation apart.
    pub provenance: PlaysetProvenance,
    /// The content-incomplete entries this pass found. Vanilla has one whenever its
    /// content was not read (no install given, or not there).
    pub completeness: ScanCompleteness,
    /// Discovery may have missed mods (see [`Discovery::incomplete`]): a mod being absent
    /// from the table is then not evidence that it is not installed.
    pub discovery_incomplete: bool,
    /// Every scan-wide diagnostic, discovery first, then the chain's.
    pub diagnostics: Vec<IngestDiagnostic>,
}

fn vanilla(install: Option<&PathBuf>) -> (SourceMeta, ModRecord) {
    let key = ModKey(VANILLA_KEY.into());
    let meta = SourceMeta {
        key: key.clone(),
        name: "vanilla".into(),
        rank: vec![0],
        activation: Activation::Active,
        role: SourceRole::Vanilla,
        deps: Vec::new(),
    };
    let record = ModRecord {
        source: SourceId::VANILLA,
        key,
        dir_name: "".into(),
        mod_file: None,
        descriptor: Descriptor::default(),
        diagnostics: Vec::new(),
        content_root: install.cloned(),
        content: install
            .map_or(ContentState::Unread(UnreadReason::NoPath), |_| ContentState::Readable),
    };
    (meta, record)
}

/// Rank and activation from the playset, honouring each confidence on its own:
/// `Fallback` order means no rank, `Fallback` activation means `Unknown` — found on disk
/// is not enabled.
fn place(playset: &Playset, key: &ModKey) -> (Vec<u32>, Activation) {
    let entry = playset.entries.iter().find(|e| &e.key == key);
    let rank = match (playset.order, entry) {
        (Confidence::Fallback, _) | (_, None) => Vec::new(),
        (_, Some(e)) => vec![2, e.position],
    };
    let activation = match (playset.activation, entry) {
        (Confidence::Fallback, _) => Activation::Unknown,
        (_, Some(e)) if e.enabled => Activation::Active,
        _ => Activation::Inactive,
    };
    (rank, activation)
}

fn row(id: SourceId, m: &DiscoveredMod, playset: &Playset) -> (SourceMeta, ModRecord) {
    let (rank, activation) = place(playset, &m.key);
    let meta = SourceMeta {
        key: m.key.clone(),
        name: m.name.clone(),
        rank,
        activation,
        role: m.role,
        deps: Vec::new(),
    };
    let mut diagnostics = m.diagnostics.clone();
    for d in &mut diagnostics {
        if d.site.source == UNASSIGNED {
            d.site.source = id;
        }
    }
    let record = ModRecord {
        source: id,
        key: m.key.clone(),
        dir_name: m.dir_name.clone(),
        mod_file: m.mod_file.clone(),
        descriptor: m.descriptor.clone(),
        diagnostics,
        content_root: m.content_root.clone(),
        content: m.content,
    };
    (meta, record)
}

fn completeness(found: &Discovery) -> ScanCompleteness {
    let mut c = ScanCompleteness::default();
    if found.install.is_none() {
        // Vanilla is `Active` with nothing read; an empty vanilla is not a scanned one.
        let reason = ContentIncompleteReason::Truncated;
        c.content_incomplete.push((ModKey(VANILLA_KEY.into()), reason, None));
    }
    for m in &found.mods {
        let reason = match m.content {
            ContentState::Readable => continue,
            ContentState::Unread(UnreadReason::Archive) => ContentIncompleteReason::ArchiveUnread,
            // The core enum has no reason for "never opened"; `Truncated` is the nearest
            // honest one: it gaps `Deleted` claims over this source.
            ContentState::Unread(_) => ContentIncompleteReason::Truncated,
        };
        c.content_incomplete.push((m.key.clone(), reason, None));
    }
    c
}

/// Discover the mods under `loc`, run the load-order chain, and assemble the table.
#[must_use]
pub fn inventory(
    loc: &ResolvedLocations,
    chain: &OrderChain,
    opts: &DiscoveryOptions,
) -> Inventory {
    let found = discover(loc, opts);
    let outcome = chain.run_with(loc, &found);
    let (v_meta, v_record) = vanilla(found.install.as_ref());
    let mut metas = vec![v_meta];
    let mut records = vec![v_record];
    for (i, m) in (1u32..).zip(&found.mods) {
        let (meta, record) = row(SourceId(i), m, &outcome.playset);
        metas.push(meta);
        records.push(record);
    }
    let provenance = PlaysetProvenance {
        order_source: outcome.order_source,
        order: outcome.playset.order,
        activation: outcome.playset.activation,
        snapshot: outcome.playset.snapshot,
    };
    let mut diagnostics = found.diagnostics.clone();
    diagnostics.extend(outcome.diagnostics);
    Inventory {
        completeness: completeness(&found),
        discovery_incomplete: found.incomplete,
        table: SourceTable::new(metas),
        records,
        provenance,
        diagnostics,
    }
}

#[cfg(test)]
mod tests;
