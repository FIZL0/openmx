//! The on-screen pad of the iPhone app (P200): P197's `web/touch.js`, the browser's, in the shell
//! itself — the same layout, the same fingers, the same standard-mapping pad — for a window that
//! hands over its touches rather than a page that has a DOM to put buttons in.
//!
//!   the stick       axes 0/1    steer and lean (and the menus' cursor)
//!   GAS / BRAKE     R2 / L2     the throttle and the brake, full travel while held
//!   ✕ ○ □ △         0 1 2 3     OK and back in the menus; ○ △ tricks in a ride
//!   CLUTCH / R1     L1 / R1     the clutch, the third trick button
//!   CAMERA / PAUSE  Select / Start
//!
//! The pad is read as one more controller ([`crate::ios`] lists it while it is shown), so the shell
//! takes it through the same pad layer and bindings as a real one. A finger keeps the control it
//! landed on until it lifts, wherever it slides; a button stays down until a read has seen it, so a
//! tap shorter than a frame still counts once. A short tap on the stick's area that hardly moves is
//! a tap on the game — the menus' click — and so is a touch that lands on no control, or any touch
//! while the pad is hidden.
//!
//! Sizes are touch.js's in its `u`: `min(7vw, 12.5vh)`, in window pixels here, laid out from the
//! safe area's edges.
#![cfg_attr(not(target_os = "ios"), allow(dead_code))]

use crate::menu::ink as menu_art;
use menu_art::Weight;
use game_render::gpu::Sprite;

/// A finger's part of a touch, as winit names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Down,
    Move,
    Up,
    Cancel,
}

/// One touch event: which finger, what it did, and where, in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touch {
    pub id: u64,
    pub phase: Phase,
    pub at: (f32, f32),
}

/// The standard mapping's button numbers of the pad's ten buttons, in [`LAYOUT`]'s order.
const CROSS: usize = 0;
const CIRCLE: usize = 1;
const SQUARE: usize = 2;
const TRIANGLE: usize = 3;
const L1: usize = 4;
const R1: usize = 5;
const L2: usize = 6;
const R2: usize = 7;
const SELECT: usize = 8;
const START: usize = 9;
const BUTTONS: usize = 10;

/// What a button looks like: a pedal (a rounded box), a face (a circle with its symbol), a small
/// pill along the top.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    Pedal(&'static str),
    Face,
    Small(&'static str),
}

/// Where a button sits, in `u`: its right and bottom edges `x`, `y` in from the safe area's
/// bottom-right corner, or along the top from the left or right edge or the middle.
#[derive(Clone, Copy, Debug)]
enum Place {
    Right { x: f32, y: f32 },
    TopLeft(f32),
    TopRight(f32),
    TopMiddle(f32),
}

/// The buttons: which, where, how big (`u`), what they look like. touch.js's `layout`.
const LAYOUT: [(usize, Place, [f32; 2], Look); BUTTONS] = [
    (R2, Place::Right { x: 0.35, y: 0.35 }, [1.9, 2.5], Look::Pedal("GAS")),
    (L2, Place::Right { x: 2.5, y: 0.35 }, [1.6, 1.7], Look::Pedal("BRAKE")),
    (CROSS, Place::Right { x: 1.6, y: 3.1 }, [1.15, 1.15], Look::Face),
    (CIRCLE, Place::Right { x: 0.35, y: 4.2 }, [1.15, 1.15], Look::Face),
    (SQUARE, Place::Right { x: 2.85, y: 4.2 }, [1.15, 1.15], Look::Face),
    (TRIANGLE, Place::Right { x: 1.6, y: 5.3 }, [1.15, 1.15], Look::Face),
    (L1, Place::TopLeft(0.4), [1.6, 0.7], Look::Small("CLUTCH")),
    (R1, Place::TopRight(0.4), [1.6, 0.7], Look::Small("R1")),
    (SELECT, Place::TopMiddle(-1.8), [1.6, 0.7], Look::Small("CAMERA")),
    (START, Place::TopMiddle(0.2), [1.6, 0.7], Look::Small("PAUSE")),
];

