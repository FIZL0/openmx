//! The menus' own ink (P118): our type and our shapes, drawn at the frame's pixels on both paths.
//!
//! The front end used to draw the disc's bitmap font and its pill textures stretched to the frame,
//! soft at 4K. Everything here is vector instead, as opentennis's menus are (B136), and drawn the
//! same way on the GPU and on the CPU: a screen is a list of [`Sprite`]s, which the GPU draws
//! (`game_render::gpu`'s [`Shape::Round`] and [`Shape::Text`]) and [`raster`] draws into a software
//! [`Frame`] by the same rules, so the two paths are one layout.
//!
//! - **Type** is M PLUS Rounded 1c, ExtraBold and Bold (`assets/fonts`, under the OFL beside them).
//!   At load every glyph's outline is turned into a distance field, [`ATLAS_EM`] texels to the em,
//!   with the glyphs packed into one picture ([`Type::atlas`]). A glyph is drawn as a quad of its
//!   cell at any size, its edge found from the distance at the frame pixel, so a label is a pixel's
//!   edge wide at 640x448 and at 4K alike, and slanting it is a sheared quad.
//! - **Shapes** — the pills, tabs, boxes, the hint tab, the pad's buttons — are rounded boxes and
//!   needles measured in frame pixels: nothing is a stretched texture.

use ab_glyph::{Font as _, FontRef, GlyphId, OutlineCurve};
use game_render::gpu::{Shape, Sprite};
use game_render::{Frame, Texture};
use std::collections::HashMap;
use std::sync::OnceLock;

/// The fonts, in the crate (OFL: `assets/fonts/OFL.txt`).
pub const HEAVY_TTF: &[u8] = include_bytes!("../assets/fonts/MPLUSRounded1c-ExtraBold.ttf");
pub const BOLD_TTF: &[u8] = include_bytes!("../assets/fonts/MPLUSRounded1c-Bold.ttf");
/// The ride HUD's digital speedometer face (P121): DSEG7 Classic Bold Italic, a seven-segment
/// display's lit segments only (OFL: `assets/fonts/DSEG-OFL.txt`).
pub const DSEG_TTF: &[u8] = include_bytes!("../assets/fonts/DSEG7Classic-BoldItalic.ttf");

/// Atlas texels to the em.
pub const ATLAS_EM: f32 = 64.0;
/// How far from the outline, in atlas texels, the field still tells distance apart: past it every
/// texel is wholly in or out.
pub const SPREAD: f32 = 8.0;
/// The atlas' width; its height is what the glyphs need.
const ATLAS_WIDTH: usize = 1024;

/// The CSS concept's slant: `skewX(-11deg)`.
pub const SLANT: f32 = 0.194_380_3;

/// Which of the two faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weight {
    /// ExtraBold: lit labels, titles, values.
    Heavy,
    /// Bold: everything else.
    Bold,
}

/// One glyph in the atlas.
#[derive(Clone, Copy, Debug)]
struct Glyph {
    id: GlyphId,
    /// Its cell, in atlas texels: left, top, right, bottom. Empty for a space.
    cell: [f32; 4],
    /// The cell's top-left from the pen on the baseline, in atlas texels at [`ATLAS_EM`].
    at: [f32; 2],
    /// How far the pen moves, in atlas texels.
    advance: f32,
}

/// Both faces' glyphs and the atlas they are in.
pub struct Type {
    pub atlas: Texture,
    glyphs: [HashMap<char, Glyph>; 2],
    fonts: [FontRef<'static>; 2],
    /// Atlas texels a font unit is, each face's.
    unit: [f32; 2],
    /// The ascent and descent, in ems, ExtraBold's (both faces share them).
    pub ascent: f32,
    pub descent: f32,
}

/// The characters the atlas holds: printable ASCII, and the few others the menus say, where the
/// fonts have them.
fn charset() -> impl Iterator<Item = char> + Clone {
    (' '..='~').chain(['·', '°', '…', '–', '—', '×', '✓'])
}

/// The type, made once a process.
pub fn ink() -> &'static Type {
    static TYPE: OnceLock<Type> = OnceLock::new();
    TYPE.get_or_init(Type::new)
}

