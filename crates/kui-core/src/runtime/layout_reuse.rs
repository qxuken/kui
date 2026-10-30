//! Conservative layout retention for frames containing replayed content
//! (F284). This first path reuses a complete supported layout only when
//! all layout inputs match. Only layout computation and eligible quad
//! generation are skipped; frame-dependent state still updates normally.

use super::*;
use crate::spec::LayoutSpec;
use crate::text::LayoutTextCache;

// Bound retained memory independently of an app's largest fresh tree.
const MAX_NODES: usize = 4096;

#[derive(Clone, Copy, PartialEq)]
struct Context {
    viewport: Size,
    scale: f32,
    host_area: Rect,
    menu_edge: f32,
    fonts: u64,
    weights: u64,
}

#[derive(Clone, Copy, PartialEq)]
struct Input {
    key: Key,
    parent: u32,
    content: NodeContent,
    layout: LayoutSpec,
}

#[derive(Default)]
pub(super) struct LayoutReuse {
    pub(super) replayed: bool,
    pub(super) refilled: bool,
    pub(super) used: bool,
    pub(super) paint: super::paint_reuse::PaintReuse,
    pub(super) paint_candidates: Vec<bool>,
    has_paint_candidates: bool,
    context: Option<Context>,
    valid: bool,
    inputs: Vec<Input>,
    layouts: Vec<LayoutSpec>,
    size: Vec<Size>,
    pos: Vec<Vec2>,
    scroll_max: Vec<Vec2>,
    line: Vec<u32>,
    baseline: Vec<f32>,
    text: LayoutTextCache,
}

impl LayoutReuse {
    /// Answers whether this frame's nodes go through the retained-paint
    /// cache, and readies the cache for them (F284).
    pub(super) fn begin_paint(&mut self) -> bool {
        let enabled = self.valid
            && self.context.is_some()
            && self.replayed
            && !self.refilled
            && self.has_paint_candidates;
        self.paint.begin(self.inputs.len(), enabled);
        enabled
    }

    fn clear(&mut self) {
        self.valid = false;
        self.context = None;
        self.inputs.clear();
        self.layouts.clear();
        self.size.clear();
        self.pos.clear();
        self.scroll_max.clear();
        self.line.clear();
        self.baseline.clear();
        self.text.clear();
        self.paint_candidates.clear();
        self.has_paint_candidates = false;
    }
}

impl Core {
    pub(crate) fn note_layout_replay(&mut self) {
        self.layout_reuse.replayed = true;
    }

    fn layout_cache_supported(&self) -> bool {
        let t = &self.tree;
        !t.is_empty()
            && t.len() <= MAX_NODES
            && self.dt_app.is_none()
            && !t.any_float
            && !t.any_table
            && !t.any_hidden
            && !t.any_slide
            && !t.any_moved
            && !t.any_offset_stop
            && !t.any_exit
            && !t.any_keep_room
            && t.ruled.is_empty()
            && t.content
                .iter()
                .all(|c| matches!(c, NodeContent::Container | NodeContent::Text(_)))
            && t.specs
                .iter()
                .all(|s| !s.layout.scroll_x && !s.layout.scroll_y && s.rules_ref().is_none())
            && self.text.layout_cacheable()
    }

