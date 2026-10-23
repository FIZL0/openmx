//! A software rasteriser: depth-buffered, perspective-correct, textured, and about as simple as one
//! can be while still drawing a stadium correctly.
//!
//! It takes triangles already in clip space, clips them against the near plane (without which anything
//! behind the camera wraps around to the front), and fills them with an edge function. Attributes are
//! interpolated divided by `w` and put back at the end, which is what keeps a texture from swimming
//! across a large ground quad.
//!
//! A frame's worth of triangles can also be gathered into a [`Batch`] and drawn by [`Frame::draw`] on
//! every core at once: the frame is cut into bands of rows, each triangle is filed under the bands it
//! touches, and each band is filled by one thread with its triangles in the order they were queued.
//! Every pixel sees the same triangles in the same order through the same arithmetic as
//! [`Frame::triangle_with`] would have given it, so the picture is the same picture, byte for byte
//! (P47: the shell drew at one frame a second at a desktop's size on one core).
//!
//! There is no backface culling on purpose. The PS2 culls in VU1, using data this crate does not read,
//! and a strip's winding is not reliable without it; the depth buffer gets the same answer for a closed
//! model and a kinder one for the open ones.

use crate::pst::Texture;

use std::sync::OnceLock;

/// A vertex handed to the rasteriser.
#[derive(Clone, Copy, Debug)]
pub struct Shade {
    /// Clip space: `w` is view depth.
    pub clip: [f32; 4],
    pub uv: [f32; 2],
    /// Light already applied per vertex, as a multiplier on the texture.
    pub light: [f32; 3],
    /// 0 = clear, 1 = entirely fog.
    pub fog: f32,
}

/// How a triangle's pixels are coloured and laid over what is already in the frame.
pub struct Look<'a> {
    /// The texel at an interpolated texture coordinate, RGBA, before the vertex light.
    pub paint: &'a dyn Fn(f32, f32) -> [u8; 4],
    /// Texels with less alpha than this are not drawn at all: the material's alpha test.
    pub cutoff: u8,
    /// Texels with less alpha than this are drawn but do not write depth: an alpha test that fails
    /// into the colour buffer only.
    pub depth_cutoff: u8,
    /// Mix over the frame by the texel's alpha rather than replacing it.
    pub blend: bool,
    /// With `blend`, add to the frame instead of mixing into it: `Cs * As + Cd`, glows and glints.
    pub additive: bool,
    /// Whether a drawn pixel hides what is drawn behind it later. Translucent materials say no.
    pub depth_write: bool,
    /// The share of its pixels drawn at all, by [`crate::eco::covers`]' screen pattern: 1 for every
    /// one (V55's dissolving plants are less).
    pub cover: f32,
}

/// How a queued triangle's pixels are laid over the frame: [`Look`] without the paint, which a
/// [`Batch`] is drawn with as a whole and tells apart by each triangle's tag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mode {
    pub cutoff: u8,
    pub depth_cutoff: u8,
    pub blend: bool,
    pub additive: bool,
    pub depth_write: bool,
    pub cover: f32,
}

impl Mode {
    /// [`Look::cut_out`]'s.
    pub const CUT_OUT: Mode = Mode {
        cutoff: 32,
        depth_cutoff: 0,
        blend: false,
        additive: false,
        depth_write: true,
        cover: 1.0,
    };
}