/// The seven-segment digits (P121), made once a process: [`DSEG_TTF`] as both weights, holding
/// only what a speedometer says.
pub fn digits() -> &'static Type {
    static TYPE: OnceLock<Type> = OnceLock::new();
    TYPE.get_or_init(|| Type::of([DSEG_TTF, DSEG_TTF], "0123456789.-: ".chars()))
}

impl Type {
    fn new() -> Type {
        Type::of([HEAVY_TTF, BOLD_TTF], charset())
    }

    /// The type of the two faces `ttfs` (ExtraBold's place, then Bold's), holding `chars`.
    fn of(ttfs: [&'static [u8]; 2], chars: impl Iterator<Item = char> + Clone) -> Type {
        let fonts = ttfs.map(|t| FontRef::try_from_slice(t).expect("the font reads"));
        let unit = fonts.clone().map(|f| ATLAS_EM / f.units_per_em().unwrap_or(1000.0));
        let mut fields: Vec<(usize, char, GlyphId, Field, f32)> = Vec::new();
        for (w, font) in fonts.iter().enumerate() {
            for c in chars.clone() {
                let id = font.glyph_id(c);
                if id.0 == 0 && c != ' ' {
                    continue;
                }
                let advance = font.h_advance_unscaled(id) * unit[w];
                fields.push((w, c, id, field(font, id, unit[w]), advance));
            }
        }
        // Shelves, tallest first.
        let mut order: Vec<usize> = (0..fields.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(fields[i].3.h));
        let (mut x, mut y, mut shelf) = (0usize, 0usize, 0usize);
        let mut places = vec![(0usize, 0usize); fields.len()];
        for &i in &order {
            let f = &fields[i].3;
            if x + f.w + 1 > ATLAS_WIDTH {
                (x, y, shelf) = (0, y + shelf + 1, 0);
            }
            places[i] = (x, y);
            x += f.w + 1;
            shelf = shelf.max(f.h);
        }
        let height = (y + shelf + 1).next_power_of_two();
        let mut rgba = vec![0u8; ATLAS_WIDTH * height * 4];
        for px in rgba.chunks_exact_mut(4) {
            px[..3].fill(255);
        }
        let mut glyphs: [HashMap<char, Glyph>; 2] = Default::default();
        for (i, (w, c, id, f, advance)) in fields.into_iter().enumerate() {
            let (px, py) = places[i];
            for row in 0..f.h {
                for col in 0..f.w {
                    rgba[((py + row) * ATLAS_WIDTH + px + col) * 4 + 3] = f.alpha[row * f.w + col];
                }
            }
            let cell = [px as f32, py as f32, (px + f.w) as f32, (py + f.h) as f32];
            glyphs[w].insert(c, Glyph { id, cell, at: f.at, advance });
        }
        let em = fonts[0].units_per_em().unwrap_or(1000.0);
        Type {
            atlas: Texture { width: ATLAS_WIDTH, height, rgba },
            glyphs,
            ascent: fonts[0].ascent_unscaled() / em,
            descent: -fonts[0].descent_unscaled() / em,
            fonts,
            unit,
        }
    }

    fn glyph(&self, w: Weight, c: char) -> Option<&Glyph> {
        let g = &self.glyphs[w as usize];
        g.get(&c).or_else(|| g.get(&'?'))
    }

    /// How far above the baseline `c`'s ink reaches, in ems (to an atlas texel): what lines a
    /// line's top up with something beside it, as the ascent cannot.
    pub fn top(&self, w: Weight, c: char) -> f32 {
        self.glyph(w, c).map_or(self.ascent, |g| -(g.at[1] + SPREAD) / ATLAS_EM)
    }

    /// How wide `text` is at `size` frame pixels to the em.
    pub fn measure(&self, w: Weight, text: &str, size: f32) -> f32 {
        let k = size / ATLAS_EM;
        let mut x = 0.0;
        let mut last: Option<GlyphId> = None;
        for c in text.chars() {
            let Some(g) = self.glyph(w, c) else { continue };
            if let Some(prev) = last {
                x += self.fonts[w as usize].kern_unscaled(prev, g.id) * self.unit[w as usize];
            }
            x += g.advance;
            last = Some(g.id);
        }
        x * k
    }

    /// `text` as sprites into `out`, its pen starting at `x` on the baseline `y`, `size` frame pixels
    /// to the em, slanted by `slant` (the tangent: x moves right this much a pixel up), in `colour`.
    /// `image` is where [`Type::atlas`] is among the pictures the sprites count into.
    #[allow(clippy::too_many_arguments)]
    pub fn text(&self, out: &mut Vec<Sprite>, image: usize, w: Weight, text: &str, x: f32, y: f32, size: f32, slant: f32, colour: [u8; 4]) {
        let k = size / ATLAS_EM;
        let range = SPREAD * k;
        let mut pen = 0.0;
        let mut last: Option<GlyphId> = None;
        for c in text.chars() {
            let Some(g) = self.glyph(w, c) else { continue };
            if let Some(prev) = last {
                pen += self.fonts[w as usize].kern_unscaled(prev, g.id) * self.unit[w as usize];
            }
            let [l, t, r, b] = g.cell;
            if r > l {
                let x0 = x + (pen + g.at[0]) * k;
                let y0 = y + g.at[1] * k;
                let (x1, y1) = (x0 + (r - l) * k, y0 + (b - t) * k);
                let lean = |yy: f32| (y - yy) * slant;
                let corners = [[x0 + lean(y0), y0], [x1 + lean(y0), y0], [x1 + lean(y1), y1], [x0 + lean(y1), y1]];
                out.push(Sprite {
                    corners,
                    uv: [[l, t], [r, t], [r, b], [l, b]],
                    local: [[0.0; 2]; 4],
                    clamp: [l, t, r - 1.0, b - 1.0],
                    image: Some(image),
                    tint: colour,
                    shape: Shape::Text { range },
                });
            }
            pen += g.advance;
            last = Some(g.id);
        }
    }
}

/// One glyph's distance field: `w` by `h` texels of alpha, and where its top-left is from the pen.
struct Field {
    w: usize,
    h: usize,
    at: [f32; 2],
    alpha: Vec<u8>,
}

/// A glyph's outline as straight segments, in atlas texels, y down from the baseline.
fn segments(font: &FontRef, id: GlyphId, unit: f32) -> Vec<[[f32; 2]; 2]> {
    let Some(outline) = font.outline(id) else { return Vec::new() };
    let p = |q: ab_glyph::Point| [q.x * unit, -q.y * unit];
    let mut out = Vec::new();
    let mut push = |pts: &[[f32; 2]]| {
        for w in pts.windows(2) {
            out.push([w[0], w[1]]);
        }
    };
    for curve in &outline.curves {
        match *curve {
            OutlineCurve::Line(a, b) => push(&[p(a), p(b)]),
            OutlineCurve::Quad(a, c, b) => {
                let (a, c, b) = (p(a), p(c), p(b));
                let pts: Vec<[f32; 2]> = (0..=8)
                    .map(|i| {
                        let t = i as f32 / 8.0;
                        let u = 1.0 - t;
                        [u * u * a[0] + 2.0 * u * t * c[0] + t * t * b[0], u * u * a[1] + 2.0 * u * t * c[1] + t * t * b[1]]
                    })
                    .collect();
                push(&pts);
            }
            OutlineCurve::Cubic(a, c, d, b) => {
                let (a, c, d, b) = (p(a), p(c), p(d), p(b));
                let pts: Vec<[f32; 2]> = (0..=12)
                    .map(|i| {
                        let t = i as f32 / 12.0;
                        let u = 1.0 - t;
                        let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                        [k0 * a[0] + k1 * c[0] + k2 * d[0] + k3 * b[0], k0 * a[1] + k1 * c[1] + k2 * d[1] + k3 * b[1]]
                    })
                    .collect();
                push(&pts);
            }
        }
    }
    out
}

/// A glyph's field: for every texel of its box (the outline's, [`SPREAD`] round it), the distance
/// from its centre to the nearest segment, signed by whether the centre is inside by the outline's
/// winding, as alpha — 0.5 on the outline.
fn field(font: &FontRef, id: GlyphId, unit: f32) -> Field {
    let segs = segments(font, id, unit);
    if segs.is_empty() {
        return Field { w: 0, h: 0, at: [0.0; 2], alpha: Vec::new() };
    }
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for s in &segs {
        for q in s {
            lo = [lo[0].min(q[0]), lo[1].min(q[1])];
            hi = [hi[0].max(q[0]), hi[1].max(q[1])];
        }
    }
    let ox = (lo[0] - SPREAD).floor();
    let oy = (lo[1] - SPREAD).floor();
    let w = (hi[0] + SPREAD).ceil() as i32 - ox as i32;
    let h = (hi[1] + SPREAD).ceil() as i32 - oy as i32;
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let mut dist = vec![SPREAD; w * h];
    for &[a, b] in &segs {
        let x0 = ((a[0].min(b[0]) - SPREAD - ox).floor().max(0.0)) as usize;
        let x1 = ((a[0].max(b[0]) + SPREAD - ox).ceil().min(w as f32)) as usize;
        let y0 = ((a[1].min(b[1]) - SPREAD - oy).floor().max(0.0)) as usize;
        let y1 = ((a[1].max(b[1]) + SPREAD - oy).ceil().min(h as f32)) as usize;
        let d = [b[0] - a[0], b[1] - a[1]];
        let len2 = (d[0] * d[0] + d[1] * d[1]).max(1e-12);
        for row in y0..y1 {
            let py = oy + row as f32 + 0.5;
            for col in x0..x1 {
                let px = ox + col as f32 + 0.5;
                let t = (((px - a[0]) * d[0] + (py - a[1]) * d[1]) / len2).clamp(0.0, 1.0);
                let (ex, ey) = (px - a[0] - t * d[0], py - a[1] - t * d[1]);
                let e = (ex * ex + ey * ey).sqrt();
                let at = row * w + col;
                if e < dist[at] {
                    dist[at] = e;
                }
            }
        }
    }
    // The sign, row by row: where each segment crosses the row's centre line, and which way.
    let mut alpha = vec![0u8; w * h];
    let mut cross: Vec<(f32, i32)> = Vec::new();
    for row in 0..h {
        let py = oy + row as f32 + 0.5;
        cross.clear();
        for &[a, b] in &segs {
            if (a[1] <= py) != (b[1] <= py) {
                let t = (py - a[1]) / (b[1] - a[1]);
                cross.push((a[0] + t * (b[0] - a[0]), if b[1] > a[1] { 1 } else { -1 }));
            }
        }
        cross.sort_by(|p, q| p.0.total_cmp(&q.0));
        let mut wind = 0;
        let mut next = 0;
        for col in 0..w {
            let px = ox + col as f32 + 0.5;
            while next < cross.len() && cross[next].0 <= px {
                wind += cross[next].1;
                next += 1;
            }
            let d = if wind != 0 { dist[row * w + col] } else { -dist[row * w + col] };
            alpha[row * w + col] = ((0.5 + d / (2.0 * SPREAD)).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    Field { w, h, at: [ox, oy], alpha }
}

// ---- shapes ----------------------------------------------------------------------------------------

/// A box in frame pixels: left, top, width, height.
pub type Box4 = [f32; 4];

/// A quad over `b` grown a pixel each way so its smoothed edge has room, with the shape's own
/// coordinates from `b`'s top-left, its corners pushed right by `slant` a pixel above `base`.
fn quad(b: Box4, slant: f32, base: f32, tint: [u8; 4], shape: Shape) -> Sprite {
    let [x, y, w, h] = b;
    let (x0, y0, x1, y1) = (x - 1.0, y - 1.0, x + w + 1.0, y + h + 1.0);
    let lean = |yy: f32| (base - yy) * slant;
    let corners = [[x0 + lean(y0), y0], [x1 + lean(y0), y0], [x1 + lean(y1), y1], [x0 + lean(y1), y1]];
    let local = [[-1.0, -1.0], [w + 1.0, -1.0], [w + 1.0, h + 1.0], [-1.0, h + 1.0]];
    Sprite { corners, uv: [[0.0; 2]; 4], local, clamp: [0.0; 4], image: None, tint, shape }
}

/// A rounded box, `top` shading to `bottom`.
pub fn round(out: &mut Vec<Sprite>, b: Box4, radius: f32, top: [u8; 4], bottom: [u8; 4]) {
    out.push(quad(b, 0.0, 0.0, top, Shape::Round { size: [b[2], b[3]], radius, ring: 0.0, bottom }));
}

/// [`round`], slanted: its corners pushed right by `slant` a pixel above its foot.
pub fn round_slanted(out: &mut Vec<Sprite>, b: Box4, radius: f32, slant: f32, top: [u8; 4], bottom: [u8; 4]) {
    out.push(quad(b, slant, b[1] + b[3], top, Shape::Round { size: [b[2], b[3]], radius, ring: 0.0, bottom }));
}

/// [`round_slanted`] with no corners and its right end square at the box's right edge (P172): the
/// hint tab, which ends at its view's edge rather than running off it into the next view.
pub fn tab(out: &mut Vec<Sprite>, b: Box4, slant: f32, colour: [u8; 4]) {
    let [x, y, w, h] = b;
    let (x0, y0, x1, y1) = (x - 1.0, y - 1.0, x + w, y + h + 1.0);
    let lean = |yy: f32| (y + h - yy) * slant;
    let corners = [[x0 + lean(y0), y0], [x1, y0], [x1, y1], [x0 + lean(y1), y1]];
    // The shape runs on past the square end, so nothing fades at the view's edge.
    let local = [[-1.0, -1.0], [w, -1.0], [w, h + 1.0], [-1.0, h + 1.0]];
    let shape = Shape::Round { size: [w + 2.0, h], radius: 0.0, ring: 0.0, bottom: colour };
    out.push(Sprite { corners, uv: [[0.0; 2]; 4], local, clamp: [0.0; 4], image: None, tint: colour, shape });
}

/// The band `width` wide inside a rounded box's edge.
pub fn ring(out: &mut Vec<Sprite>, b: Box4, radius: f32, width: f32, colour: [u8; 4]) {
    out.push(quad(b, 0.0, 0.0, colour, Shape::Round { size: [b[2], b[3]], radius, ring: width, bottom: colour }));
}

/// A plain box, its edges on the pixel grid's halves as the GPU draws a plain quad.
pub fn fill(out: &mut Vec<Sprite>, b: Box4, colour: [u8; 4]) {
    let [x, y, w, h] = b;
    let corners = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    out.push(Sprite { corners, uv: [[0.0; 2]; 4], local: [[0.0; 2]; 4], clamp: [0.0; 4], image: None, tint: colour, shape: Shape::Plain });
}

/// A disc of radius `r` round `(cx, cy)`, smoothed.
pub fn disc(out: &mut Vec<Sprite>, cx: f32, cy: f32, r: f32, colour: [u8; 4]) {
    round(out, [cx - r, cy - r, 2.0 * r, 2.0 * r], r, colour, colour);
}

/// A bar from `a` to `b`, `half` either side of the line, its ends square: an arrowhead with
/// `tip_half` 0.
pub fn bar(out: &mut Vec<Sprite>, a: [f32; 2], b: [f32; 2], half: f32, tip_half: f32, colour: [u8; 4]) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-3);
    let (ux, uy) = (d[0] / len, d[1] / len);
    let (nx, ny) = (-uy, ux);
    let m = half.max(tip_half) + 1.0;
    let at = |s: f32, t: f32| [a[0] + ux * s + nx * t, a[1] + uy * s + ny * t];
    let corners = [at(-1.0, -m), at(len + 1.0, -m), at(len + 1.0, m), at(-1.0, m)];
    let local = [[-1.0, -m], [len + 1.0, -m], [len + 1.0, m], [-1.0, m]];
    let shape = Shape::Needle { tail: 0.0, tip: len, tail_half: half, tip_half };
    out.push(Sprite { corners, uv: [[0.0; 2]; 4], local, clamp: [0.0; 4], image: None, tint: colour, shape });
}

/// A picture's `src` texels over the box `b`, multiplied by `tint`.
pub fn picture(out: &mut Vec<Sprite>, image: usize, t: &Texture, src: [f32; 4], b: Box4, tint: [u8; 4]) {
    out.push(Sprite::image(image, t, src, [b[0], b[1], b[0] + b[2], b[1] + b[3]], tint));
}

// ---- the CPU's way -------------------------------------------------------------------------------

/// Draw `sprites` into `frame` as the GPU's sprite pass does: each quad's pixels whose centres it
/// covers, the picture read between the clamp's texel centres, the shape's coverage, blended over —
/// or for [`Shape::Glow`] added. `images` are what the sprites' `image` counts into.
pub fn raster(frame: &mut Frame, sprites: &[Sprite], images: &[&Texture]) {
    let (fw, fh) = (frame.width, frame.height);
    for s in sprites {
        let [c0, c1, _, c3] = s.corners;
        let e1 = [c1[0] - c0[0], c1[1] - c0[1]];
        let e2 = [c3[0] - c0[0], c3[1] - c0[1]];
        let det = e1[0] * e2[1] - e1[1] * e2[0];
        if det.abs() < 1e-6 {
            continue;
        }
        let xs = s.corners.map(|c| c[0]);
        let ys = s.corners.map(|c| c[1]);
        let x0 = (xs.iter().copied().fold(f32::MAX, f32::min).floor().max(0.0)) as usize;
        let x1 = (xs.iter().copied().fold(f32::MIN, f32::max).ceil().min(fw as f32)).max(0.0) as usize;
        let y0 = (ys.iter().copied().fold(f32::MAX, f32::min).floor().max(0.0)) as usize;
        let y1 = (ys.iter().copied().fold(f32::MIN, f32::max).ceil().min(fh as f32)).max(0.0) as usize;
        let image = s.image.and_then(|i| images.get(i).copied());
        let lerp = |q: [[f32; 2]; 4], u: f32, v: f32| {
            [q[0][0] + u * (q[1][0] - q[0][0]) + v * (q[3][0] - q[0][0]), q[0][1] + u * (q[1][1] - q[0][1]) + v * (q[3][1] - q[0][1])]
        };
        let tint = s.tint.map(|c| c as f32 / 255.0);
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5 - c0[0], y as f32 + 0.5 - c0[1]);
                let u = (px * e2[1] - py * e2[0]) / det;
                let v = (e1[0] * py - e1[1] * px) / det;
                if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                    continue;
                }
                let raw = match image {
                    Some(t) => sample(t, lerp(s.uv, u, v), s.clamp),
                    None => [1.0; 4],
                };
                let mut c = [raw[0] * tint[0], raw[1] * tint[1], raw[2] * tint[2], raw[3] * tint[3]];
                let p = lerp(s.local, u, v);
                let cover = match s.shape {
                    Shape::Plain | Shape::Glow => 1.0,
                    Shape::Disc { radius } => f32::from(p[0] * p[0] + p[1] * p[1] <= radius * radius),
                    Shape::Capsule { size, split, right } => {
                        let rad = size[0].min(size[1]) * 0.5;
                        let (a, b) = if size[0] >= size[1] { ([rad, rad], [size[0] - rad, rad]) } else { ([rad, rad], [rad, size[1] - rad]) };
                        let q = [p[0].clamp(a[0].min(b[0]), a[0].max(b[0])), p[1].clamp(a[1].min(b[1]), a[1].max(b[1]))];
                        if p[0] >= split {
                            c = right.map(|v| v as f32 / 255.0);
                        }
                        (rad - ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt() + 0.5).clamp(0.0, 1.0)
                    }
                    Shape::Needle { tail, tip, tail_half, tip_half } => {
                        let k = (p[0] + tail) / (tail + tip);
                        let half = tail_half + k * (tip_half - tail_half);
                        let inside = (half - p[1].abs()).min((p[0] + tail).min(tip - p[0]));
                        (inside + 0.5).clamp(0.0, 1.0)
                    }
                    Shape::Round { size, radius, ring, bottom } => {
                        let k = (p[1] / size[1].max(1.0)).clamp(0.0, 1.0);
                        let b = bottom.map(|v| v as f32 / 255.0);
                        c = std::array::from_fn(|i| tint[i] + (b[i] - tint[i]) * k);
                        let d = round_distance(p, size, radius);
                        let inside = if ring > 0.0 { (-d).min(d + ring) } else { -d };
                        (inside + 0.5).clamp(0.0, 1.0)
                    }
                    Shape::Text { range } => {
                        c = tint;
                        ((raw[3] - 0.5) * 2.0 * range + 0.5).clamp(0.0, 1.0)
                    }
                };
                let a = c[3] * cover;
                if a <= 0.0 {
                    continue;
                }
                let at = 3 * (y * fw + x);
                let px = &mut frame.color[at..at + 3];
                for i in 0..3 {
                    let d = px[i] as f32 / 255.0;
                    let out = if s.shape == Shape::Glow { d + c[i] * a } else { d + (c[i] - d) * a };
                    px[i] = (out.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                }
            }
        }
    }
}

