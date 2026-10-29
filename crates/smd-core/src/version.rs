//! Version matching.
//!
//! An absent or unparseable `supported_version` yields [`Match::Unknown`], never
//! [`Match::No`] — an unset field is not a claim.

use smallvec::SmallVec;

/// One component of a version pattern.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum VersionComponent {
    /// A literal number.
    Exact(u16),
    /// `*` — matches this component and every one after it.
    Wildcard,
}

/// A `supported_version` pattern such as `3.13.*`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct VersionPattern(pub SmallVec<[VersionComponent; 4]>);

/// The installed game version, read from the game directory.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct GameVersion(pub SmallVec<[u16; 4]>);

/// The three-valued answer. `Unknown` is not a soft `No`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum Match {
    /// The pattern covers the version.
    Yes,
    /// The pattern excludes the version.
    No,
    /// No claim: the field was absent, empty, or unparseable.
    #[default]
    Unknown,
}

impl VersionPattern {
    /// Whether this pattern covers `v`.
    ///
    /// `3.13.*` vs `4.0.3` → [`No`](Match::No); `4.*` vs `4.0.3` → [`Yes`](Match::Yes);
    /// `4.0.*` vs `4.0.3` → [`Yes`](Match::Yes).
    #[must_use]
    pub fn matches(&self, v: &GameVersion) -> Match {
        if self.0.is_empty() {
            return Match::Unknown;
        }
        for (i, component) in self.0.iter().enumerate() {
            match component {
                VersionComponent::Wildcard => return Match::Yes,
                VersionComponent::Exact(want) => match v.0.get(i) {
                    Some(got) if got == want => {}
                    Some(_) => return Match::No,
                    None => return Match::Unknown,
                },
            }
        }
        Match::Yes
    }
}

#[cfg(test)]
mod tests {
    use super::VersionComponent::{Exact, Wildcard};
    use super::*;

    fn pat(components: &[VersionComponent]) -> VersionPattern {
        VersionPattern(SmallVec::from_slice(components))
    }

    fn ver(parts: &[u16]) -> GameVersion {
        GameVersion(SmallVec::from_slice(parts))
    }

    #[test]
    fn the_documented_examples_hold() {
        assert_eq!(pat(&[Exact(3), Exact(13), Wildcard]).matches(&ver(&[4, 0, 3])), Match::No);
        assert_eq!(pat(&[Exact(4), Wildcard]).matches(&ver(&[4, 0, 3])), Match::Yes);
        assert_eq!(pat(&[Exact(4), Exact(0), Wildcard]).matches(&ver(&[4, 0, 3])), Match::Yes);
    }

    #[test]
    fn an_absent_or_short_pattern_is_unknown_not_no() {
        assert_eq!(pat(&[]).matches(&ver(&[4, 0, 3])), Match::Unknown);
        assert_eq!(pat(&[Exact(4), Exact(0), Exact(3)]).matches(&ver(&[4, 0])), Match::Unknown);
        assert_eq!(Match::default(), Match::Unknown);
    }
}
