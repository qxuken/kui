//! kui-core: the data-driven contract every frontend and backend binds to.
//!
//! - Flat per-frame tree ([`tree::Tree`]) built through [`ui::Ui`]
//! - Clay-style flex solver ([`layout`])
//! - Session-owned text stack: shaping, wrapping, caching, atlas ([`text`], [`atlas`], [`session`])
//! - Events as data ([`input`], [`value::Value`]) routed by origin
//! - Renderer boundary: a flat quad list ([`display::DisplayList`])

pub mod access;
pub mod anim;
pub mod atlas;
pub mod audio;
pub mod color;
pub(crate) mod composite;
pub mod conformance;
pub mod cursor;
pub mod depart;
pub mod diag;
pub mod display;
pub mod edit;
pub mod enter;
pub mod env;
pub mod geom;
pub mod input;
pub mod key;
pub mod keyframes;
pub mod layout;
pub mod resources;
pub mod runtime;
pub mod schema;
pub mod scroll;
pub mod session;
pub mod spec;
pub mod stats;
pub mod text;
pub mod tree;
pub mod ui;
pub mod value;
pub mod widgets;
pub mod window;

pub use access::{
    AccessAction, AccessNode, AccessRequest, AccessRun, AccessTree, Orientation, Role, ScrollState,
    TextPos,
};
pub use anim::{Easing, Repeat, Transition};
pub use audio::{AudioCommand, AudioSpec, AudioStore, PlayOptions, PlaybackId};
pub use color::Color;
pub use cursor::CursorShape;
pub use depart::DepartStore;
pub use diag::Warning;
pub use display::{Clip, DisplayList, NO_CLIP, Quad, QuadKind};
pub use edit::EditOptions;
pub use enter::Enter;
pub use env::Env;
pub use geom::{Edges, Rect, Size, Vec2};
pub use input::ScrollAxis;
pub use input::{
    EditKey, InputEvent, KeyCode, KeyMods, KeyPhase, KeyPress, Mods, MouseButton, UiEvent,
};
pub use key::Key;
pub use keyframes::Keyframe;
pub use resources::{FontId, ImageId, Resources, SoundId};
pub use runtime::{Core, Extension};
pub use scroll::ScrollGeometry;
pub use session::{Session, SharedAudio, SharedResources};
pub use spec::{
    Align, Dir, FLOAT_PRESETS, FloatAnchor, FloatConfig, FontFamily, NodeSpec, OVERFLOW_CLIP,
    OVERFLOW_SCROLL_X, OVERFLOW_SCROLL_Y, PadShorthand, Shadow, Sizing, TextStyle, TextWrap,
    Vec2Offset, corner,
};
pub use stats::{FrameSample, FrameStats};
pub use text::{Span, TextMetrics};
pub use tree::OriginId;
pub use ui::Ui;
pub use value::Value;
pub use window::{WindowButton, WindowCommand, WindowEnv, WindowId, WindowRole};
