//! What one `replace_path` declaration drops from *other mods* (doc 08
//! `pdx.replace_path_shadow`). Stage A already removes a dropped file's definitions from
//! every key group (Stage B), so this is the declaration-side view of the same fates.

use smallvec::SmallVec;
use smd_core::{Activation, Contender, ResolutionGap, SourceRole};

use crate::index::ScanIndex;
use crate::replace::{Fate, FateCtx, ReplaceDecl, covers, fate};

/// One other mod's file under a declaration's prefix that it drops or may drop.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReplaceHit {
    /// The file, with its overlay slot.
    pub contender: Contender,
    /// `true`: stated dropped by this declaration. `false`: dropped under some reading of
    /// the inputs in `gaps`.
    pub stated: bool,
    /// Why a possible drop is not stated; empty when `stated`.
    pub gaps: SmallVec<[ResolutionGap; 1]>,
}

impl ScanIndex {
    /// Every `replace_path` declaration, ascending by `(declarer index, prefix)`.
    #[must_use]
    pub fn replace_decls(&self) -> &[ReplaceDecl] {
        &self.decls
    }

    /// The files of other non-`Inactive` mods (not vanilla, not DLC) under `d`'s prefix
    /// that `d` alone drops or may drop, ascending by `(PathId, source index)`.
    #[must_use]
    pub fn replace_hits(&self, d: &ReplaceDecl) -> Vec<ReplaceHit> {
        let mut out = Vec::new();
        for p in self.overlay.paths() {
            let path = self.path_text(p);
            if !covers(&d.prefix, path) {
                continue;
            }
            let cx = FateCtx {
                sources: &self.sources,
                decls: std::slice::from_ref(d),
                scope: self.scope,
                path,
            };
            for c in self.overlay.contributions(p) {
                let Some(meta) = self.sources.get(c.source) else { continue };
                let other_mod = !matches!(meta.role, SourceRole::Vanilla | SourceRole::Dlc);
                if c.source == d.declarer || !other_mod || meta.activation == Activation::Inactive {
                    continue;
                }
                let contender = Contender { source: c.source, path: p, slot: Some(c.slot) };
                let (stated, gaps) = match fate(&cx, c) {
                    Fate::Keep => continue,
                    Fate::Drop(_) => (true, SmallVec::new()),
                    Fate::Maybe(doubt) => (false, doubt.gaps().into_iter().collect()),
                };
                out.push(ReplaceHit { contender, stated, gaps });
            }
        }
        out
    }
}
