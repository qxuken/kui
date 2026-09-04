//! The scene corpus: what every binding must agree on, as data.
//!
//! `schema::CUSTOM` and `schema::ELEMENTS` name the props and elements each
//! frontend lowers by hand. Naming them makes the transports agree on
//! *identity*; nothing made them agree on *behaviour*, so a binding could
//! (and did) drop a piece of one — `kui_tooltip` set no description, the
//! docs named a Lua float key the parser never read — with a green build.
//!
//! This module is the fix: a list of small named [`Scene`]s, each declaring
//! which `CUSTOM` and `ELEMENTS` rows it exercises, plus the input to
//! replay and the [`Expect`]ed semantics. [`drive`] runs one against a
//! `Core` and renders a [`report`] — a line-oriented text block with no
//! floating-point formatting in it, so four languages can produce it
//! byte-identically.
//!
//! Two tiers, because only one of them is portable across machines:
//!
//! - [`Scene::expect`] is checked in and font-independent (access rows,
//!   events, warnings, solid/image quad counts). `kui-core`'s own test
//!   asserts it, which pins the reference behaviour.
//! - The report carries a digest over full quad geometry, which depends on
//!   the installed fonts. It is never checked in: the reference is dumped
//!   at test time (`cargo run -p kui-core --example conformance-dump`) and
//!   the other three bindings — Lua, C, Node — reproduce their scenes and
//!   compare against that dump, on the same machine, in the same CI job.
//!
//! Adding a binding-visible prop or element means adding it to a scene
//! here; a binding that lowers it differently then fails to build.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::access::{AccessTree, Role};
use crate::audio::AudioSpec;
use crate::color::Color;
use crate::display::{Quad, QuadKind};
use crate::edit::EditOptions;
use crate::geom::{Edges, Size, Vec2};
use crate::input::InputEvent;
use crate::key::Key;
use crate::resources::{ImageId, SoundId};
use crate::runtime::Core;
use crate::spec::{Align, FloatConfig, NodeSpec, Sizing, TextStyle};
use crate::text::Span;
use crate::ui::Ui;
use crate::value::Value;
use crate::widgets;

/// Every scene is built at this viewport and scale. A binding that drives
/// its own frames has to use the same numbers or nothing lines up.
pub const VIEWPORT: Size = Size { w: 320.0, h: 240.0 };
pub const SCALE: f32 = 1.0;

/// The fixture image: 4x4 opaque white RGBA. Every binding registers the
/// same bytes in the same order, so the handles match and the image quad
/// comes out of the same atlas slot.
pub const IMAGE_W: u32 = 4;
pub const IMAGE_H: u32 = 4;

pub fn image_pixels() -> Vec<u8> {
    vec![0xff; (IMAGE_W * IMAGE_H * 4) as usize]
}

/// The fixture sound. Not decodable audio — an `<audio>` node draws nothing
/// and the corpus never plays it; it only has to be a registrable handle.
pub const SOUND_BYTES: &[u8] = b"RIFF....WAVE";

/// The `wrap` scene's boxes, as (width, height). Shared so every adapter
/// writes the same four and a typo cannot pass as a wrapping difference.
pub const WRAP_BOXES: &[(f32, f32)] = &[(30.0, 12.0), (40.0, 16.0), (50.0, 20.0), (20.0, 24.0)];

/// Handles a scene's builder needs, registered before the first frame.
#[derive(Clone, Copy)]
pub struct Fixtures {
    pub image: ImageId,
    pub sound: SoundId,
}

/// Registers the corpus fixtures on a fresh core, in this order.
pub fn fixtures(core: &mut Core) -> Fixtures {
    let image = core.resources.add_image(IMAGE_W, IMAGE_H, image_pixels());
    let sound = core.add_sound(SOUND_BYTES.to_vec());
    Fixtures { image, sound }
}

