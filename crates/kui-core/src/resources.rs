//! Long-lived, host-registered resources. Slotmap keys give typed handles
//! with generational use-after-free protection, and convert to/from `u64`
//! (`KeyData::as_ffi`) so they cross the scripting boundary as plain integers
//! with the generation check intact on the way back.

use slotmap::{SlotMap, new_key_type};

use crate::spec::FontFamily;

new_key_type! {
    pub struct ImageId;
    pub struct PainterId;
    /// A registered font (`Core::add_font_data` / `add_system_font`), used
    /// through `TextStyle::font`.
    pub struct FontId;
}

impl FontId {
    /// The handle as a plain integer for C/Lua/JS (generation check intact).
    pub fn to_ffi(self) -> u64 {
        use slotmap::Key as _;
        self.data().as_ffi()
    }

    pub fn from_ffi(raw: u64) -> Self {
        Self::from(slotmap::KeyData::from_ffi(raw))
    }
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

/// A registered font: the family name shaping resolves it by, and the
/// faces it loaded into the font database (empty for installed fonts).
pub struct FontEntry {
    pub family: String,
    pub faces: Vec<cosmic_text::fontdb::ID>,
}

#[derive(Default)]
pub struct Resources {
    pub images: SlotMap<ImageId, ImageEntry>,
    pub fonts: SlotMap<FontId, FontEntry>,
}

impl Resources {
    pub(crate) fn add_font(
        &mut self,
        family: String,
        faces: Vec<cosmic_text::fontdb::ID>,
    ) -> FontId {
        self.fonts.insert(FontEntry { family, faces })
    }

    pub(crate) fn remove_font(&mut self, id: FontId) -> Option<FontEntry> {
        self.fonts.remove(id)
    }

    /// The registered family name, if the handle is live.
    pub fn font_family(&self, id: FontId) -> Option<&str> {
        self.fonts.get(id).map(|f| f.family.as_str())
    }

    /// The cosmic-text family a style's `FontFamily` shapes with; an unknown
    /// or removed custom font falls back to sans-serif.
    pub(crate) fn family_of(&self, f: FontFamily) -> cosmic_text::Family<'_> {
        match f {
            FontFamily::Sans => cosmic_text::Family::SansSerif,
            FontFamily::Serif => cosmic_text::Family::Serif,
            FontFamily::Mono => cosmic_text::Family::Monospace,
            FontFamily::Custom(id) => match self.font_family(id) {
                Some(name) => cosmic_text::Family::Name(name),
                None => cosmic_text::Family::SansSerif,
            },
        }
    }

    pub fn add_image(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> ImageId {
        debug_assert_eq!(rgba.len(), (width * height * 4) as usize);
        self.images.insert(ImageEntry {
            width,
            height,
            rgba,
        })
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
