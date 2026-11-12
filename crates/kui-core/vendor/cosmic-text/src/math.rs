// Modified for kui to embed this source as a private module.
#[cfg(any())]
pub use libm::{floorf, roundf, truncf};

#[inline]
pub fn floorf(x: f32) -> f32 {
    x.floor()
}

#[inline]
pub fn roundf(x: f32) -> f32 {
    x.round()
}

#[inline]
pub fn truncf(x: f32) -> f32 {
    x.trunc()
}