/// One replayed input. Values are integers so every adapter can print and
/// parse the step list without agreeing on float formatting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Logical coordinates.
    Cursor(i32, i32),
    CursorLeft,
    MouseDown,
    MouseUp,
    /// The secondary (context-menu) button.
    SecondaryDown,
    SecondaryUp,
    /// Wheel delta in logical px; positive y scrolls up.
    Scroll(i32, i32),
}

impl Step {
    /// The step's spelling in a report (`step cursor 50 30`).
    pub fn write(&self, out: &mut String) {
        match *self {
            Step::Cursor(x, y) => {
                let _ = writeln!(out, "step cursor {x} {y}");
            }
            Step::CursorLeft => out.push_str("step cursorleft\n"),
            Step::MouseDown => out.push_str("step mousedown\n"),
            Step::MouseUp => out.push_str("step mouseup\n"),
            Step::SecondaryDown => out.push_str("step secondarydown\n"),
            Step::SecondaryUp => out.push_str("step secondaryup\n"),
            Step::Scroll(x, y) => {
                let _ = writeln!(out, "step scroll {x} {y}");
            }
        }
    }

    pub fn event(&self) -> InputEvent {
        match *self {
            Step::Cursor(x, y) => InputEvent::CursorMoved(Vec2::new(x as f32, y as f32)),
            Step::CursorLeft => InputEvent::CursorLeft,
            Step::MouseDown => InputEvent::mouse_down(1),
            Step::MouseUp => InputEvent::mouse_up(),
            Step::SecondaryDown => InputEvent::MouseDown {
                button: crate::input::MouseButton::Secondary,
                clicks: 1,
            },
            Step::SecondaryUp => InputEvent::MouseUp {
                button: crate::input::MouseButton::Secondary,
            },
            Step::Scroll(x, y) => InputEvent::Scroll(Vec2::new(x as f32, y as f32)),
        }
    }
}

/// What a scene must produce, in the parts that do not depend on which
/// fonts are installed. Checked in; asserted by `tests/conformance.rs`.
pub struct Expect {
    /// Exact solid-quad count (backgrounds, borders, scrollbars, rings):
    /// geometry-driven, so it does not move with the font.
    pub solid: usize,
    /// Exact drop-shadow-quad count.
    pub shadows: usize,
    /// Exact image-quad count.
    pub images: usize,
    /// Glyph quads are one per rendered glyph — a lower bound keeps a font
    /// that maps a run differently from failing the build.
    pub glyphs_min: usize,
    /// The access tree in tree order, one `depth role name|description|value`
    /// per node (see [`report`]'s `node` lines, without the key and flags).
    pub access: &'static [&'static str],
    /// Events in order, `kind tag`.
    pub events: &'static [&'static str],
    /// Diagnostic codes, in order.
    pub warnings: &'static [&'static str],
    pub title: Option<&'static str>,
}

/// One scene: a builder every binding re-expresses, the input to replay,
/// and the rows of `schema::CUSTOM` / `schema::ELEMENTS` it pins.
pub struct Scene {
    pub name: &'static str,
    pub doc: &'static str,
    /// `schema::CUSTOM` names this scene exercises.
    pub custom: &'static [&'static str],
    /// `schema::ELEMENTS` names this scene exercises.
    pub elements: &'static [&'static str],
    /// The reference lowering. Every other binding expresses the same tree
    /// in its own surface; the report says whether it did.
    pub build: fn(&mut Ui<'_>, &Fixtures),
    pub steps: &'static [Step],
    pub expect: Expect,
}

pub fn scene(name: &str) -> Option<&'static Scene> {
    SCENES.iter().find(|s| s.name == name)
}

// ---------------------------------------------------------------------------
// The corpus

