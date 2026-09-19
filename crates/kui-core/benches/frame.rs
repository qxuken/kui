//! Steady-state frame cost: build + layout + emit over a reused Core, the way
//! a real app runs (text caches warm after the first frame).
//!
//! Run: cargo bench -p kui-core

use kui_core::{
    Color, Core, Key, NodeSpec, Size, Sizing, Stroke, TextStyle, Ui, Value, Vec2, widgets,
};

/// What one grid frame contains. Every grid bench below goes through the
/// same builder and differs only in these switches, so their medians can be
/// compared with each other directly.
#[derive(Clone, Copy)]
struct Grid {
    rows: usize,
    cols: usize,
    /// Every eighth cell holds a label.
    text: bool,
    /// Every fourth cell is clickable (a hit region, and a semantic node in
    /// the access tree).
    clicks: bool,
    /// Every cell casts an outer drop shadow — one more quad each.
    shadows: bool,
    /// The root is faded, so emission alpha-multiplies every quad.
    opacity: bool,
    /// Every cell declares a `transition` (nine retained tween slots each).
    transitions: bool,
    /// Every cell also declares an `exit`, so the frame is kept whole for
    /// the next one to diff against (see `kui_core::depart`).
    exits: bool,
    /// Every row clips, so all 10k cells inherit a clip rather than the
    /// `NO_CLIP` the unclipped benches take a shortcut for.
    clip: bool,
    /// Implies `clip`: every row also has a radius, so the clip its cells
    /// inherit is rounded and each one costs the per-corner intersect.
    rounded_clip: bool,
}

impl Grid {
    fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            text: false,
            clicks: false,
            shadows: false,
            opacity: false,
            transitions: false,
            exits: false,
            clip: false,
            rounded_clip: false,
        }
    }

    fn text(mut self) -> Self {
        self.text = true;
        self
    }

    fn clicks(mut self) -> Self {
        self.clicks = true;
        self
    }

    fn shadows(mut self) -> Self {
        self.shadows = true;
        self
    }

    fn opacity(mut self) -> Self {
        self.opacity = true;
        self
    }

    fn transitions(mut self) -> Self {
        self.transitions = true;
        self
    }

    /// Implies `transitions`: an `exit` without one is not an exit.
    fn exits(mut self) -> Self {
        self.transitions = true;
        self.exits = true;
        self
    }

    fn clip(mut self) -> Self {
        self.clip = true;
        self
    }

    /// Implies `clip`: a radius rounds nothing on a node that does not
    /// clip.
    fn rounded_clip(mut self) -> Self {
        self.clip = true;
        self.rounded_clip = true;
        self
    }
}

fn grid(ui: &mut Ui<'_>, g: Grid) {
    let mut root = NodeSpec::column().fill().pad(8.0).gap(4.0);
    if g.opacity {
        root = root.opacity(0.85);
    }
    ui.configure_root(root);
    for r in 0..g.rows {
        let mut row = NodeSpec::row().width(Sizing::Grow(1.0)).gap(4.0);
        if g.clip {
            row = row.clip();
        }
        if g.rounded_clip {
            row = row.radius(6.0);
        }
        ui.with(row, |ui| {
            for c in 0..g.cols {
                let mut spec = NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(14.0))
                    .bg(Color::rgb8((r % 255) as u8, (c % 255) as u8, 128))
                    .radius(2.0);
                if g.shadows {
                    spec = spec
                        .shadow_color(Color::rgba8(0, 0, 0, 96))
                        .shadow_blur(6.0)
                        .shadow_y(2.0);
                }
                if g.transitions {
                    spec = spec.transition(200.0);
                }
                if g.exits {
                    spec = spec.exit(kui_core::Enter::default().opacity(0.0));
                }
                if g.clicks && c % 4 == 0 {
                    spec = spec.on_click(Value::Int((r * g.cols + c) as i64));
                }
                if g.text && c % 8 == 0 {
                    ui.with(spec, |ui| {
                        // 64 distinct strings -> realistic warm-cache text load.
                        let s = format!("cell {}", (r * g.cols + c) % 64);
                        ui.text(&s, TextStyle::new(10.0));
                    });
                } else {
                    ui.with(spec, |_| {});
                }
            }
        });
    }
}

