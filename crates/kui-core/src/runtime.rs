//! The `Core`: owns everything that survives across frames (text caches,
//! glyph atlas, interaction state, registered resources) plus the reusable
//! per-frame tree — and the frame-builder state itself. Builder state living
//! here (not in a borrowing wrapper) is what lets flat C bindings drive a
//! frame through one opaque pointer; the Rust `Ui` is a thin safe façade.

use crate::anim::{AnimStore, Slot};
use crate::atlas::GlyphAtlas;
use crate::color::Color;
use crate::display::{DisplayList, NO_CLIP, Quad, QuadKind};
use crate::edit::{EditOptions, EditStore};
use crate::env::Env;
use crate::geom::{Rect, Size, Vec2};
use crate::input::{
    EditKey, HitRegion, InputEvent, Interaction, ScrollAxis, ScrollRegion, ScrollbarRegion, UiEvent,
};
use crate::key::Key;
use crate::layout::{self, TextMeasure};
use crate::resources::Resources;
use crate::scroll::ScrollStore;
use crate::spec::{NodeSpec, Sizing, TextStyle};
use crate::stats::FrameStats;
use crate::text::{Span, TextSystem};
use crate::tree::{NIL, NodeContent, OriginId, Tree};
use crate::ui::Ui;
use crate::value::Value;

/// Wheel line-delta to logical px.
const SCROLL_LINE_PX: f32 = 40.0;
const SCROLLBAR_W: f32 = 4.0;
/// Thumb width while hovered or dragged.
const SCROLLBAR_ACTIVE_W: f32 = 6.0;
/// Grabbable gutter width (wider than the drawn thumb).
const SCROLLBAR_HIT_W: f32 = 10.0;
const SCROLLBAR_INSET: f32 = 2.0;
const SCROLLBAR_MIN: f32 = 24.0;

pub struct Core {
    pub text: TextSystem,
    pub atlas: GlyphAtlas,
    pub interaction: Interaction,
    pub resources: Resources,
    pub scroll: ScrollStore,
    pub edit: EditStore,
    /// Transition tweens, keyed by node; see `anim`. Fed by `set_time`.
    pub anim: AnimStore,
    /// Frame timing pushed by the frame driver; see `widgets::latency_graph`.
    pub stats: FrameStats,
    /// Host facts pushed by the frame driver (refresh rate, focus).
    pub env: Env,
    /// Window title declared this frame (immediate-mode: cleared each
    /// `begin_frame`; the driver diffs and applies). None = leave as-is.
    window_title: Option<String>,
    /// Which key sink (a node that declared `on_key`) receives full-
    /// keyboard `KeyDown` input. Moved by clicks on sinks; apps that
    /// own their text model declare it per frame via `set_key_focus`.
    key_focus: Option<Key>,
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
    /// Per-node "inside a floating subtree" marker (only filled when needed).
    in_float: Vec<bool>,
    any_float: bool,
    /// Whether any node this frame eases its position (`NodeSpec::slide`).
    any_slide: bool,
    /// A view asked for one more frame (`request_frame`); cleared by
    /// `begin_frame`, reported through `animating`.
    frame_requested: bool,
    /// The focused editor's caret rect (logical, viewport coords) as of the
    /// last finish_frame — where drivers should anchor the OS IME window.
    ime_rect: Option<Rect>,
}

impl Core {
    pub fn new() -> Self {
        Self {
            text: TextSystem::new(),
            atlas: GlyphAtlas::new(),
            interaction: Interaction::default(),
            resources: Resources::default(),
            scroll: ScrollStore::default(),
            edit: EditStore::default(),
            anim: AnimStore::default(),
            stats: FrameStats::default(),
            env: Env::default(),
            window_title: None,
            key_focus: None,
            tree: Tree::new(),
            display: DisplayList::default(),
            viewport: Size::ZERO,
            scale: 1.0,
            stack: Vec::new(),
            counters: Vec::new(),
            origin: OriginId::HOST,
            clips: Vec::new(),
            any_clip: false,
            in_float: Vec::new(),
            any_float: false,
            any_slide: false,
            frame_requested: false,
            ime_rect: None,
        }
    }

