//! Rendering over a wire — an experiment, not an example of the API.
//!
//! The core's boundary with a backend is already data: a `DisplayList` of
//! quads in physical pixels and a glyph atlas out, `InputEvent`s in. This
//! cuts the process there. The *server* has the app and a headless `Core`
//! — model, view, layout, shaping, rasterizing — and no window; the
//! *client* has a winit window and the wgpu renderer and knows nothing of
//! the app. A TCP stream joins them, and the client holds every message
//! back by `--latency` ms each way, so what a decoupled core feels like at
//! 20, 80 or 200 ms can be felt.
//!
//! What crosses, server to client, per frame: the quads as runs of the
//! last frame's — copied in place, or moved by one offset, which is what
//! a scroll is — and only the quads in neither; the clip table when it
//! changed; the atlas as the rects that changed; fragment sources and
//! texture pixels the first time a handle is met; the ground colour, the
//! pointer shape and the scroll containers' geometry; all of it deflated.
//! Client to server: the window's size and scale, the pointer, buttons,
//! wheel and keys. The window title carries the numbers: bytes a frame,
//! bytes a second, and the time from an input leaving to the frame that
//! answers it arriving.
//!
//! One thing is predicted: a wheel moves the quads clipped to the
//! scroller under it at once, by the offsets the server has not answered
//! yet, and the answer replaces the guess. What the move uncovers is
//! there to show because the server's core paints 300 px past every clip
//! (`Core::set_overscan`, `--overscan PX`): quads the clip hides until
//! the client moves them under it. Scrolled further ahead than that, the
//! rest is bare until the answer lands.
//!
//! What does not cross: the clipboard, IME, the access tree, menus, file
//! dialogs, audio, other windows. Each is a platform service the runner
//! performs beside the core, and each would be one more message pair.
//! Both ends must be this build: a quad travels as its bytes.
//!
//! Run: cargo run -p kui-native --example wire [-- --latency 80] [--stats]
//!      F1 / F2 in the window: 10 ms less / more, each way.
//!      F3: predicted scrolling off / on (`--no-predict` starts it off).
//!      --diff splice|ops, --zip 0..10, --overscan PX: how the server
//!      spends its bytes.
//!      --script: the client drives itself and prints what each gesture
//!      cost.
//! Two processes (or two machines):
//!      cargo run -p kui-native --example wire -- --serve 0.0.0.0:7878
//!      cargo run -p kui-native --example wire -- --connect HOST:7878

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use kui_native::atlas::GlyphAtlas;
use kui_native::display::{TextureDraw, TexturePixels};
use kui_native::resources::{FragmentId, ImageId};
use kui_native::widgets;
use kui_native::{
    Align, App, Clip, Color, Core, CursorShape, DisplayList, FragmentDraw, FragmentImage,
    InputEvent, KeyCode, KeyMods, KeyPress, Message, MouseButton, NodeSpec, Quad, QuadKind, Rect,
    Renderer, Size, TextStyle, Ui, UiEvent, Vec2,
};

// ---------------------------------------------------------------- the app

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Inc,
    Dec,
    Animate,
    Volume,
    Pick { i: u64 },
}

/// What the server runs: enough controls to feel the link with — a hover
/// and a click, a drag, typing, a scroll, and a box that moves by itself.
struct Demo {
    count: i64,
    volume: f32,
    animate: bool,
    picked: Option<u64>,
    frames: u64,
    started: Instant,
}

impl App for Demo {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        self.frames += 1;
        ui.with(
            NodeSpec::column().fill().pad(24.0).gap(14.0).bg(t.bg),
            |ui| {
                ui.text("kui over the wire", TextStyle::new(20.0));
                ui.text(
                    &format!(
                        "laid out, shaped and rasterized by pid {} · frame {}",
                        std::process::id(),
                        self.frames
                    ),
                    TextStyle::new(12.0).color(t.muted),
                );
                ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                    widgets::button(ui, "-1", Msg::Dec);
                    widgets::button(ui, "+1", Msg::Inc);
                    ui.text(&self.count.to_string(), TextStyle::new(24.0));
                    widgets::slider(ui, "Volume", self.volume, 0.0, 100.0, 1.0, Msg::Volume);
                    ui.text(
                        &format!("{:.0}", self.volume),
                        TextStyle::new(13.0).color(t.muted),
                    );
                    widgets::switch(ui, "Animate", self.animate, Msg::Animate);
                });
                widgets::text_input(ui, "Type here", "");
                // The server's own clock moves this: a stream of frames
                // nobody's input asked for.
                if self.animate {
                    let track = (ui.viewport().w - 48.0 - 24.0).max(0.0);
                    let phase = (self.started.elapsed().as_secs_f32() * 0.5).fract();
                    let x = track * (1.0 - (2.0 * phase - 1.0).abs());
                    ui.with(NodeSpec::row().height(24.0), |ui| {
                        ui.leaf(NodeSpec::row().width(x).height(1.0));
                        ui.leaf(
                            NodeSpec::row()
                                .width(24.0)
                                .height(24.0)
                                .radius(12.0)
                                .bg(t.accent)
                                .animate(),
                        );
                    });
                }
                ui.with_keyed(
                    "rows",
                    NodeSpec::column()
                        .grow_width()
                        .grow_height()
                        .scroll_y()
                        .bg(t.surface)
                        .border(1.0, t.border)
                        .radius(8.0),
                    |ui| {
                        for i in 0..300u64 {
                            let on = self.picked == Some(i);
                            ui.text_in_indexed(
                                i,
                                NodeSpec::row()
                                    .grow_width()
                                    .pad_xy(12.0, 6.0)
                                    .bg(if on { t.accent_soft } else { t.surface })
                                    .hover_bg(t.raised)
                                    .on_click(Msg::Pick { i }),
                                &format!("row {i} — hover, click, scroll"),
                                TextStyle::new(13.0),
                            );
                        }
                    },
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Inc) => self.count += 1,
            Some(Msg::Dec) => self.count -= 1,
            Some(Msg::Animate) => self.animate = !self.animate,
            Some(Msg::Volume) => {
                if let Some(v) = ev.payload.get_float("value") {
                    self.volume = v as f32;
                }
            }
            Some(Msg::Pick { i }) => self.picked = Some(i),
            None => {}
        }
    }
}

// --------------------------------------------------------------- the wire

/// A message is `[len u32][payload]`, little-endian throughout.
fn write_msg(s: &mut TcpStream, payload: &[u8]) -> std::io::Result<()> {
    s.write_all(&(payload.len() as u32).to_le_bytes())?;
    s.write_all(payload)
}

fn read_msg(s: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    s.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > 256 << 20 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut buf = vec![0; len];
    s.read_exact(&mut buf)?;
    Ok(buf)
}

