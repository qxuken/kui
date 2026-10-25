//! The `svg` element's builder half (ADR 0063): an SVG lowered onto the
//! nodes kui already has — a box holding one `path` per shape, or one
//! `image` the document was rasterized into — so it lays out, clips,
//! fades, takes input and replays like the rest of the tree, and adds no
//! node kind for a backend to learn.

use std::sync::Arc;
use std::time::SystemTime;

use rustc_hash::FxHashMap;

use super::*;
use crate::access::Role;
use crate::svg::{Svg, SvgFit, SvgMode};

/// Where an `svg` element's document comes from.
#[derive(Clone, Copy, Debug)]
pub enum SvgSource<'a> {
    /// The document itself: an `.svg` file's bytes, an icon's markup.
    Data(&'a [u8]),
    /// A file, read once and again whenever its size or modification
    /// time moves — so a picture an editor saves redraws.
    File(&'a str),
}

/// The element's rows beyond its spec: how it draws, how the picture sits
/// in the box, and what `currentColor` paints.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SvgOpts {
    pub mode: SvgMode,
    pub fit: SvgFit,
    /// What `currentColor` paints; the theme's `fg` when `None`, so an
    /// icon drawn `stroke="currentColor"` takes the ink of the text
    /// beside it.
    pub color: Option<Color>,
}

/// What the core keeps for the `svg` elements: each document parsed once
/// by content, each file's last stamp, and each raster by what it drew,
/// shared by every element that draws the same picture at the same size.
#[derive(Default)]
pub(crate) struct SvgCache {
    docs: FxHashMap<u64, Doc>,
    files: FxHashMap<String, FileSeen>,
    rasters: FxHashMap<u64, Raster>,
    /// The size each box drawn from layout came out at, and in which
    /// frame: what the next frame draws it at.
    sizes: FxHashMap<Key, (Size, u64)>,
    /// The boxes this frame drew at the size their layout had last frame,
    /// by tree index, with that size: after layout, one that came out
    /// another size asks for the frame that draws it right.
    laid: Vec<(u32, Size)>,
}

struct Doc {
    svg: Result<Arc<Svg>, String>,
    seen: u64,
}

struct FileSeen {
    stamp: Option<(SystemTime, u64)>,
    /// The content's hash; `None` while the file cannot be read.
    hash: Result<u64, String>,
    checked: u64,
}

struct Raster {
    id: crate::resources::ImageId,
    seen: u64,
}

/// More rasters than this and the ones no frame drew lately go early: a
/// live resize mints one per size.
const RASTERS_SOFT_CAP: usize = 64;
/// How recently a raster must have been drawn to survive the soft cap: a
/// departing subtree still draws for an exit's length.
const RASTER_GRACE: u64 = 30;

impl SvgCache {
    /// The stores swept on the core's by-last-use cadence, and the rasters
    /// past their soft cap on any frame; what goes is handed back for the
    /// registry to drop.
    pub(crate) fn begin_frame(
        &mut self,
        frame_no: u64,
        dropped: &mut Vec<crate::resources::ImageId>,
    ) {
        self.laid.clear();
        if let Some(cutoff) = crate::retain::sweep_cutoff(frame_no) {
            self.docs.retain(|_, d| d.seen >= cutoff);
            self.files.retain(|_, f| f.checked >= cutoff);
            self.sizes.retain(|_, (_, seen)| *seen >= cutoff);
            self.rasters.retain(|_, r| {
                let keep = r.seen >= cutoff;
                if !keep {
                    dropped.push(r.id);
                }
                keep
            });
        }
        if self.rasters.len() > RASTERS_SOFT_CAP {
            let cutoff = frame_no.saturating_sub(RASTER_GRACE);
            self.rasters.retain(|_, r| {
                let keep = r.seen >= cutoff;
                if !keep {
                    dropped.push(r.id);
                }
                keep
            });
        }
    }

    pub(crate) fn raster_count(&self) -> usize {
        self.rasters.len()
    }
}

impl Core {
    /// An SVG drawn in a box: as kui paths when every shape is one `path`
    /// draws exactly — crisp at any size and scale, `currentColor` painted
    /// `opts.color` — and as an image rasterized at the drawn size when it
    /// is not (a gradient, a mask, a filter, an embedded picture); see
    /// [`SvgMode`]. With no width or height the box is the document's own
    /// size; one axis given, the other keeps its aspect; `grow` or a
    /// percentage draws at the size the box was laid out at.
    ///
    /// The box is the spec's, read by assistive technology as an image —
    /// a clickable one as the button it derives as — `label` naming it and
    /// `role="none"` making it decoration. A document
    /// that does not parse, or a file that cannot be read, draws an empty
    /// box and raises `svg-malformed` or `svg-unreadable` once.
    pub fn svg_node(&mut self, src: SvgSource<'_>, opts: SvgOpts, spec: NodeSpec) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.auto_key();
        self.svg_with_key(key, src, opts, spec, None);
        key
    }

