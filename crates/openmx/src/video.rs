//! Video presets and the Auto preset (P223), as opentennis's `hst-base/src/graphics.rs` (its P53,
//! B412, B343, B345, B364, B538, B547, B644): the Graphics tab's rows that cost frames, set together
//! by a preset — High, Medium, Low, Deck or Phone — or by Auto, which picks one for the device it runs
//! on ([`Device::here`], [`pick`]) and, while it is on, times the first seconds of every ride and steps
//! the rows down a notch at a time ([`cheaper`], [`Tune`]) until the display's rate holds. Changing any
//! of the rows by hand turns Auto off. Drawing only: the simulation's tick and RNG never see any of it.
//!
//! What opentennis has and this does not: MSAA (the renderer has none) and a separate cull switch
//! (actors out of view are always culled now, `game_render::gpu`, as nothing is lost by it).

use super::{Filtering, FrameLimit, RenderScale, Settings};
use game_render::eco::Lod;
use game_render::shadow::Shadows;
use game_render::world::Graphics;

/// Which shadows the sun casts under [`Shadows::Better`] (P223, opentennis's Fewer Shadows, B343):
/// everything near, or only the riders, the machines and the big props.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Casters {
    #[default]
    All,
    Fewer,
}

impl Casters {
    pub const ALL: [Casters; 2] = [Casters::All, Casters::Fewer];

    pub fn label(self) -> &'static str {
        match self {
            Casters::All => "All",
            Casters::Fewer => "Fewer",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Casters::All => "all",
            Casters::Fewer => "fewer",
        }
    }

    pub fn from_word(w: &str) -> Option<Casters> {
        Casters::ALL.into_iter().find(|c| c.word() == w)
    }
}

/// How detailed the models are drawn (P223, opentennis's Mesh, B364): the disc's own level-of-detail
/// pick, or its coarser models at half the distance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Detail {
    #[default]
    Full,
    Reduced,
}

impl Detail {
    pub const ALL: [Detail; 2] = [Detail::Full, Detail::Reduced];

    pub fn label(self) -> &'static str {
        match self {
            Detail::Full => "Full",
            Detail::Reduced => "Reduced",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Detail::Full => "full",
            Detail::Reduced => "reduced",
        }
    }

    pub fn from_word(w: &str) -> Option<Detail> {
        Detail::ALL.into_iter().find(|d| d.word() == w)
    }

    /// The renderer's `lod_bias`.
    pub fn lod_bias(self) -> f32 {
        match self {
            Detail::Full => 1.0,
            Detail::Reduced => 2.0,
        }
    }
}

/// The rows a preset sets: every Graphics tab row that costs frames. World Sky, Leaves and the
/// texture packs are looks rather than costs, and VSync is the player's, so no preset touches them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub draw: Graphics,
    pub plants: Lod,
    pub filtering: Filtering,
    pub shadows: Shadows,
    pub casters: Casters,
    pub detail: Detail,
    pub scale: RenderScale,
    pub limit: FrameLimit,
}

impl Look {
    /// The rows as `s` has them.
    pub fn of(s: &Settings) -> Look {
        Look {
            draw: s.graphics,
            plants: s.plants,
            filtering: s.filtering,
            shadows: s.shadows,
            casters: s.casters,
            detail: s.detail,
            scale: s.render_scale,
            limit: s.frame_limit,
        }
    }

    /// Set the rows on `s`.
    pub fn apply(self, s: &mut Settings) {
        s.graphics = self.draw;
        s.plants = self.plants;
        s.filtering = self.filtering;
        s.shadows = self.shadows;
        s.casters = self.casters;
        s.detail = self.detail;
        s.render_scale = self.scale;
        s.frame_limit = self.limit;
    }
}

/// The presets, best looking first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Everything as openmx draws it at its best: the whole map, every shadow, 16x filtering, the
    /// resolution only dropped when frames run long (Render Scale Auto). What a desktop GPU gets.
    High,
    /// High's look with the speed-ups on (Fewer casters) and the fog eight times the original's.
    Medium,
    /// The cheapest that still looks like the game: the original's fog, plants and shadow, half scale.
    Low,
    /// The Steam Deck's: its native 1280×800 drawn whole, Far fog, fewer casters, 8x.
    Deck,
    /// A phone's (P215, opentennis's B351/B413): the original's shadow under each racer, 75% scale,
    /// the fog four times the original's, the original's plants (fewer models near), 4x filtering,
    /// coarser models and 60 frames a second at most (a 120 Hz screen's battery).
    Phone,
}