fn run_frame(core: &mut Core, g: Grid) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    grid(&mut ui, g);
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

#[divan::bench]
fn frame_10k_rects(bencher: divan::Bencher) {
    let g = Grid::new(100, 100);
    let mut core = Core::new();
    run_frame(&mut core, g);
    bencher.bench_local(|| run_frame(&mut core, g));
}

#[divan::bench]
fn frame_10k_rects_with_text_and_hits(bencher: divan::Bencher) {
    let g = Grid::new(100, 100).text().clicks();
    let mut core = Core::new();
    run_frame(&mut core, g);
    bencher.bench_local(|| run_frame(&mut core, g));
}

/// The same frame with the access tree derived after it — what a frame
/// costs while assistive technology is attached. Every fourth cell is a
/// button (a semantic node); the other rects are elided.
#[divan::bench]
fn frame_10k_rects_with_access_tree(bencher: divan::Bencher) {
    let g = Grid::new(100, 100).text().clicks();
    let mut core = Core::new();
    run_frame(&mut core, g);
    bencher.bench_local(|| {
        run_frame(&mut core, g);
        core.access_tree().nodes.len()
    });
}

// -- Hit-testing (ADR 0026) --------------------------------------------------
// A pointer move re-resolves the hovered region: a scan of every hit
// region, rect first, and the shape past it for the ones under the
// pointer. `frame_10k_rects_with_text_and_hits` above is where the
// regions are *built* (2,500 of them, every one rounded, so every one
// carries a shape); this is where they are *read*, once per input event.

/// The cursor moving between two cells of the 10k-region frame, so the
/// hovered node changes every move and the scan runs to its end.
#[divan::bench]
fn hover_over_10k_regions(bencher: divan::Bencher) {
    let g = Grid::new(100, 100).clicks();
    let mut core = Core::new();
    run_frame(&mut core, g);
    let mut flip = false;
    bencher.bench_local(|| {
        flip = !flip;
        let p = if flip {
            Vec2::new(30.0, 20.0)
        } else {
            Vec2::new(1800.0, 1000.0)
        };
        core.handle_input(kui_core::InputEvent::CursorMoved(p))
            .len()
    });
}

#[divan::bench]
fn frame_1k_typical(bencher: divan::Bencher) {
    let g = Grid::new(32, 32).text().clicks();
    let mut core = Core::new();
    run_frame(&mut core, g);
    bencher.bench_local(|| run_frame(&mut core, g));
}

/// The same 10k grid as `frame_10k_rects` with the paint props that cost
/// extra work switched on: every cell casts a shadow (one more quad each)
/// under a faded root (an alpha multiply over every quad emitted). Same
/// builder, same geometry, so the difference between the two is what group
/// opacity and shadows cost.
#[divan::bench]
fn frame_10k_rects_with_shadows_and_opacity(bencher: divan::Bencher) {
    let g = Grid::new(100, 100).shadows().opacity();
    let mut core = Core::new();
    run_frame(&mut core, g);
    bencher.bench_local(|| run_frame(&mut core, g));
}

// -- Strokes ----------------------------------------------------------------
// A `line` is a float sized to its own box that emits one segment quad per
// straight piece (`docs/adr/0010-a-segment-primitive.md`). The first bench
// has exactly the quad count of `frame_10k_rects`, so the difference
// between the two is what a segment costs over a box: the line store, the
// float placement, the endpoint encoding. The second is the flattening's
// own bill — a thousand curves through eight knots, cut into pieces by
// chord length every frame.

fn segments(ui: &mut Ui<'_>, rows: usize, cols: usize) {
    ui.configure_root(NodeSpec::column().fill());
    for r in 0..rows {
        for c in 0..cols {
            let x = 8.0 + c as f32 * 19.0;
            let y = 8.0 + r as f32 * 10.6;
            ui.line(
                Vec2::new(x, y),
                Vec2::new(x + 14.0, y + 6.0),
                Stroke::new(1.5, Color::rgb8((r % 255) as u8, (c % 255) as u8, 128)),
                NodeSpec::column(),
            );
        }
    }
}

