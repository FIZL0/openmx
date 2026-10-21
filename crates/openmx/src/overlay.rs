//! The original's own HUD over each view (V26): its speedometer and its free-ride radar, drawn from
//! the disc's art and laid out by the disc's layouts.
//!
//! Both are read out of `fstyleui.res`, the free-ride UI archive:
//!
//! - **The speedometer** (`ui\speedometer.lyt`, `ui\speedometer1.lyr`): the analog dial
//!   `speedometer_analog` (128x128 at 480,301), the odometer's plate `odometer_background` (128x32 at
//!   486,398) with its dim `00000.0` behind (`eurostileboldbold_14`, 204,204,204,77 at 501,403) and
//!   the miles over it (alpha 255, right edge at 573), and the needle, the model
//!   `ui\geom\speedometer_needle.psg` at 536,357; or the digital readout,
//!   `yearlingdashboldoblique_55`, right edge at 588, top at 329 — the `Speedometer Digital`
//!   container, which has no odometer (its `SpeedometerBackground_digital` is in no container, so
//!   is never drawn). The digital readout is the default here, the dial a setting (V42), as the
//!   original's own overlay options choose; with two players or more the original forces the
//!   readout, and so does this. A split view's readout and meter are placed by the split layouts
//!   ([`Place`]).
//! - **The radar** (`ui\freestyleradar.lyr`): `freestyle_radar`, a compass rose 128x128 centred on
//!   87,363 and turned so the rider's heading is up; the yellow `map_icon_player_freestyle` fixed at
//!   its middle; and an 8x8 `map_icon_hit_freestyle` for every other rider, at their bearing and
//!   distance off the rider's nose, `RadarRadius` 104 over a world `192` terrain samples across.
//!
//! - **The score** (`ui\stuntscoreoverlays.lyt`'s `Score`, at 14,48 from the top-left, and
//!   `Overlay_1p_Score.fsh`): a dark bar, its label end darker, `SCORE` in `eurostileboldbold_12` and
//!   the number in `eurostileboldbold_14`, both 200,200,200. The bar is a Flash movie's shapes,
//!   which nothing here plays, so it is drawn to the original's own picture by eye (V28), at the
//!   movie's own tints (black at 0x80 over the label, 0x66 over the rest).
//! - **The preload meter** (`speedometer.lyt`'s `PreloadMeter`, `Ingame_Preloadmeter.fsh`): the dark
//!   capsule right of the speed, black at the movie's 0xb3, likewise drawn by eye (V28), in every
//!   view (V42). The plane has neither it nor the odometer: the original's biplane shows the speed
//!   alone.
//!
//! What the numbers are, and where they come from, is in `research/oracle.txt` (V26, V28). The pure
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

use game_render::gpu::{Shape, Sprite, clamp_of};
use game_render::{Disc, Frame, Texture};
use game_sim::camera::Ridden;

use crate::menu::{Font, Speedometer, bilinear, blend, span};

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

/// Which of the disc's speedometer layouts a view's readout and preload meter are placed by (V42):
/// `speedometer.lyt` for a whole screen; `speedometerhorizontal1.lyt` and `…2.lyt` for the views of
/// the top row and the rest of a screen cut across; `speedometervertical1free.lyt` and `…2.lyt` —
/// which say the same — for views side by side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    #[default]
    Full,
    Top,
    Bottom,
    Side,
}

impl Place {
    /// View `v` of `view`'s size in a window of `window`'s, its views in rows of equal ones.
    pub fn of(view: (usize, usize), window: (usize, usize), v: usize) -> Place {
        let cols = (window.0 as f32 / view.0.max(1) as f32).round().max(1.0) as usize;
        let rows = (window.1 as f32 / view.1.max(1) as f32).round().max(1.0) as usize;
        match (cols, rows) {
            (_, 2..) if v / cols == 0 => Place::Top,
            (_, 2..) => Place::Bottom,
            (2.., _) => Place::Side,
            _ => Place::Full,
        }
    }