    /// [`Self::svg_node`] under a label key.
    pub fn svg_node_keyed(
        &mut self,
        label: &str,
        src: SvgSource<'_>,
        opts: SvgOpts,
        spec: NodeSpec,
    ) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.child_key(label);
        self.svg_with_key(key, src, opts, spec, None);
        self.note_label(key, label);
        key
    }

    pub(crate) fn svg_with_key(
        &mut self,
        key: Key,
        src: SvgSource<'_>,
        opts: SvgOpts,
        mut spec: NodeSpec,
        tooltip: Option<String>,
    ) {
        if self.tree.is_empty() {
            return;
        }
        let doc = self.svg_doc(key, src);
        let intrinsic = doc.as_ref().map_or((0.0, 0.0), |(_, d)| d.intrinsic());
        let (w, h, laid) = self.svg_size(key, &spec, intrinsic);
        if matches!(spec.layout.width, Sizing::Fit) {
            spec.layout.width = Sizing::Fixed(w);
        }
        if matches!(spec.layout.height, Sizing::Fit) {
            spec.layout.height = Sizing::Fixed(h);
        }
        // A picture to assistive technology; a clickable one is the button
        // its role derives as, as a clickable path is.
        if spec.access.as_ref().and_then(|a| a.role).is_none() && spec.events().on_click().is_none()
        {
            spec = spec.role(Role::Image);
        }
        if opts.fit == SvgFit::Cover {
            spec.layout.clip = true;
        }
        let color = opts.color.unwrap_or(self.theme.fg);
        let as_paths = doc.as_ref().is_some_and(|(_, d)| match opts.mode {
            SvgMode::Auto => d.drawable_as_paths(),
            SvgMode::Paths => true,
            SvgMode::Image => false,
        });
        // Overlapping strokes in a translucent ink would darken where
        // they cross, which one picture does not: the shapes go opaque
        // and the box's opacity, a group (ADR 0056), takes the alpha.
        let mut ink = color;
        if as_paths && color.a < 1.0 {
            spec.style.opacity *= color.a;
            ink = color.with_alpha(1.0);
        }
        let at = self.tree.len() as u32;
        self.open_with_key(key, spec);
        if self.tree.len() as u32 == at {
            return;
        }
        if let Some(hint) = tooltip {
            self.hint(key, hint);
        }
        if laid {
            // Drawn at last frame's size: a replay would keep it at that
            // size through a resize, so the fill is not kept.
            self.taint_kept("it drew an svg at its laid-out size");
            self.svgs.laid.push((at, Size::new(w, h)));
        }
        if let Some((hash, doc)) = doc
            && w > 0.0
            && h > 0.0
        {
            if as_paths {
                if opts.mode == SvgMode::Paths && !doc.approximations().is_empty() {
                    self.warn(crate::diag::svg_approximated(key, doc.approximations()));
                }
                for shape in doc.shapes(w, h, opts.fit, ink) {
                    let mut s = NodeSpec::default();
                    if let Some(fill) = shape.fill {
                        s = s.bg(fill);
                    }
                    let k = self.auto_key();
                    let p = &shape.path;
                    self.path_with_key(k, p.ops(), p.rule(), p.stroke(), None, s);
                }
            } else {
                // A raster is the registry's; a replay would draw a handle
                // a sweep may since have dropped.
                self.taint_kept("it drew an svg as an image");
                if doc.reaches_past_raster(w, h, self.scale, opts.fit) {
                    self.warn(crate::diag::svg_too_large(key));
                }
                let id = self.svg_raster(hash, &doc, w, h, opts.fit, color);
                let k = self.auto_key();
                let s = NodeSpec::default().size(w, h).role(Role::None);
                self.image_with_key(k, id, crate::resources::ImageOpts::default(), s);
            }
        }
        self.close();
    }

    /// The box's size: what the spec fixes, what last frame laid out for
    /// an axis that grows, and the document's own size — the aspect kept —
    /// for an axis left to fit. And whether any of it came from layout.
    fn svg_size(&self, key: Key, spec: &NodeSpec, intrinsic: (f32, f32)) -> (f32, f32, bool) {
        let laid_out = || {
            let last = self.frame_no - u64::from(self.building);
            self.svg_rects_last(key, last)
        };
        let mut laid = false;
        let mut axis = |s: &Sizing, pick: fn(Size) -> f32| match s {
            Sizing::Fixed(px) => Some(*px),
            Sizing::Fit => None,
            _ => {
                laid = true;
                laid_out().map(pick)
            }
        };
        let w = axis(&spec.layout.width, |s| s.w);
        let h = axis(&spec.layout.height, |s| s.h);
        let (iw, ih) = intrinsic;
        let (w, h) = match (w, h) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) if iw > 0.0 => (w, w * ih / iw),
            (None, Some(h)) if ih > 0.0 => (h * iw / ih, h),
            (w, h) => (w.unwrap_or(iw), h.unwrap_or(ih)),
        };
        (w.max(0.0), h.max(0.0), laid)
    }

    /// The size the box at `key` was laid out at in frame `last`, from the
    /// laid-out boxes this store noted after that frame's layout.
    fn svg_rects_last(&self, key: Key, last: u64) -> Option<Size> {
        self.svgs
            .sizes
            .get(&key)
            .filter(|(_, seen)| *seen == last)
            .map(|(s, _)| *s)
    }

    /// After layout: the boxes drawn at a size from layout note the size
    /// they came out at, and one that differs from what it was drawn at
    /// owes the frame that draws it at its new size.
    pub(crate) fn settle_svg_sizes(&mut self) {
        if self.svgs.laid.is_empty() {
            return;
        }
        let laid = std::mem::take(&mut self.svgs.laid);
        let mut owed = false;
        for &(i, drawn) in &laid {
            let i = i as usize;
            if i >= self.tree.len() {
                continue;
            }
            let size = self.tree.size[i];
            let size = Size::new(size.w, size.h);
            self.svgs
                .sizes
                .insert(self.tree.keys[i], (size, self.frame_no));
            owed |= (size.w - drawn.w).abs() > 0.01 || (size.h - drawn.h).abs() > 0.01;
        }
        self.svgs.laid = laid;
        if owed {
            self.owe_frame("an svg drawn at last frame's size was laid out at another");
        }
    }

    /// The parsed document behind `src`, parsing it on first sight and a
    /// file again when its stamp moves; `None`, with its warning raised
    /// once under `key`, when it does not parse or cannot be read.
    fn svg_doc(&mut self, key: Key, src: SvgSource<'_>) -> Option<(u64, Arc<Svg>)> {
        let now = self.frame_no;
        let hash = match src {
            SvgSource::Data(bytes) => {
                let hash = crate::key::hash_bulk(bytes);
                if !self.svgs.docs.contains_key(&hash) {
                    self.svg_parse(hash, bytes);
                }
                hash
            }
            SvgSource::File(path) => match self.svg_file(path) {
                Ok(hash) => hash,
                Err(why) => {
                    self.warn(crate::diag::svg_unreadable(key, path, &why));
                    return None;
                }
            },
        };
        let doc = self.svgs.docs.get_mut(&hash)?;
        doc.seen = now;
        match &doc.svg {
            Ok(svg) => Some((hash, svg.clone())),
            Err(why) => {
                let why = why.clone();
                self.warn(crate::diag::svg_malformed(key, &why));
                None
            }
        }
    }

    fn svg_parse(&mut self, hash: u64, bytes: &[u8]) {
        let svg = Svg::parse(bytes).map(Arc::new).map_err(|e| e.0);
        self.svgs.docs.insert(
            hash,
            Doc {
                svg,
                seen: self.frame_no,
            },
        );
    }

    /// A file's content hash, its document parsed: the stamp read once a
    /// frame, the file read again only when it moved.
    fn svg_file(&mut self, path: &str) -> Result<u64, String> {
        let now = self.frame_no;
        if let Some(f) = self.svgs.files.get(path)
            && f.checked == now
        {
            return f.hash.clone();
        }
        let stamp = std::fs::metadata(path)
            .ok()
            .map(|m| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()));
        let unchanged = self
            .svgs
            .files
            .get(path)
            .is_some_and(|f| f.stamp.is_some() && f.stamp == stamp);
        let hash = if unchanged {
            self.svgs.files[path].hash.clone()
        } else {
            match std::fs::read(path) {
                Ok(bytes) => {
                    let hash = crate::key::hash_bulk(&bytes);
                    if !self.svgs.docs.contains_key(&hash) {
                        self.svg_parse(hash, &bytes);
                    }
                    Ok(hash)
                }
                Err(e) => Err(e.to_string()),
            }
        };
        match self.svgs.files.get_mut(path) {
            Some(f) => {
                f.stamp = stamp;
                f.hash = hash.clone();
                f.checked = now;
            }
            None => {
                self.svgs.files.insert(
                    path.to_owned(),
                    FileSeen {
                        stamp,
                        hash: hash.clone(),
                        checked: now,
                    },
                );
            }
        }
        hash
    }

    /// The image the picture rasterizes to at this size, scale, fit and
    /// colour, registered once and shared by every element drawing it.
    fn svg_raster(
        &mut self,
        hash: u64,
        doc: &Svg,
        w: f32,
        h: f32,
        fit: SvgFit,
        color: Color,
    ) -> crate::resources::ImageId {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        hash.hash(&mut hasher);
        w.to_bits().hash(&mut hasher);
        h.to_bits().hash(&mut hasher);
        self.scale.to_bits().hash(&mut hasher);
        fit.hash(&mut hasher);
        color.to_hex().hash(&mut hasher);
        let sig = hasher.finish();
        let now = self.frame_no;
        if let Some(r) = self.svgs.rasters.get_mut(&sig) {
            r.seen = now;
            return r.id;
        }
        let (pw, ph, rgba) = doc.raster(w, h, self.scale, fit, color);
        let id = self.session.state().resources.add_image(pw, ph, rgba);
        self.svgs.rasters.insert(sig, Raster { id, seen: now });
        id
    }

    /// How many distinct rasters the `svg` elements hold registered — for
    /// a test of which road a picture took and that sizes are shared.
    pub fn svg_raster_count(&self) -> usize {
        self.svgs.raster_count()
    }
}
