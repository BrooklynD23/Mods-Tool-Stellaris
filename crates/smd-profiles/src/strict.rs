//! Strict loading (doc 10 § Keys, by resolution stage; REVIEW-2026-10 H6): a key the
//! loader does not recognise is a load error, and so are two `[[dir]]` globs that tie on
//! specificity while matching a common path.
//!
//! Tables whose every key is typed use `#[serde(deny_unknown_fields)]`. Tables with keys
//! this reader carries but does not model yet (the Stage C/D keys of `[load_order]`,
//! `[syntax]`, `[[dir]]`) collect them into a map checked here against the doc 10 list.

use std::collections::BTreeMap;

use crate::glob;
use crate::profile::ProfileError;

/// Keys the rest of the pipeline owns, by table. A `*_confidence` key is allowed in
/// `[load_order]` and `[[dir]]` (doc 10: prior-art strength, for the reader).
const LOAD_ORDER: &[&str] = &[
    "later_wins",
    "vanilla_is_lowest",
    "filename_compare",
    "default_order",
    "filename_key",
    "vanilla_in_filename_axis",
    "in_file_duplicate",
];
const DIR: &[&str] = &[
    "domain",
    "merge",
    "additive_children",
    "scope",
    "tier",
    "order",
    "level",
    "parser",
    "extractor",
    "encoding",
    "verified",
];
const SYNTAX: &[&str] = &[
    "script",
    "localisation",
    "descriptor",
    "operators",
    "variables",
    "inline_scripts",
    "encoding",
];

/// The unmodelled keys of one table.
pub(crate) type Rest = BTreeMap<String, toml::Value>;

fn check(
    table: &'static str,
    rest: &Rest,
    allowed: &[&str],
    confidence: bool,
) -> Result<(), ProfileError> {
    let ok = |k: &str| allowed.contains(&k) || (confidence && k.ends_with("_confidence"));
    match rest.keys().find(|k| !ok(k)) {
        Some(k) => Err(ProfileError::UnknownKey { table, key: k.clone() }),
        None => Ok(()),
    }
}

pub(crate) fn load_order(rest: &Rest) -> Result<(), ProfileError> {
    check("load_order", rest, LOAD_ORDER, true)
}

pub(crate) fn dir(rest: &Rest) -> Result<(), ProfileError> {
    check("dir", rest, DIR, true)
}

pub(crate) fn syntax(rest: &Rest) -> Result<(), ProfileError> {
    check("syntax", rest, SYNTAX, false)
}

/// Reject two globs of equal specificity that can match the same path: classification
/// would fall to declaration order, which doc 10 forbids. O(n²) over ~80 entries.
pub(crate) fn no_tied_overlap<'a>(
    globs: impl Iterator<Item = &'a str>,
) -> Result<(), ProfileError> {
    let globs: Vec<&str> = globs.collect();
    for (i, a) in globs.iter().enumerate() {
        for b in &globs[i + 1..] {
            if glob::specificity(a) == glob::specificity(b) && glob::overlaps(a, b) {
                return Err(ProfileError::OverlappingGlobs {
                    first: (*a).to_owned(),
                    second: (*b).to_owned(),
                });
            }
        }
    }
    Ok(())
}