fn run_segments(core: &mut Core, rows: usize, cols: usize) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    segments(&mut ui, rows, cols);
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// 10,000 one-segment lines: the same 10k quads as `frame_10k_rects`.
#[divan::bench]
fn frame_10k_segments(bencher: divan::Bencher) {
    let mut core = Core::new();
    run_segments(&mut core, 100, 100);
    bencher.bench_local(|| run_segments(&mut core, 100, 100));
}

fn curves(ui: &mut Ui<'_>, n: usize) {
    ui.configure_root(NodeSpec::column().fill());
    let mut knots = [Vec2::ZERO; 8];
    for i in 0..n {
        let x0 = (i % 40) as f32 * 48.0;
        let y0 = (i / 40) as f32 * 40.0;
        for (k, knot) in knots.iter_mut().enumerate() {
            *knot = Vec2::new(
                x0 + k as f32 * 6.0,
                y0 + if k % 2 == 0 { 0.0 } else { 24.0 },
            );
        }
        ui.polyline(
            &knots,
            Stroke::new(1.0, Color::rgb8(200, 200, 200)).curve(),
            NodeSpec::column(),
        );
    }
}

fn run_curves(core: &mut Core, n: usize) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    curves(&mut ui, n);
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// 1,000 curves through eight knots each, flattened per frame: every span
/// is a ~24.7px chord, five pieces, so 35 segments a curve.
#[divan::bench]
fn frame_1k_curves(bencher: divan::Bencher) {
    let mut core = Core::new();
    run_curves(&mut core, 1000);
    bencher.bench_local(|| run_curves(&mut core, 1000));
}

// -- Fills and textures (ADR 0025) -----------------------------------------
// A `polygon` is a float sized to its own box that emits one fragment quad
// painted by the stock source; the first pair below is a thousand six-point
// fills against a thousand closed six-point strokes, so the difference is
// the fill's bill (the point normalisation, the fragment list entry) against
// six segment quads. The second pair is `frame_1k_typical` with eight
// texture-backed images beside it: what the side list and the pixel `Arc`
// clones cost a frame that draws one, and nothing to a frame that draws
// none (the plain bench is unchanged, which is the point).

fn hexagon(i: usize, out: &mut [Vec2; 6]) {
    let cx = 20.0 + (i % 40) as f32 * 48.0;
    let cy = 20.0 + (i / 40) as f32 * 40.0;
    for (k, p) in out.iter_mut().enumerate() {
        let a = k as f32 / 6.0 * std::f32::consts::TAU;
        *p = Vec2::new(cx + 16.0 * a.cos(), cy + 16.0 * a.sin());
    }
}

fn run_polygons(core: &mut Core, n: usize) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut pts = [Vec2::ZERO; 6];
    for i in 0..n {
        hexagon(i, &mut pts);
        ui.polygon(
            &pts,
            NodeSpec::column().bg(Color::rgb8((i % 255) as u8, 120, 200)),
        );
    }
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

fn run_closed_lines(core: &mut Core, n: usize) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut pts = [Vec2::ZERO; 7];
    let mut hex = [Vec2::ZERO; 6];
    for i in 0..n {
        hexagon(i, &mut hex);
        pts[..6].copy_from_slice(&hex);
        pts[6] = hex[0];
        ui.polyline(
            &pts,
            Stroke::new(1.0, Color::rgb8((i % 255) as u8, 120, 200)),
            NodeSpec::column(),
        );
    }
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// 1,000 six-point fills, one fragment quad each.
#[divan::bench]
fn frame_1k_polygons(bencher: divan::Bencher) {
    let mut core = Core::new();
    run_polygons(&mut core, 1000);
    bencher.bench_local(|| run_polygons(&mut core, 1000));
}

/// The same thousand outlines as closed strokes: six segment quads each.
#[divan::bench]
fn frame_1k_closed_lines(bencher: divan::Bencher) {
    let mut core = Core::new();
    run_closed_lines(&mut core, 1000);
    bencher.bench_local(|| run_closed_lines(&mut core, 1000));
}

