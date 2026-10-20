//! The original's own HUD over each view (V26): its speedometer and its free-ride radar, drawn from
//! the disc's art and laid out by the disc's layouts.
//!
//! Both are read out of `fstyleui.res`, the free-ride UI archive:
//!
//! - **The speedometer** (`ui\speedometer.lyt`, `ui\speedometer1.lyr`): the analog dial
//!   `speedometer_analog` (128x128 at 480,301), the odometer's plate `odometer_background` (128x32 at
//!   486,398) with its dim `00000.0` behind (`eurostileboldbold_14`, 204,204,204,77 at 501,403) and
//!   the miles over it (alpha 255, right edge at 573), and the needle, the model
//!   `ui\geom\speedometer_needle.psg` at 536,357. With two players or more the original drops the
//!   dial for the digital readout alone: `yearlingdashboldoblique_55`, right edge at 588, top at 329.
//! - **The radar** (`ui\freestyleradar.lyr`): `freestyle_radar`, a compass rose 128x128 centred on
//!   87,363 and turned so the rider's heading is up; the yellow `map_icon_player_freestyle` fixed at
//!   its middle; and an 8x8 `map_icon_hit_freestyle` for every other rider, at their bearing and
//!   distance off the rider's nose, `RadarRadius` 104 over a world `192` terrain samples across.
//!
//! What the numbers are, and where they come from, is in `research/oracle.txt` (V26). The pure
//! parts — [`mph`], [`speed_text`], [`needle_degrees`], [`odometer_text`] and [`radar_mark`] — take
//! plain numbers, so `tests/overlay.rs` can hold them to the riders without a disc.
//!
//! # Laying it out in a view
//!
//! The layouts are for one 640x448 screen. Each view gets the parts at the window's scale — the size
//! they are on a full screen, which is what the original's own two-player layouts keep
//! (`speedometerhorizontal1.lyt` puts the readout 69 lines off the bottom of a half-height view) —
//! with the radar counted from the view's bottom-left corner and the speedometer from its
//! bottom-right, so a wide or a narrow window (V24) moves them into its corners rather than
//! stretching them. A view too small for both at that size (four to a screen) gets them smaller, to
//! fit: they take 311 by 150 layout pixels between them.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use game_render::{Disc, Frame, Texture};
use game_sim::camera::Ridden;

use crate::menu::{Font, bilinear, blend, span};

/// The layouts' screen.
pub const LAYOUT: (f32, f32) = (640.0, 448.0);

/// Feet a second to the miles an hour the speedometer shows; its metric setting's `1.08` is not
/// offered here.
pub const MPH: f32 = 0.68182;
/// The needle's angle at standstill, in degrees anticlockwise off straight up, and how far each mile
/// an hour turns it back: 0 sits at the dial's lower left, 50 straight up and 100 at its lower right.
pub const NEEDLE_REST: f32 = 132.0;
pub const NEEDLE_PER_MPH: f32 = 2.64;
/// The needle stops here, past the dial's 100.
pub const NEEDLE_TOP_MPH: f32 = 105.0;
/// Feet to the odometer's miles.
pub const MILES_PER_FOOT: f32 = 0.000_189_393_93;

/// `freestyleradar.lyr`'s `RadarRadius`: a rider half the radar's span away sits `RadarRadius / 2`
/// pixels off its middle, on the rose's ring.
pub const RADAR_RADIUS: f32 = 104.0;
/// The radar's span, in terrain samples: the world it covers is the sample spacing times this.
pub const RADAR_SAMPLES: f32 = 192.0;
/// The radar's middle, from the view's bottom-left (`Radar Icon`'s and `Player Icon`'s `X`, `Y`).
pub const RADAR_AT: (f32, f32) = (87.0, 363.0);
const RADAR_SIZE: f32 = 128.0;
const PLAYER_SIZE: f32 = 16.0;
const MARK_SIZE: f32 = 8.0;
const PLAYER_COLOUR: [u8; 4] = [255, 255, 0, 255];
/// The colour the radar code gives another human's mark; an AI rider's would be `[0, 0, 255, 255]`,
/// but every rider here is a player.
pub const HUMAN_MARK: [u8; 4] = [0, 255, 255, 255];

