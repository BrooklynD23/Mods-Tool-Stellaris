//! What the two symbol rules share: roles and evidence from a [`SymbolVerdict`].

use smd_core::{EvidenceBag, EvidenceValue, Outcome, Participant, Role, SymbolKey};
use smd_index::{DefSite, ScanIndex, SymbolVerdict};

/// Roles for `sites` under `v` (doc 07 R1): `Winner` for the stated winner; `Shadowed`
/// only under `SingleWinner` or `Deleted`, and only for a stated (not possible) survivor
/// or a stated drop; everything else `Contributor`.
pub(crate) fn participants(
    index: &ScanIndex,
    v: &SymbolVerdict,
    sites: &[DefSite],
) -> Vec<Participant> {
    let r = &v.resolution;
    let mut out: Vec<Participant> = sites
        .iter()
        .map(|d| {
            let is_winner =
                r.winner.is_some_and(|w| w.slot == Some(d.file_slot) && w.source == d.site.source);
            let stated_loser = r.outcome == Outcome::SingleWinner
                && index.derived().is_surviving(d.file_slot)
                || r.outcome == Outcome::Deleted;
            let role = if is_winner {
                Role::Winner
            } else if stated_loser {
                Role::Shadowed
            } else {
                Role::Contributor
            };
            Participant { site: d.site, role }
        })
        .collect();
    out.sort_by_key(|p| p.site.sort_key());
    out
}

/// The `domain` evidence entry, when the domain name is interned.
pub(crate) fn domain(index: &ScanIndex, key: SymbolKey, facts: &mut EvidenceBag) {
    if let Some(sym) = index.domain_symbol(key.domain) {
        facts.push(("domain", EvidenceValue::Sym(sym)));
    }
}