fn run_typical_with_textures(core: &mut Core, g: Grid, images: &[kui_core::ImageId]) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    grid(&mut ui, g);
    for (i, id) in images.iter().enumerate() {
        ui.image(
            *id,
            NodeSpec::column()
                .float(kui_core::FloatConfig::parent().offset(20.0 + i as f32 * 60.0, 900.0))
                .width(Sizing::Fixed(48.0))
                .height(Sizing::Fixed(48.0)),
        );
    }
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// `frame_1k_typical` plus eight texture-backed images — registered once
/// and updated once, so each is its own texture and a side-list entry.
#[divan::bench]
fn frame_1k_typical_with_8_textures(bencher: divan::Bencher) {
    let g = Grid::new(32, 32).text().clicks();
    let mut core = Core::new();
    let images: Vec<_> = (0..8)
        .map(|_| {
            let id = core.resources.add_image(64, 64, vec![0x80; 64 * 64 * 4]);
            core.update_image(id, 64, 64, vec![0x90; 64 * 64 * 4]);
            id
        })
        .collect();
    run_typical_with_textures(&mut core, g, &images);
    bencher.bench_local(|| run_typical_with_textures(&mut core, g, &images));
}

/// What replacing a 1080p frame costs the core: the `Vec` handoff and the
/// revision bump, then the frame that draws it (the pixel `Arc` clone and
/// the side entry). The app's own copy of the frame is measured beside it
/// (`copy_1080p_frame`), so the core's share is the difference; the
/// upload is the backend's and is `benches/split.rs` in kui-wgpu under
/// `TEX=1`.
#[divan::bench]
fn copy_1080p_frame(bencher: divan::Bencher) {
    let frame = vec![0x40u8; 1920 * 1080 * 4];
    bencher.bench_local(|| frame.clone());
}

