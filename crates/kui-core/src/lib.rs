//! The headless model behind kui: a per-frame flat tree, flex layout, text shaping, events as data and a quad display list.
//!
//! A [`Core`] owns one window's worth of state. Each frame
//! the app rebuilds a flat tree of [`NodeSpec`]s through a
//! [`Ui`], the core runs a clay-style flex layout over it, shapes
//! the text, and emits a [`DisplayList`] of quads for
//! a renderer to draw. Input arrives as [`InputEvent`]s
//! and comes back out as [`UiEvent`]s whose payloads are
//! plain-data [`Value`]s. There is no window, no GPU and no
//! clock in this crate: a driver supplies the viewport, the input, the
//! time and the renderer.
//!
//! Most Rust apps do not depend on `kui-core` directly. They use
//! [`kui-native`](https://crates.io/crates/kui-native), the windowed runner,
//! which re-exports all of this crate and pairs it with
//! [`kui-wgpu`](https://crates.io/crates/kui-wgpu), the renderer. Reach for
//! `kui-core` on its own when you are writing a custom runner, a binding to
//! another language (`kui-lua`, `kui-ffi` and `kui-node` are built on it),
//! or headless tests that build frames and feed input without a window.
//!
//! # Quick start
//!
//! A frame built and clicked with no window at all:
//!
//! ```rust
//! use kui_core::{Core, InputEvent, NodeSpec, Size, TextStyle, widgets};
//!
//! let mut core = Core::new();
//! core.set_inspect(true); // keep a readable snapshot of each finished frame
//!
//! // One frame: begin, declare the tree, finish (layout and emission).
//! let mut ui = core.frame(Size::new(320.0, 200.0), 1.0);
//! ui.configure_root(NodeSpec::column().fill().pad(16.0).gap(8.0));
//! ui.text("Hello from kui-core", TextStyle::new(16.0));
//! widgets::button(&mut ui, "Save", "save");
//! ui.finish();
//!
//! // What a renderer draws: a flat list of quads.
//! let (list, _atlas) = core.output();
//! assert!(!list.quads.is_empty());
//!
//! // Input goes in as `InputEvent`s and comes out as `UiEvent`s carrying
//! // the payload the view declared.
//! let button = core
//!     .nodes()
//!     .into_iter()
//!     .find(|n| n.label.as_deref() == Some("Save"))
//!     .expect("the button was laid out");
//! core.handle_input(InputEvent::CursorMoved(button.rect.center()));
//! core.handle_input(InputEvent::mouse_down(1));
//! let events = core.handle_input(InputEvent::mouse_up());
//! let click = events
//!     .iter()
//!     .find(|e| e.payload.as_str() == Some("save"))
//!     .expect("the click reached the button");
//! assert_eq!(click.key, button.key);
//! ```
//!
//! A real driver repeats the frame whenever input arrives or the core asks
//! for one, hands the display list to a renderer, and calls
//! [`Core::set_time`](runtime::Core::set_time) before each frame so
//! transitions can run.
//!
//! # Where to look
//!
//! - [`ui::Ui`]: the frame builder. `open`/`close`, `with`, `leaf`, `text`,
//!   and the readbacks a view needs (`is_hovered`, `focused`, `theme`).
//! - [`runtime::Core`]: the per-window state. `frame`, `handle_input`,
//!   `output`, `set_time`, fonts and images, focus and scrolling.
//! - [`spec::NodeSpec`] and [`spec::TextStyle`]: everything a node and a
//!   text declare, as plain data with a builder.
//! - [`layout`]: the flex solver, and what each sizing means.
//! - [`text`]: shaping, wrapping, measuring and the glyph atlas.
//! - [`input`] and [`input::UiEvent`]: what goes in and what comes out;
//!   [`event`] has typed readings of the core's own event payloads.
//! - [`value::Value`]: the payload type, and [`message`] for typed messages
//!   over it.
//! - [`display::DisplayList`]: the renderer boundary.
//! - [`widgets`]: buttons, toggles, inputs, menus and virtual lists built
//!   from the primitives.
//! - [`theme`], [`metrics`] and [`tokens`]: the colours, sizes and named
//!   values a view paints with.
//! - [`session`]: fonts, images and sounds shared between windows.
//!
//! # Features
//!
//! - `devtools` (default): the inspector panel the core can draw into any
//!   app's frame (`Core::set_devtools`, or `KUI_DEVTOOLS=1`).
//! - `derive`: re-exports `#[derive(Message)]` from `kui-derive`, a typed
//!   Rust enum to and from the payload map.
//! - `conformance`: the scene corpus and the headless test driver
//!   (`testing`). Test infrastructure, off in every shipped binary.
//!
//! # Links
//!
//! - The book: <https://kui-book.qxuken.dev>
//! - The repository: <https://github.com/qxuken/kui> (design records live
//!   under `docs/adr` there)

