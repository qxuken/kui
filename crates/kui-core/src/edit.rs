//! Editable text: retained editor state keyed by widget `Key`, built on
//! cosmic-text's `Editor` so cursor motion, selection, and click-to-position
//! all come from the same shaping truth the rest of the text stack uses.
//! Edits arrive as data (`InputEvent::Text` / `InputEvent::Key`) routed to
//! the focused editor; hosts read text back with `Core::edit_text`.

use cosmic_text::{
    Action, Attrs, Buffer, Edit as _, Editor, FontSystem, Metrics, Motion, Selection, Shaping,
};
use rustc_hash::FxHashMap;

use crate::color::Color;
use crate::display::{Quad, QuadKind};
use crate::geom::{Rect, Size, Vec2};
use crate::input::{EditKey, Mods};
use crate::key::Key;
use crate::spec::TextStyle;
use crate::text::TextSystem;
use crate::tree::OriginId;

#[derive(Clone, Debug)]
pub struct EditOptions {
    pub style: TextStyle,
    pub multiline: bool,
    /// Grab focus when first created (if nothing else is focused).
    pub autofocus: bool,
    /// Selection highlight color.
    pub accent: Color,
}

impl Default for EditOptions {
    fn default() -> Self {
        Self {
            style: TextStyle::default(),
            multiline: false,
            autofocus: false,
            accent: Color::rgba8(0x3b, 0x5b, 0xd4, 0x66),
        }
    }
}

pub(crate) struct EditState {
    editor: Editor<'static>,
    pub(crate) style: TextStyle,
    pub(crate) accent: Color,
    pub(crate) multiline: bool,
    pub(crate) origin: OriginId,
    scale: f32,
    /// Wrap width (physical) the buffer is laid out at; None = unwrapped.
    wrap: Option<f32>,
    /// Bumped on every content change.
    pub version: u64,
    /// Cached wrapped measurement: (version, wrap bits, size).
    measured: Option<(u64, u32, Size)>,
}

#[derive(Default)]
pub struct EditStore {
    states: FxHashMap<Key, EditState>,
    pub(crate) focused: Option<Key>,
    /// Edit node being drag-selected (with its content origin, logical).
    pub(crate) dragging: Option<(Key, Vec2)>,
}

fn attrs() -> Attrs<'static> {
    Attrs::new().family(cosmic_text::Family::SansSerif)
}

fn attrs_for(style: &TextStyle) -> Attrs<'static> {
    match style.family {
        crate::spec::FontFamily::Sans => attrs(),
        crate::spec::FontFamily::Serif => Attrs::new().family(cosmic_text::Family::Serif),
        crate::spec::FontFamily::Mono => Attrs::new().family(cosmic_text::Family::Monospace),
    }
}

impl EditStore {
    pub fn focused(&self) -> Option<Key> {
        self.focused
    }

    pub fn set_focus(&mut self, key: Option<Key>) {
        self.focused = key;
    }

    /// Ensures state exists for `key`, seeding `initial` on first creation.
    pub(crate) fn declare(
        &mut self,
        key: Key,
        initial: &str,
        opts: &EditOptions,
        origin: OriginId,
        scale: f32,
        fs: &mut FontSystem,
    ) {
        let state = self.states.entry(key).or_insert_with(|| {
            let metrics = Metrics::new(opts.style.size * scale, opts.style.line_height * scale);
            let mut buffer = Buffer::new(fs, metrics);
            buffer.set_size(fs, None, None);
            buffer.set_text(fs, initial, attrs_for(&opts.style), Shaping::Advanced);
            EditState {
                editor: Editor::new(buffer),
                style: opts.style,
                accent: opts.accent,
                multiline: opts.multiline,
                origin,
                scale,
                wrap: None,
                version: 0,
                measured: None,
            }
        });
        state.origin = origin;
        state.multiline = opts.multiline;
        state.accent = opts.accent;
        // Style/scale changes re-metric the buffer (text and cursor survive).
        if state.scale != scale || state.style != opts.style {
            state.style = opts.style;
            state.scale = scale;
            let metrics = Metrics::new(opts.style.size * scale, opts.style.line_height * scale);
            state.editor.with_buffer_mut(|b| b.set_metrics(fs, metrics));
            state.wrap = None;
        }
        if opts.autofocus && self.focused.is_none() {
            self.focused = Some(key);
        }
    }

