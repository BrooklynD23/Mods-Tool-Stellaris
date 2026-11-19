//! The definition layer (doc 07 § `DefinitionTable`): every `Definition` fact, sited and
//! interned, grouped by [`SymbolKey`], and resolved through Stages C and D.

mod resolve;
mod rules;
mod stage_d;

pub use resolve::SymbolVerdict;
pub use rules::{PathClass, SymbolRules};

use std::collections::BTreeMap;

use smallvec::SmallVec;
use smd_core::{
    DomainId, FactPayload, ParseCompleteness, PathId, RawFact, Site, SourceId, SymbolId, SymbolKey,
};

use crate::interner::{Interner, InternerBuilder};
use crate::overlay::OverlayTable;

/// One definition of one key.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct DefSite {
    /// Which mod, which file, which bytes.
    pub site: Site,
    /// blake3 over the normalised body, truncated.
    pub body_hash: [u8; 16],
    /// From `AdditiveSplit`, where the extractor emitted one.
    pub residual_hash: Option<[u8; 16]>,
    /// The overlay slot of the file holding it: Stage B's `contends` answers for it.
    pub file_slot: u32,
    /// Position among the file's definitions: the in-file ordinal of Stage D.
    pub ordinal: u32,
}

/// A file in a domain's scope whose facts are not all known (`Partial` or `Unparsed`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct IncompleteFile {
    /// Who ships it.
    pub source: SourceId,
    /// Where.
    pub path: PathId,
    /// Its overlay slot.
    pub slot: u32,
}

/// `SymbolKey` → its definitions, ascending by key; each list ascending by
/// `(source index, path, ordinal)`. Built in discovery order, independent of load order.
#[derive(Clone, Debug, Default)]
pub struct DefinitionTable {
    map: BTreeMap<SymbolKey, SmallVec<[DefSite; 2]>>,
    incomplete: BTreeMap<DomainId, Vec<IncompleteFile>>,
}

impl DefinitionTable {
    /// The definitions of `k`.
    #[must_use]
    pub fn sites(&self, k: SymbolKey) -> &[DefSite] {
        self.map.get(&k).map_or(&[], |v| v.as_slice())
    }

    /// Every key and its definitions, ascending by key.
    pub fn iter(&self) -> impl Iterator<Item = (SymbolKey, &[DefSite])> + '_ {
        self.map.iter().map(|(k, v)| (*k, v.as_slice()))
    }

    /// Keys with at least two definitions on disk.
    pub fn multi_defined(&self) -> impl Iterator<Item = (SymbolKey, &[DefSite])> + '_ {
        self.iter().filter(|(_, v)| v.len() >= 2)
    }

    /// How many keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether no definition was extracted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Files in `domain` whose facts are not all known, whether or not they contend.
    #[must_use]
    pub fn incomplete_in(&self, domain: DomainId) -> &[IncompleteFile] {
        self.incomplete.get(&domain).map_or(&[], Vec::as_slice)
    }

    /// Every domain's incomplete files, ascending by domain.
    pub fn incomplete_all(&self) -> impl Iterator<Item = (DomainId, &IncompleteFile)> + '_ {
        self.incomplete.iter().flat_map(|(d, v)| v.iter().map(move |f| (*d, f)))
    }
}

/// One file's facts, as the pipeline hands them over.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct FactBatch {
    pub source: SourceId,
    pub path: Box<str>,
    pub facts: Vec<RawFact>,
}

/// Put every definition name and domain name into the symbol interner.
pub(crate) fn intern(sb: &mut InternerBuilder, batches: &[FactBatch], domains: &[Box<str>]) {
    for d in domains {
        sb.insert(d);
    }
    for f in batches.iter().flat_map(|b| &b.facts) {
        if let FactPayload::Definition { symbol, .. } = &f.payload {
            sb.insert(symbol);
        }
    }
}

/// What [`build`] reads.
pub(crate) struct BuildCtx<'a> {
    pub paths: &'a Interner,
    pub symbols: &'a Interner,
    pub overlay: &'a OverlayTable,
    pub classes: &'a [PathClass],
}

fn sym(symbols: &Interner, s: &str) -> SymbolId {
    SymbolId(symbols.find(s).unwrap_or(0))
}

fn add_batch(
    map: &mut BTreeMap<SymbolKey, SmallVec<[DefSite; 2]>>,
    cx: &BuildCtx<'_>,
    b: &FactBatch,
) {
    let Some(pid) = cx.paths.find(&b.path).map(PathId) else { return };
    let Some(file_slot) = cx.overlay.slot_of(pid, b.source) else { return };
    let mut ordinal = 0u32;
    let mut last: Option<SymbolKey> = None;
    for f in &b.facts {
        match &f.payload {
            FactPayload::Definition { domain, symbol, body_hash } => {
                let key = SymbolKey { domain: *domain, symbol: sym(cx.symbols, symbol) };
                let site = Site { source: b.source, path: pid, span: f.span };
                let def = DefSite {
                    site,
                    body_hash: *body_hash,
                    residual_hash: None,
                    file_slot,
                    ordinal,
                };
                map.entry(key).or_default().push(def);
                ordinal += 1;
                last = Some(key);
            }
            FactPayload::AdditiveSplit { residual_hash, .. } => {
                // Emitted immediately after its `Definition` (doc 03).
                let target = last.and_then(|k| map.get_mut(&k)).and_then(|v| v.last_mut());
                if let Some(d) = target.filter(|d| d.file_slot == file_slot) {
                    d.residual_hash = Some(*residual_hash);
                }
            }
            // Owned by tables that are not built yet (LocTable, references, M4); they are
            // not definitions, so this table skips them.
            FactPayload::LocKey { .. } | FactPayload::Reference { .. } => {}
            // Owned by the event, namespace and define tables (`keyed.rs`, M3).
            FactPayload::Namespace { .. }
            | FactPayload::EventId { .. }
            | FactPayload::DefineKey { .. } => {}
            // `FactPayload` is non-exhaustive: a variant added later must be routed here
            // on purpose, so fail loudly in debug builds instead of dropping it silently.
            other => debug_assert!(false, "definition table: unrouted fact {other:?}"),
        }
    }
}

/// Site and group every batch's definitions; record each domain's incomplete files.
pub(crate) fn build(batches: &[FactBatch], cx: &BuildCtx<'_>) -> DefinitionTable {
    let mut map = BTreeMap::new();
    for b in batches {
        add_batch(&mut map, cx, b);
    }
    for v in map.values_mut() {
        v.sort_by_key(|d: &DefSite| (d.site.source.index(), d.site.path, d.ordinal));
    }
    let mut incomplete: BTreeMap<DomainId, Vec<IncompleteFile>> = BTreeMap::new();
    for p in cx.overlay.paths() {
        let Some(domain) = cx.classes.get(p.0 as usize).and_then(|c| c.domain) else { continue };
        for c in cx.overlay.contributions(p) {
            if matches!(c.completeness, ParseCompleteness::Partial | ParseCompleteness::Unparsed(_))
            {
                let f = IncompleteFile { source: c.source, path: p, slot: c.slot };
                incomplete.entry(domain).or_default().push(f);
            }
        }
    }
    DefinitionTable { map, incomplete }
}