impl Preset {
    pub const ALL: [Preset; 5] = [Preset::High, Preset::Medium, Preset::Low, Preset::Deck, Preset::Phone];

    pub fn label(self) -> &'static str {
        match self {
            Preset::High => "High",
            Preset::Medium => "Medium",
            Preset::Low => "Low",
            Preset::Deck => "Steam Deck",
            Preset::Phone => "Phone",
        }
    }

    pub fn look(self) -> Look {
        let high = Look {
            draw: Graphics::Full,
            plants: Lod::Smooth,
            filtering: Filtering::X16,
            shadows: Shadows::Better,
            casters: Casters::All,
            detail: Detail::Full,
            scale: RenderScale::Auto,
            limit: FrameLimit::Off,
        };
        match self {
            Preset::High => high,
            Preset::Medium => Look { draw: Graphics::Farther, casters: Casters::Fewer, ..high },
            Preset::Low => Look {
                draw: Graphics::Original,
                plants: Lod::Original,
                filtering: Filtering::X2,
                shadows: Shadows::Original,
                casters: Casters::Fewer,
                detail: Detail::Reduced,
                scale: RenderScale::Half,
                limit: FrameLimit::Off,
            },
            Preset::Deck => Look { draw: Graphics::Far, filtering: Filtering::X8, casters: Casters::Fewer, scale: RenderScale::Full, ..high },
            Preset::Phone => Look {
                draw: Graphics::Far,
                plants: Lod::Original,
                filtering: Filtering::X4,
                shadows: Shadows::Original,
                casters: Casters::Fewer,
                detail: Detail::Reduced,
                scale: RenderScale::ThreeQuarters,
                limit: FrameLimit::Fps(60),
            },
        }
    }

    /// The preset whose rows `look` are, if any.
    pub fn of(look: Look) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.look() == look)
    }
}

/// The Preset row's choices: Auto, each preset, and Custom last (shown, never picked: a press from it
/// goes on to Auto).
pub fn row_labels() -> Vec<&'static str> {
    std::iter::once("Auto").chain(Preset::ALL.map(Preset::label)).chain(std::iter::once("Custom")).collect()
}

/// The Preset row's choice for `s`: 0 Auto, 1.. a preset, the last Custom.
pub fn row_at(s: &Settings) -> usize {
    if s.video_auto {
        return 0;
    }
    match Preset::of(Look::of(s)) {
        Some(p) => 1 + Preset::ALL.iter().position(|&q| q == p).unwrap_or(0),
        None => 1 + Preset::ALL.len(),
    }
}

/// The Preset row set to choice `i` of [`row_labels`]: Auto picks for this device again, a preset
/// sets its rows, and Custom (which a press only lands on by stepping past the ends) is Auto.
pub fn set_row(s: &mut Settings, i: usize) {
    match i.checked_sub(1).and_then(|i| Preset::ALL.get(i)) {
        Some(p) => {
            s.video_auto = false;
            p.look().apply(s);
        }
        None => {
            s.video_auto = true;
            pick(&Device::here()).apply(s);
        }
    }
}

// ---------------------------------------------------------------- the device

/// What the Auto preset picks from ([`pick`]), opentennis's P53. Safari hides the model and the GPU,
/// so the browser gives the screen (CSS px and DPR), the cores and `deviceMemory` (Chromium only); the
/// iPhone app gives the model and the memory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Device {
    /// A phone or tablet: the browser on a touch screen, or the iPhone app.
    pub touch: bool,
    pub steam_deck: bool,
    /// The app's device model, iOS's `hw.machine` (`iPhone14,5`).
    pub model: Option<String>,
    /// The screen in CSS px, narrow side first; 0 unknown.
    pub css: [u32; 2],
    /// Device pixels per CSS px; 0 unknown.
    pub dpr: f32,
    /// CPU cores; 0 unknown.
    pub cores: u32,
    /// Memory in MB; 0 unknown (Safari, Firefox).
    pub ram_mb: u32,
}

