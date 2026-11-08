#![cfg_attr(feature = "gui", doc = include_str!("gui.md"))]
#![cfg_attr(not(feature = "gui"), doc = "Native system integration for kui hosts.")]

pub use kui_core::widgets;
pub use kui_core::*;

pub mod spawn;
pub mod system;
pub use spawn::set_spawner;
pub use system::{open_external, open_recording, set_runner, show_in_folder};

#[cfg(feature = "image-codecs")]
mod decode;

#[cfg(feature = "image-codecs")]
pub use decode::{
    Animation, AnimationFrame, MAX_ANIMATION_BYTES, Pixels, Showing, decode_animation, decode_image,
};

#[cfg(feature = "image-codecs")]
mod encode;

#[cfg(feature = "image-codecs")]
pub use encode::{AnimationEncoder, AnimationEncoding, ImageEncoding, encode_image};

#[cfg(feature = "gui")]
#[path = "gui.rs"]
mod gui;
#[cfg(feature = "gui")]
pub use gui::*;
#[cfg(feature = "gui")]
pub use gui::{audio, testing};

/// kui's reference documents, carried with the crate so they read as this
/// version wrote them (ADR 0071): [`props`](docs::props) — every prop,
/// element, event, warning, env fact, theme role and door, in each
/// binding's spelling — [`howto`](docs::howto), the task index, and
/// [`adr`](docs::adr), the design records listed and linked. Copies of
/// `docs/` in the repository that `scripts/gen.nu` writes and checks.
///
/// The pages are written for GitHub and the npm package as much as for
/// rustdoc: a `<A-u>` in their prose, or a Rust fragment shown rather than
/// compiled, is meant as it is.
#[allow(rustdoc::invalid_html_tags, rustdoc::invalid_rust_codeblocks)]
pub mod docs {
    #[doc = include_str!("../docs/props.md")]
    pub mod props {}
    #[doc = include_str!("../docs/howto.md")]
    pub mod howto {}
    #[doc = include_str!("../docs/adr.md")]
    pub mod adr {}
}