#[divan::bench]
fn update_image_1080p_and_frame(bencher: divan::Bencher) {
    let mut core = Core::new();
    let (w, h) = (1920u32, 1080u32);
    let id = core
        .resources
        .add_image(w, h, vec![0; (w * h * 4) as usize]);
    let frame = vec![0x40u8; (w * h * 4) as usize];
    bencher.bench_local(|| {
        // The app's frame is a fresh buffer each time, as a decoder's is.
        core.update_image(id, w, h, frame.clone());
        let mut ui = core.frame(Size::new(1920.0, 1080.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.image(id, NodeSpec::column().fill());
        ui.finish();
        core.output().0.quads.len()
    });
}

// -- Clipping ---------------------------------------------------------------
// A clipping node with a radius rounds what it clips, which is four more
// floats on `Quad` and a per-corner intersect for every node under a
// clipper. These two are the same tree twice, clipping square and clipping
// rounded, so the difference between them is what the rounding costs; the
// difference from `frame_10k_rects` is what clipping at all costs, which it
// always did.

/// 100 clipping rows of 100 cells: every cell inherits a clip.
#[divan::bench]
fn frame_10k_rects_square_clip(bencher: divan::Bencher) {
    let g = Grid::new(100, 100).clip();
    let mut core = Core::new();
    run_frame(&mut core, g);
    bencher.bench_local(|| run_frame(&mut core, g));
}

/// The same, with a radius on every clipping row — the pathological
/// declaration, since a real view rounds the card and not each of its rows.
#[divan::bench]
fn frame_10k_rects_rounded_clip(bencher: divan::Bencher) {
    let g = Grid::new(100, 100).rounded_clip();
    let mut core = Core::new();
    run_frame(&mut core, g);
    bencher.bench_local(|| run_frame(&mut core, g));
}

// -- Exits ------------------------------------------------------------------
// What a departing subtree costs, in the three places it can: a frame that
// declares no `exit` at all (the baseline every other bench here is), a
// frame where every node declares one, and the frame that drops them.

/// An `exit` needs a `transition`, and a transition on 10k nodes is nine
/// retained tween slots each — so this is the baseline the exit bench below
/// is read against, not `frame_10k_rects`.
#[divan::bench]
fn frame_10k_rects_all_transitioning(bencher: divan::Bencher) {
    let g = Grid::new(100, 100).transitions();
    let mut core = Core::new();
    core.set_time(0.0);
    run_frame(&mut core, g);
    run_frame(&mut core, g);
    bencher.bench_local(|| run_frame(&mut core, g));
}

/// The same frame with an `exit` on every one of the 10k cells — the
/// pathological declaration, since `exit` is per node and a grid this size
/// would put it on the grid, not on the cells. The difference from
/// `frame_10k_rects_all_transitioning` is what declaring an exit costs a
/// frame where nothing departs: the previous frame kept instead of cleared,
/// and a diff that is two flat key arrays compared.
#[divan::bench]
fn frame_10k_rects_all_declaring_exit(bencher: divan::Bencher) {
    let g = Grid::new(100, 100).exits();
    let mut core = Core::new();
    core.set_time(0.0);
    run_frame(&mut core, g);
    run_frame(&mut core, g);
    bencher.bench_local(|| run_frame(&mut core, g));
}

/// The realistic shape: one `exit`, on one node, in a 10k-node frame — a
/// dialog inside a big app. Keeping the previous frame to diff against is
/// per *frame*, not per exit, so this is where that price shows.
#[divan::bench]
fn frame_10k_rects_one_exit(bencher: divan::Bencher) {
    let mut core = Core::new();
    core.set_time(0.0);
    let g = Grid::new(100, 100);
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
        grid(&mut ui, g);
        ui.with_keyed(
            "dialog",
            NodeSpec::column()
                .width(Sizing::Fixed(200.0))
                .height(Sizing::Fixed(100.0))
                .bg(Color::rgb8(20, 20, 30))
                .transition(200.0)
                .exit(kui_core::Enter::default().opacity(0.0)),
            |_| {},
        );
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.len()
    };
    frame(&mut core);
    frame(&mut core);
    bencher.bench_local(|| frame(&mut core));
}

/// The mass removal the budget is for: 1000 rows, every one declaring an
/// `exit`, dropped in a single frame. What is measured is that frame — the
/// key diff that notices them all gone, the count of what they come to,
/// and — since ADR 0012 judges a removal whole and this one is over the
/// budget — the refusal of the lot: no subtree is copied. Before that ADR
/// this row also paid for 512 copies, which is why it fell.
#[divan::bench]
fn drop_1k_rows_declaring_exit(bencher: divan::Bencher) {
    bench_drop(bencher, 1000, true)
}

/// The same pair of frames with no `exit` on the rows: what dropping a
/// thousand rows costs today, so the bench above reads as a difference.
#[divan::bench]
fn drop_1k_rows_plain(bencher: divan::Bencher) {
    bench_drop(bencher, 1000, false)
}

/// The removal the budget admits: 500 rows declaring an `exit`, under
/// the budget, dropped in one frame — the diff, the count, and 500 subtree
/// copies into the store. The copy cost the 1k row used to carry.
#[divan::bench]
fn drop_500_rows_declaring_exit(bencher: divan::Bencher) {
    bench_drop(bencher, 500, true)
}

fn bench_drop(bencher: divan::Bencher, rows_n: usize, exits: bool) {
    fn rows(core: &mut Core, n: usize, exits: bool) {
        let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
        ui.configure_root(NodeSpec::column().fill());
        for i in 0..n {
            let mut spec = NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Fixed(14.0))
                .bg(Color::rgb8((i % 255) as u8, 90, 140));
            if exits {
                spec = spec
                    .transition(200.0)
                    .exit(kui_core::Enter::default().opacity(0.0));
            }
            ui.with_indexed(i as u64, spec, |_| {});
        }
        ui.finish();
    }
    let mut core = Core::new();
    core.set_time(0.0);
    rows(&mut core, rows_n, exits);
    bencher.bench_local(|| {
        // Back to a full list (the ghosts of the last drop are discarded
        // the moment their keys return), then drop the lot.
        rows(&mut core, rows_n, exits);
        rows(&mut core, 0, exits);
        core.depart.node_count()
    });
}

/// The 1k-typical grid with a hundred tooltips floating over it, the same
/// hundred every frame: what the float stack (ADR 0023) costs at its
/// steady state — the per-root layer walk, and one pass over the stack to
/// find that nothing opened or closed. Against `frame_1k_typical`, the
/// difference is a hundred floats.
#[divan::bench]
fn frame_1k_typical_with_100_floats(bencher: divan::Bencher) {
    fn build(core: &mut Core) -> usize {
        let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
        grid(&mut ui, Grid::new(32, 32).text().clicks());
        for i in 0..100u64 {
            ui.with_indexed(
                i,
                NodeSpec::column()
                    .float(
                        kui_core::FloatConfig::viewport()
                            .offset((i % 10) as f32 * 40.0, (i / 10) as f32 * 30.0),
                    )
                    .width(Sizing::Fixed(60.0))
                    .height(Sizing::Fixed(20.0))
                    .bg(Color::rgb8(30, 30, 40)),
                |ui| ui.text("tip", TextStyle::new(10.0)),
            );
        }
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.len()
    }
    let mut core = Core::new();
    build(&mut core);
    bencher.bench_local(|| build(&mut core));
}

