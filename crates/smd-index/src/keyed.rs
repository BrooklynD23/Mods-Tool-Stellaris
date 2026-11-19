//! The event, namespace and define tables (doc 07 § symbol layer, M3): `EventId`,
//! `Namespace` and `DefineKey` facts, sited and interned like definitions and grouped by
//! key. Event and define groups resolve through the same Stages C and D as definitions
//! ([`ScanIndex::resolve_event`], [`ScanIndex::resolve_define`]); namespaces are only
//! grouped, because two declarations of one namespace do not compete.

use std::collections::BTreeMap;

use smallvec::SmallVec;
use smd_core::{EventOrdinal, FactPayload, PathId, Site, SymbolId, SymbolKey};

use crate::defs::{BuildCtx, DefSite, FactBatch, SymbolVerdict};
use crate::index::ScanIndex;
use crate::interner::{Interner, InternerBuilder};

/// `namespace.ordinal`. The ordinal stays an [`EventOrdinal`]: `ns.setup` is never coerced.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct EventKey {
    /// The namespace part.
    pub namespace: SymbolId,
    /// The ordinal part.
    pub ordinal: EventOrdinal<SymbolId>,
}

/// Key → its sites, ascending by key; each list ascending by `(source index, path,
/// ordinal)`, as in [`DefinitionTable`](crate::DefinitionTable).
#[derive(Clone, Debug)]
pub struct KeyTable<K> {
    map: BTreeMap<K, SmallVec<[DefSite; 2]>>,
}

impl<K> Default for KeyTable<K> {
    fn default() -> Self {
        KeyTable { map: BTreeMap::new() }
    }
}

impl<K: Ord> KeyTable<K> {
    /// The sites of `k`.
    #[must_use]
    pub fn sites(&self, k: &K) -> &[DefSite] {
        self.map.get(k).map_or(&[], |v| v.as_slice())
    }

    /// Every key and its sites, ascending by key.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &[DefSite])> + '_ {
        self.map.iter().map(|(k, v)| (k, v.as_slice()))
    }

    /// Keys with at least two sites on disk.
    pub fn multi_defined(&self) -> impl Iterator<Item = (&K, &[DefSite])> + '_ {
        self.iter().filter(|(_, v)| v.len() >= 2)
    }

    /// How many keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn push(&mut self, k: K, d: DefSite) {
        self.map.entry(k).or_default().push(d);
    }

    fn sort(&mut self) {
        for v in self.map.values_mut() {
            v.sort_by_key(|d: &DefSite| (d.site.source.index(), d.site.path, d.ordinal));
        }
    }
}

/// The three tables, built together in one pass over the facts.
#[derive(Clone, Debug, Default)]
pub(crate) struct FactTables {
    pub events: KeyTable<EventKey>,
    pub namespaces: KeyTable<SymbolId>,
    pub defines: KeyTable<SymbolKey>,
}

/// The interned text a define is keyed by. ponytail: `group.key` in one symbol, so a group
/// name containing `.` could alias another group's key; none does in shipped scripts.
fn define_text(group: &str, key: &str) -> String {
    format!("{group}.{key}")
}

/// Put every namespace, named ordinal and define key into the symbol interner.
pub(crate) fn intern(sb: &mut InternerBuilder, batches: &[FactBatch]) {
    for f in batches.iter().flat_map(|b| &b.facts) {
        match &f.payload {
            FactPayload::Namespace { symbol } => sb.insert(symbol),
            FactPayload::EventId { namespace, ordinal } => {
                sb.insert(namespace);
                if let EventOrdinal::Name(n) = ordinal {
                    sb.insert(n);
                }
            }
            FactPayload::DefineKey { group, key, .. } => sb.insert(&define_text(group, key)),
            // Owned by the definition and localisation layers.
            FactPayload::Definition { .. }
            | FactPayload::LocKey { .. }
            | FactPayload::Reference { .. }
            | FactPayload::AdditiveSplit { .. } => {}
            other => debug_assert!(false, "keyed tables: unrouted fact {other:?}"),
        }
    }
}

fn sym(symbols: &Interner, s: &str) -> SymbolId {
    SymbolId(symbols.find(s).unwrap_or(0))
}