    /// Called at exactly the old `layout::compute` boundary, after root
    /// and menu geometry are set, before any layout-dependent side effects.
    pub(super) fn reuse_layout(&mut self) -> bool {
        self.layout_reuse.used = false;
        if !self.layout_reuse.replayed
            || self.layout_reuse.refilled
            || !self.layout_cache_supported()
        {
            self.layout_reuse.clear();
            return false;
        }
        let context = Context {
            viewport: self.viewport,
            scale: self.scale,
            host_area: self.tree.host_area,
            menu_edge: self.tree.menu_edge,
            fonts: self.fonts_rev,
            weights: self.weights_rev,
        };
        let inputs = || {
            self.tree.specs.iter().enumerate().map(|(i, s)| Input {
                key: self.tree.keys[i],
                parent: self.tree.parent[i],
                content: self.tree.content[i],
                layout: s.layout,
            })
        };
        let cache = &mut self.layout_reuse;
        if cache.context != Some(context) {
            cache.paint.invalidate();
        }
        if cache.valid
            && cache.context == Some(context)
            && cache.inputs.len() == self.tree.len()
            && inputs().eq(cache.inputs.iter().copied())
            && self.text.layout_cache_matches(&cache.text)
        {
            for (s, layout) in self.tree.specs.iter_mut().zip(&cache.layouts) {
                s.layout = *layout;
            }
            self.tree.size.clone_from(&cache.size);
            self.tree.pos.clone_from(&cache.pos);
            self.tree.scroll_max.clone_from(&cache.scroll_max);
            self.tree.line.clone_from(&cache.line);
            self.tree.baseline.clone_from(&cache.baseline);
            self.text.layout_cache_restore(&cache.text);
            cache.used = true;
            return true;
        }
        cache.valid = false;
        cache.context = Some(context);
        cache.inputs.clear();
        cache.inputs.extend(inputs());
        self.text.layout_cache_inputs(&mut cache.text);
        false
    }

    /// Save compute's output, before reveal, easing, events or other
    /// frame-dependent work can change positions. Those still run on hits.
    pub(super) fn save_layout(&mut self) {
        let cache = &mut self.layout_reuse;
        if cache.context.is_none() {
            return;
        }
        cache.layouts.clear();
        cache
            .layouts
            .extend(self.tree.specs.iter().map(|s| s.layout));
        cache.size.clone_from(&self.tree.size);
        cache.pos.clone_from(&self.tree.pos);
        cache.scroll_max.clone_from(&self.tree.scroll_max);
        cache.line.clone_from(&self.tree.line);
        cache.baseline.clone_from(&self.tree.baseline);
        self.text.layout_cache_save(&mut cache.text);
        cache.paint_candidates.clear();
        cache
            .paint_candidates
            .extend(self.tree.content.iter().map(|c| match c {
                NodeContent::Text(tid) => self.text.paint_cache_candidate(*tid),
                _ => false,
            }));
        cache.has_paint_candidates = cache.paint_candidates.iter().any(|c| *c);
        cache.valid = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, FillAnswer, NodeSpec, TextStyle, Value};

    fn frame(
        core: &mut Core,
        width: f32,
        scale: f32,
        outside: &str,
        reference: bool,
    ) -> FillAnswer {
        let mut ui = core.frame(Size::new(width, 500.0), scale);
        ui.configure_root(NodeSpec::column().fill().gap(3.0));
        ui.text(outside, TextStyle::new(13.0));
        let answer = ui.memo("screen", &Value::Null, |ui| {
            ui.with(NodeSpec::column().grow_width().gap(2.0), |ui| {
                for i in 0..12 {
                    ui.with(
                        NodeSpec::row()
                            .grow_width()
                            .gap(8.0)
                            .pad(3.0)
                            .on_click(Value::Int(i))
                            .hover_bg(Color::hex(0x334455ff)),
                        |ui| {
                            ui.text(&format!("unit {i}"), TextStyle::new(13.0));
                            ui.text(
                                "the same shared text at several widths, with enough characters to reuse its emitted quad run",
                                TextStyle::new(12.0),
                            );
                        },
                    );
                }
                for width in [80.0, 160.0, 80.0] {
                    ui.with(NodeSpec::column().width(width), |ui| {
                        ui.text(
                            "the same shared text at several widths, with enough characters to reuse its emitted quad run",
                            TextStyle::new(12.0),
                        );
                    });
                }
            });
        });
        if reference {
            ui.core().layout_reuse.replayed = false;
        }
        ui.finish();
        answer
    }

    fn equal(a: &Core, b: &Core) {
        assert_eq!(a.tree.keys, b.tree.keys);
        assert_eq!(a.tree.size, b.tree.size);
        assert_eq!(a.tree.pos, b.tree.pos);
        assert_eq!(a.tree.line, b.tree.line);
        assert_eq!(a.tree.scroll_max, b.tree.scroll_max);
        assert_eq!(
            a.tree
                .baseline
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            b.tree
                .baseline
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            a.tree.specs.iter().map(|s| s.layout).collect::<Vec<_>>(),
            b.tree.specs.iter().map(|s| s.layout).collect::<Vec<_>>()
        );
        assert_eq!(
            format!("{:?}", a.drawn().0.quads),
            format!("{:?}", b.drawn().0.quads)
        );
        assert_eq!(a.drawn().0.clips, b.drawn().0.clips);
    }