impl<'a> Look<'a> {
    /// Opaque, with the cut-out every texture got before materials had their own alpha test: a
    /// texel under 32 of 255 is a hole.
    pub fn cut_out(paint: &'a dyn Fn(f32, f32) -> [u8; 4]) -> Look<'a> {
        Look {
            paint,
            cutoff: 32,
            depth_cutoff: 0,
            blend: false,
            additive: false,
            depth_write: true,
            cover: 1.0,
        }
    }
}

/// A colour buffer and its depth buffer.
pub struct Frame {
    pub width: usize,
    pub height: usize,
    /// RGB, row by row from the top.
    pub color: Vec<u8>,
    /// 1/w, larger being nearer. Zero is the far plane.
    pub depth: Vec<f32>,
}

/// How near the near plane is, in world units. Nothing nearer survives the clip.
const NEAR: f32 = 0.05;

impl Frame {
    pub fn new(width: usize, height: usize, clear: [u8; 3]) -> Frame {
        let mut color = Vec::with_capacity(width * height * 3);
        for _ in 0..width * height {
            color.extend_from_slice(&clear);
        }
        Frame {
            width,
            height,
            color,
            depth: vec![0.0; width * height],
        }
    }

    /// Draw one triangle. `texture` of `None` paints flat white times the vertex light.
    pub fn triangle(&mut self, v: [Shade; 3], texture: Option<&Texture>, fog: [u8; 3]) {
        let paint = |u: f32, v: f32| match texture {
            Some(t) => t.sample(u, v),
            None => [255, 255, 255, 255],
        };
        self.triangle_with(v, &Look::cut_out(&paint), fog);
    }

    /// Draw one triangle the way `look` says: its texels from a function of the interpolated `uv`
    /// rather than from one texture, and its own alpha test and blend.
    pub fn triangle_with(&mut self, v: [Shade; 3], look: &Look, fog: [u8; 3]) {
        // Clip against the near plane: a polygon of at most four vertices comes out, as at most two
        // triangles.
        let (a, b) = clip_near(v);
        for set in [a, b].into_iter().flatten() {
            self.fill(&set, look, fog);
        }
    }

    fn fill(&mut self, v: &[Shade; 3], look: &Look, fog: [u8; 3]) {
        if let Some(set) = Setup::of(v, self.width, self.height) {
            let mut rows = Rows {
                width: self.width,
                y0: 0,
                color: &mut self.color,
                depth: &mut self.depth,
            };
            rows.fill(&set, look, fog);
        }
    }