    pub fn contains(&self, key: Key) -> bool {
        self.states.contains_key(&key)
    }

    pub fn origin_of(&self, key: Key) -> Option<OriginId> {
        self.states.get(&key).map(|s| s.origin)
    }

    pub fn text(&self, key: Key) -> Option<String> {
        let s = self.states.get(&key)?;
        Some(s.editor.with_buffer(buffer_text))
    }

    pub fn set_text(&mut self, key: Key, text: &str, fs: &mut FontSystem) {
        if let Some(s) = self.states.get_mut(&key) {
            let a = attrs_for(&s.style);
            s.editor.with_buffer_mut(|b| b.set_text(fs, text, a, Shaping::Advanced));
            s.editor.set_selection(Selection::None);
            s.editor.action(fs, Action::Motion(Motion::BufferEnd));
            s.version += 1;
            s.measured = None;
            s.wrap = None;
        }
    }

    pub fn version(&self, key: Key) -> u64 {
        self.states.get(&key).map_or(0, |s| s.version)
    }

    pub fn copy_selection(&self, key: Key) -> Option<String> {
        self.states.get(&key)?.editor.copy_selection()
    }

    /// Deletes the selection; returns true if anything was deleted.
    pub fn delete_selection(&mut self, key: Key, fs: &mut FontSystem) -> bool {
        let Some(s) = self.states.get_mut(&key) else { return false };
        let _ = fs;
        if s.editor.delete_selection() {
            s.version += 1;
            s.measured = None;
            true
        } else {
            false
        }
    }

    // -- Input application (focused editor). Returns true if content changed.

    pub(crate) fn apply_text(&mut self, key: Key, text: &str, fs: &mut FontSystem) -> bool {
        let Some(s) = self.states.get_mut(&key) else { return false };
        let filtered: String = text
            .chars()
            .filter(|c| !c.is_control() || (*c == '\n' && s.multiline) || *c == '\t')
            .collect();
        if filtered.is_empty() {
            return false;
        }
        s.editor.delete_selection();
        s.editor.insert_string(&filtered, None);
        s.editor.shape_as_needed(fs, false);
        s.version += 1;
        s.measured = None;
        true
    }