pub const SCENES: &[Scene] = &[
    Scene {
        name: "layout",
        doc: "Containers: both directions, the pad shorthand's four edges, a \
              border, a stable key, and plain and rich text at a declared \
              size. The card also carries the paint props that fade and \
              lift it: group opacity and a drop shadow.",
        custom: &["dir", "pad", "border", "key", "size"],
        elements: &["box", "text"],
        build: build_layout,
        steps: &[],
        expect: Expect {
            solid: 2,
            shadows: 1,
            images: 0,
            glyphs_min: 7,
            access: &[
                "0 window ||",
                "1 staticText ab||",
                "1 staticText cd||",
                "1 staticText a b c||",
            ],
            events: &[],
            warnings: &[],
            title: None,
        },
    },
    Scene {
        name: "wrap",
        doc: "A wrapping row: four fixed boxes broken onto two lines by a \
              width they don't fit, with a gap along a line and a cross gap \
              between them. All geometry, no text, so the digest is the \
              same wherever it runs.",
        custom: &["dir", "pad"],
        elements: &["box"],
        build: build_wrap,
        steps: &[],
        expect: Expect {
            solid: 5,
            shadows: 0,
            images: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            warnings: &[],
            title: None,
        },
    },
    Scene {
        name: "overflow",
        doc: "A clipping wrapper around a scrolling list, scrolled once: the \
              overflow composite, its retained offset and its scrollbar.",
        custom: &["overflow", "key"],
        elements: &["box"],
        build: build_overflow,
        steps: &[Step::Cursor(40, 40), Step::Scroll(0, -30)],
        expect: Expect {
            solid: 5,
            shadows: 0,
            images: 0,
            glyphs_min: 0,
            access: &["0 window ||", "1 scrollView ||"],
            events: &[],
            warnings: &[],
            title: None,
        },
    },
    Scene {
        name: "float",
        doc: "Both float spellings: the \"below\" shorthand and the full \
              config (anchor, at, self, dx/dy, fit).",
        custom: &["float", "key"],
        elements: &["box"],
        build: build_float,
        steps: &[],
        expect: Expect {
            solid: 3,
            shadows: 0,
            images: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            warnings: &[],
            title: None,
        },
    },
    Scene {
        name: "tooltip",
        doc: "The tooltip prop: hover tracking, the accessible description \
              it sets, and the hint that floats only while hovered.",
        custom: &["tooltip", "key"],
        elements: &["tooltip", "box", "text"],
        build: build_tooltip,
        steps: &[Step::Cursor(50, 30)],
        expect: Expect {
            solid: 2,
            shadows: 0,
            images: 0,
            glyphs_min: 10,
            access: &[
                "0 window ||",
                "1 group |a hint|",
                "2 staticText badge||",
                "2 staticText a hint||",
            ],
            events: &[],
            warnings: &[],
            title: None,
        },
    },
    Scene {
        name: "chrome",
        doc: "Window chrome: the frame's declared title, an adaptive \
              titlebar hosting custom content, the window buttons, and a \
              focusable box that claims key focus while it is declared.",
        custom: &["title", "keyFocus", "size"],
        elements: &["titlebar", "windowButtons", "box", "text"],
        build: build_chrome,
        steps: &[],
        expect: Expect {
            solid: 1,
            shadows: 0,
            images: 0,
            glyphs_min: 3,
            access: &[
                "0 window kui conformance||",
                "1 titleBar ||",
                "2 staticText app||",
                "1 group Sink||",
            ],
            events: &[],
            warnings: &[],
            title: Some("kui conformance"),
        },
    },
    Scene {
        name: "controls",
        doc: "A clicked button and a keyed editor, inside a panel that asks \
              for a context menu: the secondary press routes to the panel \
              and moves neither focus nor the caret.",
        custom: &["key", "size"],
        elements: &["button", "edit", "box", "text"],
        build: build_controls,
        steps: &[
            Step::Cursor(30, 24),
            Step::MouseDown,
            Step::MouseUp,
            Step::Cursor(4, 4),
            Step::SecondaryDown,
            Step::SecondaryUp,
        ],
        expect: Expect {
            solid: 1,
            shadows: 0,
            images: 0,
            glyphs_min: 7,
            access: &["0 window ||", "1 button go||", "1 textInput Note||hello"],
            events: &["go -", "contextmenu menu"],
            warnings: &[],
            title: None,
        },
    },
    Scene {
        name: "media",
        doc: "The non-text leaves: a registered image (deliberately unnamed, \
              so the diagnostic shows up too), a retained audio playback \
              that draws nothing, and the latency graph's empty chrome.",
        custom: &["size"],
        elements: &["image", "audio", "latencyGraph"],
        build: build_media,
        steps: &[],
        expect: Expect {
            solid: 2,
            shadows: 0,
            images: 1,
            glyphs_min: 20,
            access: &["0 window ||", "1 image ||"],
            events: &[],
            warnings: &["image-without-label"],
            title: None,
        },
    },
];