    /// Draw every triangle of `batch`, in the order they were queued, on as many threads as the
    /// machine has: `paint` is each pixel's texel, given the triangle's tag and the interpolated
    /// `uv`. The frame comes out exactly as one [`Frame::triangle_with`] a triangle would leave it.
    pub fn draw(&mut self, batch: &Batch, paint: &(dyn Fn(u32, f32, f32) -> [u8; 4] + Sync), fog: [u8; 3]) {
        let (width, height) = (self.width, self.height);
        if batch.tris.is_empty() || width == 0 || height == 0 {
            return;
        }
        assert_eq!((batch.width, batch.height), (width, height), "a batch is set up for the frame it is drawn into");
        let fill = |rows: &mut Rows, list: &mut dyn Iterator<Item = &Queued>| {
            for q in list {
                let look_paint = |u: f32, v: f32| paint(q.tag, u, v);
                let look = Look {
                    paint: &look_paint,
                    cutoff: q.mode.cutoff,
                    depth_cutoff: q.mode.depth_cutoff,
                    blend: q.mode.blend,
                    additive: q.mode.additive,
                    depth_write: q.mode.depth_write,
                    cover: q.mode.cover,
                };
                rows.fill(&q.setup, &look, fog);
            }
        };
        let threads = threads();
        // A small frame or a handful of triangles is not worth waking anyone for.
        if threads == 1 || width * height < 64 * 64 || batch.tris.len() < 8 {
            let mut rows = Rows {
                width,
                y0: 0,
                color: &mut self.color,
                depth: &mut self.depth,
            };
            fill(&mut rows, &mut batch.tris.iter());
            return;
        }
        // Several bands a thread, so a thread whose bands are all sky is not the only one idle.
        let band = height.div_ceil(threads * 4).max(4);
        let bands = height.div_ceil(band);
        let mut filed: Vec<Vec<u32>> = vec![Vec::new(); bands];
        for (i, q) in batch.tris.iter().enumerate() {
            for list in &mut filed[q.setup.min_y / band..=q.setup.max_y / band] {
                list.push(i as u32);
            }
        }
        let mut work: Vec<Vec<(Rows, &[u32])>> = (0..threads).map(|_| Vec::new()).collect();
        let color = self.color.chunks_mut(band * width * 3);
        let depth = self.depth.chunks_mut(band * width);
        for (b, ((color, depth), list)) in color.zip(depth).zip(&filed).enumerate() {
            if !list.is_empty() {
                work[b % threads].push((Rows { width, y0: b * band, color, depth }, list));
            }
        }
        std::thread::scope(|scope| {
            for mine in work.into_iter().filter(|w| !w.is_empty()) {
                scope.spawn(move || {
                    for (mut rows, list) in mine {
                        fill(&mut rows, &mut list.iter().map(|&i| &batch.tris[i as usize]));
                    }
                });
            }
        });
    }
}

/// How many threads [`Frame::draw`] and the other per-pixel passes share a frame between: every
/// core, or `OPENMX_THREADS` of them.
pub fn threads() -> usize {
    static THREADS: OnceLock<usize> = OnceLock::new();
    *THREADS.get_or_init(|| {
        std::env::var("OPENMX_THREADS")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
            .max(1)
    })
}

/// A triangle on the screen, ready to fill: where its corners are, their `1/w`, and the pixels it
/// can touch.
#[derive(Clone, Copy, Debug)]
struct Setup {
    v: [Shade; 3],
    p: [[f32; 3]; 3],
    iw: [f32; 3],
    inv_area: f32,
    min_x: usize,
    max_x: usize,
    min_y: usize,
    max_y: usize,
}

impl Setup {
    fn of(v: &[Shade; 3], width: usize, height: usize) -> Option<Setup> {
        let (w, h) = (width as f32, height as f32);
        // To screen, keeping 1/w and attribute/w for the perspective-correct interpolation.
        let mut p = [[0.0f32; 3]; 3];
        let mut iw = [0.0f32; 3];
        for i in 0..3 {
            let c = v[i].clip;
            let inv = 1.0 / c[3];
            iw[i] = inv;
            p[i] = [
                (c[0] * inv * 0.5 + 0.5) * w,
                (0.5 - c[1] * inv * 0.5) * h,
                c[2] * inv,
            ];
        }
        let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
        if area.abs() < 1e-7 {
            return None;
        }
        let inv_area = 1.0 / area;
        let min_x = p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let max_x = (p.iter().map(|q| q[0]).fold(f32::MIN, f32::max).ceil()).clamp(0.0, w - 1.0) as usize;
        let min_y = p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let max_y = (p.iter().map(|q| q[1]).fold(f32::MIN, f32::max).ceil()).clamp(0.0, h - 1.0) as usize;
        if min_x > max_x || min_y > max_y {
            return None;
        }
        Some(Setup {
            v: *v,
            p,
            iw,
            inv_area,
            min_x,
            max_x,
            min_y,
            max_y,
        })
    }
}

/// Some whole rows of a frame, from row `y0` down: the whole frame, or one band of it.
struct Rows<'f> {
    width: usize,
    y0: usize,
    color: &'f mut [u8],
    depth: &'f mut [f32],
}

