//! The `Core`: owns everything that survives across frames (text caches,
//! glyph atlas, interaction state, registered resources) plus the reusable
//! per-frame tree — and the frame-builder state itself. Builder state living
//! here (not in a borrowing wrapper) is what lets flat C bindings drive a
//! frame through one opaque pointer; the Rust `Ui` is a thin safe façade.

use crate::atlas::GlyphAtlas;
use crate::color::Color;
use crate::display::{DisplayList, NO_CLIP, Quad, QuadKind};
use crate::geom::{Rect, Size, Vec2};
use crate::input::{HitRegion, InputEvent, Interaction, ScrollRegion, UiEvent};
use crate::key::Key;
use crate::layout;
use crate::resources::Resources;
use crate::scroll::ScrollStore;
use crate::spec::{NodeSpec, Sizing, TextStyle};
use crate::text::{Span, TextSystem};
use crate::tree::{NIL, NodeContent, OriginId, Tree};
use crate::ui::Ui;

/// Wheel line-delta to logical px.
const SCROLL_LINE_PX: f32 = 40.0;
const SCROLLBAR_W: f32 = 4.0;
const SCROLLBAR_INSET: f32 = 2.0;
const SCROLLBAR_MIN: f32 = 24.0;

pub struct Core {
    pub text: TextSystem,
    pub atlas: GlyphAtlas,
    pub interaction: Interaction,
    pub resources: Resources,
    pub scroll: ScrollStore,
    pub(crate) tree: Tree,
    pub(crate) display: DisplayList,
    pub(crate) viewport: Size,
    pub(crate) scale: f32,
    // Frame-builder state.
    stack: Vec<u32>,
    counters: Vec<u64>,
    origin: OriginId,
    /// Per-node inherited clip (logical), rebuilt each finish_frame.
    clips: Vec<Rect>,
    /// Whether any node this frame clips — lets emission skip clip math
    /// entirely for the common unclipped case.
    any_clip: bool,
}

impl Core {
    pub fn new() -> Self {
        Self {
            text: TextSystem::new(),
            atlas: GlyphAtlas::new(),
            interaction: Interaction::default(),
            resources: Resources::default(),
            scroll: ScrollStore::default(),
            tree: Tree::new(),
            display: DisplayList::default(),
            viewport: Size::ZERO,
            scale: 1.0,
            stack: Vec::new(),
            counters: Vec::new(),
            origin: OriginId::HOST,
            clips: Vec::new(),
            any_clip: false,
        }
    }

    /// Feeds one input event; returns any UI events it resolved to,
    /// hit-tested against the previous frame's layout.
    pub fn handle_input(&mut self, ev: InputEvent) -> Vec<UiEvent> {
        if let InputEvent::Scroll(delta) = ev {
            // Wheel up (positive y) reveals earlier content: offset decreases.
            if let Some(key) = self.interaction.scroll_target() {
                self.scroll.scroll_by(key, Vec2::new(-delta.x, -delta.y));
            }
            return Vec::new();
        }
        let mut out = Vec::new();
        self.interaction.handle(ev, &mut out);
        out
    }

    /// Wheel line-deltas (e.g. winit's LineDelta) to logical px.
    pub fn lines_to_px(lines: f32) -> f32 {
        lines * SCROLL_LINE_PX
    }

