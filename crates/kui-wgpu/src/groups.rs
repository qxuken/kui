//! A faded subtree drawn as one picture (backlog V11, ADR 0056).
//!
//! The core brackets a group's quads with `GroupBegin` / `GroupEnd`
//! (`kui_core::QuadKind`), the quads inside unfaded by the group, the end
//! carrying its bounds and its fade. Before the frame's own pass, each
//! group is drawn into a texture at a place of its own, deepest first, and
//! the end marker draws that picture into whatever holds it — the frame,
//! or the group around it — faded once (`shade`, kind 8).
//!
//! A group is drawn with the frame's own instances and pipeline, through
//! a viewport moved so its bounds land on its place in the texture: no
//! instance is rewritten and the shader does not know it is in a group,
//! except that the clip is tested against the fragment's place in the
//! frame (`VsOut::screen`) rather than in the target. Groups at even
//! depths go in one texture and odd in the other, so a pass never samples
//! the texture it draws into: a group's pass reads only the pictures of
//! the groups directly inside it, one depth deeper.

use kui_core::{Quad, QuadKind};

/// No group: the frame.
pub(crate) const FRAME: u32 = u32::MAX;

/// One group of the frame, where it is and where it is drawn.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Plan {
    /// Its markers' indices.
    pub begin: u32,
    pub end: u32,
    /// How many groups it is inside: 0 for one drawn into the frame.
    pub depth: u32,
    /// The plan it is drawn into, or [`FRAME`].
    pub parent: u32,
    /// Its bounds in the frame, whole physical px.
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Its place in its texture.
    pub at: (u32, u32),
}

impl Plan {
    /// Which of the two textures it is drawn into.
    pub fn tex(&self) -> usize {
        (self.depth % 2) as usize
    }

    /// What `shade` adds to a pixel's place in the frame to find it in the
    /// texture.
    pub fn offset(&self) -> [f32; 2] {
        [
            self.at.0 as f32 - self.x as f32,
            self.at.1 as f32 - self.y as f32,
        ]
    }
}

/// The frame's groups from its markers, inner before outer — the order
/// they close in — each with the group it is drawn into. A marker with no
/// partner, which a core never emits, is passed over.
pub(crate) fn plan(quads: &[Quad], out: &mut Vec<Plan>) {
    out.clear();
    let mut open: Vec<u32> = Vec::new();
    for (i, q) in quads.iter().enumerate() {
        match q.kind {
            QuadKind::GroupBegin => open.push(i as u32),
            QuadKind::GroupEnd => {
                let Some(begin) = open.pop() else {
                    continue;
                };
                let r = q.rect;
                out.push(Plan {
                    begin,
                    end: i as u32,
                    depth: open.len() as u32,
                    parent: FRAME,
                    x: r.x.max(0.0) as u32,
                    y: r.y.max(0.0) as u32,
                    w: r.w.max(0.0) as u32,
                    h: r.h.max(0.0) as u32,
                    at: (0, 0),
                });
            }
            _ => {}
        }
    }
    // The group a group is drawn into closes after it, and is the first
    // one a depth shallower to: any other that shallow closing between
    // them would be inside the first, so deeper.
    for j in 0..out.len() {
        let d = out[j].depth;
        if d == 0 {
            continue;
        }
        if let Some(k) = (j + 1..out.len()).find(|&k| out[k].depth == d - 1) {
            out[j].parent = k as u32;
        }
    }
}

/// Gives each group its place in its texture, in rows no wider than the
/// frame (or the widest group), and returns each texture's size; `None`
/// when a texture would be larger than `max` on a side.
pub(crate) fn pack(plans: &mut [Plan], width: u32, max: u32) -> Option<[(u32, u32); 2]> {
    let mut sizes = [(0u32, 0u32); 2];
    for (t, size) in sizes.iter_mut().enumerate() {
        let widest = plans.iter().filter(|p| p.tex() == t).map(|p| p.w).max();
        let Some(widest) = widest else {
            continue;
        };
        let row = widest.max(width).min(max);
        let (mut x, mut y, mut tall) = (0u32, 0u32, 0u32);
        for p in plans.iter_mut().filter(|p| p.tex() == t) {
            if p.w > row {
                return None;
            }
            if x + p.w > row {
                (x, y, tall) = (0, y + tall, 0);
            }
            p.at = (x, y);
            x += p.w;
            tall = tall.max(p.h);
            size.0 = size.0.max(x);
        }
        size.1 = y + tall;
        if size.1 > max {
            return None;
        }
    }
    Some(sizes)
}

