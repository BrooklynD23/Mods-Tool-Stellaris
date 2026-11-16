//! A 128-bit content hash, implemented in-crate.
//!
//! # Why not blake3
//!
//! `docs/architecture/03-data-contracts.md` specifies the canonical evidence hash (sort
//! key 6) as "blake3 over the `EvidenceBag` sorted by key". `blake3` is a third-party
//! crate, and `smd-core` may depend on nothing (doc 02, ADR-0002 § 1) — the two documents
//! contradict each other, and the zero-dependency rule is the load-bearing one.
//!
//! What the sort key actually needs is a *deterministic total function of the finding's
//! content* whose collision probability is negligible; it is never stored, never
//! compared across builds, and never a content address. FNV-1a widened to 128 bits gives
//! exactly that in twenty lines with no dependency. Content addressing proper — file
//! blobs, `body_hash`, cache keys — stays blake3 and stays in `smd-fs`, where a
//! dependency is allowed.

/// A 128-bit digest.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct ContentHash(pub u128);

/// Streaming FNV-1a, widened to 128 bits.
///
/// Deterministic across runs, threads, and machines for a given input sequence — which is
/// the property the determinism gate depends on, and the one `DefaultHasher` explicitly
/// does not promise across toolchain releases.
#[derive(Copy, Clone, Debug)]
pub struct Hasher128(u128);

const FNV_OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

impl Default for Hasher128 {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher128 {
    /// A fresh hasher.
    #[must_use]
    pub const fn new() -> Self {
        Hasher128(FNV_OFFSET)
    }

    /// Absorb raw bytes.
    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u128;
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }

    /// Absorb a length-prefixed byte string, so that `("ab", "c")` and `("a", "bc")`
    /// hash differently.
    pub fn write_framed(&mut self, bytes: &[u8]) {
        self.write(&(bytes.len() as u64).to_le_bytes());
        self.write(bytes);
    }

    /// Absorb a length-prefixed string.
    pub fn write_str(&mut self, s: &str) {
        self.write_framed(s.as_bytes());
    }

    /// Absorb a `u64`.
    pub fn write_u64(&mut self, v: u64) {
        self.write(&v.to_le_bytes());
    }

    /// Absorb a `u8`.
    pub fn write_u8(&mut self, v: u8) {
        self.write(&[v]);
    }

    /// Finish.
    #[must_use]
    pub const fn finish(self) -> ContentHash {
        ContentHash(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_of(parts: &[&str]) -> ContentHash {
        let mut h = Hasher128::new();
        for p in parts {
            h.write_str(p);
        }
        h.finish()
    }

    #[test]
    fn framing_prevents_concatenation_collisions() {
        assert_ne!(hash_of(&["ab", "c"]), hash_of(&["a", "bc"]));
    }

    #[test]
    fn the_same_input_hashes_the_same_every_time() {
        assert_eq!(hash_of(&["core.symbol_collision"]), hash_of(&["core.symbol_collision"]));
        assert_ne!(hash_of(&["a"]), hash_of(&["b"]));
    }

    #[test]
    fn empty_input_is_the_offset_basis() {
        assert_eq!(Hasher128::new().finish(), ContentHash(FNV_OFFSET));
    }
}
