//! Long-lived, host-registered resources. Slotmap keys give typed handles
//! with generational use-after-free protection, and convert to/from `u64`
//! (`KeyData::as_ffi`) so they cross the scripting boundary as plain integers
//! with the generation check intact on the way back.

use slotmap::{SlotMap, new_key_type};

new_key_type! {
    pub struct ImageId;
    pub struct PainterId;
}

impl ImageId {
    /// The handle as a plain integer for C/Lua (generation check intact).
    pub fn to_ffi(self) -> u64 {
        use slotmap::Key as _;
        self.data().as_ffi()
    }

    pub fn from_ffi(raw: u64) -> Self {
        Self::from(slotmap::KeyData::from_ffi(raw))
    }
}

/// An RGBA image registered by the host (rendering lands in a later pass).
pub struct ImageEntry {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Default)]
pub struct Resources {
    pub images: SlotMap<ImageId, ImageEntry>,
}

impl Resources {
    pub fn add_image(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> ImageId {
        debug_assert_eq!(rgba.len(), (width * height * 4) as usize);
        self.images.insert(ImageEntry { width, height, rgba })
    }

    pub fn remove_image(&mut self, id: ImageId) -> Option<ImageEntry> {
        self.images.remove(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::{Key, KeyData};

    #[test]
    fn stale_handle_is_rejected_after_removal() {
        let mut r = Resources::default();
        let id = r.add_image(1, 1, vec![0; 4]);
        r.remove_image(id);
        assert!(r.images.get(id).is_none());
        // A new insert reuses the slot but bumps the generation.
        let id2 = r.add_image(1, 1, vec![0; 4]);
        assert_ne!(id, id2);
        assert!(r.images.get(id).is_none());
        assert!(r.images.get(id2).is_some());
    }

    #[test]
    fn handles_round_trip_through_u64() {
        let mut r = Resources::default();
        let id = r.add_image(1, 1, vec![0; 4]);
        let raw = id.data().as_ffi();
        let back = ImageId::from(KeyData::from_ffi(raw));
        assert_eq!(id, back);
        assert!(r.images.get(back).is_some());
    }
}