    /// The digital readout's right edge and top, and the preload meter's left and top, as
    /// [`Corner::BottomRight`] counts them on a whole screen. A split layout's text is in its own
    /// view's pixels (a 640x224 or 320x448 one), which moves here by the part of the screen the
    /// view is short of; its meter, a Flash movie, is in the whole screen's — the movie's own
    /// place, 599,326, moved by the layout's `X`/`Y` — so the bottom view's meter is the screen's
    /// lower half's, and the right-hand view's sits at 279 in its own pixels like the left's.
    pub fn speedometer(self) -> ((f32, f32), (f32, f32)) {
        // Each layout's text, the size of the view it is in, the meter movie's `X`/`Y`, and where
        // that view starts on the screen (the left view's numbers serve the right's).
        let (w, h) = LAYOUT;
        let (text, view, (mx, my), within) = match self {
            Place::Full => (DIGITAL, LAYOUT, (0.0, -12.0), (0.0, 0.0)),
            Place::Top => ((588.0, 155.0), (w, h / 2.0), (0.0, -194.0), (0.0, 0.0)),
            Place::Bottom => ((588.0, 145.0), (w, h / 2.0), (0.0, 10.0), (0.0, h / 2.0)),
            Place::Side => ((270.0, 340.0), (w / 2.0, h), (-320.0, 0.0), (0.0, 0.0)),
        };
        let (dx, dy) = (w - view.0, h - view.1);
        let meter = (PRELOAD_MOVIE.0 + mx - within.0 + dx, PRELOAD_MOVIE.1 + my - within.1 + dy);
        ((text.0 + dx, text.1 + dy), meter)
    }
}

/// The score bar (left, top, width, height), from the view's top-left, where its darker label end
/// stops, and the label's and the number's left edges and tops — measured off the original's frame.
const SCORE_BAR: (f32, f32, f32, f32) = (14.0, 25.0, 168.0, 20.5);
const SCORE_LABEL_END: f32 = 82.0;
const SCORE_LABEL: (f32, f32) = (31.0, 26.5);
const SCORE_TEXT: (f32, f32) = (88.0, 28.0);
/// How far below the score's label the modified-ride mark sits (P55).
const MODIFIED_DROP: f32 = 24.0;
/// The movie's tints over its black shapes: the label end's, the rest's, the preload meter's.
const SCORE_LABEL_SHADE: u8 = 0x80;
const SCORE_SHADE: u8 = 0x66;
const PRELOAD_SHADE: u8 = 0xb3;
const SCORE_COLOUR: [u8; 4] = [200, 200, 200, 255];
/// The preload meter's capsule's size, and the colour of its charge. On a whole screen it is at
/// 599,314, from the view's bottom-right: the movie's own place, [`PRELOAD_MOVIE`], moved by
/// `speedometer.lyt`'s `Y=-12`.
const PRELOAD_METER: (f32, f32) = (20.0, 85.0);
const PRELOAD_MOVIE: (f32, f32) = (599.0, 326.0);
const PRELOAD_COLOUR: [u8; 4] = [255, 153, 0, 255];
/// How far the charge sits inside the capsule.
const PRELOAD_INSET: f32 = 3.0;

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

/// The score's text: the number, unpadded.
pub fn score_text(score: u32) -> String {
    score.to_string()
}

/// How full the preload meter is, `0..=1`: the shove a preload is giving, as a share of the bike's
/// `MaxPreload`, running down with its timer — `force` and `timer` the preload's own, `max` and
/// `time` the `.phy`'s `MaxPreload` and `PreloadTime`. Empty unless it `firing`.
pub fn preload_fill(firing: bool, force: f32, max: f32, timer: f32, time: f32) -> f32 {
    if !firing || max <= 0.0 || time <= 0.0 {
        return 0.0;
    }
    ((force / max) * (timer / time)).clamp(0.0, 1.0)
}

/// What the view's own rider shows besides where everybody is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gauges {
    /// The odometer's distance, in feet.
    pub feet: f32,
    /// On the bike: the preload meter, and the dial's odometer. The plane has the speed alone.
    pub bike: bool,
    /// The preload meter, [`preload_fill`].
    pub preload: f32,
    /// The stunt score.
    pub score: u32,
    /// The ride has a game modifier off the original's (P55): the port's own mark under the score.
    pub modified: bool,
    /// The speed's style, the settings' (V42). A split view is digital whichever it is, as the
    /// original forces it with two players or more.
    pub speedometer: Speedometer,
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
    /// `eurostileboldbold_14`, the odometer's and the score's number's.
    small: Font,
    /// `eurostileboldbold_12`, the score's label.
    label: Font,
}