#[derive(Default)]
struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32x4(&mut self, v: [u32; 4]) {
        for v in v {
            self.u32(v);
        }
    }
    fn bytes(&mut self, b: &[u8]) {
        self.u32(b.len() as u32);
        self.0.extend_from_slice(b);
    }
}

struct R<'a>(&'a [u8]);

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (head, rest) = self.0.split_at_checked(n)?;
        self.0 = rest;
        Some(head)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_bits(self.u32()?))
    }
    fn bytes(&mut self) -> Option<&'a [u8]> {
        let n = self.u32()? as usize;
        self.take(n)
    }
    fn str(&mut self) -> Option<&'a str> {
        std::str::from_utf8(self.bytes()?).ok()
    }
    fn u32x4(&mut self) -> Option<[u32; 4]> {
        Some([self.u32()?, self.u32()?, self.u32()?, self.u32()?])
    }
}

const QUAD: usize = std::mem::size_of::<Quad>();
const CLIP: usize = std::mem::size_of::<Clip>();

/// The quads as their bytes. `Quad` is `repr(C)` and all four-byte
/// fields, so there is no padding to leak; the two ends being one build
/// on one byte order is the experiment's standing assumption.
fn quad_bytes(quads: &[Quad]) -> &[u8] {
    // SAFETY: `Quad` is `Copy`, `repr(C)`, without padding; any initialized
    // value is readable as bytes.
    unsafe { std::slice::from_raw_parts(quads.as_ptr().cast(), std::mem::size_of_val(quads)) }
}

fn clip_bytes(clips: &[Clip]) -> &[u8] {
    // SAFETY: as `quad_bytes`; a `Clip` is eight `f32`s.
    unsafe { std::slice::from_raw_parts(clips.as_ptr().cast(), std::mem::size_of_val(clips)) }
}

/// Bytes back to quads, refusing a `kind` that is not one: the only field
/// of a quad some bit pattern is invalid for.
fn quads_from(bytes: &[u8], out: &mut Vec<Quad>) -> Option<()> {
    let kind_at = std::mem::offset_of!(Quad, kind);
    for chunk in bytes.as_chunks::<QUAD>().0 {
        let kind = u32::from_ne_bytes(chunk[kind_at..kind_at + 4].try_into().ok()?);
        if kind as usize >= QuadKind::ALL.len() {
            return None;
        }
        // SAFETY: `chunk` is `size_of::<Quad>()` bytes, every field but
        // `kind` is valid for any bits, and `kind` was just checked.
        out.push(unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast::<Quad>()) });
    }
    Some(())
}

// Client → server, each after a `u32` sequence number the frame that
// answers it acknowledges.
const C_RESIZE: u8 = 0;
const C_CURSOR: u8 = 1;
const C_CURSOR_LEFT: u8 = 2;
const C_MOUSE_DOWN: u8 = 3;
const C_MOUSE_UP: u8 = 4;
const C_SCROLL: u8 = 5;
const C_KEY: u8 = 6;
const C_MODIFIERS: u8 = 7;

// ------------------------------------------------------------- the server

/// How the server spends its bytes, so one build can be measured each way.
#[derive(Clone, Copy)]
struct Opts {
    /// `--diff ops` (the default): quads as runs copied out of the last
    /// frame, moved or not. `--diff splice`: only an unchanged head and
    /// tail are spared.
    ops: bool,
    /// `--zip N`: deflate a frame at level N; 0 sends it as it is.
    zip: u8,
    /// `--overscan PX`: how far past a clip the core still paints
    /// (`Core::set_overscan`), which is what a predicted scroll has to
    /// show before the answer lands. 0 leaves it bare.
    overscan: f32,
}

/// What the server believes the client holds, so a frame carries only
/// what is new to it.
struct Link {
    /// The atlas page as last sent.
    shadow: Vec<u8>,
    epoch: u64,
    /// The last frame's quads and clips, as bytes.
    quads: Vec<u8>,
    clips: Vec<u8>,
    fragments: HashSet<u64>,
    /// Texture handle → the revision sent.
    textures: HashMap<u64, u32>,
}

/// The atlas as the rects that differ from what the client has: rows that
/// changed, runs of them joined, each run cut to the columns it touched.
fn encode_atlas(w: &mut W, atlas: &mut GlyphAtlas, link: &mut Link) {
    let reset = link.shadow.len() != atlas.pixels.len();
    if reset {
        link.shadow = vec![0; atlas.pixels.len()];
    }
    w.u32(atlas.size);
    w.u8(reset as u8);
    let count_at = w.0.len();
    w.u32(0);
    if !(reset || atlas.dirty || atlas.epoch != link.epoch) {
        return;
    }
    let size = atlas.size as usize;
    let stride = size * 4;
    let mut count = 0u32;
    let mut open: Option<(usize, usize, usize)> = None;
    for y in 0..=size {
        let span = (y < size)
            .then(|| {
                let new = &atlas.pixels[y * stride..(y + 1) * stride];
                let old = &link.shadow[y * stride..(y + 1) * stride];
                if new == old {
                    return None;
                }
                let differs = |(a, b): (&[u8; 4], &[u8; 4])| a != b;
                let mut px = new.as_chunks::<4>().0.iter().zip(old.as_chunks::<4>().0);
                let x0 = px.clone().position(differs)?;
                let x1 = px.rposition(differs)? + 1;
                Some((x0, x1))
            })
            .flatten();
        match (span, open.as_mut()) {
            (Some((x0, x1)), Some(run)) => {
                run.1 = run.1.min(x0);
                run.2 = run.2.max(x1);
            }
            (Some((x0, x1)), None) => open = Some((y, x0, x1)),
            (None, Some(&mut (y0, x0, x1))) => {
                open = None;
                count += 1;
                for v in [x0, y0, x1 - x0, y - y0] {
                    w.u32(v as u32);
                }
                for row in y0..y {
                    let at = row * stride + x0 * 4;
                    w.0.extend_from_slice(&atlas.pixels[at..at + (x1 - x0) * 4]);
                }
            }
            (None, None) => {}
        }
    }
    w.0[count_at..count_at + 4].copy_from_slice(&count.to_le_bytes());
    link.shadow.copy_from_slice(&atlas.pixels);
    link.epoch = atlas.epoch;
    atlas.dirty = false;
}

/// FNV-1a over a frame's quads.
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

// How a frame's quads are said: runs, until the count is reached.
const Q_LITERAL: u8 = 0;
const Q_COPY: u8 = 1;
const Q_MOVED: u8 = 2;

// A moved copy rewrites the first two floats of a quad.
const _: () = assert!(std::mem::offset_of!(Quad, rect) == 0);

fn quad_xy(q: &[u8; QUAD]) -> (f32, f32) {
    let f = |at: usize| f32::from_ne_bytes([q[at], q[at + 1], q[at + 2], q[at + 3]]);
    (f(0), f(4))
}