    /// Returns (content_changed, submit) — submit is Enter in single-line.
    pub(crate) fn apply_key(
        &mut self,
        key: Key,
        ek: EditKey,
        mods: Mods,
        fs: &mut FontSystem,
    ) -> (bool, bool) {
        let Some(s) = self.states.get_mut(&key) else { return (false, false) };
        let mut changed = false;
        let mut submit = false;
        match ek {
            EditKey::Left | EditKey::Right | EditKey::Up | EditKey::Down
            | EditKey::Home | EditKey::End | EditKey::PageUp | EditKey::PageDown => {
                let motion = match (ek, mods.word, mods.doc) {
                    (EditKey::Left, true, _) => Motion::LeftWord,
                    (EditKey::Right, true, _) => Motion::RightWord,
                    (EditKey::Left, _, true) => Motion::Home,
                    (EditKey::Right, _, true) => Motion::End,
                    (EditKey::Up, _, true) => Motion::BufferStart,
                    (EditKey::Down, _, true) => Motion::BufferEnd,
                    (EditKey::Left, ..) => Motion::Left,
                    (EditKey::Right, ..) => Motion::Right,
                    (EditKey::Up, ..) => Motion::Up,
                    (EditKey::Down, ..) => Motion::Down,
                    (EditKey::Home, _, true) => Motion::BufferStart,
                    (EditKey::End, _, true) => Motion::BufferEnd,
                    (EditKey::Home, ..) => Motion::Home,
                    (EditKey::End, ..) => Motion::End,
                    (EditKey::PageUp, ..) => Motion::PageUp,
                    (EditKey::PageDown, ..) => Motion::PageDown,
                    _ => unreachable!(),
                };
                if mods.shift {
                    if s.editor.selection() == Selection::None {
                        s.editor.set_selection(Selection::Normal(s.editor.cursor()));
                    }
                } else {
                    s.editor.set_selection(Selection::None);
                }
                s.editor.action(fs, Action::Motion(motion));
            }
            EditKey::Backspace => {
                if !s.editor.delete_selection() {
                    if mods.word {
                        s.editor.set_selection(Selection::Normal(s.editor.cursor()));
                        s.editor.action(fs, Action::Motion(Motion::LeftWord));
                        s.editor.delete_selection();
                    } else {
                        s.editor.action(fs, Action::Backspace);
                    }
                }
                changed = true;
            }
            EditKey::Delete => {
                if !s.editor.delete_selection() {
                    s.editor.action(fs, Action::Delete);
                }
                changed = true;
            }
            EditKey::Enter => {
                if s.multiline {
                    s.editor.action(fs, Action::Enter);
                    changed = true;
                } else {
                    submit = true;
                }
            }
            EditKey::Tab => {
                if s.multiline {
                    s.editor.delete_selection();
                    s.editor.insert_string("    ", None);
                    changed = true;
                }
            }
            EditKey::SelectAll => {
                s.editor.action(fs, Action::Motion(Motion::BufferStart));
                s.editor.set_selection(Selection::Normal(s.editor.cursor()));
                s.editor.action(fs, Action::Motion(Motion::BufferEnd));
            }
            EditKey::Escape => {
                s.editor.action(fs, Action::Escape);
            }
        }
        s.editor.shape_as_needed(fs, false);
        if changed {
            s.version += 1;
            s.measured = None;
        }
        (changed, submit)
    }

    /// Mouse press inside the edit at content-local logical position.
    pub(crate) fn click(&mut self, key: Key, local: Vec2, fs: &mut FontSystem) {
        if let Some(s) = self.states.get_mut(&key) {
            let (x, y) = ((local.x * s.scale) as i32, (local.y * s.scale) as i32);
            s.editor.action(fs, Action::Click { x, y });
        }
    }

    pub(crate) fn drag(&mut self, key: Key, local: Vec2, fs: &mut FontSystem) {
        if let Some(s) = self.states.get_mut(&key) {
            let (x, y) = ((local.x * s.scale) as i32, (local.y * s.scale) as i32);
            s.editor.action(fs, Action::Drag { x, y });
        }
    }

    // -- Layout measurement (logical units)

    pub(crate) fn intrinsic(&mut self, key: Key, fs: &mut FontSystem) -> Size {
        let Some(s) = self.states.get_mut(&key) else { return Size::ZERO };
        s.editor.shape_as_needed(fs, false);
        let (w, h, lh) = s.editor.with_buffer(|b| {
            let mut w = 0.0f32;
            let mut lines = 0u32;
            for run in b.layout_runs() {
                w = w.max(run.line_w);
                lines += 1;
            }
            (w, lines.max(1) as f32 * b.metrics().line_height, b.metrics().line_height)
        });
        let _ = lh;
        // Caret margin so the cursor at line end isn't clipped.
        Size::new((w + 2.0 * s.scale) / s.scale, h / s.scale)
    }

    pub(crate) fn wrapped(&mut self, key: Key, max_w: f32, fs: &mut FontSystem) -> Size {
        let Some(s) = self.states.get_mut(&key) else { return Size::ZERO };
        let target = (max_w * s.scale).max(1.0);
        let differs = match s.wrap {
            Some(a) => (a - target).abs() > 0.5,
            None => true,
        };
        if differs {
            s.editor.with_buffer_mut(|b| b.set_size(fs, Some(target), None));
            s.wrap = Some(target);
        }
        let stamp = (s.version, target.to_bits());
        if let Some((v, w, size)) = s.measured
            && (v, w) == stamp
        {
            return size;
        }
        s.editor.shape_as_needed(fs, false);
        let h = s.editor.with_buffer(|b| {
            let mut lines = 0u32;
            for _ in b.layout_runs() {
                lines += 1;
            }
            lines.max(1) as f32 * b.metrics().line_height
        });
        let size = Size::new(max_w, h / s.scale);
        s.measured = Some((stamp.0, stamp.1, size));
        size
    }

