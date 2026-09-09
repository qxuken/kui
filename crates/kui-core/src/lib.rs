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
pub mod cells;
pub mod color;
pub(crate) mod composite;
/// The scene corpus every binding is checked against. Test infrastructure,
/// behind the `conformance` feature so no shipped binary carries it.
#[cfg(feature = "conformance")]
pub mod conformance;
pub mod cursor;
pub mod depart;
pub mod diag;
pub mod display;
pub mod edit;
pub mod enter;
pub mod env;
pub mod fragment;
pub mod geom;
pub mod input;
pub mod key;
pub mod keyframes;
pub mod layout;
pub mod line;
pub mod menu;
pub mod resources;
pub mod runtime;
pub mod schema;
pub mod scroll;
pub mod select;
pub mod session;
pub mod slot;
pub mod spec;
pub mod stats;
pub mod text;
pub mod tree;
pub mod ui;
pub mod value;
pub mod widgets;
pub mod window;

pub use access::{
    AccessAction, AccessNode, AccessRequest, AccessRun, AccessTree, Announcement, Live,
    Orientation, Role, ScrollState, TextPos,
};
pub use anim::{Easing, Repeat, Transition};
pub use audio::{AudioCommand, AudioSpec, AudioStore, PlayOptions, PlaybackId, Why};
pub use cells::{Cell, CellGrid, CellStore, CellsId, CursorShape as CellCursor};
pub use color::Color;
pub use cursor::CursorShape;
pub use depart::DepartStore;
pub use diag::Warning;
pub use display::{Clip, ClipId, DisplayList, FragmentDraw, NO_CLIP, NO_CLIP_ID, Quad, QuadKind};
pub use edit::{EditOptions, MAX_UNDECLARED_EDITS};
pub use enter::Enter;
pub use env::{Appearance, Env, Locale, MotionPref, SystemEnv};
pub use fragment::{FragmentDrawId, FragmentList};
pub use geom::{Edges, Rect, Size, Vec2};
pub use input::ScrollAxis;
pub use input::{
    EditKey, InputEvent, KeyCode, KeyMods, KeyPhase, KeyPress, Mods, MouseButton, UiEvent,
};
pub use key::Key;
pub use keyframes::Keyframe;
pub use line::{LineId, LineStore, Stroke};
pub use menu::{Menu, MenuAction, MenuItem, MenuRole};
pub use resources::{FontId, FragmentId, ImageId, Resources, SessionId, SoundId};
pub use runtime::{Core, Extension};
pub use scroll::{MAX_UNDECLARED_SCROLLS, ScrollGeometry};
pub use select::{CellEnd, CellSelection, Endpoint, Selection};
pub use session::{Session, SharedAudio, SharedResources};
pub use slot::{Extensions, Fill, NAMESPACE_SEPARATOR, ROOT_SLOT, Slot, full_name, split_name};
pub use spec::{
    Align, Dir, FLOAT_PRESETS, FloatAnchor, FloatConfig, FontFamily, FontFeatures, Min, NodeSpec,
    OVERFLOW_CLIP, OVERFLOW_SCROLL_X, OVERFLOW_SCROLL_Y, PadShorthand, Shadow, Sizing, TextStyle,
    TextWrap, Vec2Offset, corner,
};
pub use stats::{FrameSample, FrameStats};
pub use text::{DEFAULT_TEXT_CACHE_BYTES, LONG_LINE_BYTES, Span, TextHit, TextMetrics};
pub use tree::OriginId;
pub use ui::Ui;
pub use value::Value;
pub use window::{
    DismissReason, WindowButton, WindowCommand, WindowConfig, WindowEnv, WindowId, WindowKind,
    WindowRole,
};