impl Device {
    /// This device.
    pub fn here() -> Device {
        // Filled in further on a browser and an iPhone.
        #[allow(unused_mut)]
        let mut d = Device {
            touch: cfg!(target_os = "ios"),
            steam_deck: std::env::var("SteamDeck").is_ok_and(|v| v == "1"),
            cores: std::thread::available_parallelism().map_or(0, |n| n.get() as u32),
            ..Device::default()
        };
        #[cfg(target_arch = "wasm32")]
        crate::host_device(&mut d);
        #[cfg(target_os = "ios")]
        {
            d.model = openmx_ios::model();
            d.ram_mb = openmx_ios::memory_mb();
        }
        d
    }

    /// How many [`cheaper`] steps below Phone a phone or tablet starts (0–5). Biased high: the timing
    /// in a ride steps down from there and never up, so only signs of a weaker device lower it.
    pub fn steps(&self) -> u32 {
        let mut n = match self.model.as_deref().and_then(ios_tier) {
            Some(n) => n,
            None => {
                let mut n = 0;
                // Chromium's deviceMemory: a core count only means something beside it (Safari's is
                // not the device's).
                if self.ram_mb > 0 {
                    if self.ram_mb <= 3 * 1024 || (1..=4).contains(&self.cores) {
                        n = 2;
                    } else if self.ram_mb <= 4 * 1024 {
                        n = 1;
                    }
                } else if self.model.is_none() {
                    // Safari on an iPhone: its screen narrows down the generation.
                    n = iphone_screen_tier(self.css, self.dpr);
                }
                n
            }
        };
        // A tablet's many pixels: one more.
        let px = self.css[0] as f32 * self.css[1] as f32 * self.dpr * self.dpr;
        if px > 4.5e6 {
            n += 1;
        }
        n.min(5)
    }
}

/// An iOS model identifier's steps below Phone: an iPhone 15 Pro (`iPhone16,1`) or newer, or an M1
/// iPad or newer, 0; an iPhone 13/14 (`iPhone14`/`15`) 1; older 2.
fn ios_tier(model: &str) -> Option<u32> {
    let (kind, rest) = model.split_at(model.find(|c: char| c.is_ascii_digit())?);
    let major: u32 = rest.split(',').next()?.parse().ok()?;
    match kind {
        "iPhone" => Some(if major >= 16 { 0 } else if major >= 14 { 1 } else { 2 }),
        "iPad" => Some(if major >= 13 { 0 } else { 2 }),
        _ => None,
    }
}

/// Safari on an iPhone, by its screen (CSS px at DPR 2 or 3): the sizes only iPhones up to the 14 and
/// the 16e have 1, the ones before the X and the XR/11 2, anything else 0.
fn iphone_screen_tier([w, h]: [u32; 2], dpr: f32) -> u32 {
    if dpr < 2.0 {
        return 0;
    }
    match (w.min(h), w.max(h)) {
        (375, 667) | (414, 736) | (414, 896) => 2,
        (375, 812) | (360, 780) | (390, 844) | (428, 926) => 1,
        _ => 0,
    }
}

/// The Auto preset's first pick on `d`: the Deck's on a Deck, High elsewhere off a touch screen, and
/// Phone stepped down by [`Device::steps`] on a phone or tablet.
pub fn pick(d: &Device) -> Look {
    if d.steam_deck {
        return Preset::Deck.look();
    }
    if !d.touch {
        return Preset::High.look();
    }
    let mut look = Preset::Phone.look();
    for _ in 0..d.steps() {
        look = cheaper(look).unwrap_or(look);
    }
    look
}

/// The biggest side a model's texture is uploaded at on `d` (P223, opentennis's B538): 512 on a phone
/// or tablet, whose memory a texture pack's 2048s fill; none elsewhere.
pub fn texture_cap(d: &Device) -> u32 {
    if d.touch { 512 } else { 0 }
}

