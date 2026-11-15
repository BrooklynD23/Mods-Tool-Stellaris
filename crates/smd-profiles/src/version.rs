//! `[version]`: where the installed game version is read from, and which versions the
//! profile's data was checked against.

use serde::Deserialize;
use smallvec::SmallVec;
use smd_core::{GameVersion, Match, VersionComponent, VersionPattern};

use crate::profile::ProfileError;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawVersion {
    source: Option<String>,
    // Documentation of where the file lives; the reader tries the user directory, then
    // the install directory.
    #[allow(dead_code)]
    source_dir: Option<String>,
    #[serde(default)]
    fields: Vec<String>,
    // `wildcard_dotted` is the only shape modelled.
    #[allow(dead_code)]
    pattern: Option<String>,
    #[allow(dead_code)]
    pattern_forms_confirmed: Option<bool>,
    #[serde(default)]
    verified_range: Vec<String>,
}

/// The `[version]` table, typed.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct VersionSpec {
    /// File name holding the version, e.g. `launcher-settings.json`.
    pub file: Box<str>,
    /// JSON keys to try, in order.
    pub fields: Vec<Box<str>>,
    /// Versions the profile's data was checked against; empty makes no claim.
    pub verified_range: Vec<VersionPattern>,
}

impl VersionSpec {
    pub(crate) fn from_raw(raw: RawVersion) -> Result<VersionSpec, ProfileError> {
        let verified_range = raw
            .verified_range
            .iter()
            .map(|s| {
                parse_pattern(s)
                    .ok_or_else(|| ProfileError::Value { key: "verified_range", found: s.clone() })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(VersionSpec {
            file: raw.source.unwrap_or_default().into(),
            fields: raw.fields.into_iter().map(Into::into).collect(),
            verified_range,
        })
    }

    /// Whether `v` is inside the verified range. [`Match::Unknown`] when the profile
    /// declares no range.
    #[must_use]
    pub fn check(&self, v: &GameVersion) -> Match {
        let mut verdict = Match::Unknown;
        for p in &self.verified_range {
            match p.matches(v) {
                Match::Yes => return Match::Yes,
                Match::No => verdict = Match::No,
                _ => {}
            }
        }
        verdict
    }
}

/// Parse `3.13.*` / `4.*` / `4.0.3`; `None` when malformed or empty.
#[must_use]
pub fn parse_pattern(s: &str) -> Option<VersionPattern> {
    let mut parts = SmallVec::new();
    for c in s.trim().split('.') {
        parts.push(match c {
            "*" => VersionComponent::Wildcard,
            n => VersionComponent::Exact(n.parse().ok()?),
        });
    }
    Some(VersionPattern(parts))
}

/// The first dotted number in `s` (`v4.0.5`, `Cetus v4.0.5 (abc)`), or `None`.
#[must_use]
pub fn parse_game_version(s: &str) -> Option<GameVersion> {
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let end =
        s[start..].find(|c: char| !(c.is_ascii_digit() || c == '.')).map_or(s.len(), |i| start + i);
    let parts = s[start..end]
        .trim_end_matches('.')
        .split('.')
        .map(|n| n.parse().ok())
        .collect::<Option<SmallVec<[u16; 4]>>>()?;
    Some(GameVersion(parts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Profile;

    fn v(s: &str) -> GameVersion {
        parse_game_version(s).expect("parses")
    }

    #[test]
    fn versions_parse_from_launcher_shapes() {
        assert_eq!(v("v4.0.5").0.as_slice(), &[4, 0, 5]);
        assert_eq!(v("Cetus v3.13.2 (abc)").0.as_slice(), &[3, 13, 2]);
        assert!(parse_game_version("none").is_none());
        assert!(parse_game_version("99999.1").is_none());
    }

    #[test]
    fn range_check_is_three_valued() {
        let p = Profile::shipped_default().unwrap();
        assert_eq!(p.version().check(&v("4.0.5")), Match::Yes);
        assert_eq!(p.version().check(&v("3.13.1")), Match::No);
        assert_eq!(VersionSpec::default().check(&v("3.1")), Match::Unknown);
        assert_eq!(&*p.version().file, "launcher-settings.json");
        assert_eq!(&*p.version().fields[0], "rawVersion");
    }

    #[test]
    fn unknown_version_keys_and_bad_patterns_are_rejected() {
        assert!(Profile::from_toml("[version]\nbogus = 1\n").is_err());
        assert!(Profile::from_toml("[version]\nverified_range = [\"x.*\"]\n").is_err());
    }
}