    #[test]
    fn unchanged_layout_reuses_sizes_and_per_width_text_state() {
        let (mut cached, mut reference) = (Core::new(), Core::new());
        for _ in 0..6 {
            frame(&mut cached, 400.0, 1.0, "outside", false);
            frame(&mut reference, 400.0, 1.0, "outside", true);
            equal(&cached, &reference);
        }
        assert!(
            cached.layout_reuse.used,
            "this test must actually hit the cache"
        );
    }

    #[test]
    fn viewport_scale_and_fresh_text_outside_the_memo_invalidate_layout() {
        let (mut cached, mut reference) = (Core::new(), Core::new());
        for _ in 0..4 {
            frame(&mut cached, 400.0, 1.0, "outside", false);
            frame(&mut reference, 400.0, 1.0, "outside", true);
        }
        for (width, scale, text) in [
            (240.0, 1.0, "outside"),
            (240.0, 2.0, "outside"),
            (240.0, 2.0, "fresh model data outside the memo"),
        ] {
            frame(&mut cached, width, scale, text, false);
            frame(&mut reference, width, scale, text, true);
            assert!(!cached.layout_reuse.used);
            equal(&cached, &reference);
            frame(&mut cached, width, scale, text, false);
            frame(&mut reference, width, scale, text, true);
            equal(&cached, &reference);
        }
    }

    fn styled(core: &mut Core, color: Color, opacity: f32, radius: f32, reference: bool) {
        // Long unbroken lines at their natural width are profitable quad
        // candidates. Tightly wrapped runs may need ensure_wrap's fork
        // side effects and conservatively stay on the normal paint path.
        let mut ui = core.frame(Size::new(1600.0, 180.0), 1.0);
        ui.configure_root(
            NodeSpec::column()
                .fill()
                .clip()
                .radius(radius)
                .opacity(opacity)
                .text_color(color)
                .selectable(),
        );
        ui.memo("text", &Value::Null, |ui| {
            for _ in 0..4 {
                ui.text("A long plain text run that is worth retaining as quads, rather than regenerating each glyph every frame.",
                    TextStyle::new(14.0));
            }
        });
        if reference {
            ui.core().layout_reuse.replayed = false;
        }
        ui.finish();
    }

    #[test]
    fn color_opacity_and_rounded_clips_repaint_without_stale_quads() {
        let (mut cached, mut reference) = (Core::new(), Core::new());
        for _ in 0..5 {
            styled(&mut cached, Color::WHITE, 1.0, 0.0, false);
            styled(&mut reference, Color::WHITE, 1.0, 0.0, true);
        }
        assert!(cached.layout_reuse.paint.reused > 0);
        for (color, opacity, radius) in [
            (Color::hex(0xff0044ff), 1.0, 0.0),
            (Color::WHITE, 0.5, 0.0),
            (Color::WHITE, 1.0, 18.0),
            (Color::WHITE, 1.0, 0.0),
        ] {
            styled(&mut cached, color, opacity, radius, false);
            styled(&mut reference, color, opacity, radius, true);
            assert!(
                cached.layout_reuse.used,
                "paint changes need not invalidate geometry"
            );
            equal(&cached, &reference);
        }
    }

    #[test]
    fn an_atlas_reset_rebuilds_glyphs_before_any_quad_run_is_reused() {
        let (mut cached, mut reference) = (Core::new(), Core::new());
        for _ in 0..5 {
            styled(&mut cached, Color::WHITE, 1.0, 0.0, false);
            styled(&mut reference, Color::WHITE, 1.0, 0.0, true);
        }
        assert!(cached.layout_reuse.paint.reused > 0);
        cached.atlas.clear();
        reference.atlas.clear();
        styled(&mut cached, Color::WHITE, 1.0, 0.0, false);
        styled(&mut reference, Color::WHITE, 1.0, 0.0, true);
        assert!(cached.layout_reuse.used);
        assert_eq!(cached.layout_reuse.paint.reused, 0);
        equal(&cached, &reference);
    }