    /// Starts a frame. Build the tree through the returned `Ui` (or the
    /// `Core` builder methods directly), then `finish_frame()`.
    pub fn frame(&mut self, viewport: Size, scale: f32) -> Ui<'_> {
        self.begin_frame(viewport, scale);
        Ui::new(self)
    }

    /// The finished frame's draw data: display list plus the glyph atlas the
    /// renderer mirrors (mutable so it can clear the dirty flag).
    pub fn output(&mut self) -> (&DisplayList, &mut GlyphAtlas) {
        (&self.display, &mut self.atlas)
    }

    // -- Frame builder ------------------------------------------------------
    // Flat, non-panicking, callable through FFI. Misuse (close past the root,
    // building outside a frame) is ignored rather than UB or panic.

    pub fn begin_frame(&mut self, viewport: Size, scale: f32) {
        self.viewport = viewport;
        self.scale = scale;
        self.tree.clear();
        self.display.clear();
        self.text.begin_frame(scale);
        self.tree.push(
            NIL,
            Key::ROOT,
            OriginId::HOST,
            NodeSpec::column().width(Sizing::Grow(1.0)).height(Sizing::Grow(1.0)),
            NodeContent::Container,
        );
        self.stack.clear();
        self.stack.push(0);
        self.counters.clear();
        self.counters.push(0);
        self.origin = OriginId::HOST;
        self.any_clip = false;
    }

    /// Tags subsequently created nodes with an origin (set by the runner
    /// before handing the frame to an extension).
    pub fn set_origin(&mut self, origin: OriginId) {
        self.origin = origin;
    }

    /// Replaces the implicit root's spec (e.g. to make the top level a row).
    /// Root sizing is resolved against the viewport regardless.
    pub fn configure_root(&mut self, spec: NodeSpec) {
        if !self.tree.is_empty() {
            self.tree.specs[0] = spec;
        }
    }

    fn current(&self) -> u32 {
        self.stack.last().copied().unwrap_or(0)
    }

    fn auto_key(&mut self) -> Key {
        let parent = self.current() as usize;
        let i = self.counters.last().copied().unwrap_or(0);
        if let Some(c) = self.counters.last_mut() {
            *c += 1;
        }
        self.tree.keys[parent].index(i)
    }

    /// The key a child labeled `label` would get — usable before creating it,
    /// e.g. to check hover state for styling.
    pub fn child_key(&self, label: &str) -> Key {
        self.tree.keys[self.current() as usize].str(label)
    }

    pub fn is_hovered(&self, key: Key) -> bool {
        self.interaction.is_hovered(key)
    }

    pub fn is_pressed(&self, key: Key) -> bool {
        self.interaction.is_pressed(key)
    }

    pub fn open(&mut self, spec: NodeSpec) -> Key {
        let key = self.auto_key();
        self.open_with_key(key, spec);
        key
    }

    pub fn open_keyed(&mut self, label: &str, spec: NodeSpec) -> Key {
        let key = self.child_key(label);
        self.open_with_key(key, spec);
        key
    }

    fn open_with_key(&mut self, key: Key, spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        if spec.layout.clips() {
            self.any_clip = true;
        }
        let parent = self.current();
        let idx = self.tree.push(parent, key, self.origin, spec, NodeContent::Container);
        self.stack.push(idx);
        self.counters.push(0);
    }

    pub fn close(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
            self.counters.pop();
        }
    }

    pub fn text_node(&mut self, content: &str, style: TextStyle) {
        if self.tree.is_empty() {
            return;
        }
        let tid = self.text.add(content, &style);
        let key = self.auto_key();
        let parent = self.current();
        self.tree.push(parent, key, self.origin, NodeSpec::default(), NodeContent::Text(tid));
    }

    /// A paragraph of styled spans, shaped and wrapped as one flow.
    pub fn rich_text_node(&mut self, spans: &[Span<'_>], base: TextStyle) {
        if self.tree.is_empty() {
            return;
        }
        let tid = self.text.add_rich(spans, &base);
        let key = self.auto_key();
        let parent = self.current();
        self.tree.push(parent, key, self.origin, NodeSpec::default(), NodeContent::Text(tid));
    }

    /// Runs layout and emission into `output()`, and installs this frame's
    /// hit and scroll regions for input handling.
    pub fn finish_frame(&mut self) {
        // Tolerate unclosed containers (an FFI caller may have bailed early).
        self.stack.truncate(1);
        self.counters.truncate(1);

        layout::compute(&mut self.tree, &mut self.text, &mut self.scroll, self.viewport);

        let scale = self.scale;
        let mut hits: Vec<HitRegion> = self.interaction.take_hit_buffer();
        let mut scroll_regions: Vec<ScrollRegion> = Vec::new();
        self.display.viewport = Size::new(self.viewport.w * scale, self.viewport.h * scale);
        self.display.scale = scale;

        // configure_root can also introduce a clipper.
        let any_clip = self.any_clip
            || (!self.tree.is_empty() && self.tree.specs[0].layout.clips());

        // inherited clip per node (logical): ancestors only, not the node
        // itself. Only materialized when something actually clips.
        self.clips.clear();
        if any_clip {
            self.clips.resize(self.tree.len(), NO_CLIP);
        }

        for i in 0..self.tree.len() {
            let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
            let clip = if !any_clip {
                NO_CLIP
            } else {
                let parent = self.tree.parent[i];
                let clip = if parent == NIL {
                    NO_CLIP
                } else {
                    let p = parent as usize;
                    if self.tree.specs[p].layout.clips() {
                        self.clips[p]
                            .intersect(&Rect::from_pos_size(self.tree.pos[p], self.tree.size[p]))
                    } else {
                        self.clips[p]
                    }
                };
                self.clips[i] = clip;

                // Entirely clipped away: skip drawing and hit-testing.
                let visible = rect.intersect(&clip);
                if visible.w <= 0.0 || visible.h <= 0.0 {
                    continue;
                }
                clip
            };

            let spec = &self.tree.specs[i];
            let style = spec.style;
            if style.bg.is_visible() || (style.border_w > 0.0 && style.border_color.is_visible()) {
                self.display.quads.push(Quad {
                    rect: rect.scaled(scale),
                    color: style.bg,
                    border_color: style.border_color,
                    radius: style.radius * scale,
                    border_w: style.border_w * scale,
                    kind: QuadKind::Solid,
                    uv: [0; 4],
                    clip: clip.scaled(scale),
                });
            }
            if let Some(payload) = &spec.on_click {
                hits.push(HitRegion {
                    key: self.tree.keys[i],
                    origin: self.tree.origins[i],
                    rect,
                    clip,
                    payload: payload.clone(),
                });
            }
            if spec.layout.scroll_x || spec.layout.scroll_y {
                scroll_regions.push(ScrollRegion { key: self.tree.keys[i], rect, clip });
            }
            if let NodeContent::Text(tid) = self.tree.content[i] {
                self.text.emit(
                    tid,
                    self.tree.pos[i],
                    self.tree.size[i].w,
                    clip.scaled(scale),
                    &mut self.atlas,
                    &mut self.display.quads,
                );
            }
        }

        // Scrollbar indicators, on top of content.
        for r in &scroll_regions {
            let i = self
                .tree
                .keys
                .iter()
                .position(|k| *k == r.key)
                .expect("scroll region from this frame");
            let max = self.tree.scroll_max[i];
            let offset = self.scroll.offset(r.key);
            let clip = self.clips[i].scaled(scale);
            if max.y > 0.0 {
                let track_h = r.rect.h - 2.0 * SCROLLBAR_INSET;
                let bar_h = (track_h * r.rect.h / (r.rect.h + max.y)).max(SCROLLBAR_MIN);
                let t = (offset.y / max.y).clamp(0.0, 1.0);
                let bar = Rect::new(
                    r.rect.x + r.rect.w - SCROLLBAR_W - SCROLLBAR_INSET,
                    r.rect.y + SCROLLBAR_INSET + t * (track_h - bar_h),
                    SCROLLBAR_W,
                    bar_h,
                );
                self.display.quads.push(scrollbar_quad(bar, scale, clip));
            }
            if max.x > 0.0 {
                let track_w = r.rect.w - 2.0 * SCROLLBAR_INSET;
                let bar_w = (track_w * r.rect.w / (r.rect.w + max.x)).max(SCROLLBAR_MIN);
                let t = (offset.x / max.x).clamp(0.0, 1.0);
                let bar = Rect::new(
                    r.rect.x + SCROLLBAR_INSET + t * (track_w - bar_w),
                    r.rect.y + r.rect.h - SCROLLBAR_W - SCROLLBAR_INSET,
                    bar_w,
                    SCROLLBAR_W,
                );
                self.display.quads.push(scrollbar_quad(bar, scale, clip));
            }
        }

        self.interaction.set_hits(hits);
        self.interaction.scroll_regions = scroll_regions;
    }
}

fn scrollbar_quad(bar: Rect, scale: f32, clip: Rect) -> Quad {
    Quad {
        rect: bar.scaled(scale),
        color: Color::rgba(1.0, 1.0, 1.0, 0.18),
        border_color: Color::TRANSPARENT,
        radius: SCROLLBAR_W / 2.0 * scale,
        border_w: 0.0,
        kind: QuadKind::Solid,
        uv: [0; 4],
        clip,
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

/// A frontend that draws into the shared tree each frame — the trait the
/// runner uses to host Lua (or any other) extensions without knowing what
/// they are. Origins are assigned by the runner.
pub trait Extension {
    fn name(&self) -> &str;
    fn view(&mut self, ui: &mut Ui<'_>) -> Result<(), String>;
    fn on_event(&mut self, ev: &UiEvent);
}