/// How far past a button's edge a touch still hits it, in `u`.
const SLOP: f32 = 0.15;
/// The stick's base radius and its knob's, in `u`; where it waits, `u` in from the bottom-left.
const BASE: f32 = 1.3;
const KNOB: f32 = 0.6;
const REST: f32 = 1.8;
/// The stick's area: the left 42% of the window, its bottom 78%.
const ZONE: [f32; 2] = [0.42, 0.78];
/// A tap on the stick: down for less than this (seconds), and never more than this share of the
/// base's radius from where it landed.
const TAP_TIME: f32 = 0.35;
const TAP_FAR: f32 = 0.25;

/// What a finger is holding.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Held {
    Button(usize),
    Stick,
}

/// The stick while a finger is on it: where it landed and when, the farthest it went (in base
/// radii), and the finger.
#[derive(Clone, Copy, Debug)]
struct Stick {
    id: u64,
    at: (f32, f32),
    when: f32,
    far: f32,
    knob: (f32, f32),
}

/// The pad: its layout for the window, its fingers and what the game is told.
#[derive(Debug, Default)]
pub struct TouchPad {
    /// The window's size and safe insets (left, top, right, bottom), in pixels.
    size: (f32, f32),
    safe: [f32; 4],
    /// Whether it is on screen: no real controller and no keyboard last.
    pub shown: bool,
    fingers: Vec<(u64, Held)>,
    /// Each button's fingers, whether it is down for the game, and whether a read has seen it down.
    holding: [Vec<u64>; BUTTONS],
    pressed: [bool; BUTTONS],
    seen: [bool; BUTTONS],
    stick: Option<Stick>,
    axes: [f32; 2],
}

impl TouchPad {
    /// Lay it out for a window of `size` pixels with `safe` insets (left, top, right, bottom).
    pub fn layout(&mut self, size: (f32, f32), safe: [f32; 4]) {
        self.size = size;
        self.safe = safe;
    }

    /// touch.js's `u`.
    fn u(&self) -> f32 {
        (0.07 * self.size.0).min(0.125 * self.size.1)
    }

    /// A button's box (left, top, width, height) in window pixels.
    fn rect(&self, place: Place, size: [f32; 2]) -> [f32; 4] {
        let u = self.u();
        let (w, h) = (size[0] * u, size[1] * u);
        let [l, t, r, b] = self.safe;
        let top = t + 0.3 * u;
        let (x, y) = match place {
            Place::Right { x, y } => (self.size.0 - r - x * u - w, self.size.1 - b - y * u - h),
            Place::TopLeft(x) => (l + x * u, top),
            Place::TopRight(x) => (self.size.0 - r - x * u - w, top),
            Place::TopMiddle(x) => (self.size.0 / 2.0 + x * u, top),
        };
        [x, y, w, h]
    }