/// The frame *after* a mass removal: nothing left to diff, and the budget's
/// worth of frozen subtrees replayed on top of an empty view.
#[divan::bench]
fn replay_a_full_depart_store(bencher: divan::Bencher) {
    let mut core = Core::new();
    core.set_time(0.0);
    let fill = |core: &mut Core, n: usize| {
        let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
        ui.configure_root(NodeSpec::column().fill());
        for i in 0..n {
            ui.with_indexed(
                i as u64,
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(14.0))
                    .bg(Color::rgb8((i % 255) as u8, 90, 140))
                    .transition(100_000.0)
                    .exit(kui_core::Enter::default().opacity(0.0)),
                |_| {},
            );
        }
        ui.finish();
    };
    // Exactly the budget: a removal past it is refused whole (ADR 0012),
    // so a full store is one that a frame filled precisely.
    fill(&mut core, kui_core::depart::MAX_NODES);
    fill(&mut core, 0);
    assert_eq!(core.depart.node_count(), kui_core::depart::MAX_NODES);
    bencher.bench_local(|| {
        let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.len()
    });
}

/// The wrapping row at scale: 10k chips of varying width in 100 rows that
/// each break onto several lines. The solver is the hot path, and wrapping
/// adds a break pass plus per-line grow/align walks on top of the flat
/// ones, so this is the shape that would show it — paired with the same
/// tree not wrapping (`frame_10k_chips_unwrapped`) so the difference is
/// the wrapping and not the chips.
fn chips(ui: &mut Ui<'_>, wrap: bool) {
    ui.configure_root(NodeSpec::column().fill().pad(8.0).gap(4.0));
    for r in 0..100 {
        let mut row = NodeSpec::row().width(Sizing::Grow(1.0)).gap(4.0);
        if wrap {
            row = row.wrap().cross_gap(4.0);
        }
        ui.with(row, |ui| {
            for c in 0..100 {
                ui.with(
                    NodeSpec::column()
                        // 40..=110px: several lines per row at 1920 wide.
                        .width(Sizing::Fixed(40.0 + ((r * 100 + c) % 8) as f32 * 10.0))
                        .height(Sizing::Fixed(14.0))
                        .bg(Color::rgb8((r % 255) as u8, (c % 255) as u8, 128))
                        .radius(2.0),
                    |_| {},
                );
            }
        });
    }
}

fn chip_frame(core: &mut Core, wrap: bool) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    chips(&mut ui, wrap);
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

#[divan::bench]
fn frame_10k_chips_wrapped(bencher: divan::Bencher) {
    let mut core = Core::new();
    chip_frame(&mut core, true);
    bencher.bench_local(|| chip_frame(&mut core, true));
}

#[divan::bench]
fn frame_10k_chips_unwrapped(bencher: divan::Bencher) {
    let mut core = Core::new();
    chip_frame(&mut core, false);
    bencher.bench_local(|| chip_frame(&mut core, false));
}

#[divan::bench]
fn deep_nesting_64_levels(bencher: divan::Bencher) {
    fn nest(ui: &mut Ui<'_>, depth: usize) {
        if depth == 0 {
            ui.text("leaf", TextStyle::default());
            return;
        }
        ui.with(
            NodeSpec::column().pad(1.0).bg(Color::rgb8(20, 20, 30)),
            |ui| nest(ui, depth - 1),
        );
    }
    let mut core = Core::new();
    bencher.bench_local(|| {
        let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
        for _ in 0..16 {
            nest(&mut ui, 64);
        }
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.len()
    });
}

