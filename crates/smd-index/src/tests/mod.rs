//! Stage A / B acceptance tests. Fixtures only; no game behaviour is claimed verified.

use smd_core::{Activation, Contender, PathId, SourceId};

use crate::{PlaysetBuilder, ScanIndex};

mod overlay;
mod replace;
mod stage_b;

pub(super) const X: &str = "common/x/00.txt";

/// Vanilla `[0]`, mod A `[2,0]`, mod B `[2,1]`, all `Active`, all shipping `X`.
pub(super) fn three_way() -> (PlaysetBuilder, SourceId, SourceId) {
    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.file(SourceId::VANILLA, X, "vanilla").file(a, X, "a").file(m, X, "b");
    (b, a, m)
}

pub(super) fn pid(ix: &ScanIndex, text: &str) -> PathId {
    ix.path_id(text).expect("path interned")
}

pub(super) fn contender(source: SourceId, path: PathId) -> Contender {
    Contender { source, path, slot: None }
}