    /// Feeds one input event; returns any UI events it resolved to,
    /// hit-tested against the previous frame's layout.
    pub fn handle_input(&mut self, ev: InputEvent) -> Vec<UiEvent> {
        let mut out = Vec::new();
        match ev {
            InputEvent::Scroll(delta) => {
                // Wheel up (positive y) reveals earlier content: offset decreases.
                if let Some(key) = self.interaction.scroll_target() {
                    self.scroll.scroll_by(key, Vec2::new(-delta.x, -delta.y));
                }
            }
            InputEvent::Text(s) => {
                if let Some(key) = self.edit.focused()
                    && self.edit.apply_text(key, &s, self.text.font_system_mut())
                {
                    self.push_edit_event(key, "changed", &mut out);
                }
            }
            InputEvent::Preedit(s, cursor) => {
                if let Some(key) = self.edit.focused() {
                    self.edit
                        .set_preedit(key, &s, cursor, self.text.font_system_mut());
                }
            }
            InputEvent::Key(ek, mods) => {
                // Tab traverses between edit widgets (Shift-Tab backwards)
                // unless a multiline editor holds focus — there Tab stays
                // indentation. With nothing focused it enters the first,
                // but only when no on_key sink owns the keyboard.
                let traverse = ek == EditKey::Tab
                    && match self.edit.focused() {
                        Some(k) => !self.edit.is_multiline(k),
                        None => self.key_focus.is_none(),
                    };
                if traverse {
                    self.focus_adjacent_edit(!mods.shift);
                } else if let Some(key) = self.edit.focused() {
                    let (changed, submit) =
                        self.edit
                            .apply_key(key, ek, mods, self.text.font_system_mut());
                    if changed {
                        self.push_edit_event(key, "changed", &mut out);
                    }
                    if submit {
                        self.push_edit_event(key, "submit", &mut out);
                    }
                    if ek == EditKey::Escape {
                        self.edit.set_focus(None);
                    }
                }
            }
            InputEvent::KeyDown(kp) => {
                // The focused edit widget owns the keyboard (it takes the
                // Text/EditKey path); otherwise the key-focused sink, if it
                // still exists in the last frame, gets the press as data.
                if self.edit.focused().is_none()
                    && let Some(focus) = self.key_focus
                    && let Some(h) = self
                        .interaction
                        .hits
                        .iter()
                        .rev()
                        .find(|h| h.key == focus && h.key_sink.is_some())
                {
                    let mut payload = kp.to_value();
                    if let Some(tag) = &h.key_sink
                        && *tag != Value::Null
                        && let Value::Map(entries) = &mut payload
                    {
                        entries.push(("tag".to_string(), tag.clone()));
                    }
                    out.push(UiEvent {
                        origin: h.origin,
                        key: h.key,
                        payload,
                    });
                }
            }
            InputEvent::MouseDown(clicks) => {
                // Scrollbars win over everything under them (they draw on
                // top): a thumb press starts a drag, a track press jumps
                // there first. Neither blurs the focused edit.
                if let Some(p) = self.interaction.cursor()
                    && let Some(bar) = self.interaction.scrollbar_at(p)
                {
                    let (pos, thumb_start) = match bar.axis {
                        ScrollAxis::X => (p.x, bar.thumb.x),
                        ScrollAxis::Y => (p.y, bar.thumb.y),
                    };
                    let grab = if pos >= thumb_start && pos <= thumb_start + bar.bar_len {
                        pos - thumb_start
                    } else {
                        let center = bar.bar_len / 2.0;
                        let off = bar.offset_for(p, center);
                        self.set_scroll_axis(bar.key, bar.axis, off);
                        center
                    };
                    self.interaction.scrollbar_drag = Some((bar.key, bar.axis, grab));
                    return out;
                }
                // Click-to-focus / caret placement / start drag-selection,
                // against the previous frame's layout.
                if let Some(p) = self.interaction.cursor() {
                    let hit = self
                        .interaction
                        .hit_at(p)
                        .map(|h| (h.key, h.edit_origin, h.key_sink.is_some()));
                    match hit {
                        Some((key, Some(origin), _)) => {
                            self.edit.set_focus(Some(key));
                            let local = Vec2::new(p.x - origin.x, p.y - origin.y);
                            self.edit
                                .click(key, local, clicks, self.text.font_system_mut());
                            self.edit.dragging = Some((key, origin));
                        }
                        Some((key, None, true)) => {
                            self.edit.set_focus(None);
                            self.key_focus = Some(key);
                        }
                        _ => self.edit.set_focus(None),
                    }
                }
                self.interaction
                    .handle(InputEvent::MouseDown(clicks), &mut out);
            }
            InputEvent::CursorMoved(p) => {
                if let Some((key, axis, grab)) = self.interaction.scrollbar_drag
                    && let Some(bar) = self
                        .interaction
                        .scrollbars
                        .iter()
                        .rev()
                        .find(|b| b.key == key && b.axis == axis)
                        .copied()
                {
                    // Thumb drag: geometry is last frame's, which is fine —
                    // track length only changes with the container.
                    let off = bar.offset_for(p, grab);
                    self.set_scroll_axis(key, axis, off);
                }
                if let Some((key, origin)) = self.edit.dragging {
                    let local = Vec2::new(p.x - origin.x, p.y - origin.y);
                    self.edit.drag(key, local, self.text.font_system_mut());
                }
                self.interaction
                    .handle(InputEvent::CursorMoved(p), &mut out);
            }
            InputEvent::MouseUp => {
                self.edit.dragging = None;
                self.interaction.scrollbar_drag = None;
                self.interaction.handle(InputEvent::MouseUp, &mut out);
            }
            other => self.interaction.handle(other, &mut out),
        }
        out
    }