    #[test]
    fn shaping_generation_changes_invalidate_geometry() {
        let (mut cached, mut reference) = (Core::new(), Core::new());
        for _ in 0..5 {
            frame(&mut cached, 400.0, 1.0, "outside", false);
            frame(&mut reference, 400.0, 1.0, "outside", true);
        }
        // Exercise the revision gate independently of which platform
        // fonts happen to be installed on the test machine.
        cached.session.state().fonts_rev += 1;
        reference.session.state().fonts_rev += 1;
        frame(&mut cached, 400.0, 1.0, "outside", false);
        frame(&mut reference, 400.0, 1.0, "outside", true);
        assert!(!cached.layout_reuse.used);
        equal(&cached, &reference);
    }

    #[test]
    fn shaping_generation_invalidates_quads_even_when_atlas_and_text_keys_stay_the_same() {
        let (mut cached, mut reference) = (Core::new(), Core::new());
        for _ in 0..5 {
            styled(&mut cached, Color::WHITE, 1.0, 0.0, false);
            styled(&mut reference, Color::WHITE, 1.0, 0.0, true);
        }
        assert!(cached.layout_reuse.paint.reused > 0);
        let stamp = cached.atlas.stamp;
        cached.session.state().fonts_rev += 1;
        reference.session.state().fonts_rev += 1;
        styled(&mut cached, Color::WHITE, 1.0, 0.0, false);
        styled(&mut reference, Color::WHITE, 1.0, 0.0, true);
        assert_eq!(cached.atlas.stamp, stamp);
        assert!(!cached.layout_reuse.used);
        assert_eq!(cached.layout_reuse.paint.reused, 0);
        equal(&cached, &reference);
    }

    #[test]
    fn unsupported_layout_features_use_the_ordinary_engine() {
        let mut cases = Vec::new();
        cases.push(NodeSpec::column().fill().scroll_y());
        cases.push(NodeSpec::table().fill());
        cases.push(NodeSpec::column().fill().exit(crate::Enter::default()));
        let mut float = NodeSpec::column().fill();
        float.layout.float = Some(crate::spec::FloatConfig::default());
        cases.push(float);
        let mut hidden = NodeSpec::column().fill();
        hidden.hidden = true;
        cases.push(hidden);
        let mut rules = NodeSpec::column().fill();
        rules.rules_mut();
        cases.push(rules);
        for root in cases {
            let mut core = Core::new();
            for _ in 0..4 {
                let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
                ui.configure_root(root.clone());
                ui.memo("m", &Value::Null, |ui| {
                    ui.text("text", TextStyle::new(12.0))
                });
                ui.finish();
                assert!(!core.layout_reuse.used);
                assert!(!core.layout_reuse.paint.enabled);
            }
        }
    }

    #[test]
    fn a_refilled_panel_does_not_pay_to_capture_a_never_reused_layout() {
        let mut core = Core::new();
        for revision in 0..5 {
            let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
            ui.memo("static", &Value::Null, |ui| {
                ui.text("static", TextStyle::new(12.0))
            });
            ui.memo("changing", &Value::Int(revision), |ui| {
                ui.text("changing", TextStyle::new(12.0))
            });
            ui.finish();
            assert!(!core.layout_reuse.used);
            assert!(!core.layout_reuse.valid);
            assert!(!core.layout_reuse.paint.enabled);
        }
    }

    #[test]
    fn cached_geometry_still_rebuilds_hover_hits_and_delivers_clicks() {
        let (mut cached, mut reference) = (Core::new(), Core::new());
        for _ in 0..5 {
            frame(&mut cached, 400.0, 1.0, "outside", false);
            frame(&mut reference, 400.0, 1.0, "outside", true);
        }
        let i = cached
            .tree
            .specs
            .iter()
            .position(|s| s.events().on_click() == Some(&Value::Int(3)))
            .unwrap();
        let at = cached.tree.pos[i];
        for core in [&mut cached, &mut reference] {
            core.handle_input(crate::InputEvent::CursorMoved(Vec2::new(
                at.x + 4.0,
                at.y + 4.0,
            )));
        }
        frame(&mut cached, 400.0, 1.0, "outside", false);
        frame(&mut reference, 400.0, 1.0, "outside", true);
        assert!(cached.layout_reuse.used);
        equal(&cached, &reference);
        let click = |core: &mut Core| {
            crate::testing::click_at(core, at.x + 4.0, at.y + 4.0)
                .into_iter()
                .map(|e| (e.key, e.origin, e.instance, e.event, e.payload))
                .collect::<Vec<_>>()
        };
        let events = click(&mut cached);
        assert_eq!(events, click(&mut reference));
        assert!(
            events
                .iter()
                .any(|(_, _, _, _, payload)| payload.get("tag") == Some(&Value::Int(3)))
        );
    }