/// The fonts' atlases' places in [`Art::images`], after the textures.
const DIGITS: usize = TEXTURES.len();
const SMALL: usize = TEXTURES.len() + 1;
const LABEL: usize = TEXTURES.len() + 2;

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
        let (digits, small) = (font("yearlingdashboldoblique_55")?, font("eurostileboldbold_14")?);
        Ok(Art { digits, small, label: font("eurostileboldbold_12")?, textures })
    }

    /// Draw view `v`'s HUD into `frame`, which is that view's picture: `riders` is every rider's
    /// camera target in seat order, `span` the radar's ([`radar_span`]), `gauges` the view's rider's
    /// own and `window` the whole window's size, which sets the parts' scale.
    pub fn draw(
        &self,
        frame: &mut Frame,
        window: (usize, usize),
        riders: &[Ridden],
        v: usize,
        span: f32,
        gauges: &Gauges,
    ) -> Drawn {
        let (w, h) = (frame.width, frame.height);
        let place = Place::of((w, h), window, v);
        self.paint(&mut Pen::new(Ink::Frame(frame), (w, h), window), place, riders, v, span, gauges)
    }

    /// Every picture [`Art::sprites`] counts into, in its order: the textures, then the three fonts'
    /// atlases. The GPU takes them once ([`game_render::gpu::Renderer::set_images`]).
    pub fn images(&self) -> Vec<&Texture> {
        let mut all: Vec<&Texture> = TEXTURES.iter().filter_map(|n| self.textures.get(n)).collect();
        if all.len() != TEXTURES.len() {
            return Vec::new();
        }
        all.extend([&self.digits.atlas, &self.small.atlas, &self.label.atlas]);
        all
    }

    /// [`Art::draw`] as the GPU draws it (V30): view `v`'s HUD as sprites into `out`, for a view
    /// whose rectangle in the frame is `view` (x, y, width, height). The same layout, the same
    /// scale, the same pictures; nothing is drawn on the CPU.
    #[allow(clippy::too_many_arguments)]
    pub fn sprites(
        &self,
        out: &mut Vec<Sprite>,
        view: (usize, usize, usize, usize),
        window: (usize, usize),
        riders: &[Ridden],
        v: usize,
        span: f32,
        gauges: &Gauges,
    ) -> Drawn {
        let ink = Ink::Sprites { out, at: (view.0 as f32, view.1 as f32) };
        let place = Place::of((view.2, view.3), window, v);
        self.paint(&mut Pen::new(ink, (view.2, view.3), window), place, riders, v, span, gauges)
    }

    #[allow(clippy::too_many_arguments)]
    fn paint(&self, pen: &mut Pen, place: Place, riders: &[Ridden], v: usize, span: f32, gauges: &Gauges) -> Drawn {
        let me = &riders[v];
        let speed = me.speed.0;
        let mut drawn = Drawn::default();

        // The radar: the rose turned under the fixed arrow, the other riders between them.
        let own = [me.target.body_point.x(), me.target.body_point.z()];
        let forward = [me.target.forward.x(), me.target.forward.z()];
        let turn = radar_turn(forward);
        pen.corner = Corner::BottomLeft;
        if let Some(t) = self.texture("freestyle_radar") {
            pen.turned(t, RADAR_AT, RADAR_SIZE, turn);
        }
        for (i, r) in riders.iter().enumerate() {
            if i == v {
                continue;
            }
            let at = radar_mark(own, forward, [r.target.body_point.x(), r.target.body_point.z()], span);
            self.icon(pen, "map_icon_hit_freestyle", at, MARK_SIZE, HUMAN_MARK);
            drawn.marks.push((i, pen.px(at)));
        }
        self.icon(pen, "map_icon_player_freestyle", RADAR_AT, PLAYER_SIZE, PLAYER_COLOUR);
        drawn.player = pen.px(RADAR_AT);
        drawn.turn = turn;

        // The score, top left.
        pen.corner = Corner::TopLeft;
        let (x, y, w, h) = SCORE_BAR;
        pen.capsule((x, y, w, h), [0, 0, 0, SCORE_LABEL_SHADE], SCORE_LABEL_END, [0, 0, 0, SCORE_SHADE]);
        pen.text((&self.label, LABEL), "SCORE", SCORE_LABEL.0, SCORE_LABEL.1, false, SCORE_COLOUR);
        let score = score_text(gauges.score);
        pen.text((&self.small, SMALL), &score, SCORE_TEXT.0, SCORE_TEXT.1, false, SCORE_COLOUR);
        drawn.score = score;
        // A modified ride says so under the score, in the label's face: the original has no such
        // thing, so where it goes is the port's.
        if gauges.modified {
            pen.text((&self.label, LABEL), "MODIFIED", SCORE_LABEL.0, SCORE_LABEL.1 + MODIFIED_DROP, false, SCORE_COLOUR);
            drawn.modified = true;
        }

        // The speedometer (V42): the number, or for one rider who asked for it the dial with its
        // needle and odometer; and on the bike, the preload meter beside either, where the view's
        // layout puts it. The original's two-player layouts keep the dial where a whole screen's
        // is, off a half-height view, and its code forces the number there: a split is digital.
        pen.corner = Corner::BottomRight;
        let dial = gauges.speedometer == Speedometer::Analog && place == Place::Full && riders.len() < 2;
        let (digital, meter) = place.speedometer();
        drawn.place = place;
        if gauges.bike {
            let ((x, y), (w, h)) = (meter, PRELOAD_METER);
            pen.capsule((x, y, w, h), [0, 0, 0, PRELOAD_SHADE], f32::MAX, [0; 4]);
            let fill = gauges.preload.clamp(0.0, 1.0);
            if fill > 0.0 {
                let (i, inner) = (PRELOAD_INSET, h - 2.0 * PRELOAD_INSET);
                let top = y + i + inner * (1.0 - fill);
                pen.capsule((x + i, top, w - 2.0 * i, y + h - i - top), PRELOAD_COLOUR, f32::MAX, [0; 4]);
            }
            drawn.preload = Some(fill);
        }
        if !dial {
            // Its right edge at the layout's `X`: the last box's right edge, so the last glyph's
            // (negative) gap to a next pen does not count. The original's "51" inks 539 to 587 of a
            // 640 screen (V42, PCSX2); the last-character rule the odometer reads by would put it
            // past the meter.
            let text = speed_text(speed);
            let last = text.chars().last().map_or(0.0, |c| self.digits.glyph(c).after);
            let left = digital.0 - (self.digits.measure(&text) - last);
            pen.text((&self.digits, DIGITS), &text, left, digital.1, false, TEXT);
            drawn.speed = Some(text);
        } else {
            if let Some(t) = self.texture("speedometer_analog") {
                pen.blit(t, (DIAL.0, DIAL.1, 128.0, 128.0), [255; 4]);
            }
            let degrees = needle_degrees(speed);
            pen.needle(PIVOT, degrees);
            if let Some(t) = self.texture("speedometer_needlepin") {
                pen.pin(t, PIVOT, PIN_SIZE);
            }
            drawn.needle = Some(degrees);
            if gauges.bike {
                if let Some(t) = self.texture("odometer_background") {
                    pen.blit(t, (ODOMETER.0, ODOMETER.1, 128.0, 32.0), [255; 4]);
                }
                let miles = odometer_text(gauges.feet);
                pen.text((&self.small, SMALL), "00000.0", ODOMETER_TEXT.0, ODOMETER_TEXT.2, false, ODOMETER_GHOST);
                pen.text((&self.small, SMALL), &miles, ODOMETER_TEXT.1, ODOMETER_TEXT.2, true, TEXT);
                drawn.odometer = Some(miles);
            }
        }
        drawn.scale = pen.s;
        drawn
    }

    /// A texture by name, and its place in [`Art::images`].
    fn texture(&self, name: &str) -> Option<(&Texture, usize)> {
        let i = TEXTURES.iter().position(|n| *n == name)?;
        Some((self.textures.get(name)?, i))
    }

    /// A texture `size` square centred on `at`.
    fn icon(&self, pen: &mut Pen, name: &str, at: (f32, f32), size: f32, tint: [u8; 4]) {
        if let Some(t) = self.texture(name) {
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
    /// The score's number.
    pub score: String,
    /// How full the preload meter was drawn, when it was (the bike's).
    pub preload: Option<f32>,
    /// The middle of the radar, where the rider's own arrow is.
    pub player: (f32, f32),
    /// Every other rider's mark: their seat and the middle of their mark.
    pub marks: Vec<(usize, (f32, f32))>,
    /// How far the rose was turned, clockwise in radians.
    pub turn: f32,
    /// View pixels per layout pixel.
    pub scale: f32,
    /// The modified-ride mark was drawn.
    pub modified: bool,
    /// The layout the speedometer was placed by.
    pub place: Place,
}

#[derive(Clone, Copy)]
enum Corner {
    TopLeft,
    BottomLeft,
    BottomRight,
}

/// Where a [`Pen`] puts what it draws: into a frame's pixels, or as sprites at `at` in the frame.
enum Ink<'a> {
    Frame(&'a mut Frame),
    Sprites { out: &'a mut Vec<Sprite>, at: (f32, f32) },
}

/// A picture and its place in [`Art::images`].
type Image<'t> = (&'t Texture, usize);

/// A view, and the scale and corner the layout's numbers are put in it by.
struct Pen<'a> {
    ink: Ink<'a>,
    /// The view's size in pixels.
    w: usize,
    h: usize,
    s: f32,
    corner: Corner,
}