/// The dial and the needle's pivot, from the view's bottom-right.
const DIAL: (f32, f32) = (480.0, 301.0);
const PIVOT: (f32, f32) = (536.0, 357.0);
/// The needle model's reach either side of its pivot and its half-widths there, in its own units,
/// which the layout's `DesiredSizeInPixels = 55` makes pixels (its bounding sphere is 27.3 across
/// half); and its colour, white vertices under the material's `ShaderColorScale = 2.0,1.2,0.0`.
const NEEDLE_TIP: (f32, f32) = (40.4, 0.7);
const NEEDLE_TAIL: (f32, f32) = (12.1, 3.6);
const NEEDLE_COLOUR: [u8; 4] = [255, 153, 0, 255];
const PIN_SIZE: f32 = 10.0;
const ODOMETER: (f32, f32) = (486.0, 398.0);
const ODOMETER_TEXT: (f32, f32, f32) = (501.0, 573.0, 403.0);
const ODOMETER_GHOST: [u8; 4] = [204, 204, 204, 77];
const TEXT: [u8; 4] = [204, 204, 204, 255];
/// The digital readout's right edge and top.
const DIGITAL: (f32, f32) = (588.0, 329.0);

/// The width and height the two parts need between them, in layout pixels: the radar's left edge at
/// 23 to the dial's right at 608 counted from the other side, and the dial's top to the bottom.
const NEEDS: (f32, f32) = (151.0 + 160.0, 150.0);

/// What a speed in feet a second reads in miles an hour, never below zero.
pub fn mph(speed: f32) -> f32 {
    (speed * MPH).max(0.0)
}

/// The speedometer's number for a speed in feet a second: `%.0f` of [`mph`].
pub fn speed_text(speed: f32) -> String {
    format!("{:.0}", mph(speed))
}

/// The needle's angle for a speed, in degrees anticlockwise off straight up.
pub fn needle_degrees(speed: f32) -> f32 {
    NEEDLE_REST - mph(speed).min(NEEDLE_TOP_MPH) * NEEDLE_PER_MPH
}

/// The odometer's text for a distance in feet: `%07.1f` miles, or `00000.0` under a tenth.
pub fn odometer_text(feet: f32) -> String {
    let miles = feet * MILES_PER_FOOT;
    if miles >= 0.1 { format!("{miles:07.1}") } else { "00000.0".into() }
}

/// The world the radar covers across, for a terrain's sample spacing.
pub fn radar_span(spacing: f32) -> f32 {
    spacing * RADAR_SAMPLES
}

/// A ground direction's angle, `x` and `z`, as the radar takes it: off `+z` towards `+x`, `0..2π`.
pub fn bearing(x: f32, z: f32) -> f32 {
    let len = (x * x + z * z).sqrt();
    if len == 0.0 {
        return 0.0;
    }
    let a = (z / len).clamp(-1.0, 1.0).acos();
    if x < 0.0 { TAU - a } else { a }
}

/// How far the rose turns, clockwise on the screen in radians, for a rider facing `forward` (`x`,
/// `z`): `2π` less the heading, so the way the rider faces is up.
pub fn radar_turn(forward: [f32; 2]) -> f32 {
    TAU - bearing(forward[0], forward[1])
}

/// Where another rider's mark goes on the radar, in layout pixels from the view's bottom-left: `own`
/// and `other` are the two riders' ground positions (`x`, `z`), `forward` the way the viewing rider
/// faces and `span` the world the radar covers ([`radar_span`]). Up is ahead, right is to the
/// rider's right (`+x` off a nose along `+z`); past half the span a rider stays on the ring.
pub fn radar_mark(own: [f32; 2], forward: [f32; 2], other: [f32; 2], span: f32) -> (f32, f32) {
    let rel = [other[0] - own[0], other[1] - own[1]];
    let d = (rel[0] * rel[0] + rel[1] * rel[1]).sqrt().min(span / 2.0);
    let r = d * RADAR_RADIUS / span;
    let mut a = bearing(rel[0], rel[1]) - bearing(forward[0], forward[1]);
    if a > PI {
        a -= TAU;
    } else if a < -PI {
        a += TAU;
    }
    (RADAR_AT.0 + r * a.sin(), RADAR_AT.1 - r * a.cos())
}

/// The distance a rider has covered, which the odometer shows. The original keeps it in its
/// statistics tracker; this adds up the rider's path a frame at a time.
#[derive(Clone, Debug, Default)]
pub struct Odometer {
    pub feet: f32,
    last: Option<[f32; 3]>,
}

