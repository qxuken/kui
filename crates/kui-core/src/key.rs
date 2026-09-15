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
/// mixer: a digest that must stay bit-stable across versions (a key, the
/// access tree's change detector, the corpus digest) is stable because
/// it is this function and nothing else. A byte a round with a multiply
/// on the chain — a nanosecond a byte — which is the right cost for a
/// label and the wrong one for a megabyte of text: that goes through
/// [`hash_bulk`] first.
#[inline]
pub fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Below this many bytes the content is mixed by [`fnv`] as it is: the
/// word hash's set-up costs more than the bytes.
const BULK_MIN: usize = 32;

/// A word-wide hash of `bytes` — eight a round where [`fnv`] takes one —
/// for the bulk a text cache key is made of (backlog C43): a long line's
/// content used to cost its length every frame, at a nanosecond a byte,
/// in a lookup that drew none of it. Fx's round (rotate, xor, multiply)
/// with the length mixed first and murmur's finalizer after, so a tail
/// of zero bytes and a shorter text differ and every input bit reaches
/// every output bit. Not a digest anything keeps across versions.
#[inline]
pub fn hash_bulk(bytes: &[u8]) -> u64 {
    const K: u64 = 0x517c_c1b7_2722_0a95;
    let mut h = FNV_OFFSET ^ (bytes.len() as u64).wrapping_mul(K);
    let (words, tail) = bytes.as_chunks::<8>();
    for w in words {
        h = (h.rotate_left(5) ^ u64::from_le_bytes(*w)).wrapping_mul(K);
    }
    if !tail.is_empty() {
        let mut t = [0u8; 8];
        t[..tail.len()].copy_from_slice(tail);
        h = (h.rotate_left(5) ^ u64::from_le_bytes(t)).wrapping_mul(K);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^ (h >> 33)
}

/// Mixes content into a key: short content by the byte, long content
/// through [`hash_bulk`] and its eight bytes by the byte.
#[inline]
pub fn mix_content(h: u64, bytes: &[u8]) -> u64 {
    if bytes.len() < BULK_MIN {
        fnv(h, bytes)
    } else {
        fnv(h, &hash_bulk(bytes).to_le_bytes())
    }
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
    /// Answers the first key in tree order and how many there were, so
    /// the caller can warn of a clash — without allocating, since `key_of`
    /// is asked from view code every frame.
    pub(crate) fn find_for(
        &self,
        label: &str,
        origin: crate::tree::OriginId,
    ) -> (Option<Key>, usize) {
        let mut mine = self.find(label).filter(|(_, o)| *o == origin);
        let first = mine.next();
        if let Some((k, _)) = first {
            return (Some(k), 1 + mine.count());
        }
        if origin != crate::tree::OriginId::HOST {
            return (None, 0);
        }
        let mut all = self.find(label);
        match all.next() {
            Some((k, _)) => (Some(k), 1 + all.count()),
            None => (None, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The word hash tells apart what a word-at-a-time reading could
    /// confuse: a text and the same with a zero byte after it, a change
    /// in the tail, a change on a word boundary, and the two sides of
    /// `mix_content`'s threshold (backlog C43).
    #[test]
    fn the_bulk_hash_separates_tails_and_lengths() {
        let a = b"0123456789abcdef0123456789abcdef0123456789abcdef";
        let mut with_zero = a.to_vec();
        with_zero.push(0);
        assert_ne!(hash_bulk(a), hash_bulk(&with_zero));
        let mut tail = a.to_vec();
        tail[47] ^= 1;
        assert_ne!(hash_bulk(a), hash_bulk(&tail));
        let mut word = a.to_vec();
        word[8] ^= 1;
        assert_ne!(hash_bulk(a), hash_bulk(&word));
        assert_eq!(hash_bulk(a), hash_bulk(a.as_ref()));
        assert_ne!(hash_bulk(b""), hash_bulk(b"\0"));
        assert_ne!(hash_bulk(b"abcdefg"), hash_bulk(b"abcdefg\0"));
        // Either side of the threshold is a function of the bytes.
        let short = b"0123456789abcdef0123456789abcde";
        let long = b"0123456789abcdef0123456789abcdef";
        assert_eq!(mix_content(FNV_OFFSET, short), fnv(FNV_OFFSET, short));
        assert_eq!(
            mix_content(FNV_OFFSET, long),
            fnv(FNV_OFFSET, &hash_bulk(long).to_le_bytes())
        );
        assert_ne!(mix_content(FNV_OFFSET, short), mix_content(FNV_OFFSET, long));
    }

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
        assert_eq!(
            idx.find_for("a", OriginId::HOST),
            (Some(Key::ROOT.str("a")), 1)
        );
        assert_eq!(
            idx.find_for("a", OriginId(1)),
            (Some(Key::ROOT.index(0).str("a")), 1)
        );
        assert_eq!(idx.find_for("a", OriginId(2)), (None, 0));
        assert_eq!(idx.find_for("ab", OriginId(1)), (None, 0));
        assert_eq!(
            idx.find_for("ab", OriginId::HOST),
            (Some(Key::ROOT.str("ab")), 1)
        );
        idx.push(Key::ROOT.str("g"), "g", OriginId(1));
        idx.push(Key::ROOT.index(1).str("g"), "g", OriginId(2));
        assert_eq!(
            idx.find_for("g", OriginId::HOST),
            (Some(Key::ROOT.str("g")), 2),
            "the host sees both guests' and hears of the clash"
        );
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