impl Rows<'_> {
    /// The part of one triangle that falls in these rows.
    fn fill(&mut self, set: &Setup, look: &Look, fog: [u8; 3]) {
        let rows = self.depth.len() / self.width.max(1);
        let (first, last) = (set.min_y.max(self.y0), set.max_y.min(self.y0 + rows - 1));
        if first > last {
            return;
        }
        let (p, iw, inv_area, v) = (&set.p, &set.iw, set.inv_area, &set.v);
        for y in first..=last {
            let py = y as f32 + 0.5;
            for x in set.min_x..=set.max_x {
                let px = x as f32 + 0.5;
                // Barycentric weights from the three edge functions.
                let w0 = ((p[1][0] - px) * (p[2][1] - py) - (p[1][1] - py) * (p[2][0] - px)) * inv_area;
                let w1 = ((p[2][0] - px) * (p[0][1] - py) - (p[2][1] - py) * (p[0][0] - px)) * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let at = (y - self.y0) * self.width + x;
                let inv_w = w0 * iw[0] + w1 * iw[1] + w2 * iw[2];
                if inv_w <= self.depth[at] {
                    continue;
                }
                if look.cover < 1.0 && !crate::eco::covers(x, y, look.cover) {
                    continue;
                }
                let scale = 1.0 / inv_w;
                let attr = |f: &dyn Fn(&Shade) -> f32| {
                    (w0 * f(&v[0]) * iw[0] + w1 * f(&v[1]) * iw[1] + w2 * f(&v[2]) * iw[2]) * scale
                };
                let (u, vv) = (attr(&|s: &Shade| s.uv[0]), attr(&|s: &Shade| s.uv[1]));
                let texel = (look.paint)(u, vv);
                if texel[3] < look.cutoff {
                    continue; // a cut-out texture: banners and fences are full of them
                }
                let fogged = attr(&|s: &Shade| s.fog).clamp(0.0, 1.0);
                let mut rgb = [0u8; 3];
                for (k, slot) in rgb.iter_mut().enumerate() {
                    let light = attr(&|s: &Shade| s.light[k]);
                    let lit = texel[k] as f32 * light;
                    let mut out = (lit * (1.0 - fogged) + fog[k] as f32 * fogged).clamp(0.0, 255.0);
                    if look.blend {
                        // The GS's usual `(Cs - Cd) * As + Cd`, with the decoder's 255 as its 128.
                        let under = self.color[at * 3 + k] as f32;
                        let a = texel[3] as f32 / 255.0;
                        out = if look.additive {
                            (under + out * a).min(255.0)
                        } else {
                            under + (out - under) * a
                        };
                    }
                    *slot = out as u8;
                }
                self.color[at * 3..at * 3 + 3].copy_from_slice(&rgb);
                if look.depth_write && texel[3] >= look.depth_cutoff {
                    self.depth[at] = inv_w;
                }
            }
        }
    }
}

/// One queued triangle: set up for the frame, with the tag its paint is told apart by.
#[derive(Clone, Copy, Debug)]
struct Queued {
    setup: Setup,
    tag: u32,
    mode: Mode,
}

/// Triangles gathered for one [`Frame::draw`]: clipped and set up as they are queued, filled when
/// drawn.
pub struct Batch {
    width: usize,
    height: usize,
    tris: Vec<Queued>,
}

impl Batch {
    /// An empty batch for a frame of this size.
    pub fn new(width: usize, height: usize) -> Batch {
        Batch {
            width,
            height,
            tris: Vec::new(),
        }
    }

    /// Queue one triangle, near-clipped exactly as [`Frame::triangle_with`] clips it.
    pub fn triangle(&mut self, v: [Shade; 3], tag: u32, mode: Mode) {
        let (a, b) = clip_near(v);
        for set in [a, b].into_iter().flatten() {
            if let Some(setup) = Setup::of(&set, self.width, self.height) {
                self.tris.push(Queued { setup, tag, mode });
            }
        }
    }