impl Odometer {
    /// A step longer than this in one frame is a restart or a teleport, not riding.
    const JUMP: f32 = 100.0;

    pub fn step(&mut self, at: [f32; 3]) {
        if let Some(l) = self.last {
            let d = ((at[0] - l[0]).powi(2) + (at[1] - l[1]).powi(2) + (at[2] - l[2]).powi(2)).sqrt();
            if d < Self::JUMP {
                self.feet += d;
            }
        }
        self.last = Some(at);
    }
}

/// The art, off the disc.
pub struct Art {
    textures: HashMap<&'static str, Texture>,
    /// `yearlingdashboldoblique_55`, the digital readout's.
    digits: Font,
    /// `eurostileboldbold_14`, the odometer's.
    small: Font,
}

const TEXTURES: [&str; 6] = [
    "speedometer_analog",
    "speedometer_needlepin",
    "odometer_background",
    "freestyle_radar",
    "map_icon_player_freestyle",
    "map_icon_hit_freestyle",
];

impl Art {
    pub fn load(disc: &mut Disc) -> Result<Art, String> {
        let ui = disc.res("fstyleui.res")?;
        let mut textures = HashMap::new();
        for name in TEXTURES {
            let path = format!("ui\\tex\\{name}.pst");
            textures.insert(name, Texture::parse(&ui.read_suffix(&path)?).map_err(|e| format!("{path}: {e}"))?);
        }
        let font = |name: &str| -> Result<Font, String> {
            let cel = ui.read_suffix(&format!("font\\eng\\{name}.cel"))?;
            let atlas = Texture::parse(&ui.read_suffix(&format!("font\\eng\\{name}.pst"))?)
                .map_err(|e| format!("{name}.pst: {e}"))?;
            Font::parse(&cel, atlas).map_err(|e| format!("{name}.cel: {e}"))
        };
        Ok(Art { digits: font("yearlingdashboldoblique_55")?, small: font("eurostileboldbold_14")?, textures })
    }

    /// Draw view `v`'s HUD into `frame`, which is that view's picture: `riders` is every rider's
    /// camera target in seat order, `span` the radar's ([`radar_span`]), `feet` the view's rider's
    /// odometer and `window` the whole window's size, which sets the parts' scale.
    pub fn draw(
        &self,
        frame: &mut Frame,
        window: (usize, usize),
        riders: &[Ridden],
        v: usize,
        span: f32,
        feet: f32,
    ) -> Drawn {
        let mut pen = Pen::new(frame, window);
        let me = &riders[v];
        let speed = me.speed.0;
        let mut drawn = Drawn::default();

        // The radar: the rose turned under the fixed arrow, the other riders between them.
        let own = [me.target.body_point.x(), me.target.body_point.z()];
        let forward = [me.target.forward.x(), me.target.forward.z()];
        let turn = radar_turn(forward);
        pen.corner = Corner::BottomLeft;
        if let Some(t) = self.textures.get("freestyle_radar") {
            pen.turned(t, RADAR_AT, RADAR_SIZE, turn);
        }
        for (i, r) in riders.iter().enumerate() {
            if i == v {
                continue;
            }
            let at = radar_mark(own, forward, [r.target.body_point.x(), r.target.body_point.z()], span);
            self.icon(&mut pen, "map_icon_hit_freestyle", at, MARK_SIZE, HUMAN_MARK);
            drawn.marks.push((i, pen.px(at)));
        }
        self.icon(&mut pen, "map_icon_player_freestyle", RADAR_AT, PLAYER_SIZE, PLAYER_COLOUR);
        drawn.player = pen.px(RADAR_AT);
        drawn.turn = turn;

        // The speedometer: the dial with its needle and odometer for one rider, the number for more.
        pen.corner = Corner::BottomRight;
        if riders.len() >= 2 {
            let text = speed_text(speed);
            pen.text(&self.digits, &text, DIGITAL.0, DIGITAL.1, true, TEXT);
            drawn.speed = Some(text);
        } else {
            if let Some(t) = self.textures.get("speedometer_analog") {
                pen.blit(t, (DIAL.0, DIAL.1, 128.0, 128.0), [255; 4]);
            }
            let degrees = needle_degrees(speed);
            pen.needle(PIVOT, degrees);
            if let Some(t) = self.textures.get("speedometer_needlepin") {
                pen.pin(t, PIVOT, PIN_SIZE);
            }
            drawn.needle = Some(degrees);
            if let Some(t) = self.textures.get("odometer_background") {
                pen.blit(t, (ODOMETER.0, ODOMETER.1, 128.0, 32.0), [255; 4]);
            }
            let miles = odometer_text(feet);
            pen.text(&self.small, "00000.0", ODOMETER_TEXT.0, ODOMETER_TEXT.2, false, ODOMETER_GHOST);
            pen.text(&self.small, &miles, ODOMETER_TEXT.1, ODOMETER_TEXT.2, true, TEXT);
            drawn.odometer = Some(miles);
        }
        drawn.scale = pen.s;
        drawn
    }