/// How many quads from `now[i]` on are `old[j]` on, moved by one offset:
/// the same bytes past the position, and a position the offset takes the
/// old one to exactly — the client adds the same two floats, so a run is
/// only one if the sum comes out to the bit.
fn run(now: &[[u8; QUAD]], old: &[[u8; QUAD]], i: usize, j: usize) -> (usize, f32, f32) {
    let ((nx, ny), (ox, oy)) = (quad_xy(&now[i]), quad_xy(&old[j]));
    let (dx, dy) = (nx - ox, ny - oy);
    let len = now[i..]
        .iter()
        .zip(&old[j..])
        .take_while(|(n, o)| {
            let ((nx, ny), (ox, oy)) = (quad_xy(n), quad_xy(o));
            n[8..] == o[8..]
                && (ox + dx).to_bits() == nx.to_bits()
                && (oy + dy).to_bits() == ny.to_bits()
        })
        .count();
    (len, dx, dy)
}

/// The quads against the last frame's. A frame is mostly the last one: a
/// hover recolours a quad, a digit changes a glyph, and a scroll moves
/// every row by one offset and brings a row in at an edge — so it is said
/// as runs of the old frame, in place or moved, and only what is in
/// neither is sent.
fn encode_quads(w: &mut W, new: &[u8], old: &[u8], ops: bool) {
    let (now, _) = new.as_chunks::<QUAD>();
    let (old, _) = old.as_chunks::<QUAD>();
    w.u32(now.len() as u32);
    let literal = |w: &mut W, from: usize, to: usize| {
        if to > from {
            w.u8(Q_LITERAL);
            w.u32((to - from) as u32);
            w.0.extend_from_slice(now[from..to].as_flattened());
        }
    };
    let copy = |w: &mut W, src: usize, len: usize, dx: f32, dy: f32| {
        let moved = dx != 0.0 || dy != 0.0;
        w.u8(if moved { Q_MOVED } else { Q_COPY });
        w.u32(src as u32);
        w.u32(len as u32);
        if moved {
            w.f32(dx);
            w.f32(dy);
        }
    };
    if !ops {
        let head = now.iter().zip(old).take_while(|(a, b)| a == b).count();
        let rest = now.len().min(old.len()) - head;
        let tail = now
            .iter()
            .rev()
            .zip(old.iter().rev())
            .take(rest)
            .take_while(|(a, b)| a == b)
            .count();
        if head > 0 {
            copy(w, 0, head, 0.0, 0.0);
        }
        literal(w, head, now.len() - tail);
        if tail > 0 {
            copy(w, old.len() - tail, tail, 0.0, 0.0);
        }
        return;
    }
    // Where each quad, less its position, was in the old frame.
    let mut index: HashMap<&[u8], Vec<u32>> = HashMap::new();
    for (j, q) in old.iter().enumerate() {
        index.entry(&q[8..]).or_default().push(j as u32);
    }
    let (mut i, mut pending, mut expect) = (0, 0, 0);
    while i < now.len() {
        // Where the last run left off first: a frame that changed one
        // quad in place goes on from the quad after it.
        let mut best = (0, 0.0, 0.0);
        let mut at = expect;
        if expect < old.len() {
            best = run(now, old, i, expect);
        }
        if best.0 == 0
            && let Some(places) = index.get(&now[i][8..])
        {
            for &j in places.iter().take(8) {
                let found = run(now, old, i, j as usize);
                if found.0 > best.0 {
                    (best, at) = (found, j as usize);
                }
            }
        }
        let (len, dx, dy) = best;
        if len == 0 {
            i += 1;
            expect += 1;
            continue;
        }
        literal(w, pending, i);
        copy(w, at, len, dx, dy);
        i += len;
        pending = i;
        expect = at + len;
    }
    literal(w, pending, now.len());
}

/// One frame of the core's output as a message.
fn encode_frame(
    core: &mut Core,
    link: &mut Link,
    opts: Opts,
    frame: u32,
    ack: u32,
    build_us: u32,
) -> Vec<u8> {
    let ground = core.theme().bg;
    let cursor = core.cursor_shape();
    // What the client predicts a wheel with: each container that has
    // somewhere to scroll — where it is, where it is scrolled to, how far
    // it can go. Read off the node snapshot, in tree order, so an inner
    // one comes after the one around it.
    let scrollers: Vec<_> = core
        .nodes()
        .iter()
        .filter_map(|n| core.scroll_geometry(n.key))
        .filter(|g| g.max_offset.x > 0.0 || g.max_offset.y > 0.0)
        .collect();
    let (dl, atlas) = core.output();
    let mut w = W::default();
    w.u32(frame);
    w.u32(ack);
    w.u32(build_us);
    for v in [dl.viewport.w, dl.viewport.h, dl.scale, dl.time] {
        w.f32(v);
    }
    for v in [ground.r, ground.g, ground.b, ground.a] {
        w.f32(v);
    }
    w.u8(CursorShape::ALL
        .iter()
        .position(|s| *s == cursor)
        .unwrap_or(0) as u8);
    w.u32(scrollers.len() as u32);
    for g in &scrollers {
        let (r, o, m) = (g.rect, g.offset, g.max_offset);
        for v in [r.x, r.y, r.w, r.h, o.x, o.y, m.x, m.y] {
            w.f32(v);
        }
    }

    encode_atlas(&mut w, atlas, link);

    // The clip table, when it is not the last frame's.
    let clips = clip_bytes(&dl.clips);
    let moved = clips != link.clips;
    w.u8(moved as u8);
    if moved {
        w.bytes(clips);
        link.clips.clear();
        link.clips.extend_from_slice(clips);
    }

    let new = quad_bytes(&dl.quads);
    encode_quads(&mut w, new, &link.quads, opts.ops);
    // What the runs must come to: a frame rebuilt wrong is refused, not
    // drawn.
    w.u64(checksum(new));
    link.quads.clear();
    link.quads.extend_from_slice(new);

    w.u32(dl.fragments.len() as u32);
    for (draw, source) in dl.fragments.iter().zip(&dl.fragment_sources) {
        let id = draw.id.to_ffi();
        w.u64(id);
        for p in draw.params {
            w.f32(p);
        }
        match draw.image {
            FragmentImage::None => w.u8(0),
            FragmentImage::Atlas(uv) => {
                w.u8(1);
                w.u32x4(uv);
            }
            FragmentImage::Texture { index, uv } => {
                w.u8(2);
                w.u32(index);
                w.u32x4(uv);
            }
        }
        let first = link.fragments.insert(id);
        w.u8(first as u8);
        if first {
            w.bytes(source.as_bytes());
        }
    }

    w.u32(dl.textures.len() as u32);
    for (draw, px) in dl.textures.iter().zip(&dl.texture_pixels) {
        let id = draw.id.to_ffi();
        w.u64(id);
        w.u32x4(draw.uv);
        w.u32(px.width);
        w.u32(px.height);
        w.u32(px.rev);
        let moved = link.textures.insert(id, px.rev) != Some(px.rev);
        w.u8(moved as u8);
        if moved {
            w.bytes(&px.rgba);
        }
    }

    w.u32(dl.dropped_textures.len() as u32);
    for id in &dl.dropped_textures {
        link.textures.remove(&id.to_ffi());
        w.u64(id.to_ffi());
    }
    w.u32(dl.dropped_fragments.len() as u32);
    for id in &dl.dropped_fragments {
        link.fragments.remove(&id.to_ffi());
        w.u64(id.to_ffi());
    }
    // Deflated or not, behind a byte that says which.
    let mut msg = vec![(opts.zip > 0) as u8];
    if opts.zip > 0 {
        msg.extend(miniz_oxide::deflate::compress_to_vec(&w.0, opts.zip));
    } else {
        msg.extend(w.0);
    }
    msg
}