    /// Emits one node's quads and registers its hit/scroll regions.
    fn emit_node(
        &mut self,
        i: usize,
        rect: Rect,
        clip: Rect,
        scale: f32,
        hits: &mut Vec<HitRegion>,
        scroll_regions: &mut Vec<ScrollRegion>,
    ) {
        let spec = &self.tree.specs[i];
        let style = spec.style;
        if style.bg.is_visible() || (style.border_w > 0.0 && style.border_color.is_visible()) {
            self.display.quads.push(Quad {
                rect: rect.scaled(scale),
                color: style.bg,
                border_color: style.border_color,
                radius: style.radius.map(|r| r * scale),
                border_w: style.border_w * scale,
                kind: QuadKind::Solid,
                uv: [0; 4],
                clip: clip.scaled(scale),
            });
        }
        if spec.hover_tracked() {
            let parent = self.tree.parent[i];
            let parent_rect = if parent == NIL {
                Rect::new(0.0, 0.0, self.viewport.w, self.viewport.h)
            } else {
                let p = parent as usize;
                Rect::from_pos_size(self.tree.pos[p], self.tree.size[p])
            };
            hits.push(HitRegion {
                key: self.tree.keys[i],
                origin: self.tree.origins[i],
                rect,
                clip,
                payload: spec.on_click.clone(),
                drag: spec.on_drag.clone(),
                parent_rect,
                key_sink: spec.on_key.clone(),
                edit_origin: None,
                window: spec.window,
                hover: spec.on_hover.clone(),
                group: spec.hover_group,
            });
        }
        if spec.layout.scroll_x || spec.layout.scroll_y {
            scroll_regions.push(ScrollRegion {
                key: self.tree.keys[i],
                rect,
                clip,
            });
        }
        match self.tree.content[i] {
            NodeContent::Text(tid) => {
                self.text.emit(
                    tid,
                    self.tree.pos[i],
                    self.tree.size[i].w,
                    clip.scaled(scale),
                    &mut self.atlas,
                    &mut self.display.quads,
                );
            }
            NodeContent::Edit(key) => {
                let pad = spec.layout.padding;
                let content_origin = Vec2::new(rect.x + pad.l, rect.y + pad.t);
                hits.push(HitRegion {
                    key,
                    origin: self.tree.origins[i],
                    rect,
                    clip,
                    payload: None,
                    drag: None,
                    parent_rect: rect,
                    edit_origin: Some(content_origin),
                    key_sink: None,
                    window: None,
                    hover: None,
                    group: None,
                });
                let focused = self.edit.focused() == Some(key);
                let origin_phys = Vec2::new(
                    (content_origin.x * scale).round(),
                    (content_origin.y * scale).round(),
                );
                self.edit.emit(
                    key,
                    origin_phys,
                    focused,
                    clip.scaled(scale),
                    &mut self.text,
                    &mut self.atlas,
                    &mut self.display.quads,
                );
            }
            NodeContent::Image(id) => {
                if let Some(entry) = self.resources.images.get(id)
                    && let Some(slot) =
                        self.atlas
                            .get_or_insert_image(id, entry.width, entry.height, &entry.rgba)
                {
                    self.display.quads.push(Quad {
                        rect: rect.scaled(scale),
                        // White = untinted; radius rounds like a solid.
                        color: Color::WHITE,
                        border_color: Color::TRANSPARENT,
                        radius: spec.style.radius.map(|r| r * scale),
                        border_w: 0.0,
                        kind: QuadKind::Image,
                        uv: [slot.x, slot.y, slot.w, slot.h],
                        clip: clip.scaled(scale),
                    });
                }
            }
            NodeContent::Container => {}
        }
    }

    /// Moves edit focus to the next/previous edit widget in tree order
    /// (from the last laid-out frame), wrapping around; with no current
    /// focus, enters the first (or last, going backwards). The landing
    /// field scrolls its caret into view like any caret motion.
    fn focus_adjacent_edit(&mut self, forward: bool) {
        let ring: Vec<Key> = self
            .tree
            .content
            .iter()
            .filter_map(|c| match c {
                NodeContent::Edit(k) => Some(*k),
                _ => None,
            })
            .collect();
        if ring.is_empty() {
            return;
        }
        let target = match self
            .edit
            .focused()
            .and_then(|cur| ring.iter().position(|k| *k == cur))
        {
            Some(i) if forward => ring[(i + 1) % ring.len()],
            Some(i) => ring[(i + ring.len() - 1) % ring.len()],
            None if forward => ring[0],
            None => ring[ring.len() - 1],
        };
        self.edit.set_focus(Some(target));
        self.edit.caret_moved = Some(target);
    }

    /// Sets one axis of a container's scroll offset, keeping the other.
    fn set_scroll_axis(&mut self, key: Key, axis: ScrollAxis, value: f32) {
        let mut off = self.scroll.offset(key);
        match axis {
            ScrollAxis::X => off.x = value,
            ScrollAxis::Y => off.y = value,
        }
        self.scroll.set(key, off);
    }

    fn push_edit_event(&self, key: Key, kind: &str, out: &mut Vec<UiEvent>) {
        out.push(UiEvent {
            origin: self.edit.origin_of(key).unwrap_or(OriginId::HOST),
            key,
            payload: Value::map([("kind", kind.into())]),
        });
    }

    /// Selected text of the focused editor (for clipboard integration).
    pub fn copy_selection(&self) -> Option<String> {
        self.edit.copy_selection(self.edit.focused()?)
    }