fn build_layout(ui: &mut Ui<'_>, _f: &Fixtures) {
    ui.with(
        NodeSpec::column()
            .pad(8.0)
            .gap(6.0)
            .bg(Color::hex(0x14161eff)),
        |ui| {
            ui.with_keyed(
                "card",
                NodeSpec::row()
                    .padding(Edges {
                        l: 12.0,
                        r: 10.0,
                        t: 6.0,
                        b: 4.0,
                    })
                    .gap(4.0)
                    .bg(Color::hex(0x202030ff))
                    .border(2.0, Color::hex(0x2a2d3aff))
                    .radius(5.0)
                    .opacity(0.75)
                    .shadow_color(Color::hex(0x00000066))
                    .shadow_blur(8.0)
                    .shadow_y(3.0)
                    .shadow_spread(1.0)
                    .width(Sizing::Fixed(180.0))
                    .height(Sizing::Fixed(40.0)),
                |ui| {
                    ui.text("ab", TextStyle::new(12.0));
                    ui.text("cd", TextStyle::new(12.0));
                },
            );
            ui.rich_text(
                &[
                    Span::new("a "),
                    Span::new("b").bold().color(Color::hex(0x73d98cff)),
                    Span::new(" c").italic(),
                ],
                TextStyle::new(13.0),
            );
        },
    );
}

/// 92px of content, a 6px gap: 30 + 40 fit, 50 + 20 go to the second line.
/// The heights differ per box so the two lines have different cross
/// extents and a binding that dropped `crossGap` lands them elsewhere.
fn build_wrap(ui: &mut Ui<'_>, _f: &Fixtures) {
    ui.with(
        NodeSpec::row()
            .wrap()
            .pad(4.0)
            .gap(6.0)
            .cross_gap(10.0)
            .width(Sizing::Fixed(100.0))
            .bg(Color::hex(0x101018ff)),
        |ui| {
            for (w, h) in WRAP_BOXES {
                ui.with(
                    NodeSpec::column()
                        .width(Sizing::Fixed(*w))
                        .height(Sizing::Fixed(*h))
                        .bg(Color::hex(0x30344aff)),
                    |_| {},
                );
            }
        },
    );
}

fn build_overflow(ui: &mut Ui<'_>, _f: &Fixtures) {
    ui.with(NodeSpec::column().pad(4.0).clip(), |ui| {
        ui.with_keyed(
            "list",
            NodeSpec::column()
                .width(Sizing::Fixed(120.0))
                .height(Sizing::Fixed(60.0))
                .gap(4.0)
                .scroll_y()
                .bg(Color::hex(0x101018ff)),
            |ui| {
                for key in ITEM_KEYS {
                    ui.with_keyed(
                        key,
                        NodeSpec::column()
                            .width(Sizing::Fixed(100.0))
                            .height(Sizing::Fixed(20.0))
                            .bg(Color::hex(0x30344aff)),
                        |_| {},
                    );
                }
            },
        );
    });
}

/// Item labels as constants: a binding building this scene has to use the
/// same strings, since keys are hashes of the path.
pub const ITEM_KEYS: [&str; 6] = ["i0", "i1", "i2", "i3", "i4", "i5"];