/// One client message into the core; `None` for one that does not parse.
/// A resize is the server's own to keep, so it comes back in `view`.
fn apply(core: &mut Core, msg: &[u8], view: &mut Option<(Size, f32)>) -> Option<Vec<UiEvent>> {
    let mut r = R(msg);
    let ev = match r.u8()? {
        C_RESIZE => {
            *view = Some((Size::new(r.f32()?, r.f32()?), r.f32()?));
            return Some(Vec::new());
        }
        C_CURSOR => InputEvent::CursorMoved(Vec2::new(r.f32()?, r.f32()?)),
        C_CURSOR_LEFT => InputEvent::CursorLeft,
        C_MOUSE_DOWN => InputEvent::MouseDown {
            button: MouseButton::from_code(r.u32()?),
            clicks: r.u8()?,
        },
        C_MOUSE_UP => InputEvent::MouseUp {
            button: MouseButton::from_code(r.u32()?),
        },
        C_SCROLL => InputEvent::ScrollGesture {
            delta: Vec2::new(r.f32()?, r.f32()?),
            begins: r.u8()? != 0,
        },
        C_KEY => {
            let (down, repeat) = (r.u8()? != 0, r.u8()? != 0);
            let mods = KeyMods::from_bits(r.u32()?);
            let mut key = KeyPress::new(KeyCode::from_name(r.str()?)?, mods);
            key.repeat = repeat;
            let text = r.str()?;
            if !text.is_empty() {
                key = key.with_text(text);
            }
            // Both channels, as the runner sends them: the press to a
            // sink, then what the key means to an editor or the ring.
            return Some(if down {
                core.press(key)
            } else {
                core.release(key)
            });
        }
        C_MODIFIERS => InputEvent::Modifiers(KeyMods::from_bits(r.u32()?)),
        _ => return None,
    };
    Some(core.handle_input(ev))
}

/// At most this often: the server has no vsync to wait on.
const TICK: Duration = Duration::from_micros(16_667);

