//! Stable widget identity. Keys are content-addressed hashes of the path from
//! the root (scope keys mixed with labels or sibling indices), so the same
//! logical widget gets the same key every frame — and scripts can reproduce a
//! key from strings alone, with no allocation event tying identity to a slot.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(pub u64);

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

impl Key {
    pub const ROOT: Key = Key(FNV_OFFSET);

    fn mix_bytes(mut h: u64, bytes: &[u8]) -> u64 {
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
        h
    }

    /// Child key derived from a string label.
    pub fn str(self, label: &str) -> Key {
        // Tag byte separates the str/index namespaces.
        let h = Self::mix_bytes(self.0 ^ 0x53, label.as_bytes());
        Key(h)
    }

    /// Child key derived from a sibling index (auto-keys).
    pub fn index(self, i: u64) -> Key {
        let h = Self::mix_bytes(self.0 ^ 0x49, &i.to_le_bytes());
        Key(h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_stable() {
        let a = Key::ROOT.str("panel").index(3);
        let b = Key::ROOT.str("panel").index(3);
        assert_eq!(a, b);
    }

    #[test]
    fn keys_distinguish_paths() {
        assert_ne!(Key::ROOT.str("a"), Key::ROOT.str("b"));
        assert_ne!(Key::ROOT.index(0), Key::ROOT.index(1));
        assert_ne!(Key::ROOT.str("a").str("b"), Key::ROOT.str("b").str("a"));
        // str("1") and index(1) must not collide.
        assert_ne!(Key::ROOT.str("1"), Key::ROOT.index(1));
        // Nesting matters: ("ab") != ("a")("b")
        assert_ne!(Key::ROOT.str("ab"), Key::ROOT.str("a").str("b"));
    }

    #[test]
    fn no_collisions_over_many_indices() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for scope in 0..100u64 {
            let s = Key::ROOT.index(scope);
            for i in 0..100u64 {
                assert!(seen.insert(s.index(i)), "collision at {scope}/{i}");
            }
        }
    }
}