    /// A texture `size` square centred on `at`.
    fn icon(&self, pen: &mut Pen, name: &str, at: (f32, f32), size: f32, tint: [u8; 4]) {
        if let Some(t) = self.textures.get(name) {
            pen.blit(t, (at.0 - size / 2.0, at.1 - size / 2.0, size, size), tint);
        }
    }
}

/// What [`Art::draw`] put where, in the view's pixels, so a test can hold it to the ride.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawn {
    /// The digital readout, when it was drawn.
    pub speed: Option<String>,
    /// The needle's angle, when the dial was drawn.
    pub needle: Option<f32>,
    pub odometer: Option<String>,
    /// The middle of the radar, where the rider's own arrow is.
    pub player: (f32, f32),
    /// Every other rider's mark: their seat and the middle of their mark.
    pub marks: Vec<(usize, (f32, f32))>,
    /// How far the rose was turned, clockwise in radians.
    pub turn: f32,
    /// View pixels per layout pixel.
    pub scale: f32,
}

#[derive(Clone, Copy)]
enum Corner {
    BottomLeft,
    BottomRight,
}

/// A view's frame, and the scale and corner the layout's numbers are put in it by.
struct Pen<'a> {
    frame: &'a mut Frame,
    s: f32,
    corner: Corner,
}

