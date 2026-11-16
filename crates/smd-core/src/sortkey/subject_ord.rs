//! `Ord` for [`Subject`], written by hand.
//!
//! Deriving it would demand `Ord` on [`SourceId`](crate::SourceId) and reintroduce the
//! identity/precedence confusion that the split exists to remove. Instead every field is
//! projected to its raw `u32` — sources through [`SourceId::index`](crate::SourceId::index)
//! — behind a variant discriminant. The result is an arbitrary but stable total order over
//! identity, and it means nothing else.

use std::cmp::Ordering;

use super::event_ordinal_key;
use crate::finding::Subject;

/// The variant discriminant, first component of the order.
///
/// Declaration order, so a new variant appended to `Subject` sorts last and does not
/// renumber the existing ones.
const fn variant(s: &Subject) -> u8 {
    match s {
        Subject::Path(_) => 0,
        Subject::Symbol(_) => 1,
        Subject::LocKey { .. } => 2,
        Subject::Event { .. } => 3,
        Subject::Directory { .. } => 4,
        Subject::Mod(_) => 5,
        Subject::ModPair(..) => 6,
    }
}

/// Every field of a variant, flattened to comparable `u32`s. Padded so that two subjects
/// of the same variant always compare on equal-length tuples.
fn fields(s: &Subject) -> [u32; 3] {
    match s {
        Subject::Path(p) => [p.0, 0, 0],
        Subject::Symbol(k) => [u32::from(k.domain.0), k.symbol.0, 0],
        Subject::LocKey { lang, key } => [u32::from(lang.0), key.0, 0],
        Subject::Event { namespace, ordinal } => {
            let (tag, value) = event_ordinal_key(ordinal);
            [namespace.0, u32::from(tag), value]
        }
        Subject::Directory { declarer, prefix } => [declarer.index(), prefix.0, 0],
        Subject::Mod(s) => [s.index(), 0, 0],
        Subject::ModPair(a, b) => [a.index(), b.index(), 0],
    }
}

impl Ord for Subject {
    fn cmp(&self, other: &Self) -> Ordering {
        variant(self).cmp(&variant(other)).then_with(|| fields(self).cmp(&fields(other)))
    }
}

impl PartialOrd for Subject {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