    // -- Emission

    /// Draws selection, glyphs, and caret. `origin` is the content box origin
    /// in physical px; everything emitted is clipped by `clip`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit(
        &mut self,
        key: Key,
        origin: Vec2,
        focused: bool,
        clip: Rect,
        text_system: &mut TextSystem,
        atlas: &mut crate::atlas::GlyphAtlas,
        out: &mut Vec<Quad>,
    ) {
        let Some(s) = self.states.get_mut(&key) else { return };
        let (fs, swash) = text_system.raster_parts();
        s.editor.shape_as_needed(fs, false);
        let color = s.style.color;
        let accent = s.accent;
        let scale = s.scale;
        let selection = s.editor.selection_bounds();
        let cursor_pos = if focused { s.editor.cursor_position() } else { None };

        s.editor.with_buffer(|b| {
            let line_height = b.metrics().line_height;
            // Runs come in line order: skip everything above the clip and
            // stop at the first run past its bottom — a 100k-line document
            // emits only the visible screenful of quads.
            let runs = b
                .layout_runs()
                .filter(|run| origin.y + run.line_top + line_height >= clip.y)
                .take_while(|run| origin.y + run.line_top <= clip.y + clip.h);
            for run in runs {
                // Selection highlight for this run.
                if let Some((start, end)) = selection
                    && let Some((x, w)) = run.highlight(start, end)
                {
                    out.push(Quad {
                        rect: Rect::new(
                            origin.x + x,
                            origin.y + run.line_top,
                            w.max(2.0),
                            line_height,
                        ),
                        color: accent,
                        border_color: Color::TRANSPARENT,
                        radius: 0.0,
                        border_w: 0.0,
                        kind: QuadKind::Solid,
                        uv: [0; 4],
                        clip,
                    });
                }
                // Glyphs.
                for glyph in run.glyphs.iter() {
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    let Some(slot) = crate::text::raster_glyph(physical.cache_key, fs, swash, atlas)
                    else {
                        continue;
                    };
                    let x = origin.x + physical.x as f32 + slot.left as f32;
                    let y = origin.y + run.line_y.round() + physical.y as f32 - slot.top as f32;
                    let glyph_color = glyph
                        .color_opt
                        .map(|c| Color::rgba8(c.r(), c.g(), c.b(), c.a()))
                        .unwrap_or(color);
                    out.push(Quad {
                        rect: Rect::new(x, y, slot.w as f32, slot.h as f32),
                        color: glyph_color,
                        border_color: Color::TRANSPARENT,
                        radius: 0.0,
                        border_w: 0.0,
                        kind: if slot.color_glyph {
                            QuadKind::GlyphColor
                        } else {
                            QuadKind::GlyphMask
                        },
                        uv: [slot.x, slot.y, slot.w, slot.h],
                        clip,
                    });
                }
            }
            // Caret.
            if let Some((cx, cy)) = cursor_pos {
                out.push(Quad {
                    rect: Rect::new(
                        origin.x + cx as f32,
                        origin.y + cy as f32,
                        (2.0 * scale).max(2.0),
                        line_height,
                    ),
                    color,
                    border_color: Color::TRANSPARENT,
                    radius: 0.0,
                    border_w: 0.0,
                    kind: QuadKind::Solid,
                    uv: [0; 4],
                    clip,
                });
            }
        });
    }
}

fn buffer_text(b: &Buffer) -> String {
    let mut out = String::new();
    for (i, line) in b.lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line.text());
    }
    out
}