    /// Cuts the focused editor's selection, returning the removed text.
    pub fn cut_selection(&mut self) -> Option<String> {
        let key = self.edit.focused()?;
        let text = self.edit.copy_selection(key)?;
        self.edit.delete_selection(key, self.text.font_system_mut());
        Some(text)
    }

    pub fn is_focused(&self, key: Key) -> bool {
        self.edit.focused() == Some(key)
    }

    /// Current text of an editor by key.
    pub fn edit_text(&self, key: Key) -> Option<String> {
        self.edit.text(key)
    }

    pub fn set_edit_text(&mut self, key: Key, text: &str) {
        let fs = self.text.font_system_mut();
        self.edit.set_text(key, text, fs, &self.resources);
    }

    // -- Fonts ----------------------------------------------------------

    /// Registers a font from its file bytes (TTF/OTF/TTC); `None` when the
    /// data holds no usable face. Shape with it via `TextStyle::font`.
    pub fn add_font_data(&mut self, data: Vec<u8>) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::Source;
        let db = self.text.font_system_mut().db_mut();
        let ids = db.load_font_source(Source::Binary(std::sync::Arc::new(data)));
        let family = db.face(*ids.first()?)?.families.first()?.0.clone();
        Some(self.resources.add_font(family, ids.to_vec()))
    }

    /// Registers a font file (TTF/OTF/TTC) by path, memory-mapped by the
    /// font database; `None` when it cannot be read or holds no usable
    /// face. Shape with it via `TextStyle::font`.
    pub fn load_font_file(
        &mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::Source;
        let db = self.text.font_system_mut().db_mut();
        let ids = db.load_font_source(Source::File(path.into()));
        let family = db.face(*ids.first()?)?.families.first()?.0.clone();
        Some(self.resources.add_font(family, ids.to_vec()))
    }

    /// Loads every font file under `dir` (recursively) into the font
    /// database, so their families become available to `add_system_font`
    /// by name; returns how many faces were added. A bundled `fonts/`
    /// folder next to the app is the usual case.
    pub fn load_fonts_dir(&mut self, dir: impl AsRef<std::path::Path>) -> usize {
        let db = self.text.font_system_mut().db_mut();
        let before = db.len();
        db.load_fonts_dir(dir);
        db.len().saturating_sub(before)
    }

    /// The handle for a font family by name (`"Menlo"`, `"Antonio"`) —
    /// installed on the system or loaded with `load_fonts_dir` /
    /// `load_font_file`; `None` when no face matches (see
    /// `system_font_families`). Idempotent: the same family gets the same
    /// handle, so views can call it every frame.
    pub fn add_system_font(&mut self, name: &str) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::{Family, Query};
        let db = self.text.font_system().db();
        let query = Query {
            families: &[Family::Name(name)],
            ..Default::default()
        };
        let id = db.query(&query)?;
        // The canonical spelling, so the style matches the way fontdb does.
        let family = db.face(id)?.families.first()?.0.clone();
        if let Some((id, _)) = self
            .resources
            .fonts
            .iter()
            .find(|(_, f)| f.faces.is_empty() && f.family == family)
        {
            return Some(id);
        }
        Some(self.resources.add_font(family, Vec::new()))
    }

    /// Forgets a registered font; faces loaded from bytes leave the font
    /// database. Styles still naming it shape as sans-serif.
    pub fn remove_font(&mut self, id: crate::resources::FontId) {
        if let Some(entry) = self.resources.remove_font(id) {
            let db = self.text.font_system_mut().db_mut();
            for face in entry.faces {
                db.remove_face(face);
            }
        }
    }

    /// The registered family name behind a font handle, if it is live.
    pub fn font_family(&self, id: crate::resources::FontId) -> Option<&str> {
        self.resources.font_family(id)
    }

    /// Family names of every installed font the core can see (sorted,
    /// deduplicated) — what `add_system_font` accepts.
    pub fn system_font_families(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .text
            .font_system()
            .db()
            .faces()
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Drains window intents produced by chrome nodes since the last drain.
    /// Frame drivers call this after each input dispatch and apply the
    /// commands to the real window; headless drivers may simply never call.
    /// Directs full-keyboard `KeyDown` routing at a node that declared
    /// `on_key` (None releases it). Apps that own their text model call
    /// this every frame for whatever they consider focused — like the
    /// window title, the declaration is idempotent; clicking another
    /// sink moves focus too, and the next declaration wins it back.
    pub fn set_key_focus(&mut self, key: Option<Key>) {
        self.key_focus = key;
    }

    pub fn key_focus(&self) -> Option<Key> {
        self.key_focus
    }

    /// Queues a window command as if chrome had produced it, so apps can
    /// close/minimize/maximize from a keymap or command line. Drained by
    /// the frame driver with the rest.
    pub fn push_window_command(&mut self, cmd: crate::window::WindowCommand) {
        self.interaction.window_commands.push(cmd);
    }

    pub fn take_window_commands(&mut self) -> Vec<crate::window::WindowCommand> {
        std::mem::take(&mut self.interaction.window_commands)
    }

    /// Wheel line-deltas (e.g. winit's LineDelta) to logical px.
    pub fn lines_to_px(lines: f32) -> f32 {
        lines * SCROLL_LINE_PX
    }

    /// Rasterizes outline glyphs as LCD subpixel coverage
    /// (`QuadKind::GlyphSubpixel`) instead of alpha masks. Drivers set it
    /// from what their renderer can blend per channel; flipping it drops
    /// the glyph atlas so every glyph re-rasterizes in the new mode.
    pub fn set_subpixel_text(&mut self, on: bool) {
        if self.text.set_subpixel(on) {
            self.atlas.clear();
        }
    }

    pub fn subpixel_text(&self) -> bool {
        self.text.subpixel()
    }

    /// The frame clock for transitions: monotonic seconds, any origin.
    /// Drivers set it before every frame; a driver that never does gets
    /// snapping instead of animation.
    pub fn set_time(&mut self, now_secs: f64) {
        self.anim.set_time(now_secs);
    }

    /// True when the last frame left a transition mid-flight, or a view
    /// asked for another frame — drivers schedule one without waiting for
    /// input.
    pub fn animating(&self) -> bool {
        self.anim.animating() || self.frame_requested
    }

    /// Asks the driver for one more frame right after this one. A view
    /// that sets up a transition by drawing a starting state (a new split
    /// drawn collapsed so it can slide open) needs the next frame to come
    /// without waiting for input — the starting state itself snaps, so
    /// nothing is mid-flight yet to request it.
    pub fn request_frame(&mut self) {
        self.frame_requested = true;
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
        self.window_title = None;
        self.tree.clear();
        self.display.clear();
        self.text.begin_frame(scale);
        self.anim.begin_frame();
        self.tree.push(
            NIL,
            Key::ROOT,
            OriginId::HOST,
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0)),
            NodeContent::Container,
        );
        self.stack.clear();
        self.stack.push(0);
        self.counters.clear();
        self.counters.push(0);
        self.origin = OriginId::HOST;
        self.any_clip = false;
        self.any_float = false;
        self.any_slide = false;
        self.frame_requested = false;
    }

    /// Declares this frame's window title. Like all frame state it's data:
    /// the driver diffs against what's applied and only then touches the
    /// window. Undeclared frames leave the title alone; last writer wins.
    pub fn set_window_title(&mut self, title: &str) {
        self.window_title = Some(title.to_string());
    }

    /// The title declared this frame, if any (for the frame driver).
    pub fn window_title(&self) -> Option<&str> {
        self.window_title.as_deref()
    }

    /// Tags subsequently created nodes with an origin (set by the runner
    /// before handing the frame to an extension).
    pub fn set_origin(&mut self, origin: OriginId) {
        self.origin = origin;
    }

    /// Replaces the implicit root's spec (e.g. to make the top level a row).
    /// Root sizing is resolved against the viewport regardless.
    pub fn configure_root(&mut self, mut spec: NodeSpec) {
        if !self.tree.is_empty() {
            self.ease_spec(Key::ROOT, &mut spec);
            self.tree.specs[0] = spec;
        }
    }

    /// Replaces a transitioning node's animatable values with this frame's
    /// eased ones. Nodes without a transition cost one branch.
    fn ease_spec(&mut self, key: Key, spec: &mut NodeSpec) {
        let Some(t) = spec.transition else {
            return;
        };
        let anim = &mut self.anim;
        let mut sizing = |slot: Slot, s: Sizing| match s {
            Sizing::Fit => Sizing::Fit,
            Sizing::Grow(v) => Sizing::Grow(anim.drive(key, slot, [v, 0.0, 0.0, 0.0], t)[0]),
            Sizing::Fixed(v) => Sizing::Fixed(anim.drive(key, slot, [v, 0.0, 0.0, 0.0], t)[0]),
            Sizing::Percent(v) => Sizing::Percent(anim.drive(key, slot, [v, 0.0, 0.0, 0.0], t)[0]),
        };
        spec.layout.width = sizing(Slot::Width, spec.layout.width);
        spec.layout.height = sizing(Slot::Height, spec.layout.height);
        let mut color = |slot: Slot, c: Color| {
            let v = anim.drive(key, slot, [c.r, c.g, c.b, c.a], t);
            Color {
                r: v[0],
                g: v[1],
                b: v[2],
                a: v[3],
            }
        };
        spec.style.bg = color(Slot::Bg, spec.style.bg);
        spec.style.border_color = color(Slot::Border, spec.style.border_color);
        spec.style.radius = anim.drive(key, Slot::Radius, spec.style.radius, t);
    }

    /// The root node's key — for hover/press queries or `set_key_focus` when
    /// the root itself declares the interaction (e.g. a root-level key sink).
    pub fn root_key(&self) -> Key {
        self.tree.keys.first().copied().unwrap_or(Key(0))
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

    /// Whether any member of hover group `group` (see
    /// `NodeSpec::hover_group`) is hovered.
    pub fn is_group_hovered(&self, group: u64) -> bool {
        self.interaction.is_group_hovered(group)
    }

    /// Whether hover group `group` is pressed (press started on a member,
    /// pointer still over one).
    pub fn is_group_pressed(&self, group: u64) -> bool {
        self.interaction.is_group_pressed(group)
    }

    /// Events raised outside `handle_input`: `on_hover` enter/leave caused
    /// by a finished frame changing what sits under a still cursor. Frame
    /// drivers route these after `finish_frame`; they also ride along with
    /// the next `handle_input` result, so a driver that never calls this
    /// merely sees them a little later.
    pub fn take_pending_events(&mut self) -> Vec<UiEvent> {
        self.interaction.take_pending()
    }

    /// Swaps in the hover / pressed background the spec declares for the
    /// node's (or its group's) current pointer state. Runs before easing so
    /// a `transition` tweens between the states.
    fn resolve_hover_style(&self, key: Key, spec: &mut NodeSpec) {
        if spec.hover_bg.is_none() && spec.pressed_bg.is_none() {
            return;
        }
        let group = spec.hover_group;
        let pressed = self.interaction.is_pressed(key)
            || group.is_some_and(|g| self.interaction.is_group_pressed(g));
        let hovered = pressed
            || self.interaction.is_hovered(key)
            || group.is_some_and(|g| self.interaction.is_group_hovered(g));
        if pressed && let Some(c) = spec.pressed_bg {
            spec.style.bg = c;
        } else if hovered && let Some(c) = spec.hover_bg {
            spec.style.bg = c;
        }
    }

    /// Physical modifier state as of the last `InputEvent::Modifiers`.
    pub fn modifiers(&self) -> crate::input::KeyMods {
        self.interaction.modifiers()
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

    fn open_with_key(&mut self, key: Key, mut spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        self.resolve_hover_style(key, &mut spec);
        self.ease_spec(key, &mut spec);
        if spec.layout.clips() {
            self.any_clip = true;
        }
        if spec.slide && spec.transition.is_some() {
            self.any_slide = true;
        }
        if spec.layout.float.is_some() {
            self.any_float = true;
        }
        let parent = self.current();
        let idx = self
            .tree
            .push(parent, key, self.origin, spec, NodeContent::Container);
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
        let tid = self.text.add(content, &style, &self.resources);
        let key = self.auto_key();
        let parent = self.current();
        self.tree.push(
            parent,
            key,
            self.origin,
            NodeSpec::default(),
            NodeContent::Text(tid),
        );
    }

    /// An editable text node. State (buffer, cursor, selection) is retained
    /// by key across frames; edits arrive via `handle_input` and come back to
    /// the host as "changed"/"submit" events. Read with `edit_text`.
    pub fn text_edit(
        &mut self,
        label: &str,
        initial: &str,
        opts: &EditOptions,
        mut spec: NodeSpec,
    ) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.child_key(label);
        self.ease_spec(key, &mut spec);
        self.edit.declare(
            key,
            initial,
            opts,
            self.origin,
            self.scale,
            self.text.font_system_mut(),
            &self.resources,
        );
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Edit(key));
        key
    }

    /// A registered image (see `Resources::add_image`). Fit sizing takes
    /// the image's pixel dimensions as logical px; a Fit height against a
    /// resolved width preserves the aspect ratio. `style.radius` rounds the
    /// corners.
    pub fn image_node(&mut self, id: crate::resources::ImageId, mut spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        let key = self.auto_key();
        self.resolve_hover_style(key, &mut spec);
        self.ease_spec(key, &mut spec);
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Image(id));
    }

    /// Unregisters an image and forgets its atlas slot.
    pub fn remove_image(&mut self, id: crate::resources::ImageId) {
        self.resources.remove_image(id);
        self.atlas.evict_image(id);
    }

    /// A paragraph of styled spans, shaped and wrapped as one flow.
    pub fn rich_text_node(&mut self, spans: &[Span<'_>], base: TextStyle) {
        if self.tree.is_empty() {
            return;
        }
        let tid = self.text.add_rich(spans, &base, &self.resources);
        let key = self.auto_key();
        let parent = self.current();
        self.tree.push(
            parent,
            key,
            self.origin,
            NodeSpec::default(),
            NodeContent::Text(tid),
        );
    }

    /// Runs layout and emission into `output()`, and installs this frame's
    /// hit and scroll regions for input handling.
    pub fn finish_frame(&mut self) {
        // Tolerate unclosed containers (an FFI caller may have bailed early).
        self.stack.truncate(1);
        self.counters.truncate(1);

        {
            let mut measure = Measure {
                text: &mut self.text,
                edit: &mut self.edit,
                resources: &self.resources,
            };
            layout::compute(
                &mut self.tree,
                &mut measure,
                &mut self.scroll,
                self.viewport,
            );
        }
        self.scroll_caret_into_view();
        if self.any_slide {
            self.ease_positions();
        }

        let scale = self.scale;
        let mut hits: Vec<HitRegion> = self.interaction.take_hit_buffer();
        let mut scroll_regions: Vec<ScrollRegion> = Vec::new();
        self.display.viewport = Size::new(self.viewport.w * scale, self.viewport.h * scale);
        self.display.scale = scale;

        // configure_root can also introduce a clipper.
        let any_clip =
            self.any_clip || (!self.tree.is_empty() && self.tree.specs[0].layout.clips());
        let any_float = self.any_float;

        // inherited clip per node (logical): ancestors only, not the node
        // itself. Only materialized when something actually clips.
        self.clips.clear();
        if any_clip {
            self.clips.resize(self.tree.len(), NO_CLIP);
        }
        self.in_float.clear();
        if any_float {
            self.in_float.resize(self.tree.len(), false);
        }

        // Pass 1: clip/float propagation + in-flow emission (preorder =
        // paint order; parents precede children).
        for i in 0..self.tree.len() {
            let parent = self.tree.parent[i];
            let floats_here = self.tree.specs[i].layout.float.is_some();
            if any_float {
                self.in_float[i] = floats_here || (parent != NIL && self.in_float[parent as usize]);
            }
            let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
            let clip = if !any_clip {
                NO_CLIP
            } else {
                // Floating nodes escape ancestor clips.
                let clip = if parent == NIL || floats_here {
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
                clip
            };
            if any_float && self.in_float[i] {
                continue; // deferred to the float pass
            }
            // Entirely clipped away: skip drawing and hit-testing.
            let visible = rect.intersect(&clip);
            if visible.w <= 0.0 || visible.h <= 0.0 {
                continue;
            }
            self.emit_node(i, rect, clip, scale, &mut hits, &mut scroll_regions);
        }

        // Pass 2: floating subtrees, on top of all in-flow content (their
        // hit regions land last too, so they're topmost for input).
        if any_float {
            for i in 0..self.tree.len() {
                if !self.in_float[i] {
                    continue;
                }
                let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
                let clip = if any_clip { self.clips[i] } else { NO_CLIP };
                let visible = rect.intersect(&clip);
                if visible.w <= 0.0 || visible.h <= 0.0 {
                    continue;
                }
                self.emit_node(i, rect, clip, scale, &mut hits, &mut scroll_regions);
            }
        }

        // Scrollbars, on top of content: indicator quads plus the hit
        // regions that make their thumbs draggable.
        let cursor = self.interaction.cursor();
        let mut scrollbars: Vec<ScrollbarRegion> = Vec::new();
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
                let track = Rect::new(
                    r.rect.x + r.rect.w - SCROLLBAR_HIT_W,
                    r.rect.y + SCROLLBAR_INSET,
                    SCROLLBAR_HIT_W,
                    track_h,
                );
                let active = self.interaction.is_scrollbar_dragging(r.key, ScrollAxis::Y)
                    || cursor.is_some_and(|p| track.contains(p));
                let w = if active {
                    SCROLLBAR_ACTIVE_W
                } else {
                    SCROLLBAR_W
                };
                let thumb = Rect::new(
                    r.rect.x + r.rect.w - w - SCROLLBAR_INSET,
                    r.rect.y + SCROLLBAR_INSET + t * (track_h - bar_h),
                    w,
                    bar_h,
                );
                self.display
                    .quads
                    .push(scrollbar_quad(thumb, scale, clip, active));
                scrollbars.push(ScrollbarRegion {
                    key: r.key,
                    axis: ScrollAxis::Y,
                    thumb,
                    track,
                    bar_len: bar_h,
                    max: max.y,
                });
            }
            if max.x > 0.0 {
                let track_w = r.rect.w - 2.0 * SCROLLBAR_INSET;
                let bar_w = (track_w * r.rect.w / (r.rect.w + max.x)).max(SCROLLBAR_MIN);
                let t = (offset.x / max.x).clamp(0.0, 1.0);
                let track = Rect::new(
                    r.rect.x + SCROLLBAR_INSET,
                    r.rect.y + r.rect.h - SCROLLBAR_HIT_W,
                    track_w,
                    SCROLLBAR_HIT_W,
                );
                let active = self.interaction.is_scrollbar_dragging(r.key, ScrollAxis::X)
                    || cursor.is_some_and(|p| track.contains(p));
                let w = if active {
                    SCROLLBAR_ACTIVE_W
                } else {
                    SCROLLBAR_W
                };
                let thumb = Rect::new(
                    r.rect.x + SCROLLBAR_INSET + t * (track_w - bar_w),
                    r.rect.y + r.rect.h - w - SCROLLBAR_INSET,
                    bar_w,
                    w,
                );
                self.display
                    .quads
                    .push(scrollbar_quad(thumb, scale, clip, active));
                scrollbars.push(ScrollbarRegion {
                    key: r.key,
                    axis: ScrollAxis::X,
                    thumb,
                    track,
                    bar_len: bar_w,
                    max: max.x,
                });
            }
        }

        self.interaction.set_hits(hits);
        self.interaction.scroll_regions = scroll_regions;
        self.interaction.scrollbars = scrollbars;
        self.ime_rect = self.focused_caret_rect();
    }

    /// After layout: nodes that `slide` ease from last frame's position
    /// toward where layout put them, carrying their subtree along (hit
    /// regions come from the same positions, so input follows the motion).
    /// Preorder means a parent shifts before its children are visited, so
    /// nested sliders ease relative to an already-eased parent.
    fn ease_positions(&mut self) {
        for i in 0..self.tree.len() {
            let spec = &self.tree.specs[i];
            let (Some(t), true) = (spec.transition, spec.slide) else {
                continue;
            };
            let key = self.tree.keys[i];
            let target = self.tree.pos[i];
            let v = self
                .anim
                .drive(key, Slot::Pos, [target.x, target.y, 0.0, 0.0], t);
            let d = Vec2::new(v[0] - target.x, v[1] - target.y);
            if d.x == 0.0 && d.y == 0.0 {
                continue;
            }
            let end = self.tree.subtree_end(i);
            for p in &mut self.tree.pos[i..end] {
                p.x += d.x;
                p.y += d.y;
            }
        }
    }

    /// See the `ime_rect` field. None when no editor is focused.
    pub fn ime_rect(&self) -> Option<Rect> {
        self.ime_rect
    }

    fn focused_caret_rect(&mut self) -> Option<Rect> {
        let key = self.edit.focused()?;
        let i = (0..self.tree.len()).find(|&i| self.tree.content[i] == NodeContent::Edit(key))?;
        let caret = self.edit.caret_rect(key, self.text.font_system_mut())?;
        let pad = self.tree.specs[i].layout.padding;
        Some(Rect::new(
            self.tree.pos[i].x + pad.l + caret.x / self.scale,
            self.tree.pos[i].y + pad.t + caret.y / self.scale,
            caret.w / self.scale,
            caret.h / self.scale,
        ))
    }

    /// After layout: if the focused edit's caret moved this frame, nudge the
    /// nearest scrollable ancestor so the caret stays visible, then re-run
    /// the positions pass with the adjusted offset (positions is the only
    /// pass scroll offsets feed into, so nothing else needs recomputing).
    fn scroll_caret_into_view(&mut self) {
        let Some(key) = self.edit.caret_moved.take() else {
            return;
        };
        if self.edit.focused() != Some(key) {
            return;
        }
        let Some(i) =
            (0..self.tree.len()).find(|&i| self.tree.content[i] == NodeContent::Edit(key))
        else {
            return;
        };
        let Some(caret_phys) = self.edit.caret_rect(key, self.text.font_system_mut()) else {
            return;
        };
        let pad = self.tree.specs[i].layout.padding;
        let caret = Rect::new(
            self.tree.pos[i].x + pad.l + caret_phys.x / self.scale,
            self.tree.pos[i].y + pad.t + caret_phys.y / self.scale,
            caret_phys.w / self.scale,
            caret_phys.h / self.scale,
        );
        // Slack so the caret isn't glued to the container edge.
        const MARGIN: f32 = 4.0;
        let mut a = self.tree.parent[i];
        while a != NIL {
            let spec = self.tree.specs[a as usize].layout;
            if spec.scroll_x || spec.scroll_y {
                let view =
                    Rect::from_pos_size(self.tree.pos[a as usize], self.tree.size[a as usize]);
                let mut delta = Vec2::ZERO;
                if spec.scroll_y {
                    if caret.y < view.y + MARGIN {
                        delta.y = caret.y - (view.y + MARGIN);
                    } else if caret.y + caret.h > view.y + view.h - MARGIN {
                        delta.y = caret.y + caret.h - (view.y + view.h - MARGIN);
                    }
                }
                if spec.scroll_x {
                    if caret.x < view.x + MARGIN {
                        delta.x = caret.x - (view.x + MARGIN);
                    } else if caret.x + caret.w > view.x + view.w - MARGIN {
                        delta.x = caret.x + caret.w - (view.x + view.w - MARGIN);
                    }
                }
                if delta.x != 0.0 || delta.y != 0.0 {
                    self.scroll.scroll_by(self.tree.keys[a as usize], delta);
                    layout::positions(&mut self.tree, &mut self.scroll, self.viewport);
                }
                return;
            }
            a = self.tree.parent[a as usize];
        }
    }
}

