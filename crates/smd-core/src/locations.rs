//! Resolved filesystem roots.
//!
//! Deliberately a keyed map rather than named fields: `smd-core` must never know what a
//! mod directory looks like (doc 02), and a struct with `workshop_dir` on it would know.
//! The *keys* are declared by the game profile's `[locations]` table and resolved by
//! `smd-fs`; this crate only carries the result across the
//! [`LoadOrderSource`](crate::LoadOrderSource) boundary.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The filesystem roots a scan was given, keyed by the profile-declared location name.
///
/// `BTreeMap` rather than `HashMap`: iteration order is part of what makes a scan
/// deterministic, and nothing in this workspace may sort on hash-map order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ResolvedLocations {
    roots: BTreeMap<Box<str>, PathBuf>,
}

impl ResolvedLocations {
    /// An empty set.
    #[must_use]
    pub fn new() -> ResolvedLocations {
        ResolvedLocations::default()
    }

    /// Record a resolved root, replacing any previous value for that name.
    pub fn insert(&mut self, name: &str, path: PathBuf) -> Option<PathBuf> {
        self.roots.insert(name.into(), path)
    }

    /// The root recorded under `name`, if any.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Path> {
        self.roots.get(name).map(PathBuf::as_path)
    }

    /// Every root, in stable name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Path)> {
        self.roots.iter().map(|(k, v)| (k.as_ref(), v.as_path()))
    }

    /// How many roots were resolved.
    #[must_use]
    pub fn len(&self) -> usize {
        self.roots.len()
    }

    /// Whether nothing was resolved.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iteration_is_by_name_and_therefore_deterministic() {
        let mut l = ResolvedLocations::new();
        l.insert("workshop", PathBuf::from("/w"));
        l.insert("game", PathBuf::from("/g"));
        l.insert("user", PathBuf::from("/u"));
        let names: Vec<&str> = l.iter().map(|(k, _)| k).collect();
        assert_eq!(names, vec!["game", "user", "workshop"]);
        assert_eq!(l.get("game"), Some(Path::new("/g")));
        assert_eq!(l.get("nowhere"), None);
        assert_eq!(l.len(), 3);
        assert!(!l.is_empty());
    }
}