/// One step cheaper, or `None` at the bottom: the speed-ups first (fewer casters), then the fog in
/// to Far, a fixed scale down to 75%, the original's shadow, filtering down to 4x, the original's
/// plants, coarser models, half scale, the original's fog, 2x (Phone to Low), no shadows, filtering
/// off. A frame limit
/// under 60 is lifted to 60: a step is never a 30 cap.
pub fn cheaper(l: Look) -> Option<Look> {
    let mut c = l;
    if l.casters == Casters::All && l.shadows == Shadows::Better {
        c.casters = Casters::Fewer;
    } else if matches!(l.draw, Graphics::Full | Graphics::Farther) {
        c.draw = if l.draw == Graphics::Full { Graphics::Farther } else { Graphics::Far };
    } else if l.scale == RenderScale::Full {
        c.scale = RenderScale::ThreeQuarters;
    } else if l.shadows == Shadows::Better {
        c.shadows = Shadows::Original;
    } else if matches!(l.filtering, Filtering::X16 | Filtering::X8) {
        c.filtering = Filtering::X4;
    } else if l.plants == Lod::Smooth {
        c.plants = Lod::Original;
    } else if l.detail == Detail::Full {
        c.detail = Detail::Reduced;
    } else if matches!(l.scale, RenderScale::Auto | RenderScale::ThreeQuarters) {
        c.scale = RenderScale::Half;
    } else if l.draw != Graphics::Original {
        c.draw = Graphics::Original;
    } else if l.filtering == Filtering::X4 {
        c.filtering = Filtering::X2;
    } else if l.shadows == Shadows::Original {
        c.shadows = Shadows::Off;
    } else if l.filtering != Filtering::Off {
        c.filtering = Filtering::Off;
    } else {
        return None;
    }
    if let FrameLimit::Fps(n) = c.limit
        && n < 60
    {
        c.limit = FrameLimit::Fps(60);
    }
    Some(c)
}

// ---------------------------------------------------------------- timing a ride

/// [`Tune`]: the ride before timing starts (its loading hitches), one timed window, the wait after a
/// step for it to take hold, and the most timed riding a ride, in seconds.
const AUTO_SKIP: f32 = 3.0;
const AUTO_WINDOW: f32 = 2.0;
const AUTO_SETTLE: f32 = 1.0;
const AUTO_SPAN: f32 = 16.0;

/// The Auto preset in a ride (opentennis's `auto_tune`): after its first seconds, times 2 s windows and
/// steps one notch cheaper ([`cheaper`]) after each that falls under 93% of 60 fps — or of the
/// display's rate when that is lower: a phone in Low Power Mode at 30 Hz is not the game's fault —
/// until two in a row hold, the bottom is reached, or 16 s have been timed. Never steps up; each ride
/// checks again (a heavier track).
#[derive(Default)]
pub struct Tune {
    /// The ride's time so far, s.
    played: f32,
    /// Timed riding this ride, s.
    timed: f32,
    /// Waiting after a step until this ride time.
    settle: f32,
    /// The window's frame times.
    window: Vec<f32>,
    /// Windows in a row that held.
    held: u32,
    /// Through for this ride.
    pub done: bool,
    /// What it last saw or set: another change (the pause menu's rows) starts its timing over.
    seen: Option<Look>,
}

/// The rate a window held: its frames over its time, without its slowest 5% (a pipeline's first build
/// or a texture's upload is not the device's pace).
fn held_fps(window: &mut [f32]) -> f32 {
    window.sort_by(f32::total_cmp);
    let keep = &window[..window.len() - window.len() / 20];
    keep.len() as f32 / keep.iter().sum::<f32>().max(1e-6)
}

/// The rate the Auto preset aims at on a display of `refresh` Hz: 60, or the display's when lower.
pub fn target(refresh: Option<f32>) -> f32 {
    refresh.filter(|&r| r > 1.0).unwrap_or(60.0).min(60.0)
}

impl Tune {
    /// One frame of a ride, `dt` s apart from the last, at the `target` rate: the cheaper rows to
    /// switch to, if any.
    pub fn frame_time(&mut self, look: Look, dt: f32, target: f32) -> Option<Look> {
        self.played += dt;
        if self.seen.is_some_and(|s| s != look) {
            (self.window, self.held, self.settle) = (Vec::new(), 0, self.played + AUTO_SETTLE);
        }
        self.seen = Some(look);
        if self.done || self.played < AUTO_SKIP.max(self.settle) {
            return None;
        }
        // A hidden page or a paused app: its gap is not a frame.
        if dt > 0.25 {
            self.window.clear();
            return None;
        }
        self.window.push(dt);
        self.timed += dt;
        if self.window.iter().sum::<f32>() < AUTO_WINDOW {
            return None;
        }
        let fps = held_fps(&mut self.window);
        self.window.clear();
        self.done = self.timed >= AUTO_SPAN;
        if fps >= target * 0.93 {
            self.held += 1;
            self.done |= self.held >= 2;
            if self.held >= 2 {
                println!("auto graphics: {fps:.0} fps holds at {}", name(look));
            }
            return None;
        }
        let Some(c) = cheaper(look) else {
            println!("auto graphics: {fps:.0} fps of {target:.0} at the cheapest");
            self.done = true;
            return None;
        };
        println!("auto graphics: {fps:.0} fps of {target:.0}: down to {}", name(c));
        (self.held, self.settle, self.seen) = (0, self.played + AUTO_SETTLE, Some(c));
        Some(c)
    }
}

