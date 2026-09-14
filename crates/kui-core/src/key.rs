//! Stable widget identity. Keys are content-addressed hashes of the path from
//! the root (scope keys mixed with labels or sibling indices), so the same
//! logical widget gets the same key every frame — and scripts can reproduce a
//! key from strings alone, with no allocation event tying identity to a slot.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(pub u64);

/// The FNV-1a basis every hash in the core starts from — keys, the text
/// cache's identities, the access tree's digest, the corpus digest.
pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
pub const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a over `bytes`, continuing from `h`. The one spelling of the
/// mixer: a digest that must stay bit-stable across versions (the text
/// cache key, the access tree's change detector) is stable because it is
/// this function and nothing else.
#[inline]
pub fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

impl Key {
    pub const ROOT: Key = Key(FNV_OFFSET);

    fn mix_bytes(h: u64, bytes: &[u8]) -> u64 {
        fnv(h, bytes)
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

/// The `.str`-keyed nodes of one frame with the labels they were opened
/// under — what `Core::key_of` resolves a name through. A binding that
/// holds only strings cannot rebuild a key: the path from the root runs
/// through auto-keyed ancestors it cannot spell. So the build records
/// `(key, label)` as it goes, into one `Vec` and one `String` that are
/// cleared, not dropped, between frames — a frame that keys a thousand
/// rows allocates nothing after its first.
#[derive(Default)]
pub(crate) struct LabelIndex {
    /// The key, the label's span in `text`, and the origin the node was
    /// opened under — so a lookup can prefer the asker's own nodes (a
    /// guest's `key_of` is asked from inside its fill; see `find_label`).
    entries: Vec<(Key, u32, u32, crate::tree::OriginId)>,
    text: String,
}

impl LabelIndex {
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.text.clear();
    }

    pub(crate) fn push(&mut self, key: Key, label: &str, origin: crate::tree::OriginId) {
        let start = self.text.len() as u32;
        self.text.push_str(label);
        self.entries.push((key, start, label.len() as u32, origin));
    }

    /// The label `key` was opened under, if it was opened by label.
    pub(crate) fn label_of(&self, key: Key) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, ..)| *k == key)
            .map(|(_, start, len, _)| &self.text[*start as usize..(*start + *len) as usize])
    }

    /// The keys opened under `label` and the origin each was opened
    /// under, in tree order.
    pub(crate) fn find<'a>(
        &'a self,
        label: &'a str,
    ) -> impl Iterator<Item = (Key, crate::tree::OriginId)> + 'a {
        self.entries
            .iter()
            .filter(move |(_, start, len, _)| {
                *len as usize == label.len()
                    && &self.text[*start as usize..(*start + *len) as usize] == label
            })
            .map(|(k, _, _, o)| (*k, *o))
    }

    /// `find`, narrowed to the asker. A guest sees the keys it opened
    /// under `label` and no one else's: labels are unique among siblings,
    /// not across a frame, and a guest cannot know what the host or
    /// another guest called its nodes (ADR 0014 — a script's env is a
    /// reading of its own view). The host, whose frame it is, sees its
    /// own first and everyone's when it opened none. Only a clash within
    /// what the asker sees is an ambiguity.
    pub(crate) fn find_for(&self, label: &str, origin: crate::tree::OriginId) -> Vec<Key> {
        let all: Vec<(Key, crate::tree::OriginId)> = self.find(label).collect();
        let mine: Vec<Key> = all
            .iter()
            .filter(|(_, o)| *o == origin)
            .map(|(k, _)| *k)
            .collect();
        if mine.is_empty() && origin == crate::tree::OriginId::HOST {
            all.into_iter().map(|(k, _)| k).collect()
        } else {
            mine
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_index_finds_in_tree_order_and_clears() {
        use crate::tree::OriginId;
        let mut idx = LabelIndex::default();
        idx.push(Key::ROOT.str("a"), "a", OriginId::HOST);
        idx.push(Key::ROOT.str("ab"), "ab", OriginId::HOST);
        idx.push(Key::ROOT.index(0).str("a"), "a", OriginId(1));
        let a: Vec<Key> = idx.find("a").map(|(k, _)| k).collect();
        assert_eq!(a, [Key::ROOT.str("a"), Key::ROOT.index(0).str("a")]);
        // The asker's own; a guest sees no one else's, the host sees
        // everyone's when it opened none.
        assert_eq!(idx.find_for("a", OriginId::HOST), [Key::ROOT.str("a")]);
        assert_eq!(
            idx.find_for("a", OriginId(1)),
            [Key::ROOT.index(0).str("a")]
        );
        assert_eq!(idx.find_for("a", OriginId(2)), []);
        assert_eq!(idx.find_for("ab", OriginId(1)), []);
        assert_eq!(idx.find_for("ab", OriginId::HOST), [Key::ROOT.str("ab")]);
        idx.push(Key::ROOT.str("g"), "g", OriginId(1));
        assert_eq!(idx.find_for("g", OriginId::HOST), [Key::ROOT.str("g")]);
        assert_eq!(idx.find("ab").count(), 1, "a prefix is not a match");
        assert_eq!(idx.find("b").count(), 0);
        assert_eq!(idx.label_of(Key::ROOT.str("ab")), Some("ab"));
        assert_eq!(idx.label_of(Key::ROOT.str("zz")), None);
        idx.clear();
        assert_eq!(idx.find("a").count(), 0);
        assert_eq!(idx.label_of(Key::ROOT.str("ab")), None);
    }

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
