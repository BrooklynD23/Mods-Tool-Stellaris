//! Reading the installed game version and checking it against the profile (doc 10,
//! `[version]`).
//!
//! The version file is tried in the profile's `source_dir` first, then the other one. Every
//! way it can fail is a Warn, never an error: a scan is still worth running on an
//! unverified version, but its verdicts then rest on an unchecked profile.

use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::Path;

use smd_core::{GameVersion, Match, ResolvedLocations};
use smd_fs::ReadOnlyRoot;
use smd_profiles::{Profile, parse_game_version};

use crate::codes::{self, IngestDiagnostic};
use crate::order::json_caps::{self, FILE_MAX};
use crate::{KEY_INSTALL, KEY_USER_DIR};

/// The installed version and what the profile makes of it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct GameVersionCheck {
    /// The version as the file spells it, when one was read.
    pub raw: Option<String>,
    /// [`GAME_VERSION_UNVERIFIED`](codes::GAME_VERSION_UNVERIFIED), when it applies.
    pub diagnostics: Vec<IngestDiagnostic>,
}

enum Read {
    Missing,
    Bad(String),
    Found(String),
}

fn read_one(dir: &Path, file: &str, fields: &[Box<str>]) -> Read {
    let Ok(root) = ReadOnlyRoot::open(dir) else { return Read::Missing };
    // Size first, from the directory entry, so over-cap is a typed outcome; the read
    // still enforces the cap.
    if let Ok(m) = std::fs::symlink_metadata(root.path().join(file)) {
        if m.len() > FILE_MAX {
            return Read::Bad(format!("{file} is over the {FILE_MAX}-byte cap"));
        }
    }
    let bytes = match root.read_file(&[OsString::from(file)], FILE_MAX) {
        Ok(b) => b,
        Err(e) if e.kind() == ErrorKind::NotFound => return Read::Missing,
        Err(e) => return Read::Bad(format!("{file} could not be read: {e}")),
    };
    if let Err(why) = json_caps::check_depth(&bytes) {
        return Read::Bad(format!("{file}: {why}"));
    }
    let value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => return Read::Bad(format!("{file} is not valid JSON: {e}")),
    };
    if let Err(why) = json_caps::check_entries(&value) {
        return Read::Bad(format!("{file}: {why}"));
    }
    match fields.iter().find_map(|f| value.get(&**f).and_then(|v| v.as_str())) {
        Some(t) => Read::Found(t.to_owned()),
        None => Read::Bad(format!("{file} has none of {fields:?} as a string")),
    }
}

fn warn(why: &str) -> IngestDiagnostic {
    IngestDiagnostic::warn(codes::GAME_VERSION_UNVERIFIED, "game_version", why)
}

/// Read the installed game version and compare it with the profile's verified range.
#[must_use]
pub fn check_game_version(loc: &ResolvedLocations, profile: &Profile) -> GameVersionCheck {
    let spec = profile.version();
    let mut problem = None;
    let mut found = None;
    let order = if &*spec.source_dir == "install" {
        [KEY_INSTALL, KEY_USER_DIR]
    } else {
        [KEY_USER_DIR, KEY_INSTALL]
    };
    for key in order {
        let Some(dir) = loc.get(key) else { continue };
        match read_one(dir, &spec.file, &spec.fields) {
            Read::Missing => {}
            Read::Bad(why) => problem = problem.or(Some(why)),
            Read::Found(t) => {
                found = Some(t);
                break;
            }
        }
    }
    let mut out = GameVersionCheck::default();
    let Some(raw) = found else {
        let why = problem.unwrap_or_else(|| format!("{} was not found", spec.file));
        out.diagnostics.push(warn(&format!("the game version is unreadable: {why}")));
        return out;
    };
    out.diagnostics.extend(range_problem(&raw, parse_game_version(&raw), profile));
    out.raw = Some(raw);
    out
}

fn range_problem(raw: &str, v: Option<GameVersion>, profile: &Profile) -> Option<IngestDiagnostic> {
    let spec = profile.version();
    if spec.verified_range.is_empty() {
        return None;
    }
    let Some(v) = v else {
        return Some(warn(&format!("the game version `{raw}` is not a dotted number")));
    };
    match spec.check(&v) {
        Match::Yes => None,
        Match::No => Some(warn(&format!(
            "game version {raw} is outside the profile's verified range; verdicts rest on unchecked data"
        ))),
        _ => Some(warn(&format!("game version {raw} is too short to check against the profile"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::World;

    fn run(w: &World) -> GameVersionCheck {
        let p = Profile::shipped_default().unwrap();
        check_game_version(&w.locations(), &p)
    }

    fn put(dir: &Path, body: &[u8]) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("launcher-settings.json"), body).unwrap();
    }

    #[test]
    fn in_range_is_silent_and_prefers_raw_version() {
        let w = World::new();
        put(&w.user, br#"{"version":"Old v3.1.0","rawVersion":"v4.0.5"}"#);
        let c = run(&w);
        assert_eq!(c.raw.as_deref(), Some("v4.0.5"));
        assert!(c.diagnostics.is_empty());
    }

    #[test]
    fn install_dir_is_the_fallback_and_version_the_second_field() {
        let w = World::new();
        put(&w.install, br#"{"version":"4.2.0"}"#);
        let c = run(&w);
        assert_eq!(c.raw.as_deref(), Some("4.2.0"));
        assert!(c.diagnostics.is_empty());
    }

    #[test]
    fn profile_source_dir_is_read_first() {
        let w = World::new();
        put(&w.user, br#"{"rawVersion":"v3.13.1"}"#);
        put(&w.install, br#"{"rawVersion":"v4.0.5"}"#);
        let c = run(&w);
        assert_eq!(c.raw.as_deref(), Some("v4.0.5"), "the profile says install first");
        assert!(c.diagnostics.is_empty());
    }

    #[test]
    fn out_of_range_warns() {
        let w = World::new();
        put(&w.user, br#"{"rawVersion":"v3.13.1"}"#);
        let c = run(&w);
        assert_eq!(c.raw.as_deref(), Some("v3.13.1"));
        assert_eq!(c.diagnostics.len(), 1);
        assert_eq!(c.diagnostics[0].code, codes::GAME_VERSION_UNVERIFIED);
        assert!(c.diagnostics[0].detail.contains("outside"));
    }

    #[test]
    fn missing_file_warns_unreadable() {
        let c = run(&World::new());
        assert!(c.raw.is_none());
        assert!(c.diagnostics[0].detail.contains("was not found"));
    }

    #[test]
    fn malformed_json_warns_and_names_why() {
        let w = World::new();
        put(&w.user, b"{ not json");
        let c = run(&w);
        assert!(c.raw.is_none());
        assert!(c.diagnostics[0].detail.contains("not valid JSON"));
    }

    #[test]
    fn oversize_file_warns_without_reading_it() {
        let w = World::new();
        put(&w.user, &vec![b' '; usize::try_from(FILE_MAX).unwrap() + 1]);
        let c = run(&w);
        assert!(c.diagnostics[0].detail.contains("cap"));
    }
}