    #[test]
    fn selection_is_painted_live_instead_of_replaying_an_unselected_run() {
        let (mut cached, mut reference) = (Core::new(), Core::new());
        for _ in 0..5 {
            styled(&mut cached, Color::WHITE, 1.0, 0.0, false);
            styled(&mut reference, Color::WHITE, 1.0, 0.0, true);
        }
        assert!(cached.layout_reuse.paint.reused > 0);
        let before = cached.drawn().0.quads.len();
        for core in [&mut cached, &mut reference] {
            crate::testing::press(core, Vec2::new(4.0, 8.0));
            core.handle_input(crate::InputEvent::CursorMoved(Vec2::new(180.0, 8.0)));
            crate::testing::release(core);
        }
        styled(&mut cached, Color::WHITE, 1.0, 0.0, false);
        styled(&mut reference, Color::WHITE, 1.0, 0.0, true);
        assert!(cached.layout_reuse.used);
        assert!(
            cached.drawn().0.quads.len() > before,
            "the selection must actually draw highlights"
        );
        equal(&cached, &reference);
    }

    #[test]
    fn baseline_wrap_lines_and_resolved_fit_clamps_are_restored() {
        let mut wrapped = NodeSpec::row().fill().gap(4.0);
        wrapped.layout.wrap = true;
        let mut clamped = NodeSpec::column().fill();
        clamped.layout.min_w = crate::spec::Min::FIT;
        for root in [
            NodeSpec::row().fill().cross_align(crate::Align::Baseline),
            wrapped,
            clamped,
        ] {
            let (mut cached, mut reference) = (Core::new(), Core::new());
            for _ in 0..5 {
                for (core, normal) in [(&mut cached, false), (&mut reference, true)] {
                    let mut ui = core.frame(Size::new(250.0, 160.0), 1.0);
                    ui.configure_root(root.clone());
                    ui.memo("rows", &Value::Null, |ui| {
                        for size in [12.0, 20.0, 30.0, 12.0, 20.0, 30.0] {
                            ui.text("baseline", TextStyle::new(size));
                        }
                    });
                    if normal {
                        ui.core().layout_reuse.replayed = false;
                    }
                    ui.finish();
                }
                equal(&cached, &reference);
            }
            assert!(cached.layout_reuse.used);
        }
    }

    #[test]
    fn retained_geometry_and_quad_storage_are_bounded() {
        let mut core = Core::new();
        let line = "x".repeat(1000);
        for _ in 0..4 {
            let mut ui = core.frame(Size::new(20000.0, 5000.0), 1.0);
            ui.memo("prose", &Value::Null, |ui| {
                for _ in 0..80 {
                    ui.text(&line, TextStyle::new(13.0));
                }
            });
            ui.finish();
            assert!(core.layout_reuse.paint.retained_capacity() <= 32768);
        }
        assert!(core.layout_reuse.used);
        assert!(core.layout_reuse.paint.reused > 0);
        assert!(core.drawn().0.quads.len() > 32768);
        // A larger tree takes no layout snapshot, even though the memo
        // itself replays. Existing cache capacity cannot grow with it.
        for _ in 0..4 {
            let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
            ui.memo("large", &Value::Null, |ui| {
                for _ in 0..MAX_NODES {
                    ui.leaf(NodeSpec::row().size(1.0, 1.0));
                }
            });
            ui.finish();
            assert!(!core.layout_reuse.used);
            assert!(!core.layout_reuse.paint.enabled);
            assert!(core.layout_reuse.inputs.is_empty());
        }
    }
}