    /// How many triangles the clip and the set-up left to fill.
    pub fn len(&self) -> usize {
        self.tris.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// Queue every triangle of `later` after this batch's own, in its order: how batches queued on
    /// several cores join into the one a single core would have queued.
    pub fn append(&mut self, mut later: Batch) {
        self.tris.append(&mut later.tris);
    }
}

/// The near-plane clip of [`Frame::triangle_with`]: the one or two triangles that are left.
fn clip_near(v: [Shade; 3]) -> (Option<[Shade; 3]>, Option<[Shade; 3]>) {
    let mut poly = [v[0]; 4];
    let mut n = 0;
    for i in 0..3 {
        let (a, b) = (v[i], v[(i + 1) % 3]);
        let (da, db) = (a.clip[3] - NEAR, b.clip[3] - NEAR);
        if da >= 0.0 {
            poly[n] = a;
            n += 1;
        }
        if (da >= 0.0) != (db >= 0.0) {
            poly[n] = mix(a, b, da / (da - db));
            n += 1;
        }
    }
    let tri = |i: usize| (i < n).then(|| [poly[0], poly[i - 1], poly[i]]);
    (tri(2), tri(3))
}

/// Linear blend of two vertices, for the near-plane clip.
fn mix(a: Shade, b: Shade, t: f32) -> Shade {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Shade {
        clip: [
            l(a.clip[0], b.clip[0]),
            l(a.clip[1], b.clip[1]),
            l(a.clip[2], b.clip[2]),
            l(a.clip[3], b.clip[3]),
        ],
        uv: [l(a.uv[0], b.uv[0]), l(a.uv[1], b.uv[1])],
        light: [
            l(a.light[0], b.light[0]),
            l(a.light[1], b.light[1]),
            l(a.light[2], b.light[2]),
        ],
        fog: l(a.fog, b.fog),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(f: &Frame, x: usize, y: usize) -> [u8; 3] {
        let i = (y * f.width + x) * 3;
        [f.color[i], f.color[i + 1], f.color[i + 2]]
    }

    fn vertex(x: f32, y: f32, w: f32) -> Shade {
        Shade {
            clip: [x * w, y * w, 0.0, w],
            uv: [0.0, 0.0],
            light: [1.0, 1.0, 1.0],
            fog: 0.0,
        }
    }

    #[test]
    fn a_triangle_covers_the_pixels_inside_it_and_no_others() {
        let mut f = Frame::new(16, 16, [0, 0, 0]);
        f.triangle(
            [
                vertex(-0.9, -0.9, 1.0),
                vertex(0.9, -0.9, 1.0),
                vertex(-0.9, 0.9, 1.0),
            ],
            None,
            [0, 0, 0],
        );
        assert_eq!(at(&f, 2, 8), [255, 255, 255], "inside");
        assert_eq!(at(&f, 14, 2), [0, 0, 0], "outside the hypotenuse");
    }

    #[test]
    fn the_nearer_triangle_wins_whichever_order_they_are_drawn_in() {
        for near_first in [true, false] {
            let mut f = Frame::new(8, 8, [0, 0, 0]);
            let far = [vertex(-1.0, -1.0, 8.0), vertex(1.0, -1.0, 8.0), vertex(-1.0, 1.0, 8.0)];
            let mut near = far;
            for v in near.iter_mut() {
                v.clip = [v.clip[0] / 8.0 * 2.0, v.clip[1] / 8.0 * 2.0, 0.0, 2.0];
                v.light = [0.5, 0.5, 0.5];
            }
            let (a, b) = if near_first { (near, far) } else { (far, near) };
            f.triangle(a, None, [0, 0, 0]);
            f.triangle(b, None, [0, 0, 0]);
            assert_eq!(at(&f, 1, 4), [127, 127, 127], "the nearer one is the one that shows");
        }
    }

    #[test]
    fn a_triangle_through_the_camera_is_clipped_not_wrapped() {
        let mut f = Frame::new(16, 16, [0, 0, 0]);
        // One vertex behind the eye: without the near clip this draws a mirrored mess.
        f.triangle(
            [
                Shade { clip: [0.0, 0.0, 0.0, -1.0], ..vertex(0.0, 0.0, 1.0) },
                vertex(0.5, -0.5, 1.0),
                vertex(-0.5, -0.5, 1.0),
            ],
            None,
            [0, 0, 0],
        );
        assert_eq!(at(&f, 8, 2), [0, 0, 0], "nothing above the clipped edge");
        assert_eq!(at(&f, 8, 11), [0, 0, 0], "nor above the edge the clip left behind");
        assert_eq!(at(&f, 8, 13), [255, 255, 255], "the part in front still draws");
    }
}