/// A look as a log line says it.
pub fn name(l: Look) -> String {
    format!(
        "{} (draw {}, plants {}, filtering {}, shadows {}, casters {}, detail {}, scale {}, limit {})",
        Preset::of(l).map_or("Custom", Preset::label),
        l.draw.word(),
        l.plants.word(),
        l.filtering.word(),
        l.shadows.word(),
        l.casters.word(),
        l.detail.word(),
        l.scale.word(),
        l.limit.word()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Auto preset's pick per device (P223): a desktop High, a Deck its own, a phone Phone or
    /// below it by its model, screen or memory; the ladder never caps at 30 and goes through Phone and
    /// Low on its way down.
    #[test]
    fn auto_picks_per_device() {
        let phone = Device { touch: true, ..Device::default() };
        assert_eq!(pick(&Device::default()), Preset::High.look());
        assert_eq!(pick(&Device { cores: 16, ram_mb: 32768, ..Device::default() }), Preset::High.look());
        assert_eq!(pick(&Device { steam_deck: true, ..Device::default() }), Preset::Deck.look());
        assert_eq!(pick(&phone), Preset::Phone.look());
        let steps = |d: Device| d.steps();
        assert_eq!(steps(Device { model: Some("iPhone16,1".into()), ..phone.clone() }), 0);
        assert_eq!(steps(Device { model: Some("iPhone14,5".into()), ..phone.clone() }), 1);
        assert_eq!(steps(Device { model: Some("iPhone12,1".into()), ..phone.clone() }), 2);
        assert_eq!(steps(Device { model: Some("iPad8,1".into()), ..phone.clone() }), 2);
        assert_eq!(steps(Device { model: Some("iPad13,4".into()), ..phone.clone() }), 0);
        // Safari on an iPhone 13 (390×844 @3) one down; a 15 Pro's 393×852 stays at Phone.
        assert_eq!(steps(Device { css: [390, 844], dpr: 3.0, ..phone.clone() }), 1);
        assert_eq!(steps(Device { css: [844, 390], dpr: 3.0, ..phone.clone() }), 1);
        assert_eq!(steps(Device { css: [393, 852], dpr: 3.0, ..phone.clone() }), 0);
        // Chromium's memory and cores; a tablet's pixels one more.
        assert_eq!(steps(Device { ram_mb: 8192, cores: 8, ..phone.clone() }), 0);
        assert_eq!(steps(Device { ram_mb: 4096, cores: 8, ..phone.clone() }), 1);
        assert_eq!(steps(Device { ram_mb: 8192, cores: 4, ..phone.clone() }), 2);
        assert_eq!(steps(Device { ram_mb: 2048, cores: 8, ..phone.clone() }), 2);
        assert_eq!(steps(Device { css: [1024, 1366], dpr: 2.0, ..phone.clone() }), 1);
        assert!(ios_tier("Android").is_none() && ios_tier("iPhone").is_none());
        // An iPhone 13's pick is Phone a step down: the original's fog no longer far, and so on.
        let thirteen = pick(&Device { model: Some("iPhone14,5".into()), ..phone.clone() });
        assert_eq!(Some(thirteen), cheaper(Preset::Phone.look()));
        assert_eq!(thirteen.scale, RenderScale::Half);
        // The textures: 512 on a phone, as they come on the desktop and the Deck.
        assert_eq!((texture_cap(&phone), texture_cap(&Device::default()), texture_cap(&Device { steam_deck: true, ..Device::default() })), (512, 0, 0));

        // The ladder from High to the bottom: every row once, no 30 cap, Medium on the way.
        let mut l = Preset::High.look();
        let mut rungs = vec![l];
        while let Some(c) = cheaper(l) {
            assert!(!matches!(c.limit, FrameLimit::Fps(n) if n < 60), "{c:?}");
            l = c;
            rungs.push(l);
        }
        assert!(rungs.len() <= 16, "{}", rungs.len());
        assert!(rungs.contains(&Preset::Medium.look()));
        assert_eq!(l, Look { draw: Graphics::Original, plants: Lod::Original, filtering: Filtering::Off, shadows: Shadows::Off, casters: Casters::Fewer, detail: Detail::Reduced, scale: RenderScale::Half, limit: FrameLimit::Off });
        // From Phone through Low's look (at Phone's 60 limit).
        let mut l = Preset::Phone.look();
        let mut lows = 0;
        while let Some(c) = cheaper(l) {
            l = c;
            lows += usize::from(Look { limit: FrameLimit::Off, ..l } == Preset::Low.look());
        }
        assert_eq!(lows, 1);
        assert_eq!(cheaper(Look { limit: FrameLimit::Fps(30), ..Preset::Low.look() }).unwrap().limit, FrameLimit::Fps(60));
        // Each preset is told apart, so the row can name it.
        for p in Preset::ALL {
            assert_eq!(Preset::of(p.look()), Some(p));
        }
    }

    /// The Preset row: a preset sets every row and turns Auto off, a row changed by hand is Custom,
    /// Auto picks again.
    #[test]
    fn the_preset_row() {
        let mut s = Settings::default();
        assert!(s.video_auto);
        assert_eq!(row_at(&s), 0);
        set_row(&mut s, 5);
        assert_eq!((Look::of(&s), s.video_auto, row_at(&s)), (Preset::Phone.look(), false, 5));
        s.shadows = Shadows::Better;
        assert_eq!(row_at(&s), row_labels().len() - 1);
        set_row(&mut s, 0);
        assert!(s.video_auto);
        assert_eq!(Look::of(&s), pick(&Device::here()));
    }

    /// The timing waits out the ride's first seconds, steps down after a slow window, waits for the
    /// step to take, and is through after two windows that hold (a hitch or a hidden page's gap does
    /// not count).
    #[test]
    fn auto_timing() {
        let run = |t: &mut Tune, l: &mut Look, dt: f32, secs: f32| {
            let mut steps = 0;
            for _ in 0..(secs / dt).round() as u32 {
                if let Some(c) = t.frame_time(*l, dt, 60.0) {
                    *l = c;
                    steps += 1;
                }
            }
            steps
        };
        // 30 fps: a step a window plus the wait after it, then through within the 16 s.
        let (mut t, mut l) = (Tune::default(), Preset::Phone.look());
        assert_eq!(run(&mut t, &mut l, 1.0 / 30.0, AUTO_SKIP - 0.1), 0);
        assert_eq!(run(&mut t, &mut l, 1.0 / 30.0, 2.2), 1);
        assert_eq!(Some(l), cheaper(Preset::Phone.look()));
        assert!(run(&mut t, &mut l, 1.0 / 30.0, 30.0) >= 3);
        assert!(t.done);
        // 60 fps with a hitch: no step, through after two windows.
        let (mut t, mut l) = (Tune::default(), Preset::Phone.look());
        let mut steps = run(&mut t, &mut l, 1.0 / 60.0, 4.0);
        assert!(t.frame_time(l, 0.2, 60.0).is_none() && t.frame_time(l, 5.0, 60.0).is_none());
        steps += run(&mut t, &mut l, 1.0 / 60.0, 3.0);
        assert!(!t.done);
        steps += run(&mut t, &mut l, 1.0 / 60.0, 2.0);
        assert_eq!((steps, t.done, l), (0, true, Preset::Phone.look()));
        // A 30 Hz display (Low Power Mode) holds at 30.
        let mut t = Tune::default();
        assert!((0..300).all(|_| t.frame_time(Preset::Phone.look(), 1.0 / 30.0, target(Some(30.0))).is_none()) && t.done);
        assert_eq!((target(None), target(Some(144.0)), target(Some(30.0))), (60.0, 60.0, 30.0));
        // The window's slowest 5% left out.
        let mut w = vec![1.0 / 60.0; 95];
        w.extend([0.2; 5]);
        assert!((held_fps(&mut w) - 60.0).abs() < 0.01);
    }
}
