//! The closed set of *derived* explanation placeholders.
//!
//! A `{placeholder}` in `policy/explain/<lang>.toml` is legal if it is either a key the
//! rule's `evidence()` declares, or a member of this set. `cargo xtask lint` parses every
//! locale and fails on anything that is neither.
//!
//! The set is closed and small on purpose: it is the only text in the catalogue that no
//! rule can be asked to supply, so every member needs one definition in one place. None of
//! `{winner}`, `{losers}`, or `{loser_first}` appears in any rule's evidence bag — all
//! three are derived from `Finding::participants` by [`Role`](crate::Role).
//! (Doc 09 § Placeholders are a closed set; REVIEW-2026-08 § G11.)

/// Every placeholder the policy layer derives rather than reads from evidence.
/// - `n` — the number of participants.
/// - `subject` — the finding's subject, rendered.
/// - `winner` — the single `Role::Winner` participant. Exists only when the outcome is
///   `SingleWinner`; a catalogue entry using it must be a variant guarded on that.
/// - `losers` — the `Role::Shadowed` participants, joined.
/// - `loser_first` — the first shadowed participant, for "if you prefer X's version".
pub const DERIVED_PLACEHOLDERS: [&str; 5] = ["n", "subject", "winner", "losers", "loser_first"];

/// Whether a `{placeholder}` name is one the policy layer derives.
#[must_use]
pub fn is_derived_placeholder(name: &str) -> bool {
    DERIVED_PLACEHOLDERS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_derived_set_is_closed_and_distinct() {
        let mut names = DERIVED_PLACEHOLDERS.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), DERIVED_PLACEHOLDERS.len());
        assert!(is_derived_placeholder("loser_first"));
        assert!(!is_derived_placeholder("domain"), "evidence keys are the other half");
    }
}
