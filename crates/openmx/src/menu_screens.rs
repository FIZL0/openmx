//! The front end's screens (P118), `docs/menu-concept.html` drawn in our own ink: the disc's lit
//! backdrop and logo, and everything else — the dark column, the pills, the tabs, the rows, the side
//! panels, the title over the black band, the hint tab — vector shapes and type at the frame's
//! pixels ([`super::ink`]). One layout makes the sprites; the GPU draws them as they are and
//! [`super::ink::raster`] draws them into a software frame.
//!
//! The layout is the concept's stage: 80 by 45 em, an em a frame's width over 80, centred in a
//! frame of another shape (the smaller of its width over 80 and its height over 45). The backdrop,
//! the column and the band reach the frame's edges whatever its shape.

use super::ink::{self, Box4, SLANT, Weight};
use super::*;

const CHALK: [u8; 4] = [0xf5, 0xf0, 0xe8, 255];
const BONE: [u8; 4] = [0xd8, 0xcf, 0xc2, 255];
const DUST: [u8; 4] = [0x9c, 0x8f, 0x80, 255];
const INK: [u8; 4] = [5, 3, 2, 255];
/// The dark column, `rgba(18,9,4,.74)`.
const STRIP: [u8; 4] = [18, 9, 4, 189];
/// The lit pill's gradient, top and bottom.
const LIT_TOP: [u8; 4] = [0xb3, 0x1a, 0x23, 255];
const LIT_BOTTOM: [u8; 4] = [0x5a, 0x07, 0x0c, 255];
const MAROON: [u8; 4] = [0x8e, 0x0f, 0x17, 255];
/// The hard drop shadow under lit things, `rgba(0,0,0,.33)`.
const SHADOW: [u8; 4] = [0, 0, 0, 84];
/// A label's own shadow, `rgba(0,0,0,.6)`.
const TEXT_SHADOW: [u8; 4] = [0, 0, 0, 153];
const HEADING: [u8; 4] = [0xe9, 0xb9, 0x8d, 255];
const PAPER: [u8; 4] = [0xff, 0xf3, 0xe6, 255];
const EYEBROW: [u8; 4] = [0xff, 0xe0, 0xc4, 217];
const PEACH: [u8; 4] = [0xff, 0xcf, 0x9e, 255];
const TIP: [u8; 4] = [0xff, 0xd9, 0xb8, 217];
const NOTE: [u8; 4] = [0xff, 0xe7, 0xcf, 255];
/// A chip's or a note's glass, `rgba(0,0,0,.42)`.
const GLASS: [u8; 4] = [0, 0, 0, 107];
const RIM: [u8; 4] = [255, 255, 255, 46];
const SHOULDER: [u8; 4] = [0x3a, 0x33, 0x30, 255];
const PAD_BODY: [u8; 4] = [0x1b, 0x17, 0x16, 255];
const PAD_PART: [u8; 4] = [0x2e, 0x28, 0x26, 255];
/// The orange the disc's grey background gradient is drawn in, and the light added over it.
const ORANGE: [u8; 4] = [210, 112, 52, 255];
const GLOW: [u8; 4] = [255, 150, 80, 96];
/// The pause over the ride, and the splash's still.
const PAUSE_DIM: [u8; 4] = [0, 0, 0, 140];
const SPLASH_DIM: [u8; 4] = [110, 110, 110, 255];

/// A capital's height, in ems of its type.
const CAP: f32 = 0.73;
/// The stage, in ems.
const STAGE: (f32, f32) = (80.0, 45.0);
/// The black band along the bottom.
const BAND: f32 = 4.6;
/// Where the Garage stands its bike and rider (P213): right of the plate, over the side panel, in
/// the stage's top right as the original's model stands in its screen's — x, y, width, height.
const GARAGE_STAGE: [f32; 4] = [59.0, 1.6, 20.6, 23.2];
/// The main list's type, and how far a pill reaches past its label either side, in ems of the
/// stage ([`PILL_PAD`] in the stage's ems).
const ITEM: f32 = 1.7;
/// A row of an option page, and the gap under it.
const ROW_H: f32 = 2.55;
const ROW_GAP: f32 = 0.2;
const HEAD_H: f32 = 2.0;

/// The face buttons and the others the hints and the pad draw.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Btn {
    Cross,
    Circle,
    Square,
    Triangle,
    Start,
    L1R1,
    /// The d-pad's left and right.
    LeftRight,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Centre,
    Right,
}

/// Lays sprites out on the stage.
pub(super) struct Pen<'a> {
    art: &'a Art,
    out: &'a mut Vec<Sprite>,
    base: usize,
    em: f32,
    o: [f32; 2],
    size: [f32; 2],
    /// Everything drawn is this opaque (a dimmed row).
    fade: f32,
    pub(super) parts: Vec<Part>,
}

/// The stage in a `size` frame: an em's pixels, and where the stage's corner is.
pub(super) fn fit(size: (usize, usize)) -> (f32, [f32; 2]) {
    let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let em = (w / STAGE.0).min(h / STAGE.1);
    (em, [(w - STAGE.0 * em) / 2.0, (h - STAGE.1 * em) / 2.0])
}

/// [`GARAGE_STAGE`] in a `size` frame's pixels: x, y, width, height.
pub(super) fn garage_stage(size: (usize, usize)) -> [f32; 4] {
    let (em, o) = fit(size);
    let [x, y, w, h] = GARAGE_STAGE;
    [o[0] + x * em, o[1] + y * em, w * em, h * em]
}

