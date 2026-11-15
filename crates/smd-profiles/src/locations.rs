//! A profile's `[locations]` and `[stores]` tables: where a game lives, as data.
//!
//! Both tables reject unknown keys, so a typo is a load error rather than a location that
//! is silently never searched. Templates use `{NAME}` placeholders; [`expand`] fills them.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The OS keys `[locations] user_dir` may carry.
const USER_DIR_KEYS: [&str; 4] = ["windows", "linux", "macos", "proton"];

/// `[locations]`: templates for the install, user and workshop directories.
#[derive(Clone, PartialEq, Eq, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Locations {
    /// The user data directory template per OS key (`windows`, `linux`, `macos`,
    /// `proton`).
    #[serde(default)]
    pub user_dir: BTreeMap<String, String>,
    /// The workshop content directory under a Steam library.
    pub workshop: Option<String>,
    /// The install directory under a Steam library.
    pub install: Option<String>,
    /// The launcher-descriptor subdirectory of the user directory.
    pub mods_subdir: Option<String>,
    /// Files, relative to a Steam root, that list its libraries.
    #[serde(default)]
    pub steam_library_index: Vec<String>,
}

/// `[stores]`: how to recognise installs that Steam did not make.
#[derive(Clone, PartialEq, Eq, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stores {
    /// The game's executable name; present in a GOG or Xbox install directory.
    pub executable: Option<String>,
    /// Uninstall-registry `DisplayName` prefix of a GOG (or other installer) entry.
    pub gog_display_name_prefix: Option<String>,
    /// The Xbox install path under a drive root, e.g. `XboxGames/<name>/Content`.
    pub xbox_install: Option<String>,
    /// Further user-directory templates, tried after the OS default. Entries here may be
    /// UNVERIFIED candidates; the profile file says which.
    #[serde(default)]
    pub extra_user_dirs: Vec<String>,
}

impl Locations {
    pub(crate) fn check(&self) -> Result<(), crate::ProfileError> {
        match self.user_dir.keys().find(|k| !USER_DIR_KEYS.contains(&k.as_str())) {
            Some(k) => {
                Err(crate::ProfileError::Value { key: "locations.user_dir", found: k.clone() })
            }
            None => Ok(()),
        }
    }
}

/// Fill every `{NAME}` of `template` from `var`.
///
/// # Errors
/// The name of the first placeholder `var` cannot fill (or the whole template, when a `{`
/// is never closed).
pub fn expand(template: &str, var: &dyn Fn(&str) -> Option<String>) -> Result<String, String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}').ok_or_else(|| template.to_owned())?;
        let name = &after[..close];
        out.push_str(&var(name).ok_or_else(|| name.to_owned())?);
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var(n: &str) -> Option<String> {
        match n {
            "A" => Some("x".into()),
            "B" => Some("y z".into()),
            _ => None,
        }
    }

    #[test]
    fn expands_known_placeholders() {
        assert_eq!(expand("{A}/m/{B}", &var).unwrap(), "x/m/y z");
        assert_eq!(expand("plain", &var).unwrap(), "plain");
    }

    #[test]
    fn names_the_placeholder_it_could_not_fill() {
        assert_eq!(expand("{A}/{NOPE}", &var), Err("NOPE".to_owned()));
        assert!(expand("{A", &var).is_err());
    }
}