impl<'a> Pen<'a> {
    fn new(ink: Ink<'a>, (w, h): (usize, usize), window: (usize, usize)) -> Pen<'a> {
        let (fw, fh) = (w.max(1) as f32, h.max(1) as f32);
        let whole = (window.0.max(1) as f32 / LAYOUT.0).min(window.1.max(1) as f32 / LAYOUT.1);
        let s = whole.min(fw / NEEDS.0).min(fh / NEEDS.1);
        Pen { ink, w, h, s, corner: Corner::BottomLeft }
    }

    /// A layout point in the view's pixels, counted from the current corner.
    fn px(&self, at: (f32, f32)) -> (f32, f32) {
        let (w, h) = (self.w as f32, self.h as f32);
        let y = h - (LAYOUT.1 - at.1) * self.s;
        match self.corner {
            Corner::TopLeft => (at.0 * self.s, at.1 * self.s),
            Corner::BottomLeft => (at.0 * self.s, y),
            Corner::BottomRight => (w - (LAYOUT.0 - at.0) * self.s, y),
        }
    }

    /// A sprite whose corners are `corners` in the view's pixels.
    fn sprite(&mut self, mut sprite: Sprite) {
        if let Ink::Sprites { out, at } = &mut self.ink {
            for c in &mut sprite.corners {
                (c[0], c[1]) = (c[0] + at.0, c[1] + at.1);
            }
            out.push(sprite);
        }
    }

    /// A whole texture over a layout box (left, top, width, height), multiplied by `tint`.
    fn blit(&mut self, t: Image, r: (f32, f32, f32, f32), tint: [u8; 4]) {
        let src = [0.0, 0.0, t.0.width as f32, t.0.height as f32];
        self.blit_src(t, src, r, tint);
    }

    fn blit_src(&mut self, (t, image): Image, src: [f32; 4], r: (f32, f32, f32, f32), tint: [u8; 4]) {
        let (fx0, fy0) = self.px((r.0, r.1));
        let (fx1, fy1) = (fx0 + r.2 * self.s, fy0 + r.3 * self.s);
        let frame = match &mut self.ink {
            Ink::Frame(frame) => frame,
            Ink::Sprites { .. } => return self.sprite(Sprite::image(image, t, src, [fx0, fy0, fx1, fy1], tint)),
        };
        let (x0, x1) = span(fx0, fx1, frame.width);
        let (y0, y1) = span(fy0, fy1, frame.height);
        for y in y0..y1 {
            let v = src[1] + (y as f32 + 0.5 - fy0) / (fy1 - fy0) * (src[3] - src[1]);
            for x in x0..x1 {
                let u = src[0] + (x as f32 + 0.5 - fx0) / (fx1 - fx0) * (src[2] - src[0]);
                let c = bilinear(t, u, v, src);
                let mul = |i: usize| (c[i] as u32 * tint[i] as u32 / 255) as u8;
                blend(frame, x, y, [mul(0), mul(1), mul(2), mul(3)]);
            }
        }
    }

    /// A `size` square texture centred on `at`, turned clockwise by `turn` radians.
    fn turned(&mut self, (t, image): Image, at: (f32, f32), size: f32, turn: f32) {
        let (cx, cy) = self.px(at);
        let (sin, cos) = turn.sin_cos();
        let (tw, th) = (t.width as f32, t.height as f32);
        let all = [0.0, 0.0, tw, th];
        let frame = match &mut self.ink {
            Ink::Frame(frame) => frame,
            Ink::Sprites { .. } => {
                // The texture's square, its corners turned out of the texture's own frame into the
                // view's: the inverse of the turn the pixels below are read back through.
                let half = size * self.s / 2.0;
                let corner = |lx: f32, ly: f32| [cx + lx * cos - ly * sin, cy + lx * sin + ly * cos];
                return self.sprite(Sprite {
                    corners: [corner(-half, -half), corner(half, -half), corner(half, half), corner(-half, half)],
                    uv: [[0.0, 0.0], [tw, 0.0], [tw, th], [0.0, th]],
                    local: [[0.0; 2]; 4],
                    clamp: clamp_of(t, all),
                    image: Some(image),
                    tint: [255; 4],
                    shape: Shape::Plain,
                });
            }
        };
        let reach = size * self.s * std::f32::consts::FRAC_1_SQRT_2;
        let (x0, x1) = span(cx - reach, cx + reach, frame.width);
        let (y0, y1) = span(cy - reach, cy + reach, frame.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / self.s, (y as f32 + 0.5 - cy) / self.s);
                // Back through the turn into the texture's own square.
                let (lx, ly) = (dx * cos + dy * sin, -dx * sin + dy * cos);
                let (u, v) = ((lx / size + 0.5) * tw, (ly / size + 0.5) * th);
                if !(0.0..tw).contains(&u) || !(0.0..th).contains(&v) {
                    continue;
                }
                blend(frame, x, y, bilinear(t, u, v, all));
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
        let s = self.s;
        let frame = match &mut self.ink {
            Ink::Frame(frame) => frame,
            Ink::Sprites { .. } => {
                // The bar's box in its own pixels, a pixel wider all round for the smoothed edge.
                let (back, front) = (-NEEDLE_TAIL.0 * s - 1.0, NEEDLE_TIP.0 * s + 1.0);
                let side = NEEDLE_TAIL.1.max(NEEDLE_TIP.1) * s + 1.0;
                let local = [[back, -side], [front, -side], [front, side], [back, side]];
                let corner = |[along, across]: [f32; 2]| [cx + along * dx - across * dy, cy + along * dy + across * dx];
                return self.sprite(Sprite {
                    corners: local.map(corner),
                    uv: [[0.0; 2]; 4],
                    local,
                    clamp: [0.0; 4],
                    image: None,
                    tint: NEEDLE_COLOUR,
                    shape: Shape::Needle {
                        tail: NEEDLE_TAIL.0 * s,
                        tip: NEEDLE_TIP.0 * s,
                        tail_half: NEEDLE_TAIL.1 * s,
                        tip_half: NEEDLE_TIP.1 * s,
                    },
                });
            }
        };
        let reach = NEEDLE_TIP.0.max(NEEDLE_TAIL.0) * s + 1.0;
        let (x0, x1) = span(cx - reach, cx + reach, frame.width);
        let (y0, y1) = span(cy - reach, cy + reach, frame.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let mut hits = 0;
                for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let (px, py) = ((x as f32 + ox - cx) / s, (y as f32 + oy - cy) / s);
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
                    blend(frame, x, y, c);
                }
            }
        }
    }

    /// A capsule over a layout box (left, top, width, height): a rectangle whose shorter sides are
    /// half circles, its edges smoothed by four samples a pixel, `left` up to the layout x `split`
    /// and `right` from there on.
    fn capsule(&mut self, r: (f32, f32, f32, f32), left: [u8; 4], split: f32, right: [u8; 4]) {
        let (fx0, fy0) = self.px((r.0, r.1));
        let (fw, fh) = (r.2 * self.s, r.3 * self.s);
        let s = self.s;
        let frame = match &mut self.ink {
            Ink::Frame(frame) => frame,
            Ink::Sprites { .. } => {
                let local = [[-1.0, -1.0], [fw + 1.0, -1.0], [fw + 1.0, fh + 1.0], [-1.0, fh + 1.0]];
                let split = if split == f32::MAX { f32::MAX } else { (split - r.0) * s };
                return self.sprite(Sprite {
                    corners: local.map(|[x, y]| [fx0 + x, fy0 + y]),
                    uv: [[0.0; 2]; 4],
                    local,
                    clamp: [0.0; 4],
                    image: None,
                    tint: left,
                    shape: Shape::Capsule { size: [fw, fh], split, right },
                });
            }
        };
        let rad = fw.min(fh) / 2.0;
        let (x0, x1) = span(fx0, fx0 + fw, frame.width);
        let (y0, y1) = span(fy0, fy0 + fh, frame.height);
        // The segment the capsule is every point within `rad` of.
        let (ax, ay, bx, by) = if fw >= fh {
            (fx0 + rad, fy0 + rad, fx0 + fw - rad, fy0 + rad)
        } else {
            (fx0 + rad, fy0 + rad, fx0 + rad, fy0 + fh - rad)
        };
        for y in y0..y1 {
            for x in x0..x1 {
                let mut hits = 0;
                for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let (px, py) = (x as f32 + ox, y as f32 + oy);
                    let (qx, qy) = (px.clamp(ax.min(bx), ax.max(bx)), py.clamp(ay.min(by), ay.max(by)));
                    if (px - qx).powi(2) + (py - qy).powi(2) <= rad * rad {
                        hits += 1;
                    }
                }
                if hits > 0 {
                    let lx = r.0 + (x as f32 + 0.5 - fx0) / s;
                    let mut c = if lx < split { left } else { right };
                    c[3] = (c[3] as u32 * hits / 4) as u8;
                    blend(frame, x, y, c);
                }
            }
        }
    }

    /// The needle's pin: its texture over a disc `size` across centred on `at`, the round cap the
    /// pin's mesh is.
    fn pin(&mut self, (t, image): Image, at: (f32, f32), size: f32) {
        let (cx, cy) = self.px(at);
        let r = size * self.s / 2.0;
        let all = [0.0, 0.0, t.width as f32, t.height as f32];
        let frame = match &mut self.ink {
            Ink::Frame(frame) => frame,
            Ink::Sprites { .. } => {
                let local = [[-r, -r], [r, -r], [r, r], [-r, r]];
                return self.sprite(Sprite {
                    corners: local.map(|[x, y]| [cx + x, cy + y]),
                    uv: [[0.0, 0.0], [all[2], 0.0], [all[2], all[3]], [0.0, all[3]]],
                    local,
                    clamp: clamp_of(t, all),
                    image: Some(image),
                    tint: [255; 4],
                    shape: Shape::Disc { radius: r },
                });
            }
        };
        let (x0, x1) = span(cx - r, cx + r, frame.width);
        let (y0, y1) = span(cy - r, cy + r, frame.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let (u, v) = ((dx / r + 1.0) / 2.0 * all[2], (dy / r + 1.0) / 2.0 * all[3]);
                blend(frame, x, y, bilinear(t, u, v, all));
            }
        }
    }

    /// `text` with its top at `y` and its left edge at `x`; or, when `right`, its last character
    /// starting at `x`. That is how the layouts' right-justified numbers read: the odometer's miles
    /// (`X=573`) lie exactly over its dim `00000.0` (`X=501`, seven 12-pixel digits) only so.
    fn text(&mut self, (font, image): (&Font, usize), text: &str, x: f32, y: f32, right: bool, colour: [u8; 4]) {
        let but_last = text.char_indices().last().map_or("", |(i, _)| &text[..i]);
        let mut pen = if right { x - font.measure(but_last) } else { x };
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let g = font.glyph(c);
            pen += g.before;
            if c != ' ' {
                let h = g.uv[3] - g.uv[1];
                self.blit_src((&font.atlas, image), g.uv, (pen, y, g.width, h), colour);
            }
            pen += g.width + g.after;
            if let Some(&n) = chars.peek() {
                pen += g.kern.iter().find(|k| k.0 == n as u32).map_or(0.0, |k| k.1);
            }
        }
    }
}