    fn button_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        let slop = SLOP * self.u();
        LAYOUT.iter().find_map(|&(b, place, size, _)| {
            let [l, t, w, h] = self.rect(place, size);
            (x >= l - slop && x <= l + w + slop && y >= t - slop && y <= t + h + slop).then_some(b)
        })
    }

    fn in_zone(&self, (x, y): (f32, f32)) -> bool {
        x <= self.size.0 * ZONE[0] && y >= self.size.1 * (1.0 - ZONE[1])
    }

    /// Where the stick waits.
    fn rest(&self) -> (f32, f32) {
        let u = self.u();
        (self.safe[0] + REST * u, self.size.1 - self.safe[3] - REST * u)
    }

    /// One touch, `now` seconds into the run; the menus' clicks it makes go into `clicks`.
    pub fn touch(&mut self, t: Touch, now: f32, clicks: &mut Vec<(i32, i32)>) {
        let click = |clicks: &mut Vec<(i32, i32)>, (x, y): (f32, f32)| clicks.push((x.floor() as i32, y.floor() as i32));
        match t.phase {
            Phase::Down if !self.shown => click(clicks, t.at),
            Phase::Down => {
                if self.fingers.iter().any(|(id, _)| *id == t.id) {
                    return;
                }
                if let Some(b) = self.button_at(t.at) {
                    self.fingers.push((t.id, Held::Button(b)));
                    self.holding[b].push(t.id);
                    self.pressed[b] = true;
                } else if self.in_zone(t.at) && self.stick.is_none() {
                    self.fingers.push((t.id, Held::Stick));
                    self.stick = Some(Stick { id: t.id, at: t.at, when: now, far: 0.0, knob: t.at });
                } else {
                    click(clicks, t.at);
                }
            }
            Phase::Move => {
                let radius = BASE * self.u();
                if let Some(s) = self.stick.as_mut().filter(|s| s.id == t.id) {
                    let (mut dx, mut dy) = (t.at.0 - s.at.0, t.at.1 - s.at.1);
                    let d = dx.hypot(dy);
                    s.far = s.far.max(d / radius.max(1.0));
                    if d > radius {
                        (dx, dy) = (dx * radius / d, dy * radius / d);
                    }
                    s.knob = (s.at.0 + dx, s.at.1 + dy);
                    self.axes = [dx / radius.max(1.0), dy / radius.max(1.0)];
                }
            }
            Phase::Up | Phase::Cancel => {
                let Some(i) = self.fingers.iter().position(|(id, _)| *id == t.id) else { return };
                match self.fingers.remove(i).1 {
                    Held::Button(b) => {
                        self.holding[b].retain(|id| *id != t.id);
                        self.settle(b);
                    }
                    Held::Stick => {
                        let s = self.stick.take().expect("a stick finger has a stick");
                        self.axes = [0.0; 2];
                        if t.phase == Phase::Up && s.far < TAP_FAR && now - s.when < TAP_TIME {
                            click(clicks, s.at);
                        }
                    }
                }
            }
        }
    }

    /// Let a button up once no finger holds it and a read has seen it down.
    fn settle(&mut self, b: usize) {
        if self.holding[b].is_empty() && self.pressed[b] && self.seen[b] {
            self.pressed[b] = false;
            self.seen[b] = false;
        }
    }

    /// Every finger off, everything up: the pad hidden, or the app sent away.
    pub fn reset(&mut self) {
        *self = TouchPad { size: self.size, safe: self.safe, shown: self.shown, ..TouchPad::default() };
    }

    /// The pad as the Gamepad API's standard mapping has it now — seventeen buttons (0 or 1) and
    /// the two sticks' four axes — for the pad layer's read. The buttons it hands over down count
    /// as seen, so a tap is let up on the next read.
    pub fn read(&mut self) -> ([f32; 17], [f32; 4]) {
        for b in 0..BUTTONS {
            self.settle(b);
        }
        let mut buttons = [0.0; 17];
        for b in 0..BUTTONS {
            if self.pressed[b] {
                self.seen[b] = true;
                buttons[b] = 1.0;
            }
        }
        (buttons, [self.axes[0], self.axes[1], 0.0, 0.0])
    }

    /// The pad drawn as sprites over the window (scale 1, window pixels), its type out of
    /// [`menu_art::ink`](crate::menu::ink::ink)'s atlas as picture `image`. Nothing while hidden.
    pub fn sprites(&self, image: usize) -> Vec<Sprite> {
        let mut out = Vec::new();
        if !self.shown || self.size.0 < 1.0 {
            return out;
        }
        let u = self.u();
        let white = |a: u8| [255, 255, 255, a];
        let line = (u * 0.03).max(2.0);
        // The stick: its base where the thumb landed (or waiting, half as strong), the knob.
        let (centre, knob, dim) = match &self.stick {
            Some(s) => (s.at, s.knob, 1.0),
            None => (self.rest(), self.rest(), 0.5),
        };
        let a = |v: f32| (v * dim) as u8;
        menu_art::disc(&mut out, centre.0, centre.1, BASE * u, white(a(17.0)));
        let base = [centre.0 - BASE * u, centre.1 - BASE * u, 2.0 * BASE * u, 2.0 * BASE * u];
        menu_art::ring(&mut out, base, BASE * u, line, white(a(68.0)));
        menu_art::disc(&mut out, knob.0, knob.1, KNOB * u, white(a(85.0)));
        for &(b, place, size, look) in &LAYOUT {
            let r = self.rect(place, size);
            let radius = match look {
                Look::Pedal(_) => 0.35 * u,
                Look::Face => r[2] / 2.0,
                Look::Small(_) => 0.3 * u,
            };
            let fill = if self.pressed[b] || !self.holding[b].is_empty() { white(102) } else { [0, 0, 0, 85] };
            // The small ones are a little see-through.
            let fade = |c: [u8; 4]| match look {
                Look::Small(_) => [c[0], c[1], c[2], (c[3] as f32 * 0.75) as u8],
                _ => c,
            };
            menu_art::round(&mut out, r, radius, fade(fill), fade(fill));
            menu_art::ring(&mut out, r, radius, line, fade(white(85)));
            let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
            let ink = fade(white(255));
            match look {
                Look::Pedal(word) | Look::Small(word) => {
                    let size = 0.3 * u * 1.2;
                    let ink_type = menu_art::ink();
                    let w = ink_type.measure(Weight::Heavy, word, size);
                    let baseline = cy + size * (ink_type.ascent - ink_type.descent.abs()) / 2.0;
                    ink_type.text(&mut out, image, Weight::Heavy, word, cx - w / 2.0, baseline, size, 0.0, ink);
                }
                Look::Face => symbol(&mut out, b, (cx, cy), 0.22 * u, line * 1.2, ink),
            }
        }
        out
    }
}