/// Serves one client until it goes: inputs in, frames out. The app and
/// the core outlive it, so the next client finds the model as it was.
fn serve_one(
    mut stream: TcpStream,
    app: &mut impl App,
    core: &mut Core,
    opts: Opts,
    epoch: Instant,
) {
    stream.set_nodelay(true).ok();
    let (tx, rx) = mpsc::channel::<Option<Vec<u8>>>();
    let mut reading = stream.try_clone().expect("clone the stream");
    std::thread::spawn(move || {
        loop {
            let msg = read_msg(&mut reading).ok();
            let gone = msg.is_none();
            if tx.send(msg).is_err() || gone {
                return;
            }
        }
    });
    let mut link = Link {
        shadow: Vec::new(),
        epoch: u64::MAX,
        quads: Vec::new(),
        clips: Vec::new(),
        fragments: HashSet::new(),
        textures: HashMap::new(),
    };
    let mut view: Option<(Size, f32)> = None;
    let (mut ack, mut frame, mut owed) = (0u32, 0u32, false);
    let mut last = Instant::now() - TICK;
    loop {
        // Parked until the client says something, unless a frame is owed.
        let first = if owed {
            match rx.recv_timeout(TICK.saturating_sub(last.elapsed())) {
                Ok(msg) => Some(msg),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match rx.recv() {
                Ok(msg) => Some(msg),
                Err(_) => return,
            }
        };
        let mut queued: Vec<_> = first.into_iter().collect();
        queued.extend(rx.try_iter());
        for msg in queued {
            let Some(msg) = msg else { return };
            let mut r = R(&msg);
            let Some(seq) = r.u32() else { return };
            let Some(events) = apply(core, r.0, &mut view) else {
                eprintln!("wire: a message that does not parse; dropping the client");
                return;
            };
            for ev in events {
                app.on_event_with(ev, core);
            }
            ack = seq;
            owed = true;
        }
        let Some((viewport, scale)) = view else {
            continue;
        };
        if !owed || last.elapsed() < TICK {
            continue;
        }
        last = Instant::now();
        core.set_time(epoch.elapsed().as_secs_f64());
        let mut ui = core.frame(viewport, scale);
        app.view(&mut ui);
        ui.finish();
        let pending = core.take_pending_events();
        owed = !pending.is_empty();
        for ev in pending {
            app.on_event_with(ev, core);
        }
        owed |= core.animating();
        frame += 1;
        let build_us = last.elapsed().as_micros() as u32;
        let msg = encode_frame(core, &mut link, opts, frame, ack, build_us);
        if write_msg(&mut stream, &msg).is_err() {
            return;
        }
    }
}

fn serve(listener: TcpListener, opts: Opts) {
    let mut app = Demo {
        count: 0,
        volume: 40.0,
        animate: true,
        picked: None,
        frames: 0,
        started: Instant::now(),
    };
    let mut core = Core::new();
    // The node snapshot, which `encode_frame` finds the scrollers in.
    core.set_inspect(true);
    core.set_overscan(opts.overscan);
    let epoch = Instant::now();
    for stream in listener.incoming().flatten() {
        eprintln!("wire: client {:?}", stream.peer_addr().ok());
        serve_one(stream, &mut app, &mut core, opts, epoch);
        eprintln!("wire: client gone");
    }
}

// ------------------------------------------------------------- the client

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{CursorIcon, Window};

enum Wire {
    Frame(Vec<u8>),
    Closed,
    Step(Step),
}

/// One move of `--script`: the client driving itself through the same
/// sends a hand would cause, so a change to the wire can be measured on
/// the same gestures twice.
enum Step {
    Move(f32, f32),
    Press,
    Release,
    Wheel(f32),
    Type(char),
    /// Ends a phase: prints what it cost under this name, or nothing for
    /// a phase that was only getting into place.
    Mark(Option<&'static str>),
    Quit,
}

/// The gestures, timed; `settle` is long enough for the last answer to
/// have come back. Positions are the demo's at the window's first size.
fn script(send: impl Fn(Step), settle: Duration) {
    let wait = |ms: u64| std::thread::sleep(Duration::from_millis(ms));
    let click = |x: f32, y: f32| {
        send(Step::Move(x, y));
        send(Step::Press);
        wait(30);
        send(Step::Release);
        wait(70);
    };
    let mark = |name| {
        std::thread::sleep(settle);
        send(Step::Mark(name));
    };
    wait(1500);
    mark(None);
    wait(2000);
    mark(Some("animating, no input"));
    click(413.0, 113.0);
    mark(None);
    for i in 0..90 {
        send(Step::Move(300.0, 262.0 + i as f32 * 3.0));
        wait(16);
    }
    mark(Some("hover down the rows"));
    for _ in 0..120 {
        send(Step::Wheel(-12.0));
        wait(16);
    }
    mark(Some("scroll, 12 px a frame"));
    for _ in 0..10 {
        click(100.0, 113.0);
    }
    mark(Some("ten clicks on +1"));
    click(300.0, 163.0);
    mark(None);
    for c in "the quick brown fox jumps over".chars() {
        send(Step::Type(c));
        wait(50);
    }
    mark(Some("typing thirty characters"));
    send(Step::Quit);
}

/// The server's output as the client holds it: what a `Core::output`
/// would have handed the renderer, rebuilt from the messages.
struct Remote {
    dl: DisplayList,
    atlas: GlyphAtlas,
    sources: HashMap<u64, Arc<str>>,
    pixels: HashMap<u64, TexturePixels>,
    ground: Color,
    cursor: usize,
    ack: u32,
    build_us: u32,
    scrollers: Vec<Scroller>,
}

/// A scroll container as the server last laid it out, in logical px.
#[derive(Clone, Copy)]
struct Scroller {
    rect: Rect,
    offset: Vec2,
    max: Vec2,
}

impl Remote {
    /// Takes one frame; the size it had before it was deflated comes back.
    fn decode(&mut self, msg: &[u8]) -> Option<usize> {
        let (zipped, body) = msg.split_first()?;
        let inflated;
        let body = if *zipped != 0 {
            inflated = miniz_oxide::inflate::decompress_to_vec_with_limit(body, 256 << 20).ok()?;
            &inflated[..]
        } else {
            body
        };
        let mut r = R(body);
        let _frame = r.u32()?;
        self.ack = r.u32()?;
        self.build_us = r.u32()?;
        self.dl.viewport = Size::new(r.f32()?, r.f32()?);
        self.dl.scale = r.f32()?;
        self.dl.time = r.f32()?;
        self.ground = Color {
            r: r.f32()?,
            g: r.f32()?,
            b: r.f32()?,
            a: r.f32()?,
        };
        self.cursor = r.u8()? as usize;
        self.scrollers.clear();
        for _ in 0..r.u32()? {
            let mut f = [0.0; 8];
            for v in &mut f {
                *v = r.f32()?;
            }
            self.scrollers.push(Scroller {
                rect: Rect {
                    x: f[0],
                    y: f[1],
                    w: f[2],
                    h: f[3],
                },
                offset: Vec2::new(f[4], f[5]),
                max: Vec2::new(f[6], f[7]),
            });
        }

        let size = r.u32()?;
        let reset = r.u8()? != 0;
        if reset || size != self.atlas.size {
            let epoch = self.atlas.epoch + 1;
            self.atlas = GlyphAtlas::with_size(size);
            self.atlas.epoch = epoch;
        }
        let stride = size as usize * 4;
        for _ in 0..r.u32()? {
            let [x, y, w, h] = r.u32x4()?.map(|v| v as usize);
            if x + w > size as usize || y + h > size as usize {
                return None;
            }
            let rows = r.take(w * h * 4)?;
            for (i, row) in rows.chunks_exact(w * 4).enumerate() {
                let at = (y + i) * stride + x * 4;
                self.atlas.pixels[at..at + w * 4].copy_from_slice(row);
            }
            self.atlas.dirty = true;
        }

        let clips = if r.u8()? != 0 { r.bytes()? } else { &[] };
        if !clips.is_empty() {
            self.dl.clips.clear();
        }
        for c in clips.as_chunks::<CLIP>().0 {
            let mut f = R(c);
            self.dl.clips.push(Clip {
                rect: Rect {
                    x: f.f32()?,
                    y: f.f32()?,
                    w: f.f32()?,
                    h: f.f32()?,
                },
                radius: [f.f32()?, f.f32()?, f.f32()?, f.f32()?],
            });
        }

        let total = r.u32()? as usize;
        let old = std::mem::take(&mut self.dl.quads);
        while self.dl.quads.len() < total {
            let op = r.u8()?;
            if op == Q_LITERAL {
                let n = r.u32()? as usize;
                quads_from(r.take(n.checked_mul(QUAD)?)?, &mut self.dl.quads)?;
                if n == 0 {
                    return None;
                }
                continue;
            }
            let (src, n) = (r.u32()? as usize, r.u32()? as usize);
            let run = old
                .get(src..src.checked_add(n)?)
                .filter(|r| !r.is_empty())?;
            match op {
                Q_COPY => self.dl.quads.extend_from_slice(run),
                Q_MOVED => {
                    let (dx, dy) = (r.f32()?, r.f32()?);
                    self.dl.quads.extend(run.iter().map(|q| {
                        let mut q = *q;
                        q.rect.x += dx;
                        q.rect.y += dy;
                        q
                    }));
                }
                _ => return None,
            }
        }

        if r.u64()? != checksum(quad_bytes(&self.dl.quads)) {
            return None;
        }

        self.dl.fragments.clear();
        self.dl.fragment_sources.clear();
        for _ in 0..r.u32()? {
            let id = r.u64()?;
            let mut params = [0.0; 16];
            for p in &mut params {
                *p = r.f32()?;
            }
            let image = match r.u8()? {
                0 => FragmentImage::None,
                1 => FragmentImage::Atlas(r.u32x4()?),
                _ => FragmentImage::Texture {
                    index: r.u32()?,
                    uv: r.u32x4()?,
                },
            };
            if r.u8()? != 0 {
                self.sources.insert(id, r.str()?.into());
            }
            self.dl.fragments.push(FragmentDraw {
                id: FragmentId::from_ffi(id),
                params,
                image,
            });
            self.dl
                .fragment_sources
                .push(self.sources.get(&id)?.clone());
        }

        self.dl.textures.clear();
        self.dl.texture_pixels.clear();
        for _ in 0..r.u32()? {
            let id = r.u64()?;
            let uv = r.u32x4()?;
            let (width, height, rev) = (r.u32()?, r.u32()?, r.u32()?);
            if r.u8()? != 0 {
                let rgba = Arc::new(r.bytes()?.to_vec());
                self.pixels.insert(
                    id,
                    TexturePixels {
                        width,
                        height,
                        rev,
                        rgba,
                    },
                );
            }
            self.dl.textures.push(TextureDraw {
                id: ImageId::from_ffi(id),
                uv,
            });
            self.dl.texture_pixels.push(self.pixels.get(&id)?.clone());
        }

        // Kept until a frame is drawn: two messages can land between two
        // draws, and the renderer has to hear of every removal once.
        for _ in 0..r.u32()? {
            let id = r.u64()?;
            self.pixels.remove(&id);
            self.dl.dropped_textures.push(ImageId::from_ffi(id));
        }
        for _ in 0..r.u32()? {
            let id = r.u64()?;
            self.sources.remove(&id);
            self.dl.dropped_fragments.push(FragmentId::from_ffi(id));
        }
        Some(body.len())
    }
}

struct Client {
    /// To the delay line: a message and when it may leave.
    out: mpsc::Sender<(Instant, Vec<u8>)>,
    /// The simulated one-way latency, in ms; the delay lines read it.
    latency: Arc<AtomicU64>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    remote: Remote,
    seq: u32,
    /// Inputs not yet acknowledged, with when each was made.
    sent: VecDeque<(u32, Instant)>,
    mods: KeyMods,
    cursor: Vec2,
    last_click: Option<(Instant, Vec2, u8)>,
    last_scroll: Instant,
    /// F3, `--no-predict`: whether a wheel moves the content here at
    /// once, ahead of the server's answer.
    predict: bool,
    /// Wheel deltas the server has not answered yet: the input's number,
    /// the scroller it was over (by its rect), the delta.
    wheels: VecDeque<(u32, Rect, Vec2)>,
    /// The furthest a draw has run ahead of the server this phase, px.
    ahead: f32,
    // The title's numbers.
    answered_ms: f32,
    frames: u32,
    bytes: usize,
    since: Instant,
    /// The running phase of `--script`: frames, bytes on the wire, bytes
    /// before deflating.
    phase: (u32, usize, usize),
    /// `--stats`: the title's line to stderr as well.
    stats: bool,
    /// `KUI_SMOKE_FRAMES`: close after this many, as the runner's does.
    smoke: Option<u32>,
    drawn: u32,
}

impl Client {
    fn send(&mut self, tag: u8, body: impl FnOnce(&mut W)) {
        self.seq += 1;
        let mut w = W::default();
        w.u32(self.seq);
        w.u8(tag);
        body(&mut w);
        let now = Instant::now();
        self.sent.push_back((self.seq, now));
        let due = now + Duration::from_millis(self.latency.load(Ordering::Relaxed));
        self.out.send((due, w.0)).ok();
    }

    fn send_size(&mut self) {
        let Some(window) = &self.window else { return };
        let scale = window.scale_factor() as f32;
        let px = window.inner_size();
        self.send(C_RESIZE, |w| {
            w.f32(px.width as f32 / scale);
            w.f32(px.height as f32 / scale);
            w.f32(scale);
        });
    }

    /// A wheel delta, in logical px: to the server, and — predicting —
    /// kept until the frame that answers it, so the draws in between can
    /// move what is already here.
    fn wheel(&mut self, d: Vec2, begins: bool) {
        self.send(C_SCROLL, |w| {
            w.f32(d.x);
            w.f32(d.y);
            w.u8(begins as u8);
        });
        let over = self.remote.scrollers.iter().rev().find(|s| {
            s.rect.contains(self.cursor)
                && ((d.x != 0.0 && s.max.x > 0.0) || (d.y != 0.0 && s.max.y > 0.0))
        });
        if self.predict
            && let Some(s) = over
        {
            self.wheels.push_back((self.seq, s.rect, d));
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    /// How far ahead of the server each scroller is drawn: its rect in
    /// physical px and the shift its content takes. The server's offset
    /// less every wheel it has not heard, clamped as its layout will
    /// clamp it, is where the content is about to be.
    fn shifts(&self) -> Vec<(Rect, Vec2)> {
        let scale = self.remote.dl.scale;
        let mut out = Vec::new();
        for s in &self.remote.scrollers {
            let mut to = s.offset;
            for (_, rect, d) in &self.wheels {
                if *rect == s.rect {
                    to.x = (to.x - d.x).clamp(0.0, s.max.x);
                    to.y = (to.y - d.y).clamp(0.0, s.max.y);
                }
            }
            let shift = Vec2::new((s.offset.x - to.x) * scale, (s.offset.y - to.y) * scale);
            if shift != Vec2::ZERO {
                out.push((s.rect.scaled(scale), shift));
            }
        }
        out
    }

    fn on_frame(&mut self, msg: &[u8]) -> Option<()> {
        let raw = self.remote.decode(msg)?;
        self.phase = (
            self.phase.0 + 1,
            self.phase.1 + msg.len(),
            self.phase.2 + raw,
        );
        let mut answered = None;
        while let Some(&(seq, at)) = self.sent.front() {
            if seq > self.remote.ack {
                break;
            }
            answered = Some(at);
            self.sent.pop_front();
        }
        if let Some(at) = answered {
            self.answered_ms = at.elapsed().as_secs_f32() * 1e3;
        }
        // The frame has these wheels in it, or has no such scroller.
        let (ack, scrollers) = (self.remote.ack, &self.remote.scrollers);
        self.wheels
            .retain(|(seq, rect, _)| *seq > ack && scrollers.iter().any(|s| s.rect == *rect));
        self.frames += 1;
        self.bytes += msg.len();
        let window = self.window.as_ref()?;
        window.set_cursor(match self.remote.cursor {
            1 => CursorIcon::Text,
            2 => CursorIcon::Pointer,
            3 => CursorIcon::Grab,
            4 => CursorIcon::Grabbing,
            5 => CursorIcon::NotAllowed,
            6 => CursorIcon::EwResize,
            7 => CursorIcon::NsResize,
            8 => CursorIcon::NwseResize,
            9 => CursorIcon::NeswResize,
            _ => CursorIcon::Default,
        });
        let span = self.since.elapsed().as_secs_f32();
        if span >= 0.5 {
            let line = format!(
                "kui over the wire — {} ms each way · input→frame {:.0} ms · server {:.1} ms · \
                 {:.1} kB/frame · {:.0} kB/s · {:.0} fps",
                self.latency.load(Ordering::Relaxed),
                self.answered_ms,
                self.remote.build_us as f32 / 1e3,
                self.bytes as f32 / self.frames as f32 / 1e3,
                self.bytes as f32 / span / 1e3,
                self.frames as f32 / span,
            );
            if self.stats {
                eprintln!("{line}");
            }
            window.set_title(&line);
            (self.frames, self.bytes, self.since) = (0, 0, Instant::now());
        }
        window.request_redraw();
        Some(())
    }

    fn draw(&mut self) {
        let shifts = self.shifts();
        let (Some(window), Some(renderer)) = (&self.window, &mut self.renderer) else {
            return;
        };
        let px = window.inner_size();
        let remote = &mut self.remote;
        // Predicted scrolling: every quad clipped to a scroller that is
        // ahead of the server moves by how far ahead it is, for this draw
        // only. What the move brings into view is the server's overscan;
        // past that it is bare until the answer lands.
        let settled = (!shifts.is_empty()).then(|| remote.dl.quads.clone());
        if settled.is_some() {
            let dl = &mut remote.dl;
            for q in &mut dl.quads {
                let Some(c) = dl.clips.get(q.clip as usize).map(|c| c.rect) else {
                    continue;
                };
                let inside = |r: &Rect| {
                    c.x >= r.x - 1.0
                        && c.y >= r.y - 1.0
                        && c.x + c.w <= r.x + r.w + 1.0
                        && c.y + c.h <= r.y + r.h + 1.0
                };
                if q.kind != QuadKind::Segment
                    && let Some((_, by)) = shifts.iter().rev().find(|(r, _)| inside(r))
                {
                    q.rect.x += by.x;
                    q.rect.y += by.y;
                    self.ahead = self.ahead.max(by.x.abs().max(by.y.abs()));
                }
            }
        }
        // Drawn at the window's own size whatever size the frame was laid
        // out for: a resize the server has not answered yet shows the old
        // layout unscaled, which is what the latency looks like.
        remote.dl.viewport = Size::new(px.width as f32, px.height as f32);
        let g = remote.ground;
        renderer.clear_color = kui_native::wgpu::Color {
            r: g.r as f64,
            g: g.g as f64,
            b: g.b as f64,
            a: g.a as f64,
        };
        let drawn = renderer.render(&remote.dl, &mut remote.atlas);
        if let Some(settled) = settled {
            remote.dl.quads = settled;
        }
        match drawn {
            Ok(_) => {
                remote.dl.dropped_textures.clear();
                remote.dl.dropped_fragments.clear();
                self.drawn += 1;
            }
            Err(kui_native::RenderError::Skip) => {}
            Err(_) => {
                renderer.resize(px.width, px.height);
                window.request_redraw();
            }
        }
    }
}

fn key_code(key: &WinitKey) -> KeyCode {
    match key {
        WinitKey::Character(s) => s.chars().next().map_or(KeyCode::Unknown, KeyCode::Char),
        WinitKey::Named(n) => match n {
            NamedKey::Space => KeyCode::Space,
            NamedKey::ArrowLeft => KeyCode::Left,
            NamedKey::ArrowRight => KeyCode::Right,
            NamedKey::ArrowUp => KeyCode::Up,
            NamedKey::ArrowDown => KeyCode::Down,
            NamedKey::Home => KeyCode::Home,
            NamedKey::End => KeyCode::End,
            NamedKey::PageUp => KeyCode::PageUp,
            NamedKey::PageDown => KeyCode::PageDown,
            NamedKey::Backspace => KeyCode::Backspace,
            NamedKey::Delete => KeyCode::Delete,
            NamedKey::Enter => KeyCode::Enter,
            NamedKey::Tab => KeyCode::Tab,
            NamedKey::Escape => KeyCode::Escape,
            _ => KeyCode::Unknown,
        },
        _ => KeyCode::Unknown,
    }
}

impl ApplicationHandler<Wire> for Client {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("kui over the wire")
            .with_inner_size(winit::dpi::LogicalSize::new(720.0, 560.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("a window"));
        let px = window.inner_size();
        let renderer = pollster::block_on(Renderer::new(window.clone(), px.width, px.height))
            .expect("a renderer");
        self.window = Some(window);
        self.renderer = Some(renderer);
        self.send_size();
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Wire) {
        match event {
            Wire::Frame(msg) => {
                if self.on_frame(&msg).is_none() && self.window.is_some() {
                    eprintln!("wire: a frame that does not parse");
                    event_loop.exit();
                }
            }
            Wire::Closed => {
                eprintln!("wire: the server went away");
                event_loop.exit();
            }
            Wire::Step(step) => match step {
                Step::Move(x, y) => {
                    self.cursor = Vec2::new(x, y);
                    self.send(C_CURSOR, |w| {
                        w.f32(x);
                        w.f32(y);
                    });
                }
                Step::Press => self.send(C_MOUSE_DOWN, |w| {
                    w.u32(MouseButton::Primary.code());
                    w.u8(1);
                }),
                Step::Release => self.send(C_MOUSE_UP, |w| w.u32(MouseButton::Primary.code())),
                Step::Wheel(dy) => self.wheel(Vec2::new(0.0, dy), false),
                Step::Type(c) => {
                    let code = if c == ' ' {
                        KeyCode::Space
                    } else {
                        KeyCode::Char(c)
                    };
                    for down in [true, false] {
                        self.send(C_KEY, |w| {
                            w.u8(down as u8);
                            w.u8(0);
                            w.u32(0);
                            w.bytes(code.name().as_bytes());
                            w.bytes(
                                if down && c != ' ' {
                                    c.to_string()
                                } else {
                                    String::new()
                                }
                                .as_bytes(),
                            );
                        });
                    }
                }
                Step::Mark(name) => {
                    let (frames, wire, raw) = std::mem::take(&mut self.phase);
                    let ahead = std::mem::take(&mut self.ahead);
                    if let Some(name) = name {
                        let per = |n: usize| n as f32 / frames.max(1) as f32;
                        eprintln!(
                            "{name:26} {frames:4} frames  {:8.0} B/frame on the wire  {:8.0} before deflate  {:7.1} kB in all  {:4.0} px ahead",
                            per(wire),
                            per(raw),
                            wire as f32 / 1e3,
                            ahead,
                        );
                    }
                }
                Step::Quit => event_loop.exit(),
            },
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |w| w.scale_factor() as f32);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                self.draw();
                if self.smoke.is_some_and(|n| self.drawn >= n) {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(px) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(px.width, px.height);
                }
                self.send_size();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => self.send_size(),
            WindowEvent::CursorMoved { position, .. } => {
                let p = Vec2::new(position.x as f32 / scale, position.y as f32 / scale);
                self.cursor = p;
                self.send(C_CURSOR, |w| {
                    w.f32(p.x);
                    w.f32(p.y);
                });
            }
            WindowEvent::CursorLeft { .. } => self.send(C_CURSOR_LEFT, |_| {}),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Primary,
                    winit::event::MouseButton::Right => MouseButton::Secondary,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };
                if state == ElementState::Released {
                    self.send(C_MOUSE_UP, |w| w.u32(button.code()));
                    return;
                }
                // The click count is the driver's to keep: the core has
                // no clock.
                let now = Instant::now();
                let clicks = match self.last_click {
                    Some((at, p, n))
                        if button == MouseButton::Primary
                            && now - at < Duration::from_millis(400)
                            && (p.x - self.cursor.x).abs() < 4.0
                            && (p.y - self.cursor.y).abs() < 4.0 =>
                    {
                        n % 3 + 1
                    }
                    _ => 1,
                };
                self.last_click = Some((now, self.cursor, clicks));
                self.send(C_MOUSE_DOWN, |w| {
                    w.u32(button.code());
                    w.u8(clicks);
                });
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        Vec2::new(Core::lines_to_px(x), Core::lines_to_px(y))
                    }
                    MouseScrollDelta::PixelDelta(p) => {
                        Vec2::new(p.x as f32 / scale, p.y as f32 / scale)
                    }
                };
                let now = Instant::now();
                let begins = phase == TouchPhase::Started
                    || now - self.last_scroll > Duration::from_millis(200);
                self.last_scroll = now;
                if d != Vec2::ZERO {
                    self.wheel(d, begins);
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                self.mods = KeyMods {
                    shift: s.shift_key(),
                    ctrl: s.control_key(),
                    alt: s.alt_key(),
                    super_key: s.super_key(),
                };
                let bits = self.mods.bits();
                self.send(C_MODIFIERS, |w| w.u32(bits));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                // The link's own keys, which the server never hears.
                if event.logical_key == WinitKey::Named(NamedKey::F3) {
                    if down && !event.repeat {
                        self.predict = !self.predict;
                        self.wheels.clear();
                    }
                    return;
                }
                if let WinitKey::Named(n @ (NamedKey::F1 | NamedKey::F2)) = event.logical_key {
                    if down {
                        let ms = self.latency.load(Ordering::Relaxed);
                        let ms = if n == NamedKey::F1 {
                            ms.saturating_sub(10)
                        } else {
                            ms + 10
                        };
                        self.latency.store(ms, Ordering::Relaxed);
                    }
                    return;
                }
                let code = key_code(&event.logical_key);
                if code == KeyCode::Unknown {
                    return;
                }
                let mods = self.mods;
                let plain = !mods.ctrl && !mods.alt && !mods.super_key;
                let text = match (&event.logical_key, &event.text) {
                    (WinitKey::Character(_), Some(t)) if down && plain => t.to_string(),
                    _ => String::new(),
                };
                self.send(C_KEY, |w| {
                    w.u8(down as u8);
                    w.u8(event.repeat as u8);
                    w.u32(mods.bits());
                    w.bytes(code.name().as_bytes());
                    w.bytes(text.as_bytes());
                });
            }
            _ => {}
        }
    }
}