/// A column of 1k grow rows under a staircase of `max_height`s (0.5 px to
/// 2.5 px, in 1080): the freeze loop of `distribute_run` (F71, RG5) runs
/// four passes here and freezes 348 rows on the way, so the row watches
/// what a pass costs over children the earlier passes already froze —
/// the shape that was quadratic in the frozen count before RG5.
#[divan::bench]
fn frame_1k_grow_rows_capped(bencher: divan::Bencher) {
    fn build(core: &mut Core) -> usize {
        let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
        ui.configure_root(NodeSpec::column().fill());
        for i in 0..1000 {
            ui.with(
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .max_height(0.5 + i as f32 * 0.002)
                    .bg(Color::rgb8(20, 20, 30)),
                |_| {},
            );
        }
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.len()
    }
    let mut core = Core::new();
    build(&mut core);
    bencher.bench_local(|| build(&mut core));
}

// -- Long scrolled lists ----------------------------------------------------
// The case C5 is about: a log viewer or data table whose content is far
// taller than its window. `list_naive` is what a view costs today — every row
// built and laid out, whether or not it can be seen. `list_virtual` is the
// same list through `widgets::virtual_column`, which builds the visible rows
// and two spacers. Both scroll to the middle first, so neither is measuring
// the easy case where the top of the list happens to be on screen.

const LIST_ROW_H: f32 = 24.0;

/// One row's content, the same in both benches: a coloured bar and a label,
/// so a row costs what a real row costs.
fn list_row(ui: &mut Ui<'_>, i: usize) {
    ui.with(
        NodeSpec::row()
            .fill()
            .gap(6.0)
            .pad_xy(8.0, 4.0)
            .bg(Color::rgb8(24, 24, 32))
            .on_click(Value::Int(i as i64)),
        |ui| {
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(4.0))
                    .height(Sizing::Grow(1.0))
                    .bg(Color::rgb8((i % 255) as u8, 90, 140))
                    .radius(2.0),
                |_| {},
            );
            // 64 distinct strings -> realistic warm-cache text load.
            ui.text(&format!("line {}", i % 64), TextStyle::new(12.0));
        },
    );
}

fn list_naive(core: &mut Core, rows: usize) -> usize {
    let mut ui = core.frame(Size::new(1200.0, 800.0), 2.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
        for i in 0..rows {
            ui.with_indexed(
                i as u64,
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(LIST_ROW_H)),
                |ui| list_row(ui, i),
            );
        }
    });
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