fn build_float(ui: &mut Ui<'_>, _f: &Fixtures) {
    ui.with(NodeSpec::column().pad(20.0).gap(4.0), |ui| {
        ui.with_keyed(
            "anchor",
            NodeSpec::column()
                .width(Sizing::Fixed(80.0))
                .height(Sizing::Fixed(24.0))
                .bg(Color::hex(0x333333ff)),
            |ui| {
                ui.with(
                    NodeSpec::column()
                        .float(FloatConfig::below())
                        .width(Sizing::Fixed(40.0))
                        .height(Sizing::Fixed(12.0))
                        .bg(Color::hex(0xff0000ff)),
                    |_| {},
                );
            },
        );
        ui.with(
            NodeSpec::column()
                .float(
                    FloatConfig::viewport()
                        .at(Align::End, Align::End)
                        .self_at(Align::End, Align::End)
                        .offset(-4.0, -4.0)
                        .fit(),
                )
                .width(Sizing::Fixed(10.0))
                .height(Sizing::Fixed(10.0))
                .bg(Color::hex(0x00ff00ff)),
            |_| {},
        );
    });
}

/// The tooltip prop, spelled out: the bindings' parsers turn `tooltip`
/// into `hoverable` plus an accessible `description`, and draw
/// `widgets::tooltip` as the node's last child while it is hovered.
fn build_tooltip(ui: &mut Ui<'_>, _f: &Fixtures) {
    ui.with(NodeSpec::column().pad(10.0), |ui| {
        let key = ui.child_key("tip");
        ui.with_keyed(
            "tip",
            NodeSpec::row()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(40.0))
                .bg(Color::hex(0x333333ff))
                .role(Role::Group)
                .hoverable()
                .description("a hint"),
            |ui| {
                ui.text("badge", TextStyle::new(12.0));
                if ui.is_hovered(key) {
                    widgets::tooltip(ui, "a hint");
                }
            },
        );
    });
}

fn build_chrome(ui: &mut Ui<'_>, _f: &Fixtures) {
    ui.window_title("kui conformance");
    ui.with(NodeSpec::column().gap(6.0), |ui| {
        widgets::titlebar_with(ui, |ui| {
            ui.text("app", TextStyle::new(12.0));
            widgets::window_buttons(ui);
        });
        let sink = ui.open_keyed(
            "sink",
            NodeSpec::column()
                .width(Sizing::Fixed(40.0))
                .height(Sizing::Fixed(16.0))
                .bg(Color::hex(0x22242cff))
                .focusable()
                .label("Sink"),
        );
        ui.take_key_focus(sink);
        ui.close();
    });
}

fn build_controls(ui: &mut Ui<'_>, _f: &Fixtures) {
    let panel = NodeSpec::column()
        .pad(10.0)
        .gap(6.0)
        .on_context_menu(Value::map([("kind", Value::str("menu"))]));
    ui.with(panel, |ui| {
        widgets::button(ui, "go", Value::map([("kind", Value::str("go"))]));
        ui.text_edit(
            "note",
            "hello",
            &EditOptions {
                style: TextStyle::new(13.0),
                ..Default::default()
            },
            NodeSpec::column().width(Sizing::Fixed(160.0)).label("Note"),
        );
    });
}

fn build_media(ui: &mut Ui<'_>, f: &Fixtures) {
    ui.with(NodeSpec::column().pad(6.0).gap(4.0), |ui| {
        ui.image(
            f.image,
            NodeSpec::column().width(Sizing::Fixed(16.0)).radius(2.0),
        );
        ui.audio_keyed("music", AudioSpec::new(f.sound).volume(0.5).looped());
        widgets::latency_graph(ui);
    });
}

// ---------------------------------------------------------------------------
// Running a scene

/// What one scene produced: quads as counts and a digest, the access tree,
/// the events the replay emitted, the diagnostics, the declared title.
pub struct Output {
    pub quad_count: usize,
    pub quad_digest: u64,
    /// Per [`QuadKind`], in its discriminant order.
    pub kinds: [usize; 6],
    pub nodes: Vec<NodeRow>,
    pub events: Vec<(String, String)>,
    pub warnings: Vec<&'static str>,
    pub title: Option<String>,
}