/// A face button's symbol, drawn: ✕, ○, □ or △, `r` from its middle.
fn symbol(out: &mut Vec<Sprite>, b: usize, (cx, cy): (f32, f32), r: f32, line: f32, colour: [u8; 4]) {
    let half = line / 2.0;
    match b {
        CROSS => {
            menu_art::bar(out, [cx - r, cy - r], [cx + r, cy + r], half, half, colour);
            menu_art::bar(out, [cx - r, cy + r], [cx + r, cy - r], half, half, colour);
        }
        CIRCLE => menu_art::ring(out, [cx - r, cy - r, 2.0 * r, 2.0 * r], r, line, colour),
        SQUARE => menu_art::ring(out, [cx - r * 0.9, cy - r * 0.9, 1.8 * r, 1.8 * r], line, line, colour),
        _ => {
            let top = [cx, cy - r];
            let left = [cx - r * 1.05, cy + r * 0.75];
            let right = [cx + r * 1.05, cy + r * 0.75];
            for (a, z) in [(top, left), (left, right), (right, top)] {
                menu_art::bar(out, a, z, half, half, colour);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An iPhone 15 Pro held landscape: 2556×1179 pixels, the Dynamic Island's 177 px either side and
    /// the home indicator's 63 under.
    fn phone() -> TouchPad {
        let mut p = TouchPad::default();
        p.layout((2556.0, 1179.0), [177.0, 0.0, 177.0, 63.0]);
        p.shown = true;
        p
    }

    fn centre(p: &TouchPad, button: usize) -> (f32, f32) {
        let &(_, place, size, _) = LAYOUT.iter().find(|l| l.0 == button).unwrap();
        let r = p.rect(place, size);
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    fn down(id: u64, at: (f32, f32)) -> Touch {
        Touch { id, phase: Phase::Down, at }
    }

    fn up(id: u64, at: (f32, f32)) -> Touch {
        Touch { id, phase: Phase::Up, at }
    }

    #[test]
    fn every_button_is_hit_at_its_middle_and_inside_the_window() {
        let p = phone();
        for &(b, place, size, _) in &LAYOUT {
            assert_eq!(p.button_at(centre(&p, b)), Some(b), "button {b}");
            let [x, y, w, h] = p.rect(place, size);
            assert!(x >= 177.0 && x + w <= 2556.0 - 177.0 && y >= 0.0 && y + h <= 1179.0 - 63.0, "button {b} in the safe area");
        }
        // No two buttons overlap where a finger lands.
        for &(a, ..) in &LAYOUT {
            for &(b, ..) in &LAYOUT {
                if a != b {
                    assert_ne!(p.button_at(centre(&p, a)), Some(b));
                }
            }
        }
    }

    #[test]
    fn a_tap_shorter_than_a_frame_is_read_once() {
        let mut p = phone();
        let mut clicks = Vec::new();
        let at = centre(&p, R2);
        p.touch(down(1, at), 0.0, &mut clicks);
        p.touch(up(1, at), 0.01, &mut clicks);
        assert_eq!(p.read().0[R2], 1.0);
        assert_eq!(p.read().0[R2], 0.0);
        assert!(clicks.is_empty());
    }

    #[test]
    fn a_second_finger_neither_presses_again_nor_lets_go() {
        let mut p = phone();
        let mut clicks = Vec::new();
        let at = centre(&p, CROSS);
        p.touch(down(1, at), 0.0, &mut clicks);
        p.touch(down(2, at), 0.1, &mut clicks);
        assert_eq!(p.read().0[CROSS], 1.0);
        p.touch(up(2, at), 0.2, &mut clicks);
        assert_eq!(p.read().0[CROSS], 1.0);
        p.touch(up(1, at), 0.3, &mut clicks);
        assert_eq!(p.read().0[CROSS], 0.0);
    }

    #[test]
    fn a_finger_keeps_its_button_wherever_it_slides() {
        let mut p = phone();
        let mut clicks = Vec::new();
        p.touch(down(1, centre(&p, L2)), 0.0, &mut clicks);
        p.touch(Touch { id: 1, phase: Phase::Move, at: centre(&p, R2) }, 0.1, &mut clicks);
        let (b, _) = p.read();
        assert_eq!((b[L2], b[R2]), (1.0, 0.0));
    }

    #[test]
    fn the_stick_tilts_to_full_at_its_base_radius() {
        let mut p = phone();
        let mut clicks = Vec::new();
        let at = (500.0, 900.0);
        let r = BASE * p.u();
        p.touch(down(7, at), 0.0, &mut clicks);
        p.touch(Touch { id: 7, phase: Phase::Move, at: (at.0 + r / 2.0, at.1) }, 0.1, &mut clicks);
        assert!((p.read().1[0] - 0.5).abs() < 1e-5);
        p.touch(Touch { id: 7, phase: Phase::Move, at: (at.0, at.1 - 3.0 * r) }, 0.2, &mut clicks);
        let axes = p.read().1;
        assert!(axes[0].abs() < 1e-5 && (axes[1] + 1.0).abs() < 1e-5);
        p.touch(up(7, at), 0.5, &mut clicks);
        assert_eq!(p.read().1, [0.0; 4]);
        // It moved: no click.
        assert!(clicks.is_empty());
    }

    #[test]
    fn taps_click_the_menus() {
        let mut p = phone();
        let mut clicks = Vec::new();
        // A short tap on the stick's area that hardly moved.
        p.touch(down(1, (400.0, 800.0)), 0.0, &mut clicks);
        p.touch(up(1, (402.0, 801.0)), 0.1, &mut clicks);
        // A touch on no control at all.
        p.touch(down(2, (1278.0, 500.0)), 0.2, &mut clicks);
        p.touch(up(2, (1278.0, 500.0)), 0.3, &mut clicks);
        // A long hold on the stick is not a tap.
        p.touch(down(3, (400.0, 800.0)), 1.0, &mut clicks);
        p.touch(up(3, (400.0, 800.0)), 2.0, &mut clicks);
        assert_eq!(clicks, vec![(400, 800), (1278, 500)]);
        // Hidden, every touch is a click, even on a button's place.
        let mut hidden = phone();
        hidden.shown = false;
        let at = centre(&hidden, R2);
        hidden.touch(down(4, at), 0.0, &mut clicks);
        assert_eq!(clicks.last(), Some(&(at.0.floor() as i32, at.1.floor() as i32)));
        assert_eq!(hidden.read().0, [0.0; 17]);
    }

    #[test]
    fn it_draws_only_when_shown() {
        let mut p = phone();
        assert!(!p.sprites(0).is_empty());
        p.shown = false;
        assert!(p.sprites(0).is_empty());
    }
}