fn list_virtual(core: &mut Core, rows: usize) -> usize {
    let mut ui = core.frame(Size::new(1200.0, 800.0), 2.0);
    ui.configure_root(NodeSpec::column().fill());
    widgets::virtual_column(
        &mut ui,
        "list",
        NodeSpec::column().fill(),
        rows,
        LIST_ROW_H,
        list_row,
    );
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// Scrolls the list to its middle, where a virtualizing view has rows both
/// above and below the window to skip.
fn scroll_to_middle(core: &mut Core, rows: usize) {
    core.set_scroll(
        Key::ROOT.str("list"),
        Vec2::new(0.0, rows as f32 * LIST_ROW_H / 2.0),
    );
}

#[divan::bench]
fn list_10k_rows_naive(bencher: divan::Bencher) {
    let mut core = Core::new();
    list_naive(&mut core, 10_000);
    scroll_to_middle(&mut core, 10_000);
    list_naive(&mut core, 10_000);
    bencher.bench_local(|| list_naive(&mut core, 10_000));
}

#[divan::bench]
fn list_10k_rows_virtual(bencher: divan::Bencher) {
    let mut core = Core::new();
    list_virtual(&mut core, 10_000);
    scroll_to_middle(&mut core, 10_000);
    list_virtual(&mut core, 10_000);
    bencher.bench_local(|| list_virtual(&mut core, 10_000));
}

/// The same list an order of magnitude longer. The naive frame grows with
/// it; the virtual one should not notice.
#[divan::bench]
fn list_100k_rows_virtual(bencher: divan::Bencher) {
    let mut core = Core::new();
    list_virtual(&mut core, 100_000);
    scroll_to_middle(&mut core, 100_000);
    list_virtual(&mut core, 100_000);
    bencher.bench_local(|| list_virtual(&mut core, 100_000));
}

// The same list with no stride: `widgets::virtual_rows`, whose spacers and
// search come from prefix sums over a height cache instead of `i * row_h`.
// Three questions, in this order: what does it cost when nothing has changed
// (the frame that only scrolls), what does it cost on the frame that learns a
// row's height (the prefix sums are rebuilt), and — the one that decides
// whether the uniform widget still earns its place — what does it cost when
// every row *is* the same height and the caller could have used a stride.

fn list_variable(core: &mut Core, heights: &mut widgets::RowHeights, vary: bool) -> usize {
    let mut ui = core.frame(Size::new(1200.0, 800.0), 2.0);
    ui.configure_root(NodeSpec::column().fill());
    widgets::virtual_rows(
        &mut ui,
        "list",
        NodeSpec::column().fill(),
        heights,
        |_ui, i, _w| {
            if vary {
                LIST_ROW_H + (i % 3) as f32 * 8.0
            } else {
                LIST_ROW_H
            }
        },
        list_row,
    );
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// Two frames to settle (the first has no geometry), then scrolled to the
/// middle and settled again — the state every one of these benches measures.
fn warm_variable(core: &mut Core, rows: usize, vary: bool) -> widgets::RowHeights {
    let mut h = widgets::RowHeights::new(rows, LIST_ROW_H);
    list_variable(core, &mut h, vary);
    list_variable(core, &mut h, vary);
    scroll_to_middle(core, rows);
    list_variable(core, &mut h, vary);
    list_variable(core, &mut h, vary);
    h
}

/// Nothing new to measure: two searches and the build. It comes out *under*
/// `list_10k_rows_virtual` for a reason that is not a win — its rows average
/// 32px against the uniform bench's 24, so fewer of them are on screen.
/// `list_10k_rows_variable_at_one_height` is the comparison that holds
/// everything else equal.
#[divan::bench]
fn list_10k_rows_variable(bencher: divan::Bencher) {
    let mut core = Core::new();
    let mut h = warm_variable(&mut core, 10_000, true);
    bencher.bench_local(|| list_variable(&mut core, &mut h, true));
}

/// A frame that learns a row's height — what every frame of a scroll is, and
/// what invalidates the prefix sums. The row it learns is one it is looking
/// at, which is the only kind a scroll ever measures.
#[divan::bench]
fn list_10k_rows_variable_learning(bencher: divan::Bencher) {
    let mut core = Core::new();
    let mut h = warm_variable(&mut core, 10_000, true);
    let mut i = 0usize;
    bencher.bench_local(|| {
        i = (i + 1) % 8;
        h.set(5_000 + i, LIST_ROW_H + (i % 3) as f32 * 8.0);
        list_variable(&mut core, &mut h, true)
    });
}

/// The same frame told about a row nowhere near the window — a list whose
/// data changed under it. Everything between that row and the window has to
/// be summed again, and this is what that costs.
#[divan::bench]
fn list_10k_rows_variable_learning_far(bencher: divan::Bencher) {
    let mut core = Core::new();
    let mut h = warm_variable(&mut core, 10_000, true);
    let mut i = 0usize;
    bencher.bench_local(|| {
        i = (i + 1) % 8;
        h.set(i, LIST_ROW_H + (i % 3) as f32 * 8.0);
        list_variable(&mut core, &mut h, true)
    });
}

/// The comparison that decides whether `virtual_column` is still a widget of
/// its own: the variable one told, row by row, that every row is the same
/// height. Against `list_10k_rows_virtual`, this is what the stride buys.
#[divan::bench]
fn list_10k_rows_variable_at_one_height(bencher: divan::Bencher) {
    let mut core = Core::new();
    let mut h = warm_variable(&mut core, 10_000, false);
    bencher.bench_local(|| list_variable(&mut core, &mut h, false));
}

/// And an order of magnitude more rows, where an O(n) rebuild would show.
#[divan::bench]
fn list_100k_rows_variable(bencher: divan::Bencher) {
    let mut core = Core::new();
    let mut h = warm_variable(&mut core, 100_000, true);
    bencher.bench_local(|| list_variable(&mut core, &mut h, true));
}

#[divan::bench]
fn list_100k_rows_variable_learning(bencher: divan::Bencher) {
    let mut core = Core::new();
    let mut h = warm_variable(&mut core, 100_000, true);
    let mut i = 0usize;
    bencher.bench_local(|| {
        i = (i + 1) % 8;
        h.set(50_000 + i, LIST_ROW_H + (i % 3) as f32 * 8.0);
        list_variable(&mut core, &mut h, true)
    });
}

fn main() {
    divan::main();
}