/// One access-tree node, flattened to what four languages can all report.
pub struct NodeRow {
    pub depth: usize,
    pub key: Key,
    pub role: &'static str,
    pub focused: bool,
    pub disabled: bool,
    /// `-` / `0` / `1`.
    pub checked: Option<bool>,
    pub scrollable: bool,
    /// Action names in `AccessAction::ALL` (bit) order, comma-joined.
    pub actions: String,
    pub name: String,
    pub description: String,
    pub value: String,
}

/// FNV-1a over the little-endian bytes of a quad's fields, `uv` excluded:
/// atlas coordinates depend on glyph insertion order, which a binding is
/// free to reach by a different route. Field order is `KuiQuad`'s: x, y, w,
/// h, color[4], border_color[4], radius[4], border_w, kind, clip[4] — words
/// 0..=17 and 22..=25 of the 26-word struct. Every adapter hashes the same
/// words, so a geometry difference is one mismatched hex string.
pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
pub const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn mix(h: &mut u64, word: u32) {
    for b in word.to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(FNV_PRIME);
    }
}

pub fn quad_digest(quads: &[Quad]) -> u64 {
    let mut h = FNV_OFFSET;
    for q in quads {
        for v in [
            q.rect.x,
            q.rect.y,
            q.rect.w,
            q.rect.h,
            q.color.r,
            q.color.g,
            q.color.b,
            q.color.a,
            q.border_color.r,
            q.border_color.g,
            q.border_color.b,
            q.border_color.a,
            q.radius[0],
            q.radius[1],
            q.radius[2],
            q.radius[3],
            q.border_w,
            q.blur,
        ] {
            mix(&mut h, v.to_bits());
        }
        mix(&mut h, q.kind as u32);
        for v in [q.clip.x, q.clip.y, q.clip.w, q.clip.h] {
            mix(&mut h, v.to_bits());
        }
    }
    h
}

fn rows(tree: &AccessTree) -> Vec<NodeRow> {
    let mut depth_of: HashMap<Key, usize> = HashMap::new();
    tree.nodes
        .iter()
        .map(|n| {
            let depth = n
                .parent
                .and_then(|p| depth_of.get(&p).copied())
                .map_or(0, |d| d + 1);
            depth_of.insert(n.key, depth);
            NodeRow {
                depth,
                key: n.key,
                role: n.role.name(),
                focused: n.focused,
                disabled: n.disabled,
                checked: n.checked,
                scrollable: n.scroll.is_some(),
                actions: n
                    .action_list()
                    .into_iter()
                    .map(|a| a.name())
                    .collect::<Vec<_>>()
                    .join(","),
                name: n.name.clone().unwrap_or_default(),
                description: n.description.clone().unwrap_or_default(),
                value: n.value.clone().unwrap_or_default(),
            }
        })
        .collect()
}