/// Combined measurer handed to the layout pass: static text through the
/// shape cache, editors through the edit store (sharing one FontSystem),
/// images through the resource registry.
struct Measure<'a> {
    text: &'a mut TextSystem,
    edit: &'a mut EditStore,
    resources: &'a Resources,
}

impl TextMeasure for Measure<'_> {
    fn intrinsic(&mut self, id: crate::tree::TextId) -> Size {
        self.text.intrinsic(id)
    }

    fn wrapped(&mut self, id: crate::tree::TextId, max_w: f32) -> Size {
        self.text.wrapped(id, max_w)
    }

    fn edit_intrinsic(&mut self, key: Key) -> Size {
        self.edit.intrinsic(key, self.text.font_system_mut())
    }

    fn edit_wrapped(&mut self, key: Key, max_w: f32) -> Size {
        self.edit.wrapped(key, max_w, self.text.font_system_mut())
    }

    fn image_size(&mut self, id: crate::resources::ImageId) -> Size {
        self.resources
            .images
            .get(id)
            .map_or(Size::ZERO, |e| Size::new(e.width as f32, e.height as f32))
    }
}

fn scrollbar_quad(bar: Rect, scale: f32, clip: Rect, active: bool) -> Quad {
    Quad {
        rect: bar.scaled(scale),
        color: Color::rgba(1.0, 1.0, 1.0, if active { 0.4 } else { 0.18 }),
        border_color: Color::TRANSPARENT,
        radius: [bar.w.min(bar.h) / 2.0 * scale; 4],
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
