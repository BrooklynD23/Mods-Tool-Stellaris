//! Evidence — the structured half of the one cross-crate, cross-language contract in
//! this design that no compiler otherwise checks.
//!
//! [`EvidenceType`] lives here rather than beside the `Rule` trait because it is read
//! from both ends: `smd-rules` declares with it and `smd-policy` validates against it, and
//! a type owned by either would make the other depend on its neighbour.
//! (Doc 03; REVIEW-2026-08 § G11.)

use smallvec::SmallVec;

use crate::ids::{PathId, SourceId, SymbolId};

/// A small typed map rather than a formatted string, so the same finding renders as a CLI
/// table row, a JSON object, and a GUI card without any renderer re-parsing prose.
///
/// Keys are `&'static str` because every key a rule may emit is declared up front by
/// `Rule::evidence()`, and `cargo xtask lint` cross-checks the declarations against every
/// `weights.toml` modifier and every `{placeholder}` in every locale.
pub type EvidenceBag = SmallVec<[(&'static str, EvidenceValue); 4]>;

/// One evidence value. Ids are resolved at the policy boundary, never by a renderer.
///
/// `Ord` is written by hand rather than derived, for the same reason
/// [`Subject`](crate::Subject)'s is: [`SourceId`] has no `Ord`, so the
/// [`Src`](EvidenceValue::Src) arm has to route through
/// [`SourceId::index`](crate::SourceId::index).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum EvidenceValue {
    /// A count, a byte size, a score contribution.
    Int(i64),
    /// A predicate.
    Bool(bool),
    /// A closed-vocabulary tag — an enum rendered as its `snake_case` name, never prose.
    Tag(&'static str),
    /// An interned symbol. Scan-local (Invariant C1); resolved before it leaves.
    Sym(SymbolId),
    /// An interned path. Scan-local; resolved before it leaves.
    Path(PathId),
    /// A source. Scan-local; resolved to a [`ModKey`](crate::ModKey) before it leaves.
    Src(SourceId),
}

impl EvidenceValue {
    /// A stable ordinal per variant, for the canonical evidence hash.
    ///
    /// Not a serialisation discriminant — no `EvidenceValue` is ever serialised — but it
    /// must stay stable within a build so that the finding sort key is deterministic.
    #[must_use]
    pub const fn kind_ordinal(self) -> u8 {
        match self {
            EvidenceValue::Int(_) => 0,
            EvidenceValue::Bool(_) => 1,
            EvidenceValue::Tag(_) => 2,
            EvidenceValue::Sym(_) => 3,
            EvidenceValue::Path(_) => 4,
            EvidenceValue::Src(_) => 5,
        }
    }
}

impl Ord for EvidenceValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use EvidenceValue as V;
        self.kind_ordinal().cmp(&other.kind_ordinal()).then_with(|| match (self, other) {
            (V::Int(a), V::Int(b)) => a.cmp(b),
            (V::Bool(a), V::Bool(b)) => a.cmp(b),
            (V::Tag(a), V::Tag(b)) => a.cmp(b),
            (V::Sym(a), V::Sym(b)) => a.cmp(b),
            (V::Path(a), V::Path(b)) => a.cmp(b),
            (V::Src(a), V::Src(b)) => a.index().cmp(&b.index()),
            // Unreachable: the ordinals above already differ for mismatched variants.
            _ => std::cmp::Ordering::Equal,
        })
    }
}

impl PartialOrd for EvidenceValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The declaration side of the same contract.
///
/// One variant per [`EvidenceValue`] variant; [`Tag`](EvidenceType::Tag) additionally
/// names the enum whose serialised variants are the legal vocabulary — so a modifier
/// matching `eq = "file_replaced"` against a `MergeMode` fails the lint rather than never
/// firing.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum EvidenceType {
    /// Declares [`EvidenceValue::Int`].
    Int,
    /// Declares [`EvidenceValue::Bool`].
    Bool,
    /// Declares [`EvidenceValue::Tag`], naming the enum that supplies the vocabulary.
    Tag(&'static str),
    /// Declares [`EvidenceValue::Sym`].
    Sym,
    /// Declares [`EvidenceValue::Path`].
    Path,
    /// Declares [`EvidenceValue::Src`].
    Src,
}

impl EvidenceType {
    /// Whether a value satisfies this declaration.
    ///
    /// The debug assertion behind "a rule emits only the keys it declared" is this
    /// predicate plus a key lookup.
    #[must_use]
    pub const fn accepts(self, value: EvidenceValue) -> bool {
        matches!(
            (self, value),
            (EvidenceType::Int, EvidenceValue::Int(_))
                | (EvidenceType::Bool, EvidenceValue::Bool(_))
                | (EvidenceType::Tag(_), EvidenceValue::Tag(_))
                | (EvidenceType::Sym, EvidenceValue::Sym(_))
                | (EvidenceType::Path, EvidenceValue::Path(_))
                | (EvidenceType::Src, EvidenceValue::Src(_))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_type_accepts_exactly_its_own_value() {
        let types = [
            EvidenceType::Int,
            EvidenceType::Bool,
            EvidenceType::Tag("MergeMode"),
            EvidenceType::Sym,
            EvidenceType::Path,
            EvidenceType::Src,
        ];
        let values = [
            EvidenceValue::Int(1),
            EvidenceValue::Bool(true),
            EvidenceValue::Tag("merge_by_key"),
            EvidenceValue::Sym(SymbolId(1)),
            EvidenceValue::Path(PathId(1)),
            EvidenceValue::Src(SourceId(1)),
        ];
        for (i, ty) in types.iter().enumerate() {
            for (j, v) in values.iter().enumerate() {
                assert_eq!(ty.accepts(*v), i == j, "{ty:?} vs {v:?}");
            }
        }
    }

    #[test]
    fn kind_ordinals_are_dense_and_aligned_with_the_type_side() {
        let values = [
            EvidenceValue::Int(1),
            EvidenceValue::Bool(true),
            EvidenceValue::Tag("t"),
            EvidenceValue::Sym(SymbolId(1)),
            EvidenceValue::Path(PathId(1)),
            EvidenceValue::Src(SourceId(1)),
        ];
        let got: Vec<u8> = values.iter().map(|v| v.kind_ordinal()).collect();
        assert_eq!(got, vec![0, 1, 2, 3, 4, 5]);
    }
}