impl<'a> Pen<'a> {
    pub(super) fn new(art: &'a Art, out: &'a mut Vec<Sprite>, base: usize, size: (usize, usize)) -> Pen<'a> {
        let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let (em, o) = fit(size);
        Pen { art, out, base, em, o, size: [w, h], fade: 1.0, parts: Vec::new() }
    }

    // ---- coordinates -------------------------------------------------------------------------

    fn b(&self, b: Box4) -> Box4 {
        [self.o[0] + b[0] * self.em, self.o[1] + b[1] * self.em, b[2] * self.em, b[3] * self.em]
    }

    /// The frame's edges, in the stage's ems.
    fn left(&self) -> f32 {
        -self.o[0] / self.em
    }
    fn right(&self) -> f32 {
        (self.size[0] - self.o[0]) / self.em
    }
    fn top(&self) -> f32 {
        -self.o[1] / self.em
    }
    fn bottom(&self) -> f32 {
        (self.size[1] - self.o[1]) / self.em
    }
    fn whole(&self) -> Box4 {
        [self.left(), self.top(), self.right() - self.left(), self.bottom() - self.top()]
    }

    fn c(&self, c: [u8; 4]) -> [u8; 4] {
        [c[0], c[1], c[2], (c[3] as f32 * self.fade).round() as u8]
    }

    // ---- pictures ----------------------------------------------------------------------------

    /// A texture by name and its place in [`Art::images`].
    fn texture(&self, name: &str) -> Option<(usize, &'a Texture)> {
        let at = TEXTURES.iter().position(|n| *n == name)?;
        Some((self.base + at, self.art.textures.get(name)?))
    }

    fn atlas(&self) -> usize {
        self.base + TEXTURES.len()
    }

    /// The pick's track `at`'s `i`th picture (P70) — a disc track's still or a mod's picture — and
    /// its place in [`Art::images`].
    fn track_picture(&self, menu: &Menu, at: usize, i: usize) -> Option<(usize, &'a Texture)> {
        let image = self.art.track_image(menu.track_at(at).0, i)?;
        let all = self.art.images();
        all.get(image).map(|t| (self.base + image, *t))
    }

    /// The texture's middle, the box's shape, over the box.
    fn cover(&mut self, image: usize, t: &Texture, b: Box4, tint: [u8; 4]) {
        let (w, h) = (t.width as f32, t.height as f32);
        let fit = b[2] / b[3].max(1e-3);
        let src = if w / h > fit {
            let cw = h * fit;
            [(w - cw) / 2.0, 0.0, (w + cw) / 2.0, h]
        } else {
            let ch = w / fit;
            [0.0, (h - ch) / 2.0, w, (h + ch) / 2.0]
        };
        let pb = self.b(b);
        let tint = self.c(tint);
        ink::picture(self.out, image, t, src, pb, tint);
    }

    fn picture(&mut self, name: &str, b: Box4, tint: [u8; 4], glow: bool) {
        let Some((image, t)) = self.texture(name) else { return };
        let all = [0.0, 0.0, t.width as f32, t.height as f32];
        let pb = self.b(b);
        let tint = self.c(tint);
        ink::picture(self.out, image, t, all, pb, tint);
        if glow {
            if let Some(s) = self.out.last_mut() {
                s.shape = Shape::Glow;
            }
        }
    }

    // ---- shapes ------------------------------------------------------------------------------

    fn fill(&mut self, b: Box4, colour: [u8; 4]) {
        let (pb, c) = (self.b(b), self.c(colour));
        ink::fill(self.out, pb, c);
    }

    fn round(&mut self, b: Box4, radius: f32, top: [u8; 4], bottom: [u8; 4]) {
        let (pb, t, bo) = (self.b(b), self.c(top), self.c(bottom));
        ink::round(self.out, pb, radius * self.em, t, bo);
    }

    fn ring(&mut self, b: Box4, radius: f32, width: f32, colour: [u8; 4]) {
        let (pb, c) = (self.b(b), self.c(colour));
        ink::ring(self.out, pb, radius * self.em, width * self.em, c);
    }

    fn disc(&mut self, x: f32, y: f32, r: f32, colour: [u8; 4]) {
        let c = self.c(colour);
        ink::disc(self.out, self.o[0] + x * self.em, self.o[1] + y * self.em, r * self.em, c);
    }

    fn line(&mut self, a: [f32; 2], b: [f32; 2], half: f32, tip: f32, colour: [u8; 4]) {
        let p = |q: [f32; 2]| [self.o[0] + q[0] * self.em, self.o[1] + q[1] * self.em];
        let (pa, pb, c) = (p(a), p(b), self.c(colour));
        ink::bar(self.out, pa, pb, half * self.em, tip * self.em, c);
    }

    /// An arrowhead `size` long pointing from `(x, y)` along `dir` (x, y).
    fn arrow(&mut self, x: f32, y: f32, dir: [f32; 2], size: f32, colour: [u8; 4]) {
        let a = [x - dir[0] * size / 2.0, y - dir[1] * size / 2.0];
        let b = [x + dir[0] * size / 2.0, y + dir[1] * size / 2.0];
        self.line(a, b, size * 0.55, 0.0, colour);
    }

    /// A stadium: the lit pill with its rim and hard shadow, or a box of glass.
    fn pill(&mut self, b: Box4, shadow: f32, rim: f32) {
        let r = b[3] / 2.0;
        self.round([b[0] + shadow, b[1] + shadow * 1.08, b[2], b[3]], r, SHADOW, SHADOW);
        self.round(b, r, LIT_TOP, LIT_BOTTOM);
        self.ring(b, r, rim, CHALK);
    }

    // ---- type --------------------------------------------------------------------------------

    fn wide(&self, w: Weight, s: &str, size: f32) -> f32 {
        ink::ink().measure(w, s, size)
    }

    /// `s` with its left, centre or right at `x` and its baseline at `y`, `size` ems; its box in
    /// frame pixels.
    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, w: Weight, s: &str, x: f32, y: f32, size: f32, slant: f32, colour: [u8; 4], align: Align) -> Box4 {
        let width = self.wide(w, s, size);
        let left = match align {
            Align::Left => x,
            Align::Centre => x - width / 2.0,
            Align::Right => x - width,
        };
        let px = size * self.em;
        let (atlas, c) = (self.atlas(), self.c(colour));
        let (lx, ly) = (self.o[0] + left * self.em, self.o[1] + y * self.em);
        ink::ink().text(self.out, atlas, w, s, lx, ly, px, slant, c);
        [lx, ly - CAP * px, width * self.em, CAP * px]
    }

    /// [`Pen::text`] over its own shadow `drop` ems down and right.
    #[allow(clippy::too_many_arguments)]
    fn shadowed(&mut self, w: Weight, s: &str, x: f32, y: f32, size: f32, slant: f32, colour: [u8; 4], align: Align, drop: [f32; 2], shade: [u8; 4]) -> Box4 {
        self.text(w, s, x + drop[0] * size, y + drop[1] * size, size, slant, shade, align);
        self.text(w, s, x, y, size, slant, colour, align)
    }

    /// The baseline that centres a capital of `size` on `mid`.
    fn mid(mid: f32, size: f32) -> f32 {
        mid + CAP * size / 2.0
    }

    /// `s` cut to `max` ems wide, with an ellipsis where it was cut.
    fn fit(&self, w: Weight, s: &str, size: f32, max: f32) -> String {
        if self.wide(w, s, size) <= max {
            return s.to_string();
        }
        let mut t: String = s.to_string();
        while !t.is_empty() && self.wide(w, &format!("{t}…"), size) > max {
            t.pop();
        }
        format!("{}…", t.trim_end())
    }

    /// `s` broken into lines at most `max` ems wide.
    fn wrap(&self, w: Weight, s: &str, size: f32, max: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in s.split_whitespace() {
            let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && self.wide(w, &next, size) > max {
                lines.push(std::mem::replace(&mut line, word.to_string()));
            } else {
                line = next;
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    /// A paragraph from `top`, lines `lead` of the type apart; where it ends.
    #[allow(clippy::too_many_arguments)]
    fn para(&mut self, s: &str, x: f32, top: f32, max: f32, size: f32, lead: f32, colour: [u8; 4]) -> f32 {
        let mut y = top;
        for line in self.wrap(Weight::Bold, s, size, max) {
            let base = y + size * (lead + CAP) / 2.0;
            self.text(Weight::Bold, &line, x, base, size, 0.0, colour, Align::Left);
            y += size * lead;
        }
        y
    }

    // ---- the furniture -----------------------------------------------------------------------

    /// The disc's grey gradient in orange over the whole frame and its radial light added over it.
    fn backdrop(&mut self) {
        let whole = self.whole();
        self.picture("background", whole, ORANGE, false);
        self.picture("frontend_background_radial", [(STAGE.0 - 64.0) / 2.0, -5.0, 64.0, 56.0], GLOW, true);
    }

    /// The dark column from the frame's top to the band.
    fn strip(&mut self, x: f32, w: f32) {
        let top = self.top();
        let b = [x, top, w, STAGE.1 - BAND - top];
        self.fill(b, STRIP);
        let pb = self.b(b);
        self.parts.push(Part::Plate(pb));
    }

    /// The disc's logo and its shadow, full size or small.
    fn logo(&mut self, small: bool) {
        let b = if small { [3.6, 1.8, 28.0, 3.5] } else { [2.4, 1.6, 40.0, 5.0] };
        self.picture("mainmenu_logo_shadow", [b[0] + 0.3, b[1] + 0.35, b[2], b[3]], [0, 0, 0, 90], false);
        self.picture("mainmenu_logo", b, CHALK, false);
        let pb = self.b(b);
        self.parts.push(Part::Logo(pb));
    }

    /// The screen's name, slanted over the band's left end.
    fn title(&mut self, title: &str) {
        let size = 3.1;
        self.shadowed(Weight::Heavy, title, 4.5, STAGE.1 - BAND - 1.2, size, SLANT, CHALK, Align::Left, [0.08, 0.1], [0, 0, 0, 102]);
    }

    /// The black band and its line: whose ride this is, a note, or the lobby's word.
    fn band(&mut self, line: &[(&str, bool)]) {
        let (l, r, b) = (self.left(), self.right(), self.bottom());
        self.fill([l, STAGE.1 - BAND, r - l, b - STAGE.1 + BAND], INK);
        let base = Pen::mid(STAGE.1 - BAND / 2.0, 1.5);
        let mut x = 5.0;
        for (s, bold) in line {
            let (w, c) = if *bold { (Weight::Heavy, CHALK) } else { (Weight::Bold, BONE) };
            let s = self.fit(w, s, 1.5, (STAGE.0 - 6.0 - x).max(0.0));
            self.text(w, &s, x, base, 1.5, 0.0, c, Align::Left);
            x += self.wide(w, &s, 1.5);
        }
    }

    /// A button `d` ems across centred on `(x, y)`.
    fn button(&mut self, b: Btn, x: f32, y: f32, d: f32) {
        let r = d / 2.0;
        let (colour, stroke) = match b {
            Btn::Cross => ([0x4a, 0x7b, 0xe0, 255], d * 0.07),
            Btn::Circle => ([0xe0, 0x47, 0x3f, 255], d * 0.07),
            Btn::Square => ([0xd8, 0x72, 0xb8, 255], d * 0.07),
            Btn::Triangle => ([0x3c, 0xbf, 0x93, 255], d * 0.07),
            Btn::Start | Btn::L1R1 => {
                let word = if b == Btn::Start { "START" } else { "L1" };
                let size = d * 0.5;
                let w = self.wide(Weight::Heavy, word, size) + d * 0.6;
                match b {
                    Btn::Start => {
                        self.round([x - w / 2.0, y - r * 0.8, w, d * 0.8], d * 0.25, SHOULDER, SHOULDER);
                        self.text(Weight::Heavy, word, x, Pen::mid(y, size), size, 0.0, CHALK, Align::Centre);
                    }
                    _ => {
                        for (i, word) in ["L1", "R1"].iter().enumerate() {
                            let cx = x + (i as f32 - 0.5) * (w + d * 0.15);
                            self.round([cx - w / 2.0, y - r * 0.8, w, d * 0.8], d * 0.25, SHOULDER, SHOULDER);
                            self.text(Weight::Heavy, word, cx, Pen::mid(y, size), size, 0.0, CHALK, Align::Centre);
                        }
                    }
                }
                return;
            }
            Btn::LeftRight => {
                let w = d * 0.8;
                for (i, dir) in [-1.0f32, 1.0].into_iter().enumerate() {
                    let cx = x + (i as f32 - 0.5) * (w + d * 0.15);
                    self.round([cx - w / 2.0, y - r * 0.8, w, d * 0.8], d * 0.25, SHOULDER, SHOULDER);
                    self.arrow(cx, y, [dir, 0.0], d * 0.4, CHALK);
                }
                return;
            }
        };
        self.disc(x, y, r, colour);
        let k = d * 0.32;
        match b {
            Btn::Cross => {
                self.line([x - k * 0.75, y - k * 0.75], [x + k * 0.75, y + k * 0.75], stroke, stroke, CHALK);
                self.line([x + k * 0.75, y - k * 0.75], [x - k * 0.75, y + k * 0.75], stroke, stroke, CHALK);
            }
            Btn::Circle => self.ring([x - k * 0.95, y - k * 0.95, k * 1.9, k * 1.9], k, stroke * 1.4, CHALK),
            Btn::Square => self.ring([x - k * 0.8, y - k * 0.8, k * 1.6, k * 1.6], 0.0, stroke * 1.4, CHALK),
            _ => {
                let (t, l, rr) = ([x, y - k * 0.95], [x - k * 0.95, y + k * 0.7], [x + k * 0.95, y + k * 0.7]);
                self.line(t, l, stroke * 0.7, stroke * 0.7, CHALK);
                self.line(l, rr, stroke * 0.7, stroke * 0.7, CHALK);
                self.line(rr, t, stroke * 0.7, stroke * 0.7, CHALK);
            }
        }
    }

    /// How wide a button is drawn `d` across.
    fn button_wide(&self, b: Btn, d: f32) -> f32 {
        match b {
            Btn::Start => self.wide(Weight::Heavy, "START", d * 0.5) + d * 0.6,
            Btn::L1R1 => 2.0 * (self.wide(Weight::Heavy, "L1", d * 0.5) + d * 0.6) + d * 0.15,
            Btn::LeftRight => 2.0 * d * 0.8 + d * 0.15,
            _ => d,
        }
    }

    /// The angled hint tab over the band's right end, its hints right to left from the frame's
    /// edge. The tab ends square at the frame's edge, and the hints shrink to fit a narrow one, so a
    /// split's pause keeps them in its own view (P172).
    fn hints(&mut self, hints: &[(Btn, &str)]) {
        let spacing = 1.6 * hints.len().saturating_sub(1) as f32;
        let widths_at = |pen: &Self, size: f32| -> Vec<f32> {
            let (d, gap) = (1.15 * size, 0.45 * size);
            hints.iter().map(|(b, s)| pen.button_wide(*b, d) + gap + pen.wide(Weight::Bold, s, size)).collect()
        };
        let right = STAGE.0.min(self.right());
        // The tab's slanted end leans 3 ems over its foot, and the hints sit 4 in from it.
        let room = right - 2.0 - 4.0 - 3.0 - self.left().max(0.0) - spacing;
        let mut size = 1.55;
        let full: f32 = widths_at(self, size).iter().sum();
        if full > room && room > 0.0 {
            size *= room / full;
        }
        let d = 1.15 * size;
        let gap = 0.45 * size;
        let widths = widths_at(self, size);
        let content: f32 = widths.iter().sum::<f32>() + spacing;
        let x0 = right - 2.0 - content - 4.0;
        let (top, h) = (STAGE.1 - BAND - 3.4, 3.4);
        let pb = self.b([x0, top, self.right() - x0, h]);
        let c = self.c(INK);
        ink::tab(self.out, pb, 3.0 / 3.4, c);
        let mid = top + h / 2.0;
        let mut x = x0 + 4.0;
        for ((b, s), w) in hints.iter().zip(widths) {
            let bw = self.button_wide(*b, d);
            self.button(*b, x + bw / 2.0, mid, d);
            self.text(Weight::Bold, s, x + bw + gap, Pen::mid(mid, size), size, 0.0, CHALK, Align::Left);
            x += w + 1.6;
        }
    }

    /// A chip: its words, the starred ones bold and peach.
    fn chip(&mut self, s: &str, x: f32, y: f32) -> f32 {
        let size = 1.05;
        let parts: Vec<(bool, &str)> = s.split('*').enumerate().map(|(i, p)| (i % 2 == 1, p)).collect();
        let w: f32 = parts.iter().map(|(b, p)| self.wide(if *b { Weight::Heavy } else { Weight::Bold }, p, size)).sum();
        let (pw, ph) = (w + 1.6 * size, size * 1.9);
        self.round([x, y, pw, ph], ph / 2.0, GLASS, GLASS);
        self.ring([x, y, pw, ph], ph / 2.0, 0.08 * size, RIM);
        let mut at = x + 0.8 * size;
        let base = Pen::mid(y + ph / 2.0, size);
        for (bold, p) in parts {
            let (w, c) = if bold { (Weight::Heavy, PEACH) } else { (Weight::Bold, CHALK) };
            self.text(w, p, at, base, size, 0.0, c, Align::Left);
            at += self.wide(w, p, size);
        }
        pw
    }

    /// Chips in a row from `(x, y)`, wrapping at `max`; where they end.
    fn chips(&mut self, chips: &[String], x: f32, y: f32, max: f32) -> f32 {
        if chips.is_empty() {
            return y;
        }
        let (mut cx, mut cy) = (x, y);
        let h = 1.05 * 1.9;
        for c in chips {
            let size = 1.05;
            let w: f32 = c.split('*').map(|p| self.wide(Weight::Heavy, p, size)).sum::<f32>() + 1.6 * size;
            if cx > x && cx + w > x + max {
                (cx, cy) = (x, cy + h + 0.5);
            }
            cx += self.chip(c, cx, cy) + 0.5;
        }
        cy + h
    }

    /// A box of glass with a line or two in it; where it ends.
    fn note(&mut self, s: &str, x: f32, y: f32, w: f32) -> f32 {
        let size = 1.05;
        let lines = self.wrap(Weight::Bold, s, size, w - 1.8 * size);
        let h = lines.len() as f32 * 1.35 * size + 1.1 * size;
        self.round([x, y, w, h], 0.7 * size, [0, 0, 0, 102], [0, 0, 0, 102]);
        self.para(s, x + 0.9 * size, y + 0.55 * size, w - 1.8 * size, size, 1.35, NOTE);
        y + h
    }

    /// An eyebrow, a slanted heading and a paragraph from `top`; where they end.
    fn heading(&mut self, eyebrow: &str, h: &str, p: &str, x: f32, top: f32, w: f32, size: f32) -> f32 {
        let mut y = top;
        if !eyebrow.is_empty() {
            self.text(Weight::Bold, &eyebrow.to_uppercase(), x, y + CAP, 1.0, 0.0, EYEBROW, Align::Left);
            y += 1.6;
        }
        let h = self.fit(Weight::Heavy, h, size, w);
        self.shadowed(Weight::Heavy, &h, x, y + CAP * size, size, SLANT, CHALK, Align::Left, [0.07, 0.09], [0, 0, 0, 89]);
        y += size * 1.05;
        if !p.is_empty() {
            y = self.para(p, x, y + 0.6, w.min(32.5), 1.25, 1.4, PAPER);
        }
        y
    }

    // ---- lists -------------------------------------------------------------------------------

    /// The main list's look: rows centred on the column, the chosen one on its pill, a dimmed one
    /// faded, each row's type shrunk till its pill fits `room` ems across (V111: the Online screen's
    /// "Network: Internet + LAN" ran off its plate). Returns nothing; the parts say where each went.
    fn list(&mut self, rows: &[(String, bool)], chosen: usize, centre: f32, top: f32, room: f32) {
        // Clear of the slanted title, which rises over the column from the band.
        let bottom = STAGE.1 - BAND - 4.2;
        let full = ITEM * (1.0 + 0.34 + 0.4) + 0.55;
        let pitch = full.min((bottom - top) / rows.len().max(1) as f32);
        let full_size = ITEM.min((pitch - 0.3) / 1.74);
        let h = full_size * 1.74;
        for (i, (row, dim)) in rows.iter().enumerate() {
            let y = top + pitch * i as f32;
            let mid = y + h / 2.0;
            // Measured heavy, lit or not, so a row keeps its size as the cursor passes.
            let across = self.wide(Weight::Heavy, row, full_size) + 2.0 * PILL_PAD * full_size / ITEM;
            let size = if across > room { full_size * room / across } else { full_size };
            let base = Pen::mid(mid, size);
            self.fade = if *dim { 0.42 } else { 1.0 };
            if i == chosen {
                let w = self.wide(Weight::Heavy, row, size);
                let pad = PILL_PAD * size / ITEM;
                let b = [centre - w / 2.0 - pad, mid - size * 0.87, w + 2.0 * pad, size * 1.74];
                self.pill(b, 0.28 * size, 0.1 * size);
                let label = self.text(Weight::Heavy, row, centre, base, size, SLANT, CHALK, Align::Centre);
                let pb = self.b(b);
                self.parts.push(Part::Pill { pill: pb, label });
            } else {
                self.text(Weight::Bold, row, centre, base + 0.06 * size, size, 0.0, TEXT_SHADOW, Align::Centre);
                let label = self.text(Weight::Bold, row, centre, base, size, 0.0, BONE, Align::Centre);
                self.parts.push(Part::Item(label));
            }
        }
        self.fade = 1.0;
    }

    /// An option page's rows from `top` in a column `x` to `x + w`, scrolled so the cursor shows.
    fn rows(&mut self, rows: &[Row], at: usize, x: f32, w: f32, top: f32) {
        let bottom = STAGE.1 - BAND - 0.4;
        let shown = (((bottom - top) / (ROW_H + ROW_GAP)) as usize).max(1);
        let first = at.saturating_sub(shown / 2).min(rows.len().saturating_sub(shown));
        if first > 0 {
            self.arrow(x + w / 2.0, top - 0.6, [0.0, -1.0], 0.7, BONE);
        }
        let mut y = top;
        for (i, r) in rows.iter().enumerate().skip(first).take(shown) {
            let lit = i == at;
            if r.kind == Kind::Heading {
                let base = Pen::mid(y + HEAD_H / 2.0 + 0.2, 1.0);
                self.text(Weight::Bold, &r.label.to_uppercase(), x + 0.4, base, 1.0, 0.0, HEADING, Align::Left);
                y += HEAD_H + ROW_GAP;
                continue;
            }
            self.fade = if r.off.is_some() { 0.4 } else { 1.0 };
            let b = [x, y, w, ROW_H];
            let mid = y + ROW_H / 2.0;
            if lit {
                self.pill(b, 0.25, 0.1);
            }
            let k = 1.35;
            let (kw, kc, ks) = if lit { (Weight::Heavy, CHALK, SLANT) } else { (Weight::Bold, BONE, 0.0) };
            let value_room = self.value_wide(r);
            let label = self.fit(kw, &r.label, k, (w - 2.4 - value_room - 1.0).max(4.0));
            if !lit {
                self.text(kw, &label, x + 1.3, Pen::mid(mid, k) + 0.06 * k, k, ks, TEXT_SHADOW, Align::Left);
            }
            let lb = self.text(kw, &label, x + 1.3, Pen::mid(mid, k), k, ks, kc, Align::Left);
            if lit {
                self.parts.push(Part::Pill { pill: self.b(b), label: lb });
            } else {
                self.parts.push(Part::Item(lb));
            }
            if r.world {
                let lx = x + 1.3 + self.wide(kw, &label, k) + 0.5;
                self.tag("World", lx, mid);
            }
            self.value(r, lit, x + w - 1.1, mid);
            self.fade = 1.0;
            y += ROW_H + ROW_GAP;
        }
        if first + shown < rows.len() {
            self.arrow(x + w / 2.0, y + 0.1, [0.0, 1.0], 0.7, BONE);
        }
    }

    /// A small chalk tag, `1P` or `World`, from `x` centred on `mid`; how wide it is.
    fn tag(&mut self, s: &str, x: f32, mid: f32) -> f32 {
        let size = 1.0;
        let w = self.wide(Weight::Heavy, s, size) + 1.1 * size;
        let h = 1.5 * size;
        self.round([x, mid - h / 2.0, w, h], h / 2.0, CHALK, CHALK);
        self.text(Weight::Heavy, s, x + 0.55 * size, Pen::mid(mid, size), size, 0.0, LIT_BOTTOM, Align::Left);
        w
    }

    /// How wide a row's value is drawn.
    fn value_wide(&self, r: &Row) -> f32 {
        let v = 1.3;
        let text = self.wide(Weight::Bold, &r.value, v);
        match r.kind {
            Kind::Choice | Kind::Player => text + 2.0 * (0.7 * v + 0.45 * v),
            Kind::Slider => 10.0 * 0.48 * v + 0.45 * v + text.max(2.2 * v) + 2.0 * (0.7 * v + 0.45 * v),
            Kind::Opens => text + 1.2 * v,
            Kind::Action if r.value.is_empty() => 1.15 * v,
            _ => text,
        }
    }

    /// A row's value, right-aligned at `right`.
    fn value(&mut self, r: &Row, lit: bool, right: f32, mid: f32) {
        let v = 1.3;
        let base = Pen::mid(mid, v);
        let arrows = matches!(r.kind, Kind::Choice | Kind::Player | Kind::Slider);
        let a = 0.7 * v;
        let mut x = right;
        if arrows && lit {
            self.arrow(x - a / 2.0, mid, [1.0, 0.0], a, CHALK);
        }
        if arrows {
            x -= a + 0.45 * v;
        }
        match r.kind {
            Kind::Slider => {
                let w = self.wide(Weight::Bold, &r.value, v).max(2.2 * v);
                self.text(Weight::Bold, &r.value, x, base, v, 0.0, CHALK, Align::Right);
                x -= w + 0.45 * v;
                let filled = (r.meter.unwrap_or(0.0) * 10.0).round() as usize;
                let (bw, bh, gap) = (0.36 * v, 0.9 * v, 0.12 * v);
                for i in (0..10).rev() {
                    let c = match (i < filled, lit) {
                        (true, true) => CHALK,
                        (true, false) => HEADING,
                        _ => [255, 255, 255, 46],
                    };
                    x -= bw;
                    self.round([x, mid - bh / 2.0, bw, bh], 0.12 * v, c, c);
                    x -= gap;
                }
                x -= 0.45 * v - gap;
            }
            Kind::Opens => {
                self.arrow(x - 0.35 * v, mid, [1.0, 0.0], 0.55 * v, [0xf5, 0xf0, 0xe8, 178]);
                x -= 1.2 * v;
                self.text(Weight::Bold, &r.value, x, base, v, 0.0, CHALK, Align::Right);
            }
            Kind::Action if r.value.is_empty() => self.button(Btn::Cross, x - 0.58 * v, mid, 1.15 * v),
            _ => {
                let w = self.wide(Weight::Bold, &r.value, v);
                self.text(Weight::Bold, &r.value, x, base, v, 0.0, CHALK, Align::Right);
                x -= w + 0.45 * v;
            }
        }
        if arrows && lit {
            self.arrow(x - a / 2.0, mid, [-1.0, 0.0], a, CHALK);
        }
    }

    // ---- the side panel ------------------------------------------------------------------------

    /// An option row's side panel: its name, what it does, why it is dim, every choice.
    fn side(&mut self, menu: &Menu, screen: Screen, page: &[Row], r: &Row, x: f32, top: f32, w: f32) {
        let mut y = top;
        let size = 2.3;
        let h = self.fit(Weight::Heavy, &r.label, size, w - 4.0);
        self.shadowed(Weight::Heavy, &h, x, y + CAP * size, size, SLANT, CHALK, Align::Left, [0.07, 0.09], [0, 0, 0, 89]);
        let per_player = page.iter().any(|p| p.kind == Kind::Player) && r.kind != Kind::Player && !r.world;
        if per_player {
            let hx = x + self.wide(Weight::Heavy, &h, size) + 0.9;
            // whose the modifiers page sets: All Players or one seat's (P134)
            let whose = if screen == Screen::Modifiers { menu.mods_whose() } else { format!("{}P", menu.player() + 1) };
            self.tag(&whose, hx, y + CAP * size / 2.0);
        }
        y += size * 1.1;
        y = self.para(&r.about, x, y + 0.5, w, 1.2, 1.42, PAPER);
        if let Some(why) = &r.off {
            y = self.note(why, x, y + 0.9, w);
        }
        let room = STAGE.1 - BAND - 3.8;
        match r.kind {
            Kind::Choice | Kind::Player if !r.choices.is_empty() => {
                let size = 1.25;
                let pitch = size * 1.35;
                y += 1.1;
                let fits = (((room - y) / pitch) as usize).max(1);
                let now = r.now.unwrap_or(0);
                let first = now.saturating_sub(fits / 2).min(r.choices.len().saturating_sub(fits));
                for (i, choice) in r.choices.iter().enumerate().skip(first).take(fits) {
                    let mid = y + pitch / 2.0;
                    let dot = 0.55 * size;
                    let on = Some(i) == r.now;
                    if on {
                        self.disc(x + dot / 2.0, mid, dot / 2.0 + 0.14 * size, MAROON);
                        self.disc(x + dot / 2.0, mid, dot / 2.0, CHALK);
                    } else {
                        self.ring([x, mid - dot / 2.0, dot, dot], dot / 2.0, 0.12 * size, [255, 255, 255, 140]);
                    }
                    let (cw, cc) = if on { (Weight::Heavy, CHALK) } else { (Weight::Bold, PAPER) };
                    let tx = x + dot + 0.6 * size;
                    let choice = self.fit(cw, choice, size, w - (tx - x));
                    self.text(cw, &choice, tx, Pen::mid(mid, size), size, 0.0, cc, Align::Left);
                    if let Some(tip) = r.tips.get(i).filter(|t| !t.is_empty()) {
                        let ex = tx + self.wide(cw, &choice, size) + 0.4 * size;
                        let tip = self.fit(Weight::Bold, tip, 0.8 * size, (x + w - ex).max(0.0));
                        self.text(Weight::Bold, &tip, ex, Pen::mid(mid, size), 0.8 * size, 0.0, TIP, Align::Left);
                    }
                    y += pitch;
                }
            }
            Kind::Slider => {
                let size = 4.2;
                self.text(Weight::Heavy, &r.value, x, y + 0.25 * size + CAP * size, size, 0.176, CHALK, Align::Left);
                y += size * 1.25;
            }
            _ => {}
        }
        if let Some(note) = &r.note {
            y = self.note(note, x, y + 0.9, w);
        }
        if screen == Screen::Settings && menu.tab(screen) == pages::TAB_CONTROLS {
            // The keyboard under the Player row, or under the layout row on a tab of one seat (P81),
            // for a player riding it; a player on a pad gets the pad (P178). And under the keyboard's
            // own row (P207).
            let layout_row = matches!(r.key, Key::BikeLayout | Key::Keyboard);
            if menu.controls_on_keys() && (r.kind == Kind::Player || (layout_row && !menu.many_seats())) {
                self.keys(menu, x, y + 1.0, w);
            } else if room - y > 10.0 {
                self.pad(menu, x, y + 0.6, w.min((room - y - 0.6) * 2.0));
            }
        }
    }

    /// The keyboard's keys under the player's layout, as the bindings file has them.
    fn keys(&mut self, menu: &Menu, x: f32, top: f32, w: f32) {
        let p = menu.control_seat();
        let layout = menu.controls.layouts[p];
        let rows = &menu.keyboard[keyboard_at(layout, menu.controls.presets[p], menu.controls.home_row_keys)];
        if rows.is_empty() {
            return;
        }
        let size = 1.05;
        let mut y = top;
        self.text(Weight::Bold, &format!("{}P keyboard", p + 1).to_uppercase(), x, y + CAP, 1.0, 0.0, HEADING, Align::Left);
        y += 1.7;
        let mid = x + w * 0.5;
        for (what, keys) in rows {
            if y > STAGE.1 - BAND - 4.0 {
                break;
            }
            let base = y + CAP * size;
            let what = self.fit(Weight::Bold, what, size, w * 0.5 - 0.5);
            self.text(Weight::Bold, &what, mid - 0.5, base, size, 0.0, PAPER, Align::Right);
            let keys = self.fit(Weight::Heavy, keys, size, w * 0.5 - 0.5);
            self.text(Weight::Heavy, &keys, mid + 0.5, base, size, 0.0, CHALK, Align::Left);
            y += size * 1.45;
        }
    }

    /// The pad, its controls labelled for the player's layout and throttle: the concept's 300x150
    /// drawing, `w` ems wide from `(x, top)`.
    fn pad(&mut self, menu: &Menu, x: f32, top: f32, w: f32) {
        let k = w / 300.0;
        let p = |px: f32, py: f32| [x + px * k, top + py * k];
        let seat = menu.control_seat();
        let new = menu.controls.layouts[seat] == Layout::New;
        let mut l = [("l2", "Look Back"), ("l1", "Clutch"), ("r1", "Stunt Modifier"), ("r2", "Thrill Camera"), ("tri", "Stunts"), ("o", "Stunts"), ("sq", "Brake"), ("x", "Gas")];
        let set = |l: &mut [(&str, &'static str); 8], key: &str, v: &'static str| {
            if let Some(e) = l.iter_mut().find(|e| e.0 == key) {
                e.1 = v;
            }
        };
        if new {
            set(&mut l, "r2", "Gas (pressure)");
            set(&mut l, "l2", "Brake (pressure)");
            set(&mut l, "sq", "Look Back");
            set(&mut l, "x", "Stunts");
        }
        if menu.controls.throttle == Gas::R2 {
            set(&mut l, "r2", "Gas");
        }
        let say = |key: &str| l.iter().find(|e| e.0 == key).map_or("", |e| e.1);
        // The body: two grips and the bridge, rounded boxes in the drawing's places.
        let body = |pen: &mut Pen, b: [f32; 4], r: f32| {
            let o = p(b[0], b[1]);
            pen.round([o[0], o[1], b[2] * k, b[3] * k], r * k, CHALK, CHALK);
            pen.round([o[0] + 2.2 * k, o[1] + 2.2 * k, (b[2] - 4.4) * k, (b[3] - 4.4) * k], (r - 2.2) * k, PAD_BODY, PAD_BODY);
        };
        body(self, [26.0, 60.0, 76.0, 78.0], 34.0);
        body(self, [198.0, 60.0, 76.0, 78.0], 34.0);
        body(self, [62.0, 40.0, 176.0, 68.0], 30.0);
        self.round([p(30.0, 64.0)[0], p(30.0, 64.0)[1], 68.0 * k, 70.0 * k], 32.0 * k, PAD_BODY, PAD_BODY);
        self.round([p(202.0, 64.0)[0], p(202.0, 64.0)[1], 68.0 * k, 70.0 * k], 32.0 * k, PAD_BODY, PAD_BODY);
        for (bx, by) in [(72.0, 30.0), (194.0, 30.0), (74.0, 20.0), (196.0, 20.0)] {
            let o = p(bx, by);
            let bw = if by < 25.0 { 30.0 } else { 34.0 };
            self.round([o[0], o[1], bw * k, 9.0 * k], 4.0 * k, PAD_PART, PAD_PART);
            self.ring([o[0], o[1], bw * k, 9.0 * k], 4.0 * k, k, CHALK);
        }
        // The d-pad.
        let c = p(94.0, 70.0);
        self.fill([c[0] - 12.0 * k, c[1] - 4.0 * k, 24.0 * k, 8.0 * k], SHOULDER);
        self.fill([c[0] - 4.0 * k, c[1] - 12.0 * k, 8.0 * k, 24.0 * k], SHOULDER);
        for (bx, by, b) in [(222.0, 58.0, Btn::Triangle), (236.0, 72.0, Btn::Circle), (208.0, 72.0, Btn::Square), (222.0, 86.0, Btn::Cross)] {
            let o = p(bx, by);
            self.button(b, o[0], o[1], 12.0 * k);
        }
        for sx in [122.0, 178.0] {
            let o = p(sx, 100.0);
            self.disc(o[0], o[1], 13.0 * k, CHALK);
            self.disc(o[0], o[1], 12.0 * k, PAD_PART);
        }
        let size = 9.0 * k;
        let lead = [0xf5, 0xf0, 0xe8, 128];
        let label = |pen: &mut Pen, a: f32, b: f32, y: f32, s: &str, right: bool| {
            pen.line(p(a, y), p(b, y), 0.4 * k, 0.4 * k, lead);
            let (tx, al) = if right { (p(b + 2.0, y)[0], Align::Left) } else { (p(a - 2.0, y)[0], Align::Right) };
            pen.text(Weight::Heavy, s, tx, Pen::mid(p(0.0, y)[1], size), size, 0.0, CHALK, al);
        };
        label(self, 30.0, 74.0, 24.0, say("l2"), false);
        label(self, 30.0, 72.0, 37.0, say("l1"), false);
        label(self, 226.0, 270.0, 24.0, say("r2"), true);
        label(self, 228.0, 270.0, 37.0, say("r1"), true);
        label(self, 242.0, 270.0, 58.0, say("tri"), true);
        label(self, 242.0, 270.0, 72.0, say("o"), true);
        label(self, 228.0, 270.0, 86.0, say("x"), true);
        label(self, 30.0, 82.0, 70.0, "Steer + Lean", false);
        label(self, 30.0, 109.0, 100.0, "Steer + Lean", false);
        let bottom = format!("Camera  ·  {} on Square", say("sq"));
        self.text(Weight::Heavy, &bottom, p(150.0, 0.0)[0], p(0.0, 136.0)[1] + CAP * size / 2.0, size, 0.0, CHALK, Align::Centre);
    }
}

// ---- the screens -----------------------------------------------------------------------------------

/// What the main menu's rows say beside the list.
fn main_panel(menu: &Menu, at: usize) -> (&'static str, String, Vec<String>) {
    let packs = menu.packs();
    match at {
        MAIN_FREE => ("Free Ride", "Pick a track and ride it on your own. Disc tracks first, then everything in the mods folder.".into(), Vec::new()),
        MAIN_LOCAL => (
            "Local Game",
            "Up to eight players on one machine. Every pad presses its own way into a seat, then you pick how the screen splits.".into(),
            match menu.devices.len() {
                0 => vec![],
                1 => vec!["*1* pad plugged in".into()],
                n => vec![format!("*{n}* pads plugged in")],
            },
        ),
        MAIN_ONLINE => (
            "Online",
            "Ride with friends over the internet or the LAN. Quickplay finds an open ride; Host makes one and gives you a code to share.".into(),
            vec![if menu.lobby.lan { "LAN only".into() } else { "Internet and LAN".into() }],
        ),
        MAIN_GARAGE => ("Garage", "Name, number, bike and gear, per player.".into(), Vec::new()),
        MAIN_MODS => {
            let (on, all) = menu.packs_on();
            let tracks: usize = packs.iter().map(|p| p.tracks).sum();
            let mut chips = vec![format!("*{on}* of {all} packs on")];
            if !menu.settings.mods {
                chips = vec!["The mods folder is *Off*".into()];
            } else if tracks > 0 {
                chips.push(format!("{tracks} tracks"));
            }
            ("Mods", "Tracks, maps and music from the mods folder. Switch whole packs on and off.".into(), chips)
        }
        MAIN_SETTINGS => ("Settings", "Ride, Display, Audio, Controls and Camera, one tab each.".into(), vec!["L1 / R1 between tabs".into()]),
        MAIN_CONTROLLERS => (
            "Controllers",
            "Which pad, keyboard or touch pad drives which seat. Every one moves itself with left and right, and lights up when pressed.".into(),
            Vec::new(),
        ),
        _ => ("Quit", "Back to the desktop. Everything is already saved.".into(), Vec::new()),
    }
}

/// A pause row's heading and words beside the list, by what the row says.
fn pause_panel(row: &str) -> (String, &'static str) {
    let about = match row {
        "Resume" => "",
        "Restart" => "Back to the start of this track, same machine, same modifiers.",
        "Settings" => "Every setting takes effect when the pause closes.",
        "Controllers" => "Which pad, keyboard or touch pad drives which seat. The riders swap when their pads do.",
        "Main Menu" => "Ends the ride.",
        "Leave Session" => "Gives up your seat: your bike goes and the others ride on. The last one out ends the ride.",
        "Quit" => "Ends the ride and closes the game.",
        "Leave Session" => "Takes you out of the online ride. The others ride on without you.",
        r if r.starts_with("Switch") => "The original pause's machine switch. Your bike waits where you left it.",
        r if r.starts_with("Seats") => "How many seats the online ride has. Left and right change it.",
        _ => "Lets online players drop into this ride with the seats the lobby last chose. Your seat stays yours; the code appears here, and Seats under it.",
    };
    (pause_label(row).0, about)
}

/// A pause row as the list says it: the switch row as "Switch to Biplane", dim with nowhere to go.
fn pause_label(row: &str) -> (String, bool) {
    match row.strip_prefix("Switch To Machine: ") {
        Some("None") => ("Switch Machine".into(), true),
        Some(m) => (format!("Switch to {}", m.trim_start_matches("< ").trim_end_matches(" >")), false),
        None => (row.to_string(), false),
    }
}

/// What the Online screen's rows say beside the list.
fn online_panel(at: usize) -> (&'static str, &'static str) {
    match at {
        0 => ("Quickplay", "Joins the first open ride with a free seat, or hosts one if there's none."),
        1 => ("Host", "Opens a lobby with your track, wrecks and seats. Friends join with the code it shows."),
        2 => ("Find Games", "Open rides on the LAN and the internet, newest first."),
        3 => ("Join by Code", "Type the code a host gave you."),
        _ => ("Network", "Where Quickplay, Host and Find look: the internet and the LAN, or the LAN only."),
    }
}

/// The modifiers' one-word summary.
fn modifiers_word(menu: &Menu) -> String {
    let n = Knob::MENU
        .iter()
        .filter(|k| (0..SEATS_MAX).any(|s| menu.settings.modifiers.get(s, **k) != game_sim::modifiers::ORIGINAL))
        .count();
    if n == 0 { "Original".into() } else { format!("{n} changed") }
}

impl Pen<'_> {
    pub(super) fn screen(&mut self, menu: &Menu) {
        // A ride loading (P127) is the one thing on the screen until it starts or is cancelled.
        if let Some(loading) = &menu.loading {
            return self.loading(menu, loading);
        }
        let Some(screen) = menu.screen() else { return };
        match screen {
            Screen::Pause => {
                let whole = self.whole();
                self.fill(whole, PAUSE_DIM);
            }
            Screen::Splash => {}
            _ => self.backdrop(),
        }
        match screen {
            Screen::Main => self.main(menu),
            Screen::Pause => self.pause(menu),
            Screen::Online => self.online(menu),
            Screen::Settings | Screen::Garage | Screen::Modifiers | Screen::Mods | Screen::Original => self.page(menu, screen),
            Screen::Tracks => self.tracks(menu),
            Screen::Join => self.join(menu),
            Screen::Assign => self.assign(menu),
            Screen::Store => self.store(menu),
            Screen::Split => self.split(menu),
            Screen::Keyboard => self.keyboard(menu),
            Screen::Games => self.games(menu),
            Screen::Lobby => self.lobby(menu),
            Screen::Splash => return self.splash(menu),
            // Opened as Settings' tabs; never on top of the stack.
            Screen::Controls | Screen::Audio | Screen::Camera | Screen::Graphics => {
                self.title(screen.title());
                self.hints(&[(Btn::Circle, "Back")]);
            }
        }
        self.ticker(menu, screen);
        if screen == Screen::Main {
            self.corner();
        }
    }

    /// Which build this is (P189), small in the frame's top right corner, or the stage's when the frame is wider.
    fn corner(&mut self) {
        let size = 1.0;
        let (x, y) = (STAGE.0.min(self.right()) - 1.5, self.top().max(0.0) + 1.2 + CAP * size);
        self.shadowed(Weight::Bold, BUILD_CORNER, x, y, size, 0.0, DUST, Align::Right, [0.07, 0.09], [0, 0, 0, 89]);
    }

    /// The band's line: a note, the lobby's word online, whose ride it is otherwise.
    fn ticker(&mut self, menu: &Menu, screen: Screen) {
        let online = matches!(screen, Screen::Online | Screen::Games | Screen::Lobby) || screen == Screen::Pause && menu.opening == Opening::Open;
        match &menu.note {
            Some(n) => self.band(&[(n, false)]),
            None if online && !menu.lobby.status.is_empty() => self.band(&[(&menu.lobby.status, false)]),
            None => {
                let number = menu.garages.first().map(|g| g.pick.look.number.clone()).unwrap_or_default();
                let tail = if number.is_empty() { String::new() } else { format!(" · #{number}") };
                self.band(&[("Riding as ", false), (&menu.settings.name, true), (&tail, false)]);
            }
        }
    }

    fn main(&mut self, menu: &Menu) {
        let at = menu.cursor(Screen::Main);
        self.strip(10.0, 18.0);
        self.logo(false);
        let rows: Vec<(String, bool)> = MAIN.iter().map(|s| (s.to_string(), false)).collect();
        self.list(&rows, at, 19.0, 10.4, 18.0);
        let (h, p, chips) = main_panel(menu, at);
        let (x, w) = (34.0, 40.0);
        if at == MAIN_FREE {
            let track = menu.track_index();
            let (_, town) = menu.track_at(track);
            let b = [x, 7.4, 30.0, 16.875];
            self.framed_still(menu, track, b, &format!("Last ridden · {town}"));
            let y = self.heading("", h, &p, x, b[1] + b[3] + 1.0, w, 2.6);
            let chips = vec![
                format!("Wrecks *{}*", if menu.settings.wrecks { "On" } else { "Off" }),
                format!("Modifiers *{}*", modifiers_word(menu)),
            ];
            self.chips(&chips, x, y + 1.0, w);
        } else if at == MAIN_GARAGE {
            let name = menu.rider_name(0);
            let number = menu.garages.first().map(|g| g.pick.look.number.clone()).unwrap_or_default();
            self.text(Weight::Bold, "RIDING AS", x, 10.0 + CAP, 1.0, 0.0, EYEBROW, Align::Left);
            let size = 2.6;
            let base = 11.6 + CAP * size;
            let nb = self.shadowed(Weight::Heavy, &name, x, base, size, SLANT, CHALK, Align::Left, [0.07, 0.09], [0, 0, 0, 89]);
            if !number.is_empty() {
                let nx = (nb[0] + nb[2] - self.o[0]) / self.em + 0.8;
                self.text(Weight::Heavy, &format!("#{number}"), nx, base, size, SLANT, PEACH, Align::Left);
            }
            let bike = menu.garages.first().map(|g| g.value(&menu.catalogue, roster::Row::Model)).unwrap_or_default();
            let line = if bike.is_empty() { p } else { format!("{bike}. {p}") };
            let y = self.para(&line, x, 11.6 + size * 1.05 + 0.6, 32.5, 1.25, 1.4, PAPER);
            self.chips(&chips, x, y + 1.0, w);
        } else {
            let y = self.heading("openMX", h, &p, x, 10.0, w, 2.6);
            self.chips(&chips, x, y + 1.0, w);
        }
        self.hints(&[(Btn::Cross, "Select")]);
    }

    /// The pick's track `at`'s picture in a chalk frame with its tag, turning through a disc
    /// track's stills, or a dark box for a track without one.
    fn framed_still(&mut self, menu: &Menu, at: usize, b: Box4, tag: &str) {
        self.round([b[0] + 0.35, b[1] + 0.4, b[2], b[3]], 0.9, SHADOW, SHADOW);
        self.round(b, 0.9, [0x33, 0x33, 0x33, 255], [0x22, 0x22, 0x22, 255]);
        if let Some((image, t)) = self.track_picture(menu, at, (menu.tick / STILL_FRAMES) as usize) {
            let inner = [b[0] + 0.2, b[1] + 0.2, b[2] - 0.4, b[3] - 0.4];
            self.cover(image, t, inner, CHALK);
        } else {
            self.text(Weight::Bold, "No picture for this one", b[0] + b[2] / 2.0, Pen::mid(b[1] + b[3] / 2.0, 1.2), 1.2, 0.0, DUST, Align::Centre);
        }
        self.ring(b, 0.9, 0.2, CHALK);
        if !tag.is_empty() {
            let size = 1.1;
            let tag = self.fit(Weight::Bold, tag, size, b[2] - 3.0);
            let w = self.wide(Weight::Bold, &tag, size) + 1.4 * size;
            let h = 1.6 * size;
            let y = b[1] + b[3] - 0.7 * size - h;
            self.round([b[0] + 0.8 * size, y, w, h], h / 2.0, [0, 0, 0, 140], [0, 0, 0, 140]);
            self.text(Weight::Bold, &tag, b[0] + 1.5 * size, Pen::mid(y + h / 2.0, size), size, 0.0, CHALK, Align::Left);
        }
    }

    fn pause(&mut self, menu: &Menu) {
        let at = menu.cursor(Screen::Pause);
        self.strip(10.0, 18.0);
        self.logo(true);
        let raw = menu.pause_rows();
        let rows: Vec<(String, bool)> = raw.iter().map(|r| pause_label(r)).collect();
        self.list(&rows, at, 19.0, 7.5, 18.0);
        let (_, town) = menu.track_at(menu.track_index());
        let who = if menu.riders > 1 { format!("{}P · ", menu.pauser + 1) } else { String::new() };
        let modified = if menu.modified { " · Modified" } else { "" };
        let eyebrow = format!("{who}{town} · Free Ride{modified}");
        let row = raw.get(at).map_or("", String::as_str);
        let (h, about) = pause_panel(row);
        let (x, w) = (34.0, 40.0);
        let y = self.heading(&eyebrow, &h, about, x, 10.0, w, 2.6);
        let mut chips = Vec::new();
        // The song playing is the player's to say (P174).
        if row == "Resume" {
            chips.push(menu.settings.mix.label().to_string());
        }
        // Before opening the online row's chip is the only place the pause says the count (P211).
        if row == "Open to Online" || row.starts_with("Online") || row.starts_with("Seats") {
            chips.push(format!("Seats *{}*", menu.pause_seats()));
        }
        // Another controller can ride in from here (P76). Spelt out (P171): the menu's type has no ✕.
        if menu.seats_open && menu.riders < crate::join::IN_RIDE {
            chips.push("*Cross* on a free controller joins".into());
        }
        self.chips(&chips, x, y + 1.0, w);
        let title = if menu.riders > 1 { format!("{}P Paused", menu.pauser + 1) } else { "Paused".into() };
        self.title(&title);
        if menu.player_shown() {
            self.music_player(menu);
        }
        if let Some((ask, rows, row)) = menu.confirming() {
            self.hints(&[(Btn::Circle, "Stay"), (Btn::Cross, "Select")]);
            self.popup(ask, &rows, row);
        } else {
            self.hints(&[(Btn::Circle, "Resume"), (Btn::Cross, "Select")]);
        }
    }

    /// The pause's music player (P174): a glass box with the song playing and its three controls,
    /// each a disc with its pad button under it, dimmed while the music is off.
    fn music_player(&mut self, menu: &Menu) {
        use super::player::{BOX, Deck, RADIUS, centre};
        let b = BOX;
        self.round([b[0] + 0.3, b[1] + 0.35, b[2], b[3]], 0.9, SHADOW, SHADOW);
        self.round(b, 0.9, GLASS, GLASS);
        self.ring(b, 0.9, 0.08, RIM);
        let off = menu.player_off();
        let text_w = centre(Deck::Back)[0] - RADIUS - 1.2 - (b[0] + 1.2);
        let eyebrow = match (off, menu.music_held) {
            (Some(_), _) => "MUSIC",
            (None, true) => "PAUSED",
            (None, false) => "NOW PLAYING",
        };
        self.text(Weight::Bold, eyebrow, b[0] + 1.2, b[1] + 1.4 + CAP, 0.9, 0.0, EYEBROW, Align::Left);
        let song = match (off, &menu.music_now) {
            (Some(why), _) => why.to_string(),
            (None, Some(now)) => menu.song_label(now),
            (None, None) => "No song yet".to_string(),
        };
        let size = 1.3;
        let song = self.fit(Weight::Heavy, &song, size, text_w);
        self.text(Weight::Heavy, &song, b[0] + 1.2, b[1] + 3.6 + CAP * size, size, 0.0, if off.is_some() { DUST } else { CHALK }, Align::Left);
        let was = self.fade;
        if off.is_some() {
            self.fade = was * 0.45;
        }
        for d in Deck::ALL {
            let [x, y] = centre(d);
            self.disc(x, y, RADIUS, SHOULDER);
            self.ring([x - RADIUS, y - RADIUS, 2.0 * RADIUS, 2.0 * RADIUS], RADIUS, 0.1, RIM);
            let k = RADIUS * 0.42;
            match d {
                // a bar and two heads, pointing the way the song goes
                Deck::Back | Deck::Skip => {
                    let dir = if d == Deck::Back { -1.0 } else { 1.0 };
                    self.arrow(x - dir * k * 0.45, y, [dir, 0.0], k * 1.1, CHALK);
                    self.arrow(x + dir * k * 0.45, y, [dir, 0.0], k * 1.1, CHALK);
                    let bar = x + dir * k * 1.1;
                    self.line([bar, y - k * 0.75], [bar, y + k * 0.75], k * 0.18, k * 0.18, CHALK);
                }
                Deck::Hold if menu.music_held => self.arrow(x + k * 0.15, y, [1.0, 0.0], k * 1.6, CHALK),
                Deck::Hold => {
                    for dx in [-0.42, 0.42] {
                        self.line([x + dx * k, y - k * 0.8], [x + dx * k, y + k * 0.8], k * 0.3, k * 0.3, CHALK);
                    }
                }
            }
            // its pad button, under it
            let hy = y + RADIUS + 0.95;
            match d {
                Deck::Hold => self.button(Btn::Square, x, hy, 1.05),
                _ => {
                    let word = if d == Deck::Back { "L1" } else { "R1" };
                    let size = 0.55;
                    let w = self.wide(Weight::Heavy, word, size) + 0.65;
                    self.round([x - w / 2.0, hy - 0.42, w, 0.84], 0.27, SHOULDER, SHOULDER);
                    self.text(Weight::Heavy, word, x, Pen::mid(hy, size), size, 0.0, CHALK, Align::Centre);
                }
            }
        }
        self.fade = was;
    }

    fn online(&mut self, menu: &Menu) {
        let at = menu.cursor(Screen::Online);
        self.strip(10.0, 18.0);
        self.logo(true);
        let network = if menu.lobby.lan { "LAN Only" } else { "Internet + LAN" };
        let rows: Vec<(String, bool)> = ONLINE.iter().map(|s| s.to_string()).chain([format!("Network: {network}")]).map(|s| (s, false)).collect();
        self.list(&rows, at, 19.0, 10.4, 18.0);
        let (h, p) = online_panel(at);
        let (x, w) = (34.0, 40.0);
        let mut y = self.heading("openMX", h, p, x, 10.0, w, 2.6);
        if at == 2 {
            self.game_rows(menu, x, y + 1.0, w, None);
        } else if at == 1 {
            let (_, town) = menu.track_at(menu.track_index());
            y = self.chips(&[format!("Seats *{}*", menu.settings.lobby_seats), town.to_string()], x, y + 1.0, w);
            let _ = y;
        }
        self.title(Screen::Online.title());
        self.hints(&[(Btn::Circle, "Back"), (Btn::Cross, "Select")]);
    }

    /// The lobbies found, a glass row each: its name, its track, its seats.
    fn game_rows(&mut self, menu: &Menu, x: f32, top: f32, w: f32, chosen: Option<usize>) {
        let size = 1.15;
        let h = size * 2.0;
        let mut y = top;
        for (i, l) in menu.lobby.found.iter().enumerate() {
            if y + h > STAGE.1 - BAND - 4.0 {
                break;
            }
            let track = TRACKS.iter().find(|t| t.0 == l.track).map_or(l.track.as_str(), |t| t.1);
            if chosen == Some(i) {
                self.pill([x, y, w, h], 0.2, 0.1);
            } else {
                self.round([x, y, w, h], 0.6 * size, GLASS, GLASS);
            }
            let mid = Pen::mid(y + h / 2.0, size);
            let seats = format!("{} / {}", l.taken, l.seats);
            let sw = self.wide(Weight::Heavy, &seats, size);
            self.text(Weight::Heavy, &seats, x + w - 0.9 * size, mid, size, 0.0, PEACH, Align::Right);
            let note = if !l.same_build() { "another version" } else if l.riding { "riding: drop in" } else { "" };
            let tw = self.wide(Weight::Bold, track, size);
            let tx = x + w - 0.9 * size - sw - size;
            self.text(Weight::Bold, track, tx, mid, size, 0.0, CHALK, Align::Right);
            let name = if note.is_empty() { l.name.clone() } else { format!("{}  ({note})", l.name) };
            let name = self.fit(Weight::Bold, &name, size, (tx - tw - size - x - 0.9 * size).max(2.0));
            self.text(Weight::Bold, &name, x + 0.9 * size, mid, size, 0.0, CHALK, Align::Left);
            y += h + 0.4 * size;
        }
    }

    /// An option page: its tabs, its rows, and the lit row's side panel.
    fn page(&mut self, menu: &Menu, screen: Screen) {
        self.strip(3.0, 38.0);
        self.logo(true);
        let tabs = menu.tabs(screen);
        if !tabs.is_empty() {
            self.tabs(tabs, menu.tab(screen), 4.0, 7.6);
        }
        let page = menu.page_of(screen);
        let at = menu.cursor(screen).min(page.len().saturating_sub(1));
        let top = if tabs.is_empty() { 8.2 } else { 11.4 };
        self.rows(&page, at, 3.8, 36.4, top);
        let Some(r) = page.get(at) else { return };
        let mut side = if screen == Screen::Settings && menu.tab(screen) == pages::TAB_CONTROLS { 4.5 } else { 7.6 };
        if screen == Screen::Garage {
            self.plate(menu, 45.0, 4.0);
            // under the bike and rider (P213)
            side = 25.2;
        }
        self.side(menu, screen, &page, r, 45.0, side, 31.0);
        let title = match screen {
            Screen::Modifiers => "Game Modifiers",
            s => s.title(),
        };
        self.title(title);
        let ok = if r.kind == Kind::Action || r.kind == Kind::Opens { "Select" } else { "Change" };
        let mut hints = vec![(Btn::Circle, "Back")];
        if !tabs.is_empty() {
            hints.push((Btn::L1R1, "Tabs"));
        }
        hints.push((Btn::Cross, ok));
        self.hints(&hints);
    }

    /// The tabs between L1 and R1, the one on lit, their words made smaller where they would run
    /// past the page's strip (Settings' six since P128).
    fn tabs(&mut self, tabs: &[&str], on: usize, x: f32, top: f32) {
        let chip = 0.95;
        let shoulders: f32 = ["L1", "R1"].iter().map(|w| self.wide(Weight::Heavy, w, chip) + 1.1 * chip).sum();
        let per: f32 = tabs.iter().map(|t| self.wide(Weight::Heavy, t, 1.0) + 1.6).sum();
        let room = 40.6 - x - shoulders - 0.3 * (tabs.len() + 1) as f32;
        let size = if per > 0.0 { (room / per).min(1.3) } else { 1.3 };
        let h = 1.3 * 1.8;
        let mid = top + h / 2.0;
        let mut at = x;
        let shoulder = |pen: &mut Pen, word: &str, at: f32| -> f32 {
            let w = pen.wide(Weight::Heavy, word, chip) + 1.1 * chip;
            let ch = chip * 1.5;
            pen.round([at, mid - ch / 2.0, w, ch], 0.45 * chip, SHOULDER, SHOULDER);
            pen.text(Weight::Heavy, word, at + w / 2.0, Pen::mid(mid, chip), chip, 0.0, CHALK, Align::Centre);
            w
        };
        at += shoulder(self, "L1", at) + 0.3;
        for (i, t) in tabs.iter().enumerate() {
            let lit = i == on;
            let (w, c, s) = if lit { (Weight::Heavy, CHALK, SLANT) } else { (Weight::Bold, BONE, 0.0) };
            let tw = self.wide(w, t, size) + 1.6 * size;
            if lit {
                let r = h / 2.0;
                self.round([at + 0.26, top + 0.29, tw, h], r, SHADOW, SHADOW);
                self.round([at, top, tw, h], r, LIT_TOP, LIT_BOTTOM);
                self.ring([at, top, tw, h], r, 0.13, CHALK);
            }
            self.text(w, t, at + tw / 2.0, Pen::mid(mid, size), size, s, c, Align::Centre);
            at += tw + 0.3;
        }
        shoulder(self, "R1", at);
    }

    /// The garage's number plate: the player's number over their name.
    fn plate(&mut self, menu: &Menu, x: f32, y: f32) {
        let seat = menu.player().min(menu.garages.len().saturating_sub(1));
        let number = menu.garages.get(seat).map(|g| g.pick.look.number.clone()).unwrap_or_default();
        let name = menu.rider_name(seat);
        let (w, h) = (13.6, 11.8);
        self.round([x + 0.4, y + 0.45, w, h], 1.6, SHADOW, SHADOW);
        self.round([x, y, w, h], 1.6, [0xff, 0xfd, 0xf8, 255], [0xe7, 0xe0, 0xd4, 255]);
        self.ring([x, y, w, h], 1.6, 0.3, [0x1d, 0x1d, 0x1d, 255]);
        let number = if number.is_empty() { "-".to_string() } else { number };
        let size = 6.4f32.min((w - 2.0) / self.wide(Weight::Heavy, &number, 1.0).max(0.1));
        self.text(Weight::Heavy, &number, x + w / 2.0, y + h * 0.62, size, 0.176, [0xb4, 0x14, 0x1e, 255], Align::Centre);
        let nm = self.fit(Weight::Heavy, &name.to_uppercase(), 1.3, w - 1.4);
        self.text(Weight::Heavy, &nm, x + w / 2.0, y + h - 1.4, 1.3, 0.0, [0x14, 0x14, 0x14, 255], Align::Centre);
    }

    /// Free Ride's pick: the lit filter's tracks as cards under its name (P70), each with its
    /// picture, Left and Right between the filters by source game (MD11: All Games, the disc's, the
    /// worlds, each other game),
    /// the lit track's still large, and the ride's setup.
    fn tracks(&mut self, menu: &Menu) {
        let at = menu.cursor(Screen::Tracks).min(menu.track_count().saturating_sub(1));
        self.strip(3.0, 22.0);
        self.logo(true);
        let groups = menu.track_groups();
        // The lit filter (MD11), the cursor on its first track where it is not on one of its own.
        let (group, range) = groups[menu.track_filter_group()].clone();
        let at = if range.contains(&at) { at } else { range.start };
        let top = if groups.len() > 1 {
            self.category(&group, 3.8, 7.6, 20.4);
            11.4
        } else {
            let base = Pen::mid(8.2 + HEAD_H / 2.0 + 0.2, 1.0);
            self.text(Weight::Bold, &group.to_uppercase(), 4.2, base, 1.0, 0.0, HEADING, Align::Left);
            8.2 + HEAD_H + ROW_GAP + 0.2
        };
        let tag_of = |id: &str| menu.track_game(id);
        self.cards(menu, range.clone(), at, 3.8, 20.4, top, &tag_of);
        let (id, name) = menu.track_at(at);
        let (x, w) = (28.5, 47.0);
        let tag = tag_of(id);
        let b = [x, 6.0, 36.0, 20.25];
        self.framed_still(menu, at, b, &tag);
        let count = format!("{} of {}", at - range.start + 1, range.len());
        // A long name stops short of the count beside it.
        let room = w - self.wide(Weight::Bold, &count, 1.2) - 2.0;
        let y = self.heading("", name, "", x, b[1] + b[3] + 1.0, room, 2.6);
        self.text(Weight::Bold, &count, x + w - 1.0, b[1] + b[3] + 1.0 + CAP * 1.2, 1.2, 0.0, EYEBROW, Align::Right);
        // The setup strip: the wrecks and the modifiers. Every ride starts on the bike (P177); the
        // machine is switched from the pause or at a bay.
        let chips = vec![format!("Wrecks *{}*", if menu.settings.wrecks { "On" } else { "Off" }), format!("Modifiers *{}*", modifiers_word(menu))];
        self.chips(&chips, x, y + 1.0, w);
        self.title(Screen::Tracks.title());
        let mut hints = vec![(Btn::Circle, "Back"), (Btn::Triangle, "Wrecks")];
        if groups.len() > 1 {
            hints.push((Btn::LeftRight, "Game"));
        }
        hints.push((Btn::Cross, "Ride"));
        self.hints(&hints);
    }

    /// The pick's tracks in `range` as cards (P70), in P39's pills' look: a lit card in the red
    /// pill's gradient, rim and shadow, the others glass; each the track's picture (the lit one
    /// turning through a disc track's stills), its name, and the line under it — Last ridden, or
    /// where the track is from. As many as fit from `top`, the lit one kept in view.
    #[allow(clippy::too_many_arguments)]
    fn cards(&mut self, menu: &Menu, range: Range<usize>, at: usize, x: f32, w: f32, top: f32, tag_of: &dyn Fn(&str) -> String) {
        const CARD_H: f32 = 4.4;
        const CARD_GAP: f32 = 0.5;
        let bottom = STAGE.1 - BAND - 0.4;
        let shown = (((bottom - top + CARD_GAP) / (CARD_H + CARD_GAP)) as usize).max(1);
        let n = range.len();
        let lit_at = at - range.start;
        let first = lit_at.saturating_sub(shown / 2).min(n.saturating_sub(shown));
        if first > 0 {
            self.arrow(x + w / 2.0, top - 0.6, [0.0, -1.0], 0.7, BONE);
        }
        let mut y = top;
        for k in first..(first + shown).min(n) {
            let i = range.start + k;
            let lit = i == at;
            let (id, name) = menu.track_at(i);
            let b = [x, y, w, CARD_H];
            let r = 0.7;
            if lit {
                self.round([x + 0.25, y + 0.27, w, CARD_H], r, SHADOW, SHADOW);
                self.round(b, r, LIT_TOP, LIT_BOTTOM);
                self.ring(b, r, 0.13, CHALK);
            } else {
                self.round(b, r, GLASS, GLASS);
                self.ring(b, r, 0.08, RIM);
            }
            let ph = CARD_H - 0.7;
            let pb = [x + 0.35, y + 0.35, ph * 16.0 / 9.0, ph];
            self.round(pb, 0.35, [0x22, 0x22, 0x22, 255], [0x22, 0x22, 0x22, 255]);
            let turn = if lit { (menu.tick / STILL_FRAMES) as usize } else { 0 };
            let picture = self.track_picture(menu, i, turn);
            if let Some((image, t)) = picture {
                self.cover(image, t, pb, CHALK);
            }
            self.ring(pb, 0.35, if lit { 0.1 } else { 0.06 }, if lit { CHALK } else { RIM });
            let tx = pb[0] + pb[2] + 0.8;
            let room = x + w - 0.8 - tx;
            let k_size = 1.35;
            let (kw, kc, ks) = if lit { (Weight::Heavy, CHALK, SLANT) } else { (Weight::Bold, BONE, 0.0) };
            let label = self.fit(kw, name, k_size, room);
            let base = y + CARD_H * 0.47;
            if !lit {
                self.text(kw, &label, tx, base + 0.06 * k_size, k_size, ks, TEXT_SHADOW, Align::Left);
            }
            self.text(kw, &label, tx, base, k_size, ks, kc, Align::Left);
            let under = if menu.settings.track == id { "Last ridden".to_string() } else { tag_of(id) };
            let under = self.fit(Weight::Bold, &under, 0.95, room);
            self.text(Weight::Bold, &under, tx, y + CARD_H * 0.78, 0.95, 0.0, if lit { PEACH } else { DUST }, Align::Left);
            let image = picture.map(|(image, _)| image - self.base);
            self.parts.push(Part::Card { id: id.to_string(), card: self.b(b), picture: self.b(pb), image, lit });
            y += CARD_H + CARD_GAP;
        }
        if first + shown < n {
            self.arrow(x + w / 2.0, y - CARD_GAP + 0.6, [0.0, 1.0], 0.7, BONE);
        }
    }

    /// The mod store (P198): the lit tab's mods as a grid of cards ([`STORE_COLUMNS`] across, two
    /// rows in view, the lit one kept there), each its preview, name, author and source game, size
    /// and Verified, and Installed, Update or an install's bar.
    fn store(&mut self, menu: &Menu) {
        self.logo(true);
        let shop = &menu.store;
        let tabs: Vec<&str> = STORE_TABS.iter().chain(STORE_PAGES.iter()).copied().collect();
        self.tabs(&tabs, shop.tab, 4.0, 7.6);
        if let Some(note) = &shop.note {
            let line = self.fit(Weight::Bold, note, 1.0, 34.0);
            self.text(Weight::Bold, &line, 76.0, Pen::mid(7.6 + 1.17, 1.0), 1.0, 0.0, NOTE, Align::Right);
        }
        if shop.tab >= UPLOAD_TAB {
            self.store_page(menu);
            return;
        }
        let shown = shop.shown();
        let lit = menu.store_lit();
        let (x0, top, w) = (3.8, 11.4, 72.4);
        let gap = 0.8;
        let cw = (w - gap * (STORE_COLUMNS - 1) as f32) / STORE_COLUMNS as f32;
        let ph = (cw - 0.7) * 9.0 / 16.0;
        let ch = ph + 4.25;
        let say = |pen: &mut Pen, s: &str| {
            let base = Pen::mid(top + 8.0, 1.4);
            pen.text(Weight::Bold, s, STAGE.0 / 2.0, base, 1.4, 0.0, BONE, Align::Centre);
        };
        match &shop.listed {
            _ if shop.off => say(self, "The mods folder is off (--mods off): nothing to install into"),
            Listed::Asking if shown.is_empty() => say(self, "Reading the mod store…"),
            Listed::Failed(why) if shown.is_empty() => {
                let why = self.fit(Weight::Bold, why, 1.2, w);
                say(self, &why);
            }
            _ if shown.is_empty() => say(self, &format!("No {} in the store yet", STORE_TABS[shop.tab].to_lowercase())),
            _ => {}
        }
        let at = shown.iter().position(|&i| Some(i) == lit).unwrap_or(0);
        let rows = shown.len().div_ceil(STORE_COLUMNS);
        let first_row = (at / STORE_COLUMNS).saturating_sub(1).min(rows.saturating_sub(2));
        if first_row > 0 {
            self.arrow(STAGE.0 / 2.0, top - 0.6, [0.0, -1.0], 0.7, BONE);
        }
        for (k, &i) in shown.iter().enumerate().skip(first_row * STORE_COLUMNS).take(2 * STORE_COLUMNS) {
            let c = &shop.cards[i];
            let on = Some(i) == lit;
            let (col, row) = (k % STORE_COLUMNS, k / STORE_COLUMNS - first_row);
            let (x, y) = (x0 + col as f32 * (cw + gap), top + row as f32 * (ch + gap));
            let b = [x, y, cw, ch];
            let r = 0.7;
            if on {
                self.round([x + 0.25, y + 0.27, cw, ch], r, SHADOW, SHADOW);
                self.round(b, r, LIT_TOP, LIT_BOTTOM);
                self.ring(b, r, 0.13, CHALK);
            } else {
                self.round(b, r, GLASS, GLASS);
                self.ring(b, r, 0.08, RIM);
            }
            let pb = [x + 0.35, y + 0.35, cw - 0.7, ph];
            self.round(pb, 0.35, [0x22, 0x22, 0x22, 255], [0x22, 0x22, 0x22, 255]);
            let image = self.art.track_image(&c.picture, 0);
            let picture = image.and_then(|im| self.art.images().get(im).map(|t| (self.base + im, *t)));
            if let Some((im, t)) = picture {
                self.cover(im, t, pb, CHALK);
            }
            if let Some((share, _)) = c.progress {
                let bar = [pb[0] + 0.5, pb[1] + pb[3] - 1.1, pb[2] - 1.0, 0.6];
                self.round(bar, 0.3, [0, 0, 0, 170], [0, 0, 0, 170]);
                self.round([bar[0], bar[1], (bar[2] * share.clamp(0.0, 1.0)).max(0.6), bar[3]], 0.3, PEACH, ORANGE);
            }
            self.ring(pb, 0.35, if on { 0.1 } else { 0.06 }, if on { CHALK } else { RIM });
            let (tx, room) = (x + 0.6, cw - 1.2);
            let (kw, kc, ks) = if on { (Weight::Heavy, CHALK, SLANT) } else { (Weight::Bold, BONE, 0.0) };
            let name = self.fit(kw, &c.name, 1.2, room);
            self.text(kw, &name, tx, pb[1] + ph + 1.45, 1.2, ks, kc, Align::Left);
            let by = [c.author.as_str(), c.source.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ");
            let by = self.fit(Weight::Bold, &by, 0.85, room);
            let dim = if on { PEACH } else { DUST };
            self.text(Weight::Bold, &by, tx, pb[1] + ph + 2.55, 0.85, 0.0, dim, Align::Left);
            let left = format!("{}{}", size_words(c.size), if c.verified { " · Verified" } else { " · Not reviewed" });
            self.text(Weight::Bold, &left, tx, pb[1] + ph + 3.6, 0.85, 0.0, dim, Align::Left);
            let state = match (&c.progress, c.got) {
                (Some((share, _)), _) => format!("{:.0}%", share * 100.0),
                (None, Got::Installed) => "Installed".into(),
                (None, Got::Update) => "Update".into(),
                (None, Got::Own) => "In your folder".into(),
                (None, Got::No) => String::new(),
            };
            self.text(Weight::Heavy, &state, x + cw - 0.6, pb[1] + ph + 3.6, 0.85, 0.0, if on { CHALK } else { HEADING }, Align::Right);
            let image = image;
            self.parts.push(Part::Card { id: c.key.clone(), card: self.b(b), picture: self.b(pb), image, lit: on });
        }
        if first_row + 2 < rows {
            self.arrow(STAGE.0 / 2.0, top + 2.0 * (ch + gap) - gap + 0.4, [0.0, 1.0], 0.7, BONE);
        }
        self.title(Screen::Store.title());
        let mut hints = vec![(Btn::Circle, "Back"), (Btn::L1R1, "Tabs"), (Btn::Start, "Refresh")];
        if let Some(c) = lit.map(|i| &shop.cards[i]) {
            hints.push((Btn::Triangle, "Report"));
            if c.progress.is_none() && matches!(c.got, Got::Installed | Got::Update) {
                hints.push((Btn::Square, "Remove"));
            }
            match (c.progress.is_some(), c.got) {
                (false, Got::No) => hints.push((Btn::Cross, "Install")),
                (false, Got::Update) => hints.push((Btn::Cross, "Update")),
                _ => {}
            }
        }
        self.hints(&hints);
    }

    /// P199: the store's Upload and My Mods pages, under its tabs: the rules first, then the sign-in
    /// (Hugging Face's token page, Paste token), then the account, Upload all and each of the player's
    /// own mods with its state; My Mods their mods on the store, each Verified or its pull request.
    fn store_page(&mut self, menu: &Menu) {
        let shop = &menu.store;
        let (x, top, w) = (4.0, 11.4, 72.0);
        let line = |pen: &mut Pen, s: &str, y: f32, size: f32, colour: [u8; 4]| {
            let s = pen.fit(Weight::Bold, s, size, w);
            pen.text(Weight::Bold, &s, x, Pen::mid(y, size), size, 0.0, colour, Align::Left);
        };
        // (label, right-hand words, a second line, lit)
        let mut rows: Vec<(String, String, String)> = Vec::new();
        let mut hints = vec![(Btn::Circle, "Back"), (Btn::L1R1, "Tabs")];
        let c = menu.cursor(Screen::Store);
        if shop.off {
            line(self, "The mods folder is off (--mods off): nothing to upload or take down", top + 2.0, 1.4, BONE);
        } else if shop.tab == UPLOAD_TAB && !shop.agreed {
            self.text(Weight::Heavy, "Before you upload", x, Pen::mid(top + 1.0, 1.6), 1.6, SLANT, HEADING, Align::Left);
            for (i, r) in STORE_RULES.iter().enumerate() {
                line(self, &format!("•  {r}"), top + 4.0 + 2.4 * i as f32, 1.25, BONE);
            }
            hints.push((Btn::Cross, "I agree"));
        } else if shop.tab == UPLOAD_TAB {
            for r in shop.upload_rows() {
                rows.push(match r {
                    UpRow::Tokens => ("Make a token on Hugging Face".into(), String::new(), "Opens huggingface.co: make a token with write access, and copy it".into()),
                    UpRow::Paste => ("Paste token".into(), if shop.signing { "Checking…".into() } else { String::new() }, "Signs in with the token on the clipboard; it is kept private, beside your settings".into()),
                    UpRow::SignOut => (format!("Signed in as {}", shop.account.as_deref().unwrap_or("")), "Sign out".into(), "Forgets the token".into()),
                    UpRow::All => (format!("Upload all ({})", shop.uploadable().len()), String::new(), "Each mod that can go up, one after another, in the background".into()),
                    UpRow::Mod(i) => {
                        let l = &shop.locals[i];
                        let (state, under) = match (&l.refused, &l.status) {
                            (_, Some((Phase::Busy, s))) => ("Uploading".to_string(), s.clone()),
                            (_, Some((Phase::Done, s))) => ("Done".into(), s.clone()),
                            (_, Some((Phase::Failed, s))) => ("Failed".into(), s.clone()),
                            (Some(why), None) => ("Can't share".into(), why.clone()),
                            (None, None) => ("Upload".into(), format!("{} · {}", l.kind, l.id)),
                        };
                        (l.name.clone(), state, under)
                    }
                });
            }
            if shop.account.is_some() && shop.locals.is_empty() {
                let why = if shop.web { "Uploading runs in the desktop game: the browser's mods folder holds only the store's" } else { "No mods of your own in the mods folder" };
                line(self, why, top + 2.0 * (ROW_H + 1.4) + 1.2, 1.2, BONE);
            }
            hints.push((Btn::Start, "Rescan"));
            if !rows.is_empty() {
                hints.push((Btn::Cross, "Select"));
            }
        } else if shop.account.is_none() {
            line(self, "Sign in on the Upload tab to see your mods on the store", top + 2.0, 1.4, BONE);
        } else {
            match &shop.mine_listed {
                Listed::Asking if shop.mine.is_empty() => line(self, "Reading your mods on the store…", top + 2.0, 1.4, BONE),
                Listed::Failed(why) => line(self, why, top + 2.0, 1.4, BONE),
                _ if shop.mine.is_empty() => line(self, "None of your mods are on the store yet", top + 2.0, 1.4, BONE),
                _ => {}
            }
            for m in &shop.mine {
                let state = if m.busy {
                    "Taking down…".to_string()
                } else if shop.arm.as_deref() == Some(m.key.as_str()) {
                    "Cross again: take down".into()
                } else {
                    String::new()
                };
                let place = match m.pr {
                    Some(n) => format!("pull request #{n}, not reviewed"),
                    None => "Verified".into(),
                };
                rows.push((m.name.clone(), state, format!("{} · {} · {}", m.kind, size_words(m.size), place)));
            }
            hints.push((Btn::Start, "Refresh"));
            if !shop.mine.is_empty() {
                hints.push((Btn::Cross, "Take down"));
            }
        }
        // the rows, the lit one kept in view
        let h = ROW_H + 1.4;
        let most = 6;
        let first = c.saturating_sub(most - 1).min(rows.len().saturating_sub(most));
        if first > 0 {
            self.arrow(STAGE.0 / 2.0, top - 0.6, [0.0, -1.0], 0.7, BONE);
        }
        for (k, (label, state, under)) in rows.iter().enumerate().skip(first).take(most) {
            let y = top + (k - first) as f32 * (h + ROW_GAP);
            let on = k == c.min(rows.len() - 1);
            let b = [x - 0.4, y, w + 0.8, h];
            if on {
                self.round([b[0] + 0.25, b[1] + 0.27, b[2], b[3]], 0.6, SHADOW, SHADOW);
                self.round(b, 0.6, LIT_TOP, LIT_BOTTOM);
                self.ring(b, 0.6, 0.13, CHALK);
            } else {
                self.round(b, 0.6, GLASS, GLASS);
            }
            let (kw, kc, ks) = if on { (Weight::Heavy, CHALK, SLANT) } else { (Weight::Bold, BONE, 0.0) };
            let label = self.fit(kw, label, 1.25, w - 16.0);
            self.text(kw, &label, x + 0.6, Pen::mid(y + 1.3, 1.25), 1.25, ks, kc, Align::Left);
            self.text(Weight::Heavy, state, x + w - 0.6, Pen::mid(y + 1.3, 1.0), 1.0, 0.0, if on { CHALK } else { HEADING }, Align::Right);
            let under = self.fit(Weight::Bold, under, 0.85, w - 1.2);
            self.text(Weight::Bold, &under, x + 0.6, Pen::mid(y + 2.85, 0.85), 0.85, 0.0, if on { PEACH } else { DUST }, Align::Left);
        }
        if first + most < rows.len() {
            self.arrow(STAGE.0 / 2.0, top + most as f32 * (h + ROW_GAP) + 0.2, [0.0, 1.0], 0.7, BONE);
        }
        self.title(Screen::Store.title());
        self.hints(&hints);
    }

    /// A ride loading (P127): the pick's still and name of its track over a bar the load's stages
    /// fill, what it is doing under the bar, and Circle to cancel.
    fn loading(&mut self, menu: &Menu, l: &super::Loading) {
        self.backdrop();
        self.logo(true);
        let (x, w) = (22.0, 36.0);
        let b = [x, 7.0, w, 20.25];
        self.framed_still(menu, l.still, b, &l.pack);
        let y = self.heading("Loading", &l.name, "", x, b[1] + b[3] + 1.0, w, 2.6);
        let bar = [x, y + 1.0, w, 1.1];
        self.round(bar, bar[3] / 2.0, GLASS, GLASS);
        let lit = (w * l.done.clamp(0.0, 1.0)).max(bar[3]);
        self.round([x, bar[1], lit, bar[3]], bar[3] / 2.0, LIT_TOP, LIT_BOTTOM);
        self.ring(bar, bar[3] / 2.0, 0.1, RIM);
        let line = bar[1] + bar[3] + 1.0 + CAP * 1.2;
        self.text(Weight::Bold, &l.stage, x, line, 1.2, 0.0, BONE, Align::Left);
        let share = format!("{:.0}%", l.done.clamp(0.0, 1.0) * 100.0);
        self.text(Weight::Bold, &share, x + w, line, 1.2, 0.0, EYEBROW, Align::Right);
        self.title("Loading");
        self.hints(&[(Btn::Circle, "Cancel")]);
        self.ticker(menu, Screen::Tracks);
    }

    /// The pick's category, lit between the arrows Left and Right step it by.
    fn category(&mut self, name: &str, x: f32, top: f32, w: f32) {
        let size = 1.3;
        let h = size * 1.8;
        let (mid, r) = (top + h / 2.0, h / 2.0);
        let (bx, bw) = (x + 1.6, w - 3.2);
        self.round([bx + 0.26, top + 0.29, bw, h], r, SHADOW, SHADOW);
        self.round([bx, top, bw, h], r, LIT_TOP, LIT_BOTTOM);
        self.ring([bx, top, bw, h], r, 0.13, CHALK);
        let label = self.fit(Weight::Heavy, name, size, bw - 2.0);
        self.text(Weight::Heavy, &label, bx + bw / 2.0, Pen::mid(mid, size), size, SLANT, CHALK, Align::Centre);
        self.arrow(x + 0.6, mid, [-1.0, 0.0], 0.9, CHALK);
        self.arrow(x + w - 0.6, mid, [1.0, 0.0], 0.9, CHALK);
    }

    /// The local game's seats: a card each, dashed and open or lit with the pad on it.
    fn join(&mut self, menu: &Menu) {
        self.logo(true);
        let seats = &menu.join.seats;
        let n = seats.len().max(1);
        let cols = n.min(4);
        let lines = n.div_ceil(4);
        let (x0, w) = (4.0, 72.0);
        let gap = 1.2;
        let cw = (w - gap * (cols - 1) as f32) / cols as f32;
        let top = 8.0;
        let ch = ((STAGE.1 - BAND - 5.0 - top - gap * (lines - 1) as f32) / lines as f32).min(16.0);
        let open = |i: usize| seats[i] == join::Seat::Open;
        for (i, seat) in seats.iter().enumerate() {
            let (cx, cy) = (x0 + (cw + gap) * (i % 4) as f32, top + (ch + gap) * (i / 4) as f32);
            let b = [cx, cy, cw, ch];
            let mid = cx + cw / 2.0;
            let big = (3.0f32).min(ch / 4.0);
            match seat {
                join::Seat::Open => {
                    self.round(b, 1.1, [10, 5, 2, 128], [10, 5, 2, 128]);
                    self.dashed(b, 1.1);
                    self.text(Weight::Heavy, &format!("{}P", i + 1), mid, cy + ch * 0.42, big, SLANT, [255, 240, 225, 128], Align::Centre);
                    let y = cy + ch * 0.68;
                    let pw = self.wide(Weight::Bold, "Press", 1.2);
                    let jw = self.wide(Weight::Bold, "to join", 1.2);
                    let total = pw + 0.4 + 1.5 + 0.4 + jw;
                    let lx = mid - total / 2.0;
                    self.text(Weight::Bold, "Press", lx, Pen::mid(y, 1.2), 1.2, 0.0, BONE, Align::Left);
                    self.button(Btn::Cross, lx + pw + 0.4 + 0.75, y, 1.5);
                    self.text(Weight::Bold, "to join", lx + pw + 2.3, Pen::mid(y, 1.2), 1.2, 0.0, BONE, Align::Left);
                }
                join::Seat::Taken(d) | join::Seat::Gone(d) => {
                    let gone = matches!(seat, join::Seat::Gone(_));
                    self.fade = if gone { 0.5 } else { 1.0 };
                    self.round([cx + 0.3, cy + 0.35, cw, ch], 1.1, SHADOW, SHADOW);
                    self.round(b, 1.1, [142, 15, 23, 235], [70, 6, 10, 235]);
                    self.ring(b, 1.1, 0.18, CHALK);
                    self.text(Weight::Heavy, &format!("{}P", i + 1), mid, cy + ch * 0.42, big, SLANT, CHALK, Align::Centre);
                    let name = menu.devices.get(*d).cloned().unwrap_or_else(|| format!("Pad {}", d + 1));
                    let name = self.fit(Weight::Bold, &name, 1.2, cw - 2.0);
                    self.text(Weight::Bold, &name, mid, Pen::mid(cy + ch * 0.64, 1.2), 1.2, 0.0, CHALK, Align::Centre);
                    let y = cy + ch * 0.82;
                    if gone {
                        self.text(Weight::Bold, "Gone: plug it back in", mid, Pen::mid(y, 1.05), 1.05, 0.0, PEACH, Align::Centre);
                    } else {
                        if (0..i).any(open) {
                            self.arrow(cx + 1.4, y, [-1.0, 0.0], 0.9, CHALK);
                        }
                        if (i + 1..n).any(open) {
                            self.arrow(cx + cw - 1.4, y, [1.0, 0.0], 0.9, CHALK);
                        }
                    }
                    self.fade = 1.0;
                }
            }
        }
        self.title(Screen::Join.title());
        let mut hints = Vec::new();
        if seats.iter().any(|s| matches!(s, join::Seat::Gone(_))) {
            hints.push((Btn::Square, "Let Go"));
        }
        hints.push((Btn::Circle, "Leave"));
        hints.push((Btn::Cross, "Join"));
        if menu.join.can_start() {
            hints.push((Btn::Start, "Pick Track"));
        }
        self.hints(&hints);
    }

    /// The Controllers screen (P210, opentennis's assign.rs): a column of the devices on no seat,
    /// then the seats, 1P to 8P, four to a line, each with the card of what drives it. A card slides
    /// from the column it left, and glows for a while when its device presses.
    fn assign(&mut self, menu: &Menu) {
        self.logo(true);
        let a = &menu.assign;
        let top = 8.0;
        let bottom = STAGE.1 - BAND - 5.0;
        let gap = 1.2;
        let (lx, lw) = (3.0, 14.0);
        let (x0, w) = (lx + lw + 2.0, 77.0 - (lx + lw + 2.0));
        let cw = (w - gap * 3.0) / 4.0;
        let lines = a.seats.len().div_ceil(4).max(1);
        let ch = ((bottom - top - gap * (lines - 1) as f32) / lines as f32).min(14.0);
        let seat_box = |i: usize| [x0 + (cw + gap) * (i % 4) as f32, top + (ch + gap) * (i / 4) as f32, cw, ch];
        // No seat's column: a header, and the cards in it under each other.
        let loose = a.loose();
        let lh = 4.2f32.min((bottom - top - 2.4) / loose.len().max(1) as f32 - 0.6);
        let loose_box = |k: usize| [lx, top + 2.4 + (lh + 0.6) * k as f32, lw, lh];
        self.round([lx - 0.5, top - 0.5, lw + 1.0, bottom - top + 1.0], 1.0, [10, 5, 2, 110], [10, 5, 2, 110]);
        self.text(Weight::Heavy, "No seat", lx + lw / 2.0, Pen::mid(top + 0.9, 1.3), 1.3, SLANT, [255, 240, 225, 160], Align::Centre);
        // Where a column's card goes: a seat's box, or the next free place in no seat's column.
        let home = |d: &Dev| match a.column(d) {
            0 => loose_box(loose.iter().position(|l| l == d).unwrap_or(0)),
            c => seat_box(c - 1),
        };
        let from = |c: usize| if c == 0 { loose_box(loose.len()) } else { seat_box(c - 1) };
        for i in 0..a.seats.len() {
            let b = seat_box(i);
            let shut = i >= a.open;
            self.fade = if shut { 0.4 } else { 1.0 };
            self.round(b, 1.1, [10, 5, 2, 128], [10, 5, 2, 128]);
            self.dashed(b, 1.1);
            let big = 2.6f32.min(ch / 4.0);
            self.text(Weight::Heavy, &format!("{}P", i + 1), b[0] + b[2] / 2.0, b[1] + ch * 0.42, big, SLANT, [255, 240, 225, 128], Align::Centre);
            self.fade = 1.0;
        }
        // Every card at its place, sliding there from the one it left.
        let mut cards: Vec<Dev> = a.seats.iter().flatten().cloned().collect();
        cards.extend(loose.iter().cloned());
        for d in &cards {
            let to = home(d);
            let b = match a.slid(d) {
                Some((c, t)) => {
                    let f = from(c);
                    let mix = |k: usize| f[k] + (to[k] - f[k]) * t;
                    [mix(0), mix(1), mix(2), mix(3)]
                }
                None => to,
            };
            let gone = !a.plugged(d);
            let glow = a.glow(d);
            let seat = a.column(d);
            self.fade = if gone { 0.5 } else { 1.0 };
            self.round([b[0] + 0.3, b[1] + 0.35, b[2], b[3]], 1.0, SHADOW, SHADOW);
            if seat > 0 {
                self.round(b, 1.0, [142, 15, 23, 235], [70, 6, 10, 235]);
            } else {
                self.round(b, 1.0, [60, 40, 30, 235], [30, 20, 14, 235]);
            }
            self.ring(b, 1.0, 0.18 + 0.3 * glow, if glow > 0.0 { PEACH } else { CHALK });
            let mid = b[0] + b[2] / 2.0;
            let size = 1.2f32.min(b[3] / 3.2);
            let name = self.fit(Weight::Bold, &menu.dev_name(d), size, b[2] - 2.0);
            let ny = if seat > 0 { b[1] + b[3] * 0.66 } else { b[1] + b[3] * 0.4 };
            self.text(Weight::Bold, &name, mid, Pen::mid(ny, size), size, 0.0, CHALK, Align::Centre);
            if seat > 0 {
                let big = 2.6f32.min(b[3] / 4.0);
                self.text(Weight::Heavy, &format!("{}P", seat), mid, b[1] + b[3] * 0.38, big, SLANT, CHALK, Align::Centre);
            }
            let y = b[1] + b[3] * if seat > 0 { 0.86 } else { 0.76 };
            if gone {
                self.text(Weight::Bold, "Gone: plug it back in", mid, Pen::mid(y, 0.95), 0.95, 0.0, PEACH, Align::Centre);
            } else if glow > 0.0 {
                self.text(Weight::Bold, "This is you", mid, Pen::mid(y, 0.95), 0.95, 0.0, PEACH, Align::Centre);
            } else {
                if seat > 0 {
                    self.arrow(b[0] + 1.2, y, [-1.0, 0.0], 0.8, CHALK);
                }
                if seat < a.open {
                    self.arrow(b[0] + b[2] - 1.2, y, [1.0, 0.0], 0.8, CHALK);
                }
            }
            self.fade = 1.0;
        }
        self.title(Screen::Assign.title());
        self.hints(&[(Btn::Circle, "Done"), (Btn::Square, "Take Off"), (Btn::Cross, "Find / Join"), (Btn::LeftRight, "Move")]);
    }

    /// A dashed rim round a box: short chalk dashes along its straight sides.
    fn dashed(&mut self, b: Box4, radius: f32) {
        let c = [255, 240, 225, 102];
        let (dash, half) = (0.7, 0.09);
        let along = |pen: &mut Pen, a: [f32; 2], d: [f32; 2], len: f32| {
            let n = (len / (dash * 2.0)).floor().max(1.0) as usize;
            let step = len / n as f32;
            for k in 0..n {
                let s = step * k as f32 + (step - dash) / 2.0;
                pen.line([a[0] + d[0] * s, a[1] + d[1] * s], [a[0] + d[0] * (s + dash), a[1] + d[1] * (s + dash)], half, half, c);
            }
        };
        let (x, y, w, h) = (b[0] + half, b[1] + half, b[2] - 2.0 * half, b[3] - 2.0 * half);
        along(self, [x + radius, y], [1.0, 0.0], w - 2.0 * radius);
        along(self, [x + radius, y + h], [1.0, 0.0], w - 2.0 * radius);
        along(self, [x, y + radius], [0.0, 1.0], h - 2.0 * radius);
        along(self, [x + w, y + radius], [0.0, 1.0], h - 2.0 * radius);
    }

    fn split(&mut self, menu: &Menu) {
        self.strip(10.0, 18.0);
        self.logo(true);
        let all = layouts(menu.riders);
        let at = menu.cursor(Screen::Split).min(all.len().saturating_sub(1));
        let rows: Vec<(String, bool)> = all.iter().map(|l| (layout_label(*l).to_string(), false)).collect();
        self.list(&rows, at, 19.0, 10.4, 18.0);
        if let Some(layout) = all.get(at) {
            let (x, y) = (34.0, 10.0);
            let h = self.heading("openMX", layout_label(*layout), "How the screen splits between the riders.", x, y, 40.0, 2.6);
            let (fw, fh) = (32.0, 18.0);
            let top = h + 1.2;
            self.round([x + 0.35, top + 0.4, fw, fh], 0.6, SHADOW, SHADOW);
            self.round([x, top, fw, fh], 0.6, INK, INK);
            let k = 1000.0;
            for (i, r) in layout.rects(k as usize * 16 / 9, k as usize).into_iter().enumerate() {
                let s = fw / (k * 16.0 / 9.0);
                let v = [x + r.x as f32 * s + 0.2, top + r.y as f32 * s + 0.2, r.w as f32 * s - 0.4, r.h as f32 * s - 0.4];
                if i < menu.riders {
                    self.round(v, 0.4, LIT_TOP, LIT_BOTTOM);
                    let size = (v[3] * 0.3).min(2.4);
                    self.text(Weight::Heavy, &format!("{}P", i + 1), v[0] + v[2] / 2.0, Pen::mid(v[1] + v[3] / 2.0, size), size, SLANT, CHALK, Align::Centre);
                } else {
                    self.round(v, 0.4, [40, 40, 40, 255], [40, 40, 40, 255]);
                }
            }
        }
        self.title(Screen::Split.title());
        self.hints(&[(Btn::Circle, "Back"), (Btn::Cross, "Select")]);
    }

    /// The keyboard: what is typed, the keys and the commands, in a box of glass mid-stage.
    fn keyboard(&mut self, menu: &Menu) {
        self.logo(true);
        let (x, y, w) = (18.0, 7.5, 44.0);
        let cols = KEY_ROWS[0].chars().count() as f32;
        let cell = w / cols;
        let typed_h = 3.4;
        let h = typed_h + 1.6 + cell * KEY_ROWS.len() as f32 + 3.6 + 1.2;
        self.round([x - 1.5, y - 1.2, w + 3.0, h], 1.2, [0, 0, 0, 170], [0, 0, 0, 170]);
        self.round([x, y, w, typed_h], 0.7, [0x3a, 0x05, 0x08, 255], [0x3a, 0x05, 0x08, 255]);
        self.ring([x, y, w, typed_h], 0.7, 0.1, [255, 255, 255, 70]);
        let typed = format!("{}_", menu.typing());
        self.text(Weight::Heavy, &typed, x + 1.0, Pen::mid(y + typed_h / 2.0, 2.0), 2.0, SLANT, CHALK, Align::Left);
        let (row, col) = menu.key;
        let top = y + typed_h + 1.0;
        let size = 1.6;
        for (r, keys) in KEY_ROWS.iter().enumerate() {
            for (c, k) in keys.chars().enumerate() {
                let b = [x + cell * c as f32 + 0.15, top + cell * r as f32 + 0.15, cell - 0.3, cell - 0.3];
                let s = k.to_string();
                let mid = Pen::mid(b[1] + b[3] / 2.0, size);
                if (r, c) == (row, col) {
                    self.pill(b, 0.2, 0.1);
                    let label = self.text(Weight::Heavy, &s, b[0] + b[2] / 2.0, mid, size, SLANT, CHALK, Align::Centre);
                    let pb = self.b(b);
                    self.parts.push(Part::Pill { pill: pb, label });
                } else {
                    self.round(b, 0.5, [255, 255, 255, 18], [255, 255, 255, 18]);
                    let label = self.text(Weight::Bold, &s, b[0] + b[2] / 2.0, mid, size, 0.0, BONE, Align::Centre);
                    self.parts.push(Part::Item(label));
                }
            }
        }
        let cy = top + cell * KEY_ROWS.len() as f32 + 0.6;
        let cmd = w / KEY_COMMANDS.len() as f32;
        for (c, label) in KEY_COMMANDS.iter().enumerate() {
            let b = [x + cmd * c as f32 + 0.2, cy, cmd - 0.4, 2.8];
            let mid = Pen::mid(cy + 1.4, 1.4);
            if (KEY_ROWS.len(), c) == (row, col) {
                self.pill(b, 0.2, 0.1);
                let lb = self.text(Weight::Heavy, label, b[0] + b[2] / 2.0, mid, 1.4, SLANT, CHALK, Align::Centre);
                let pb = self.b(b);
                self.parts.push(Part::Pill { pill: pb, label: lb });
            } else {
                self.round(b, 1.4, [255, 255, 255, 18], [255, 255, 255, 18]);
                let lb = self.text(Weight::Bold, label, b[0] + b[2] / 2.0, mid, 1.4, 0.0, BONE, Align::Centre);
                self.parts.push(Part::Item(lb));
            }
        }
        self.title(menu.typing_for.title());
        self.hints(&[(Btn::Square, "Delete"), (Btn::Circle, "Cancel"), (Btn::Cross, "Type")]);
    }

    fn games(&mut self, menu: &Menu) {
        self.strip(3.0, 38.0);
        self.logo(true);
        let at = menu.cursor(Screen::Games);
        let mut rows: Vec<Row> = menu
            .lobby
            .found
            .iter()
            .map(|l| {
                let track = TRACKS.iter().find(|t| t.0 == l.track).map_or(l.track.as_str(), |t| t.1);
                let mut r = Row::heading_row();
                r.kind = Kind::Action;
                r.label = l.name.clone();
                r.value = format!("{track}  {}/{}", l.taken, l.seats);
                r.off = (!l.same_build()).then(|| "Another version of the game: it can't join this one".to_string());
                r
            })
            .collect();
        let mut refresh = Row::heading_row();
        refresh.kind = Kind::Action;
        refresh.label = "Refresh".into();
        rows.push(refresh);
        self.rows(&rows, at, 3.8, 36.4, 8.2);
        let (x, w) = (45.0, 31.0);
        if let Some(l) = menu.lobby.found.get(at) {
            let track = TRACKS.iter().find(|t| t.0 == l.track).map_or(l.track.as_str(), |t| t.1);
            let mut about = format!("{track}, {} of {} seats taken.", l.taken, l.seats);
            if l.riding {
                about += " Riding now: you drop in.";
            }
            let y = self.heading("Open ride", &l.name, &about, x, 7.6, w, 2.3);
            if !l.same_build() {
                self.note("Another version of the game: it can't join this one", x, y + 0.9, w);
            }
        } else {
            let p = if menu.lobby.found.is_empty() { "No games listed yet: Refresh to look again." } else { "Look again for open rides." };
            self.heading("Find Games", "Refresh", p, x, 7.6, w, 2.3);
        }
        self.title(Screen::Games.title());
        self.hints(&[(Btn::Circle, "Back"), (Btn::Cross, "Join")]);
    }

    fn lobby(&mut self, menu: &Menu) {
        let l = &menu.lobby;
        self.strip(10.0, 18.0);
        self.logo(true);
        let rows: Vec<(String, bool)> = l.items().iter().map(|i| (i.label(l), false)).collect();
        let at = menu.cursor(Screen::Lobby);
        self.list(&rows, at, 19.0, 8.0, 18.0);
        let (x, w) = (32.0, (self.right().min(STAGE.0 + 8.0) - 33.0).max(42.0));
        self.round([x - 1.2, 7.0, w + 1.2, 31.0], 1.0, [0, 0, 0, 120], [0, 0, 0, 120]);
        match (&l.room, l.code.as_str()) {
            (None, _) => {
                self.heading("Lobby", "Not in a lobby yet", "", x, 8.0, w - 1.0, 2.3);
            }
            (Some(_), "") => {
                self.heading("Room code", "Waiting…", "", x, 8.0, w - 1.0, 2.3);
            }
            (Some(_), code) => {
                self.text(Weight::Bold, "ROOM CODE", x, 8.0 + CAP, 1.0, 0.0, EYEBROW, Align::Left);
                let size = 3.4;
                let spaced: String = code.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
                self.text(Weight::Heavy, &spaced, x, 9.6 + CAP * size, size, 0.14, CHALK, Align::Left);
            }
        }
        if let Some(room) = &l.room {
            let n = room.seats.len();
            let (cols, step) = if n <= SEAT_LINES { (1, 1.6) } else { (n.div_ceil(SEAT_COLUMN), 1.25) };
            let width = (w - 1.0) / cols as f32;
            let size = if cols == 1 { 1.15 } else { 0.95 };
            for i in 0..n {
                let line = if cols == 1 { l.seat_line(room, i) } else { l.seat_short(room, i) };
                let mine = room.seats[i].as_ref().is_some_and(|s| s.peer == l.me);
                let (col, row) = if cols == 1 { (0, i) } else { (i / SEAT_COLUMN, i % SEAT_COLUMN) };
                let (tx, ty) = (x + width * col as f32, 14.5 + step * row as f32);
                let line = self.fit(Weight::Bold, &line, size, width - 0.6);
                let (wt, c) = if mine { (Weight::Heavy, CHALK) } else { (Weight::Bold, BONE) };
                self.text(wt, &line, tx, ty + CAP * size, size, 0.0, c, Align::Left);
            }
        }
        let mut y = 28.5;
        for line in &l.chat {
            let line = self.fit(Weight::Bold, line, 1.05, w - 1.0);
            self.text(Weight::Bold, &line, x, y + CAP * 1.05, 1.05, 0.0, PAPER, Align::Left);
            y += 1.45;
        }
        self.title(Screen::Lobby.title());
        let ok = match l.items().get(at) {
            Some(Item::Start) => "Start",
            Some(Item::Ready) => "Ready",
            Some(Item::Leave) => "Leave",
            Some(Item::Chat) => "Chat",
            Some(Item::Modifiers) => "Open",
            _ => "Change",
        };
        self.hints(&[(Btn::Circle, "Leave"), (Btn::Cross, ok)]);
    }

    /// The splash: the last track's still dimmed over the whole frame, the logo, what to press or
    /// who 1P is, and the Quit popup when Back asked for it.
    fn splash(&mut self, menu: &Menu) {
        let whole = self.whole();
        self.fill(whole, INK);
        if let Some((image, t)) = self.track_picture(menu, menu.track_index(), 0) {
            self.cover(image, t, whole, SPLASH_DIM);
        }
        let (w, h) = (50.0, 6.25);
        let x = 40.0 - w * 0.68 / 2.0;
        self.picture("mainmenu_logo_shadow", [x + 0.5, 9.0 + 0.6, w, h], [0, 0, 0, 90], false);
        self.picture("mainmenu_logo", [x, 9.0, w, h], CHALK, false);
        let pb = self.b([x, 9.0, w, h]);
        self.parts.push(Part::Logo(pb));
        if menu.claimed.is_some() || menu.p1.is_some() {
            let who = format!("Player 1  {}", menu.p1_name);
            self.shadowed(Weight::Heavy, &who, 40.0, 27.0, 3.0, SLANT, PEACH, Align::Centre, [0.08, 0.1], [0, 0, 0, 102]);
        } else {
            let breath = (menu.tick as f32 * 2.6 / 60.0).cos().abs();
            self.fade = 0.6 + 0.4 * breath;
            let size = 3.0;
            let d = 3.0;
            let (press, or) = (self.wide(Weight::Heavy, "Press", size), self.wide(Weight::Heavy, "or", size));
            let sw = self.button_wide(Btn::Start, d);
            let total = press + 1.0 + sw + 1.0 + or + 1.0 + d;
            let mut x = 40.0 - total / 2.0;
            let mid = 26.0;
            let base = Pen::mid(mid, size);
            self.text(Weight::Heavy, "Press", x, base, size, SLANT, CHALK, Align::Left);
            x += press + 1.0;
            self.button(Btn::Start, x + sw / 2.0, mid, d);
            x += sw + 1.0;
            self.text(Weight::Heavy, "or", x, base, size, SLANT, CHALK, Align::Left);
            x += or + 1.0;
            self.button(Btn::Cross, x + d / 2.0, mid, d);
            self.fade = 1.0;
            self.text(Weight::Bold, "Enter or Space on the keyboard", 40.0, 31.0, 1.35, 0.0, BONE, Align::Centre);
            self.text(Weight::Bold, "Whoever presses it rides as Player 1", 40.0, 33.2, 1.35, 0.0, BONE, Align::Centre);
            if !menu.heard.is_empty() {
                let line = self.fit(Weight::Bold, &format!("Controllers: {}", names_line(&menu.heard)), 1.2, 70.0);
                self.text(Weight::Bold, &line, 40.0, 36.4, 1.2, 0.0, BONE, Align::Centre);
            }
            if !menu.skipped.is_empty() {
                let line = self.fit(Weight::Bold, &format!("Not read: {}", names_line(&menu.skipped)), 1.2, 70.0);
                self.text(Weight::Bold, &line, 40.0, 38.4, 1.2, 0.0, DUST, Align::Centre);
            }
        }
        if menu.p1.is_none() {
            self.hints(&[(Btn::Circle, "Quit"), (Btn::Cross, "Start")]);
        }
        if let Some(row) = menu.quitting() {
            self.popup("Quit the game?", &QUIT_ROWS, row);
        }
    }

    /// A two-row question over the screen, dimmed under it: the splash's Quit and the pause's
    /// confirm (P110).
    fn popup(&mut self, ask: &str, rows: &[&str], row: usize) {
        let whole = self.whole();
        self.fill(whole, [0, 0, 0, 150]);
        let b = [26.0, 14.0, 28.0, 17.0];
        self.round([b[0] + 0.4, b[1] + 0.45, b[2], b[3]], 1.2, SHADOW, SHADOW);
        self.round(b, 1.2, [0x2a, 0x14, 0x08, 255], [0x12, 0x09, 0x04, 255]);
        self.ring(b, 1.2, 0.12, [255, 255, 255, 60]);
        self.text(Weight::Heavy, ask, 40.0, 18.4, 2.3, SLANT, CHALK, Align::Centre);
        for (i, label) in rows.iter().enumerate() {
            let y = 21.0 + 4.0 * i as f32;
            let size = ITEM;
            let mid = y + size * 0.87;
            if i == row {
                let w = self.wide(Weight::Heavy, label, size);
                let pb = [40.0 - w / 2.0 - PILL_PAD, y, w + 2.0 * PILL_PAD, size * 1.74];
                self.pill(pb, 0.28 * size, 0.1 * size);
                let lb = self.text(Weight::Heavy, label, 40.0, Pen::mid(mid, size), size, SLANT, CHALK, Align::Centre);
                let pb = self.b(pb);
                self.parts.push(Part::Pill { pill: pb, label: lb });
            } else {
                let lb = self.text(Weight::Bold, label, 40.0, Pen::mid(mid, size), size, 0.0, BONE, Align::Centre);
                self.parts.push(Part::Item(lb));
            }
        }
    }
}

impl Row {
    /// A blank heading, for lists built by hand (the track pick, the games).
    fn heading_row() -> Row {
        Row {
            key: Key::Heading,
            kind: Kind::Heading,
            label: String::new(),
            value: String::new(),
            choices: Vec::new(),
            now: None,
            tips: Vec::new(),
            about: String::new(),
            off: None,
            meter: None,
            world: false,
            note: None,
        }
    }
}
