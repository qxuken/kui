//! kui-core: the data-driven contract every frontend and backend binds to.
//!
//! - Flat per-frame tree ([`tree::Tree`]) built through [`ui::Ui`]
//! - Clay-style flex solver ([`layout`])
//! - Core-owned text stack: shaping, wrapping, caching, atlas ([`text`], [`atlas`])
//! - Events as data ([`input`], [`value::Value`]) routed by origin
//! - Renderer boundary: a flat quad list ([`display::DisplayList`])

pub mod anim;
pub mod atlas;
pub mod color;
pub mod display;
pub mod edit;
pub mod env;
pub mod geom;
pub mod input;
pub mod key;
pub mod layout;
pub mod resources;
pub mod runtime;
pub mod schema;
pub mod scroll;
pub mod spec;
pub mod stats;
pub mod text;
pub mod tree;
pub mod ui;
pub mod value;
pub mod widgets;
pub mod window;

pub use anim::{Easing, Transition};
pub use color::Color;
pub use display::{DisplayList, NO_CLIP, Quad, QuadKind};
pub use edit::EditOptions;
pub use env::Env;
pub use geom::{Edges, Rect, Size, Vec2};
pub use input::ScrollAxis;
pub use input::{EditKey, InputEvent, KeyCode, KeyMods, KeyPress, Mods, UiEvent};
pub use key::Key;
pub use resources::{ImageId, Resources};
pub use runtime::{Core, Extension};
pub use spec::{Align, Dir, FloatAnchor, FloatConfig, FontFamily, NodeSpec, Sizing, TextStyle};
pub use stats::{FrameSample, FrameStats};
pub use text::Span;
pub use tree::OriginId;
pub use ui::Ui;
pub use value::Value;
pub use window::{WindowButton, WindowCommand, WindowEnv, WindowRole};