impl<'a> Pen<'a> {
    fn new(frame: &'a mut Frame, window: (usize, usize)) -> Pen<'a> {
        let (w, h) = (frame.width.max(1) as f32, frame.height.max(1) as f32);
        let whole = (window.0.max(1) as f32 / LAYOUT.0).min(window.1.max(1) as f32 / LAYOUT.1);
        let s = whole.min(w / NEEDS.0).min(h / NEEDS.1);
        Pen { frame, s, corner: Corner::BottomLeft }
    }

    /// A layout point in the view's pixels, counted from the current corner.
    fn px(&self, at: (f32, f32)) -> (f32, f32) {
        let (w, h) = (self.frame.width as f32, self.frame.height as f32);
        let y = h - (LAYOUT.1 - at.1) * self.s;
        match self.corner {
            Corner::BottomLeft => (at.0 * self.s, y),
            Corner::BottomRight => (w - (LAYOUT.0 - at.0) * self.s, y),
        }
    }

    /// A whole texture over a layout box (left, top, width, height), multiplied by `tint`.
    fn blit(&mut self, t: &Texture, r: (f32, f32, f32, f32), tint: [u8; 4]) {
        let src = [0.0, 0.0, t.width as f32, t.height as f32];
        self.blit_src(t, src, r, tint);
    }

    fn blit_src(&mut self, t: &Texture, src: [f32; 4], r: (f32, f32, f32, f32), tint: [u8; 4]) {
        let (fx0, fy0) = self.px((r.0, r.1));
        let (fx1, fy1) = (fx0 + r.2 * self.s, fy0 + r.3 * self.s);
        let (x0, x1) = span(fx0, fx1, self.frame.width);
        let (y0, y1) = span(fy0, fy1, self.frame.height);
        for y in y0..y1 {
            let v = src[1] + (y as f32 + 0.5 - fy0) / (fy1 - fy0) * (src[3] - src[1]);
            for x in x0..x1 {
                let u = src[0] + (x as f32 + 0.5 - fx0) / (fx1 - fx0) * (src[2] - src[0]);
                let c = bilinear(t, u, v, src);
                let mul = |i: usize| (c[i] as u32 * tint[i] as u32 / 255) as u8;
                blend(self.frame, x, y, [mul(0), mul(1), mul(2), mul(3)]);
            }
        }
    }

    /// A `size` square texture centred on `at`, turned clockwise by `turn` radians.
    fn turned(&mut self, t: &Texture, at: (f32, f32), size: f32, turn: f32) {
        let (cx, cy) = self.px(at);
        let reach = size * self.s * std::f32::consts::FRAC_1_SQRT_2;
        let (x0, x1) = span(cx - reach, cx + reach, self.frame.width);
        let (y0, y1) = span(cy - reach, cy + reach, self.frame.height);
        let (sin, cos) = turn.sin_cos();
        let (tw, th) = (t.width as f32, t.height as f32);
        let all = [0.0, 0.0, tw, th];
        for y in y0..y1 {
            for x in x0..x1 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / self.s, (y as f32 + 0.5 - cy) / self.s);
                // Back through the turn into the texture's own square.
                let (lx, ly) = (dx * cos + dy * sin, -dx * sin + dy * cos);
                let (u, v) = ((lx / size + 0.5) * tw, (ly / size + 0.5) * th);
                if !(0.0..tw).contains(&u) || !(0.0..th).contains(&v) {
                    continue;
                }
                blend(self.frame, x, y, bilinear(t, u, v, all));
            }
        }
    }

    /// The needle: a tapered bar through `pivot`, `degrees` anticlockwise off straight up, its edges
    /// smoothed by four samples a pixel.
    fn needle(&mut self, pivot: (f32, f32), degrees: f32) {
        let (cx, cy) = self.px(pivot);
        let a = -degrees.to_radians();
        // Along the needle towards its tip, and across it, on a screen whose y runs down.
        let (dx, dy) = (a.sin(), -a.cos());
        let reach = NEEDLE_TIP.0.max(NEEDLE_TAIL.0) * self.s + 1.0;
        let (x0, x1) = span(cx - reach, cx + reach, self.frame.width);
        let (y0, y1) = span(cy - reach, cy + reach, self.frame.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let mut hits = 0;
                for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let (px, py) = ((x as f32 + ox - cx) / self.s, (y as f32 + oy - cy) / self.s);
                    let along = px * dx + py * dy;
                    let across = (px * -dy + py * dx).abs();
                    if along < -NEEDLE_TAIL.0 || along > NEEDLE_TIP.0 {
                        continue;
                    }
                    let k = (along + NEEDLE_TAIL.0) / (NEEDLE_TIP.0 + NEEDLE_TAIL.0);
                    if across <= NEEDLE_TAIL.1 + k * (NEEDLE_TIP.1 - NEEDLE_TAIL.1) {
                        hits += 1;
                    }
                }
                if hits > 0 {
                    let mut c = NEEDLE_COLOUR;
                    c[3] = (c[3] as u32 * hits / 4) as u8;
                    blend(self.frame, x, y, c);
                }
            }
        }
    }

    /// The needle's pin: its texture over a disc `size` across centred on `at`, the round cap the
    /// pin's mesh is.
    fn pin(&mut self, t: &Texture, at: (f32, f32), size: f32) {
        let (cx, cy) = self.px(at);
        let r = size * self.s / 2.0;
        let (x0, x1) = span(cx - r, cx + r, self.frame.width);
        let (y0, y1) = span(cy - r, cy + r, self.frame.height);
        let all = [0.0, 0.0, t.width as f32, t.height as f32];
        for y in y0..y1 {
            for x in x0..x1 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let (u, v) = ((dx / r + 1.0) / 2.0 * all[2], (dy / r + 1.0) / 2.0 * all[3]);
                blend(self.frame, x, y, bilinear(t, u, v, all));
            }
        }
    }

    /// `text` with its top at `y` and its left edge at `x`; or, when `right`, its last character
    /// starting at `x`. That is how the layouts' right-justified numbers read: the odometer's miles
    /// (`X=573`) lie exactly over its dim `00000.0` (`X=501`, seven 12-pixel digits) only so.
    fn text(&mut self, font: &Font, text: &str, x: f32, y: f32, right: bool, colour: [u8; 4]) {
        let but_last = text.char_indices().last().map_or("", |(i, _)| &text[..i]);
        let mut pen = if right { x - font.measure(but_last) } else { x };
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let g = font.glyph(c);
            pen += g.before;
            if c != ' ' {
                let h = g.uv[3] - g.uv[1];
                self.blit_src(&font.atlas, g.uv, (pen, y, g.width, h), colour);
            }
            pen += g.width + g.after;
            if let Some(&n) = chars.peek() {
                pen += g.kern.iter().find(|k| k.0 == n as u32).map_or(0.0, |k| k.1);
            }
        }
    }
}