/// A rounded box's signed distance at `p`, negative inside.
fn round_distance(p: [f32; 2], size: [f32; 2], radius: f32) -> f32 {
    let half = [size[0] * 0.5, size[1] * 0.5];
    let rad = radius.min(half[0]).min(half[1]).max(0.0);
    let q = [(p[0] - half[0]).abs() - (half[0] - rad), (p[1] - half[1]).abs() - (half[1] - rad)];
    let out = (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt();
    out + q[0].max(q[1]).min(0.0) - rad
}

/// A texel read as the GPU's sampler reads it: between the clamp's texel centres, bilinear.
fn sample(t: &Texture, uv: [f32; 2], clamp: [f32; 4]) -> [f32; 4] {
    if t.width == 0 || t.height == 0 {
        return [0.0; 4];
    }
    let x = (uv[0] - 0.5).clamp(clamp[0], clamp[2].max(clamp[0]));
    let y = (uv[1] - 0.5).clamp(clamp[1], clamp[3].max(clamp[1]));
    let (xf, yf) = (x.floor(), y.floor());
    let (fx, fy) = (x - xf, y - yf);
    let at = |cx: f32, cy: f32| {
        let cx = (cx as isize).clamp(0, t.width as isize - 1) as usize;
        let cy = (cy as isize).clamp(0, t.height as isize - 1) as usize;
        let i = 4 * (cy * t.width + cx);
        [t.rgba[i], t.rgba[i + 1], t.rgba[i + 2], t.rgba[i + 3]].map(|v| v as f32 / 255.0)
    };
    let (a, b, c, d) = (at(xf, yf), at(xf + 1.0, yf), at(xf, yf + 1.0), at(xf + 1.0, yf + 1.0));
    std::array::from_fn(|i| {
        let top = a[i] + (b[i] - a[i]) * fx;
        let bot = c[i] + (d[i] - c[i]) * fx;
        top + (bot - top) * fy
    })
}
