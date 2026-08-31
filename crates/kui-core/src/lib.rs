//! kui-core: the data-driven contract every frontend and backend binds to.
//!
//! - Flat per-frame tree ([`tree::Tree`]) built through [`ui::Ui`]
//! - Clay-style flex solver ([`layout`])
//! - Core-owned text stack: shaping, wrapping, caching, atlas ([`text`], [`atlas`])
//! - Events as data ([`input`], [`value::Value`]) routed by origin
//! - Renderer boundary: a flat quad list ([`display::DisplayList`])

pub mod atlas;
pub mod color;
pub mod display;
pub mod geom;
pub mod input;
pub mod key;
pub mod layout;
pub mod resources;
pub mod runtime;
pub mod scroll;
pub mod spec;
pub mod text;
pub mod tree;
pub mod ui;
pub mod value;
pub mod widgets;

pub use color::Color;
pub use display::{DisplayList, NO_CLIP, Quad, QuadKind};
pub use geom::{Edges, Rect, Size, Vec2};
pub use input::{InputEvent, UiEvent};
pub use key::Key;
pub use runtime::{Core, Extension};
pub use spec::{Align, Dir, NodeSpec, Sizing, TextStyle};
pub use text::Span;
pub use tree::OriginId;
pub use ui::Ui;
pub use value::Value;
