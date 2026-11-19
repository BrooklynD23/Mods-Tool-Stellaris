//! Helpers shared by the descriptor-level rules (`dep.*`, `meta.*`): mod-level sites and
//! evidence. No detection logic.

use smd_core::{
    Activation, ActivationCertainty, Certainty, Completeness, Confidence, EvidenceValue,
    Participant, PathId, Role, SemanticCertainty, Site, SourceId, Span, SymbolId,
};
use smd_index::ScanIndex;

/// The rule's own ceiling when it claims nothing beyond its participants: the engine meets
/// it with what activation, order, discovery and the game version support.
pub(crate) const STATED: Certainty = Certainty {
    completeness: Completeness::Complete,
    activation: ActivationCertainty::Certain,
    order: Confidence::Exact,
    semantics: SemanticCertainty::Verified,
};

/// A participant naming a whole mod: no file, so the site is the empty path.
pub(crate) fn mod_site(source: SourceId, role: Role) -> Participant {
    Participant { site: Site { source, path: PathId(0), span: Span::default() }, role }
}

/// A non-vanilla source that is not known to be switched off.
pub(crate) fn maybe_loaded(index: &ScanIndex, s: SourceId) -> bool {
    s != SourceId::VANILLA
        && index.sources().get(s).is_some_and(|m| m.activation != Activation::Inactive)
}

/// Text as evidence. Every string a rule cites is interned at build, so a miss is the
/// empty symbol rather than a panic.
pub(crate) fn text(index: &ScanIndex, t: &str) -> EvidenceValue {
    EvidenceValue::Sym(index.symbol_id(t).unwrap_or(SymbolId(0)))
}
