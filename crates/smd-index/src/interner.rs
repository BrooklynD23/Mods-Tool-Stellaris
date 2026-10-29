//! `Interner` — text stored exactly once.
//!
//! Frozen at construction: ids are dense from 0, id 0 is the empty string, and ids are
//! assigned in **ascending byte order** of the text. That last property is what makes a
//! `PathId` a function of the set of paths alone rather than of the order the walks
//! finished in (H16); the scan's determinism gate depends on it.

use std::collections::{BTreeSet, HashMap};
use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};

use smallvec::SmallVec;

type Rev = HashMap<u64, SmallVec<[u32; 1]>>;

/// Every string concatenated, with an id -> end-offset table and a reverse index that
/// stores ids rather than a second copy of the text.
#[derive(Debug, Clone)]
pub struct Interner {
    text: Box<str>,
    ends: Box<[u32]>,
    rev: Rev,
}

fn hash_of(s: &str) -> u64 {
    BuildHasherDefault::<DefaultHasher>::default().hash_one(s)
}

impl Interner {
    /// The text of `id`, or `None` for an id from another interner.
    #[must_use]
    pub fn try_get(&self, id: u32) -> Option<&str> {
        let end = *self.ends.get(id as usize)? as usize;
        let start = if id == 0 { 0 } else { self.ends[id as usize - 1] as usize };
        self.text.get(start..end)
    }

    /// The text of `id`; the empty string for an id this interner never issued.
    #[must_use]
    pub fn get(&self, id: u32) -> &str {
        self.try_get(id).unwrap_or("")
    }

    /// The id of `s`, if interned.
    #[must_use]
    pub fn find(&self, s: &str) -> Option<u32> {
        let ids = self.rev.get(&hash_of(s))?;
        ids.iter().copied().find(|&id| self.get(id) == s)
    }

    /// How many strings are interned (including the empty string at id 0).
    #[must_use]
    pub fn len(&self) -> u32 {
        u32::try_from(self.ends.len()).unwrap_or(u32::MAX)
    }

    /// Always false: id 0 is the empty string.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }

    /// Every `(id, text)` in id order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &str)> {
        (0..self.len()).map(|id| (id, self.get(id)))
    }
}

/// Collects strings in any order; [`freeze`](InternerBuilder::freeze) assigns ids.
#[derive(Debug, Default)]
pub struct InternerBuilder {
    set: BTreeSet<Box<str>>,
}

impl InternerBuilder {
    /// A builder holding only the empty string.
    #[must_use]
    pub fn new() -> InternerBuilder {
        let mut set = BTreeSet::new();
        set.insert(Box::from(""));
        InternerBuilder { set }
    }

    /// Record `s`. Order and repetition do not matter.
    pub fn insert(&mut self, s: &str) {
        if !self.set.contains(s) {
            self.set.insert(Box::from(s));
        }
    }

    /// Assign ids in ascending byte order and freeze.
    #[must_use]
    pub fn freeze(self) -> Interner {
        let mut text = String::new();
        let mut ends = Vec::with_capacity(self.set.len());
        let mut rev: Rev = HashMap::new();
        for (id, s) in self.set.iter().enumerate() {
            let id = u32::try_from(id).unwrap_or(u32::MAX);
            text.push_str(s);
            ends.push(u32::try_from(text.len()).unwrap_or(u32::MAX));
            rev.entry(hash_of(s)).or_default().push(id);
        }
        Interner { text: text.into_boxed_str(), ends: ends.into_boxed_slice(), rev }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_ascending_bytes_and_order_independent() {
        let mut a = InternerBuilder::new();
        for s in ["b/x", "a/y", "B/z", "a/y"] {
            a.insert(s);
        }
        let mut b = InternerBuilder::new();
        for s in ["a/y", "B/z", "b/x"] {
            b.insert(s);
        }
        let (a, b) = (a.freeze(), b.freeze());
        let texts: Vec<&str> = a.iter().map(|(_, t)| t).collect();
        assert_eq!(texts, vec!["", "B/z", "a/y", "b/x"]);
        assert_eq!(texts, b.iter().map(|(_, t)| t).collect::<Vec<_>>());
        assert_eq!(a.find("a/y"), Some(2));
        assert_eq!(a.find("nope"), None);
        assert_eq!(a.get(0), "");
        assert_eq!(a.try_get(99), None);
    }
}