fn connect(addr: &str, latency_ms: u64) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_nodelay(true)?;
    let latency = Arc::new(AtomicU64::new(latency_ms));
    let event_loop = EventLoop::<Wire>::with_user_event().build()?;

    // Out: held until due, then written. One thread, so order is kept.
    let (out, outbox) = mpsc::channel::<(Instant, Vec<u8>)>();
    let mut writing = stream.try_clone()?;
    std::thread::spawn(move || {
        for (due, msg) in outbox {
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
            if write_msg(&mut writing, &msg).is_err() {
                return;
            }
        }
    });
    // In: stamped as it arrives by one thread, held and handed to the
    // window by another — a reader that slept would stamp the message
    // behind it late.
    let (held, holding) = mpsc::channel::<(Instant, Vec<u8>)>();
    let lag = latency.clone();
    std::thread::spawn(move || {
        while let Ok(msg) = read_msg(&mut stream) {
            let due = Instant::now() + Duration::from_millis(lag.load(Ordering::Relaxed));
            if held.send((due, msg)).is_err() {
                return;
            }
        }
    });
    let proxy = event_loop.create_proxy();
    std::thread::spawn(move || {
        for (due, msg) in holding {
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
            if proxy.send_event(Wire::Frame(msg)).is_err() {
                return;
            }
        }
        proxy.send_event(Wire::Closed).ok();
    });

    if std::env::args().any(|a| a == "--script") {
        let proxy = event_loop.create_proxy();
        let settle = Duration::from_millis(2 * latency_ms + 200);
        std::thread::spawn(move || {
            script(
                |step| {
                    proxy.send_event(Wire::Step(step)).ok();
                },
                settle,
            )
        });
    }

    let mut client = Client {
        out,
        latency,
        window: None,
        renderer: None,
        remote: Remote {
            dl: DisplayList::default(),
            atlas: GlyphAtlas::new(),
            sources: HashMap::new(),
            pixels: HashMap::new(),
            ground: Color::default(),
            cursor: 0,
            ack: 0,
            build_us: 0,
            scrollers: Vec::new(),
        },
        seq: 0,
        sent: VecDeque::new(),
        mods: KeyMods::default(),
        cursor: Vec2::ZERO,
        last_click: None,
        last_scroll: Instant::now(),
        predict: !std::env::args().any(|a| a == "--no-predict"),
        wheels: VecDeque::new(),
        ahead: 0.0,
        answered_ms: 0.0,
        frames: 0,
        bytes: 0,
        since: Instant::now(),
        phase: (0, 0, 0),
        stats: std::env::args().any(|a| a == "--stats"),
        smoke: std::env::var("KUI_SMOKE_FRAMES")
            .ok()
            .and_then(|n| n.parse().ok()),
        drawn: 0,
    };
    event_loop.run_app(&mut client)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .filter(|v| !v.starts_with("--"))
            .cloned()
    };
    let latency = value("--latency")
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    let opts = Opts {
        ops: value("--diff").is_none_or(|v| v != "splice"),
        zip: value("--zip").and_then(|v| v.parse().ok()).unwrap_or(1),
        overscan: value("--overscan")
            .and_then(|v| v.parse().ok())
            .unwrap_or(300.0),
    };
    if args.iter().any(|a| a == "--serve") {
        let addr = value("--serve").unwrap_or_else(|| "127.0.0.1:7878".into());
        let listener = TcpListener::bind(&addr)?;
        eprintln!("wire: serving on {addr}");
        serve(listener, opts);
        return Ok(());
    }
    if let Some(addr) = value("--connect") {
        return connect(&addr, latency);
    }
    // Neither: both ends in this process, still a socket apart.
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?.to_string();
    std::thread::spawn(move || serve(listener, opts));
    connect(&addr, latency)
}