pub mod access;
pub mod anim;
pub mod atlas;
pub mod audio;
pub mod calc;
pub mod cells;
pub mod color;
pub(crate) mod composite;
/// The scene corpus every binding is checked against. Test infrastructure,
/// behind the `conformance` feature so no shipped binary carries it.
#[cfg(feature = "conformance")]
pub mod conformance;
pub mod cursor;
pub mod deco;
pub mod depart;
pub mod diag;
pub mod dialog;
pub mod display;
pub mod edit;
pub mod enter;
pub mod env;
pub mod event;
pub mod fragment;
pub mod geom;
pub mod input;
pub(crate) mod join;
pub mod key;
pub mod keyframes;
pub mod layout;
pub mod line;
pub mod menu;
pub mod message;
pub mod metrics;
pub mod path;
pub mod resources;
pub(crate) mod retain;
pub mod runtime;
pub mod schema;
pub mod scroll;
pub mod select;
pub mod session;
pub mod slider;
pub mod slot;
pub mod slots;
pub mod spec;
pub mod stats;
/// The headless driver the crate's own tests use. Test infrastructure,
/// behind the `conformance` feature like the corpus.
#[cfg(feature = "conformance")]
pub mod testing;
pub mod text;
pub mod theme;
pub mod tokens;
pub mod tree;
pub mod ui;
pub mod value;
pub(crate) mod weights;
pub mod widgets;
pub mod window;

pub use access::{
    AccessAction, AccessNode, AccessRequest, AccessRun, AccessTree, Announcement, Live,
    Orientation, Role, ScrollState, TextPos,
};
pub use anim::{Bounce, Easing, MAX_BOUNCE, Repeat, Transition};
pub use audio::{AudioCommand, AudioSpec, AudioStore, PlayOptions, PlaybackId, Why};
pub use calc::{Calc, Expr as SizeExpr};
pub use cells::{Cell, CellGrid, CellStore, CellsId, CursorShape as CellCursor};
pub use color::Color;
pub use cursor::CursorShape;
pub use depart::DepartStore;
pub use diag::Warning;
pub use dialog::{FileDialog, FileDialogMode, FileFilter};
pub use display::{
    Clip, ClipId, DisplayList, FragmentDraw, FragmentImage, NO_CLIP, NO_CLIP_ID, Quad, QuadKind,
};
pub use edit::{EditOptions, MAX_UNDECLARED_EDITS};
pub use enter::Enter;
pub use env::{Appearance, Assistive, AudioDevice, AudioEnv, Env, Locale, MotionPref, SystemEnv};
pub use event::{
    ButtonEvent, ButtonPhase, Drag, DragPhase, Hover, HoverBy, HoverPhase, Layout, Scroll,
    TextInput,
};
pub use fragment::{FragmentDrawId, FragmentList, FragmentRef};
pub use geom::{Edges, Rect, Size, Vec2};
pub use input::ScrollAxis;
pub use input::{
    Buttons, ClipboardMarks, EditKey, InputEvent, KeyCode, KeyLocation, KeyLocks, KeyMods,
    KeyPhase, KeyPress, LayoutScript, Mods, MouseButton, OptionAsAlt, UiEvent,
};
pub use key::Key;
pub use keyframes::Keyframe;
/// `#[derive(Message)]`, with the `derive` feature (`kui-native` turns it
/// on). The generated code reaches `::kui_native` unless told otherwise,
/// so a crate that depends on kui-core alone adds
/// `#[message(crate = "kui_core")]` to the enum.
#[cfg(feature = "derive")]
pub use kui_derive::Message;
pub use line::{LineId, LineStore, Stroke};
pub use menu::{Accel, BarMenu, Menu, MenuAction, MenuBar, MenuItem, MenuRole};
pub use message::{MessageError, MessageField};
pub use metrics::Metrics;
pub use path::{FillRule, Path, PathError, PathId, PathOp, PathStore, Turn};
pub use resources::{
    FontId, FragmentId, ImageBacking, ImageFit, ImageId, ImageOpts, Resources, Sampling, SessionId,
    SoundId, SystemFont,
};
pub use runtime::cause::{FrameCause, FrameHolder, FrameRequest, OwedBy};
pub use runtime::devtools;
pub use runtime::devtools::Dock as DevtoolsDock;
pub use runtime::inspect::{NodeInfo, NodeKind};
pub use runtime::{Content, Core, Extension, Owed};
pub use scroll::{MAX_UNDECLARED_SCROLLS, ScrollGeometry};
pub use select::{CellEnd, CellSelection, CopyRequest, Endpoint, Grain, RangeEnd, Selection};
pub use session::{Session, SharedAudio, SharedResources};
pub use slot::{
    ANY_SLOT, Extensions, Fill, NAMESPACE_SEPARATOR, ROOT_SLOT, Slot, full_name, split_name,
};
pub use spec::{
    Align, Bound, Dir, FLOAT_PRESETS, FloatAnchor, FloatConfig, FontFamily, FontFeatures, Min,
    NodeSpec, OVERFLOW_CLIP, OVERFLOW_SCROLL_X, OVERFLOW_SCROLL_Y, Overscroll, PadShorthand,
    ScrollAxes, Scrollbar, ScrollbarMode, Shadow, Sizing, TextStyle, TextWrap, UnderlineStyle,
    Vec2Offset, corner,
};
pub use stats::{FrameSample, FrameStats};
pub use text::{DEFAULT_TEXT_CACHE_BYTES, LONG_LINE_BYTES, Span, TextHit, TextMetrics};
pub use theme::{Theme, ThemeSource};
pub use tokens::{
    ColorOp, ColorToken, NameRefs, OpRange, TokenError, TokenKind, TokenLookup, TokenRef, Tokens,
    Unresolved,
};
pub use tree::OriginId;
pub use ui::Ui;
pub use value::{Handles, Value};
pub use window::{
    DismissReason, WindowButton, WindowCommand, WindowConfig, WindowEnv, WindowId, WindowKind,
    WindowRole,
};