/// The two textures groups are drawn into, kept while frames draw groups
/// and grown in steps of 256 px, so a group that moves or grows a little
/// does not make one each frame.
#[derive(Default)]
pub(crate) struct Targets {
    tex: [Option<Target>; 2],
}

struct Target {
    view: wgpu::TextureView,
    size: (u32, u32),
}

impl Targets {
    /// Fits each texture to `sizes`: made or grown where it is too small,
    /// dropped where nothing is drawn into it.
    pub fn fit(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sizes: [(u32, u32); 2],
        max: u32,
    ) {
        for (slot, (w, h)) in self.tex.iter_mut().zip(sizes) {
            if w == 0 || h == 0 {
                *slot = None;
                continue;
            }
            if slot
                .as_ref()
                .is_some_and(|t| t.size.0 >= w && t.size.1 >= h)
            {
                continue;
            }
            let have = slot.as_ref().map_or((0, 0), |t| t.size);
            let step = |n: u32, had: u32| n.max(had).next_multiple_of(256).min(max);
            let size = (step(w, have.0), step(h, have.1));
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("kui.groups"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            *slot = Some(Target { view, size });
        }
    }

    pub fn view(&self, t: usize) -> Option<&wgpu::TextureView> {
        self.tex[t].as_ref().map(|t| &t.view)
    }

    /// Both dropped: a frame with no group.
    pub fn clear(&mut self) {
        self.tex = [None, None];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kui_core::{Color, Rect};

    fn q(kind: QuadKind, r: Rect) -> Quad {
        Quad {
            rect: r,
            color: Color::WHITE,
            border_color: Color::TRANSPARENT,
            radius: [0.0; 4],
            border_w: 0.0,
            blur: 0.0,
            kind,
            clip: 0,
            uv: [0; 4],
        }
    }

    #[test]
    fn nested_groups_plan_inner_first_each_drawn_into_the_one_around_it() {
        let outer = Rect::new(10.0, 10.0, 100.0, 50.0);
        let a = Rect::new(12.0, 12.0, 20.0, 10.0);
        let b = Rect::new(40.0, 12.0, 30.0, 10.0);
        let quads = vec![
            q(QuadKind::GroupBegin, outer),
            q(QuadKind::Solid, outer),
            q(QuadKind::GroupBegin, a),
            q(QuadKind::Solid, a),
            q(QuadKind::GroupEnd, a),
            q(QuadKind::GroupBegin, b),
            q(QuadKind::Solid, b),
            q(QuadKind::GroupEnd, b),
            q(QuadKind::GroupEnd, outer),
            q(QuadKind::GroupBegin, a),
            q(QuadKind::GroupEnd, a),
        ];
        let mut plans = Vec::new();
        plan(&quads, &mut plans);
        let got: Vec<(u32, u32, u32, u32)> = plans
            .iter()
            .map(|p| (p.begin, p.end, p.depth, p.parent))
            .collect();
        assert_eq!(
            got,
            vec![
                (2, 4, 1, 2),
                (5, 7, 1, 2),
                (0, 8, 0, FRAME),
                (9, 10, 0, FRAME)
            ]
        );
        let sizes = pack(&mut plans, 64, 8192).expect("fits");
        // Depth 1 in texture 1, side by side; depth 0 in texture 0.
        assert_eq!((plans[0].at, plans[1].at), ((0, 0), (20, 0)));
        assert_eq!(sizes[1], (50, 10));
        assert_eq!(plans[2].at, (0, 0));
        assert_eq!(plans[3].at, (0, 50), "the outer is wider than a row");
        assert_eq!(sizes[0], (100, 60));
        assert_eq!(plans[1].offset(), [20.0 - 40.0, 0.0 - 12.0]);
    }

    #[test]
    fn a_texture_past_the_device_s_limit_does_not_pack() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let quads: Vec<Quad> = (0..3)
            .flat_map(|_| [q(QuadKind::GroupBegin, r), q(QuadKind::GroupEnd, r)])
            .collect();
        let mut plans = Vec::new();
        plan(&quads, &mut plans);
        assert!(
            pack(&mut plans, 100, 250).is_none(),
            "three rows of 100 in 250"
        );
        assert!(pack(&mut plans, 100, 300).is_some());
    }
}