/// An event site has no body digest. A hash unique to the site keeps `Agreement` at
/// `Differs` (the no-claim value) instead of a false `AllIdentical`.
fn site_hash(file_slot: u32, ordinal: u32) -> [u8; 16] {
    let mut h = [0xEE; 16];
    h[..4].copy_from_slice(&file_slot.to_le_bytes());
    h[4..8].copy_from_slice(&ordinal.to_le_bytes());
    h
}

fn event_key(symbols: &Interner, ns: &str, o: &EventOrdinal<Box<str>>) -> Option<EventKey> {
    let ordinal = match o {
        EventOrdinal::Num(n) => EventOrdinal::Num(*n),
        EventOrdinal::Name(s) => EventOrdinal::Name(sym(symbols, s)),
        other => {
            debug_assert!(false, "keyed tables: unrouted ordinal {other:?}");
            return None;
        }
    };
    Some(EventKey { namespace: sym(symbols, ns), ordinal })
}

fn add_batch(t: &mut FactTables, cx: &BuildCtx<'_>, b: &FactBatch) {
    let Some(pid) = cx.paths.find(&b.path).map(PathId) else { return };
    let Some(file_slot) = cx.overlay.slot_of(pid, b.source) else { return };
    let domain = cx.classes.get(pid.0 as usize).and_then(|c| c.domain);
    for (ordinal, f) in (0u32..).zip(&b.facts) {
        let site = Site { source: b.source, path: pid, span: f.span };
        let def = |body_hash| DefSite { site, body_hash, residual_hash: None, file_slot, ordinal };
        match &f.payload {
            FactPayload::EventId { namespace, ordinal: o } => {
                if let Some(k) = event_key(cx.symbols, namespace, o) {
                    t.events.push(k, def(site_hash(file_slot, ordinal)));
                }
            }
            FactPayload::Namespace { symbol } => {
                t.namespaces.push(sym(cx.symbols, symbol), def(site_hash(file_slot, ordinal)));
            }
            FactPayload::DefineKey { group, key, value_hash } => {
                // A define on an unclassified path has no merge scope to compete in.
                if let Some(domain) = domain {
                    let symbol = sym(cx.symbols, &define_text(group, key));
                    t.defines.push(SymbolKey { domain, symbol }, def(*value_hash));
                }
            }
            FactPayload::Definition { .. }
            | FactPayload::LocKey { .. }
            | FactPayload::Reference { .. }
            | FactPayload::AdditiveSplit { .. } => {}
            other => debug_assert!(false, "keyed tables: unrouted fact {other:?}"),
        }
    }
}

/// Site and group every batch's event, namespace and define facts.
pub(crate) fn build(batches: &[FactBatch], cx: &BuildCtx<'_>) -> FactTables {
    let mut t = FactTables::default();
    for b in batches {
        add_batch(&mut t, cx, b);
    }
    t.events.sort();
    t.namespaces.sort();
    t.defines.sort();
    t
}

impl ScanIndex {
    /// Every `EventId`, grouped by `namespace.ordinal`.
    #[must_use]
    pub fn events(&self) -> &KeyTable<EventKey> {
        &self.keyed.events
    }

    /// Every `namespace = x` declaration, grouped by namespace.
    #[must_use]
    pub fn namespaces(&self) -> &KeyTable<SymbolId> {
        &self.keyed.namespaces
    }

    /// Every define, grouped by `(domain, group.key)`.
    #[must_use]
    pub fn defines(&self) -> &KeyTable<SymbolKey> {
        &self.keyed.defines
    }

    /// Stages C and D for one event id. The merge scope is the first site's domain.
    #[must_use]
    pub fn resolve_event(&self, k: &EventKey) -> SymbolVerdict {
        let sites = self.keyed.events.sites(k);
        let domain = sites.first().and_then(|d| self.class_of(d.site.path).domain);
        self.resolve_sites(domain, sites)
    }

    /// Stages C and D for one define key.
    #[must_use]
    pub fn resolve_define(&self, k: SymbolKey) -> SymbolVerdict {
        self.resolve_sites(Some(k.domain), self.keyed.defines.sites(&k))
    }
}