/// The one event shape the corpus compares: a payload's `kind`, and the
/// `kind` inside its `tag` when the core merged one in. Anything richer
/// would need a map walker in the C API, which there is none of.
fn event_row(payload: &Value) -> (String, String) {
    let kind = payload
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    let tag = payload
        .get("tag")
        .and_then(|t| t.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    (kind, tag)
}

/// Drives one scene's protocol against `core`, which must be fresh and
/// carry [`fixtures`]. Every binding reimplements exactly this:
///
/// 1. turn diagnostics on;
/// 2. build a frame, collect the events it left pending;
/// 3. for each step, apply the input, collect its events, build again;
/// 4. read the quads, access tree, warnings and title of the last frame.
pub fn drive(core: &mut Core, steps: &[Step], mut build: impl FnMut(&mut Ui<'_>)) -> Output {
    core.set_diagnostics(true);
    let mut events = Vec::new();
    let mut frame = |core: &mut Core, events: &mut Vec<(String, String)>| {
        let mut ui = core.frame(VIEWPORT, SCALE);
        build(&mut ui);
        ui.finish();
        events.extend(
            core.take_pending_events()
                .iter()
                .map(|e| event_row(&e.payload)),
        );
    };
    frame(core, &mut events);
    for step in steps {
        let evs = core.handle_input(step.event());
        events.extend(evs.iter().map(|e| event_row(&e.payload)));
        frame(core, &mut events);
    }

    let title = core.window_title().map(str::to_string);
    let warnings = core.take_warnings().into_iter().map(|w| w.code).collect();
    let nodes = rows(core.access_tree());
    let quads = &core.output().0.quads;
    let mut kinds = [0usize; 6];
    for q in quads.iter() {
        kinds[match q.kind {
            QuadKind::Solid => 0,
            QuadKind::GlyphMask => 1,
            QuadKind::GlyphColor => 2,
            QuadKind::Image => 3,
            QuadKind::GlyphSubpixel => 4,
            QuadKind::Shadow => 5,
        }] += 1;
    }
    Output {
        quad_count: quads.len(),
        quad_digest: quad_digest(quads),
        kinds,
        nodes,
        events,
        warnings,
        title,
    }
}

/// Runs a corpus scene through its own reference builder.
pub fn run(scene: &Scene) -> Output {
    let mut core = Core::new();
    let f = fixtures(&mut core);
    drive(&mut core, scene.steps, |ui| (scene.build)(ui, &f))
}

// ---------------------------------------------------------------------------
// The report

/// Renders a scene's block of the conformance report. The format is
/// line-oriented and carries no formatted floats — only integers, hex and
/// strings — so Rust, C and JavaScript produce the same bytes:
///
/// ```text
/// scene <name>
/// step <...>                 the replayed input, so an adapter need not restate it
/// title <text|->
/// quads <count> <digest:016x>
/// kinds <solid> <glyphMask> <glyphColor> <image> <glyphSubpixel> <shadow>
/// node <depth> <key:016x> <role> <focused> <disabled> <checked> <scroll> <actions> <name> | <description> | <value>
/// event <kind> <tag>
/// warn <code>
/// end
/// ```
pub fn report(name: &str, steps: &[Step], out: &Output) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "scene {name}");
    for step in steps {
        step.write(&mut s);
    }
    let _ = writeln!(s, "title {}", out.title.as_deref().unwrap_or("-"));
    let _ = writeln!(s, "quads {} {:016x}", out.quad_count, out.quad_digest);
    let _ = writeln!(
        s,
        "kinds {} {} {} {} {} {}",
        out.kinds[0], out.kinds[1], out.kinds[2], out.kinds[3], out.kinds[4], out.kinds[5]
    );
    for n in &out.nodes {
        let _ = writeln!(
            s,
            "node {} {:016x} {} {} {} {} {} {} {} | {} | {}",
            n.depth,
            n.key.0,
            n.role,
            n.focused as u8,
            n.disabled as u8,
            n.checked.map_or("-".to_string(), |c| (c as u8).to_string()),
            n.scrollable as u8,
            if n.actions.is_empty() {
                "-"
            } else {
                &n.actions
            },
            n.name,
            n.description,
            n.value,
        );
    }
    for (kind, tag) in &out.events {
        let _ = writeln!(s, "event {kind} {tag}");
    }
    for w in &out.warnings {
        let _ = writeln!(s, "warn {w}");
    }
    s.push_str("end\n");
    s
}

/// The whole corpus as one report — the reference the other three bindings
/// are diffed against.
pub fn reference_report() -> String {
    SCENES
        .iter()
        .map(|s| report(s.name, s.steps, &run(s)))
        .collect()
}

/// Splits a report into `(scene name, block)` pairs. Adapters use it to
/// find their scene's expected block, and the steps to replay.
pub fn blocks(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut name = String::new();
    let mut block = String::new();
    for line in text.lines() {
        if let Some(n) = line.strip_prefix("scene ") {
            name = n.to_string();
            block.clear();
        }
        block.push_str(line);
        block.push('\n');
        if line == "end" {
            out.push((std::mem::take(&mut name), std::mem::take(&mut block)));
        }
    }
    out
}
