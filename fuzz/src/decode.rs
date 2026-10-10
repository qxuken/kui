//! The runner's image decoder (`kui_native::decode_image`,
//! `decode_animation`): bytes an app ships or downloads, in any of the four
//! formats, or none of them.
//!
//! What comes back is what `Core::add_image` takes: `width * height * 4`
//! bytes per frame. [`kui_native::Animation::at`] names a frame there is
//! at every moment, and a due time that is a time.

pub fn run(data: &[u8]) {
    if let Ok(p) = kui_native::decode_image(data) {
        assert_eq!(
            p.rgba.len(),
            p.width as usize * p.height as usize * 4,
            "{}x{} image",
            p.width,
            p.height
        );
    }
    let Ok(a) = kui_native::decode_animation(data) else {
        return;
    };
    assert!(!a.frames.is_empty());
    let len = a.width as usize * a.height as usize * 4;
    for (i, f) in a.frames.iter().enumerate() {
        assert_eq!(f.rgba.len(), len, "frame {i} of a {}x{}", a.width, a.height);
        assert!(f.delay >= 0.0, "frame {i} shows for {}", f.delay);
    }
    for t in [-1.0, 0.0, 0.05, 1.0, 3600.0, f64::MAX] {
        let s = a.at(t);
        assert!(s.index < a.frames.len(), "at {t}: frame {}", s.index);
        assert!(!s.next.is_nan(), "at {t}: next is NaN");
    }
}
