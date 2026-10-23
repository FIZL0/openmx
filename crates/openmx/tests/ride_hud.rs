//! P121 — the ride HUD redone from `docs/hud-concept.html`: vector shapes and our own type over each
//! view at the disc's 640x448 places, sharp at 4K.
//!
//! The pieces are held to where the disc's layouts put them, at 1080p and at 4K and in a split
//! view; the trick pop-up's land and bail, the speedometer's two styles, the marker menu through
//! every slot and player and its hold; the settings' new default and its migration; and the shell
//! itself drawing each `--overlay` (that one needs the disc and skips without it).

#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/chase.rs"]
#[allow(dead_code)]
mod chase;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;
#[path = "../src/overlay.rs"]
#[allow(dead_code)]
mod overlay;
#[path = "../src/ride_hud.rs"]
#[allow(dead_code)]
mod ride_hud;

use game_render::Frame;
use game_render::gpu::Sprite;
use game_sim::markers::{HOLD_MS, Markers, Pick, SLOTS};
use game_sim::score::{Node, Score};
use menu::{Overlay, Settings, Speedometer};
use overlay::{Activity, Blip, Gauges, Icon};
use ps2_float::F;
use ride_hud::{Dot, Panel, Shown, Tracker};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const SIZES: [(f32, f32); 2] = [(1920.0, 1080.0), (3840.0, 2160.0)];

fn gauges(speedometer: Speedometer) -> Gauges {
    Gauges { feet: 1234.5, bike: true, preload: Some(0.5), score: 128_450, modified: false, speedometer }
}

fn panel(pick: Pick, others: Vec<(usize, Option<String>)>, holding: u32) -> Panel {
    Panel { pick, filled: [true, false, true, false, false], others, holding, machine: "Bike".into() }
}

fn shown<'a>(g: &'a Gauges, riders: &'a [Dot], blips: &'a [Blip], markers: Option<Panel>) -> Shown<'a> {
    Shown {
        speed: 20.0,
        gauges: g,
        own: [0.0, 0.0],
        forward: [0.0, 1.0],
        riders,
        blips,
        span: 384.0,
        tag: None,
        offer: Some(vehicle::Machine::Biplane),
        challenge: None,
        markers,
        scale: 1.0,
        minimap: true,
    }
}

/// A score card with a combo of `ids` open, or closed with `total` banked.
fn card(ids: &[u8], open: bool, total: f32) -> Score {
    let nodes = ids.iter().rev().map(|&id| Node { id, points: F(250.0), ..Node::default() }).collect();
    Score { nodes, combo: open, total: F(total), ..Score::default() }
}

fn name(id: u8) -> String {
    format!("Trick {id}")
}

/// A tracker with a two-trick combo up since `t0`.
fn combo_up(t0: Instant) -> Tracker {
    let mut t = Tracker::default();
    t.update(t0, &card(&[], false, 0.0), name);
    t.update(t0, &card(&[3], true, 0.0), name);
    t.update(t0 + Duration::from_millis(400), &card(&[3, 7], true, 0.0), name);
    t
}

fn near(a: f32, b: f32, by: f32) -> bool {
    (a - b).abs() <= by
}

/// The sprites' bounding box: left, top, right, bottom.
fn bounds<'a>(sprites: impl Iterator<Item = &'a Sprite>) -> [f32; 4] {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for s in sprites {
        for [x, y] in s.corners {
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
    }
    b
}

/// Every piece where the disc's layouts put it, at 1080p and at 4K: the score bar top left, the
/// trick pop-up low centre, the meter and the readout bottom right at `speedometer.lyt`'s places
/// with the readout left of the meter, the radar bottom left, the prompt low in the middle and the
/// marker menu top right.
#[test]
fn every_piece_sits_at_its_layout_place_at_1080p_and_4k() {
    let t0 = Instant::now();
    let tracker = combo_up(t0);
    let now = t0 + Duration::from_millis(500);
    let g = gauges(Speedometer::Digital);
    let riders = [Dot { at: [10.0, 30.0], ai: false }];
    let blips = [Blip { icon: Icon::Activity(Activity::Stunt), at: [-20.0, 10.0] }];
    for (w, h) in SIZES {
        let mut out = Vec::new();
        let s = ride_hud::scale_of(w, h, 1.0);
        // The layout fits the height: the sides take the slack, each piece held to its corner.
        assert!(near(s, h / 448.0, 1e-4), "{w}x{h}: scale {s}");
        let shown = shown(&g, &riders, &blips, Some(panel(Pick::Slot(1), vec![(1, Some("Ann".into()))], 0)));
        let d = ride_hud::sprites(&mut out, 0, [0.0, 0.0, w, h], &shown, &tracker, now);
        let at = format!("{w}x{h}");
        // The score bar, top left.
        let want = [14.0 * s, 25.0 * s, 168.0 * s, 20.5 * s];
        assert!(d.score_bar.iter().zip(want).all(|(a, b)| near(*a, b, 0.01)), "{at}: score bar {:?}, want {want:?}", d.score_bar);
        assert_eq!(d.score.text, "128,450");
        let r = d.score.rect;
        assert!(r[0] >= 82.0 * s && r[2] <= (14.0 + 168.0) * s, "{at}: the score's number {r:?} is out of its bar");
        assert!(r[1] >= 25.0 * s && r[3] <= 45.5 * s, "{at}: the score's number {r:?} is out of its bar");
        // The meter at speedometer.lyt's place from the bottom right, the readout left of it with
        // its right at the layout's and its top by the meter's.
        let m = d.meter.expect("the bike's meter");
        let want = [w - (640.0 - 599.0) * s, h - (448.0 - 314.0) * s, 20.0 * s, 85.0 * s];
        assert!(m.iter().zip(want).all(|(a, b)| near(*a, b, 0.01)), "{at}: meter {m:?}, want {want:?}");
        assert_eq!(d.preload, Some(0.5));
        let speed = d.speed.as_ref().expect("the digital readout");
        assert!(near(speed.rect[2], w - (640.0 - 588.0) * s, 0.5), "{at}: the readout's right {:?}", speed.rect);
        assert!(speed.rect[2] < m[0], "{at}: the readout {:?} is not left of the meter {m:?}", speed.rect);
        assert!(near(speed.rect[1], m[1], 4.0 * s), "{at}: the readout's top {} is not by the meter's {}", speed.rect[1], m[1]);
        // The radar where the original's is, from the bottom left, N E S W outside its rim.
        let (c, r) = d.radar.expect("the radar");
        assert!(near(c[0], 87.0 * s, 0.01) && near(c[1], h - (448.0 - 363.0) * s, 0.01), "{at}: radar at {c:?}");
        assert!(near(r, 52.0 * s, 0.01), "{at}: radar radius {r}");
        assert_eq!(d.compass.iter().map(|c| c.0).collect::<String>(), "NESW");
        for (letter, p) in &d.compass {
            let off = ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt();
            assert!(off > r, "{at}: {letter} at {off} sits inside the rim {r}");
        }
        // Facing +z, north is up.
        assert!(d.compass[0].1[1] < c[1] && near(d.compass[0].1[0], c[0], 0.5), "{at}: N at {:?}", d.compass[0].1);
        assert_eq!(d.dots.len(), 1);
        assert_eq!(d.dots[0].2, overlay::HUMAN_MARK);
        assert_eq!(d.icons.len(), 1);
        // The pop-up: centred across, low, its lines between the radar's top and the frame's foot.
        let (popup, lines) = d.trick.as_ref().expect("the combo's pop-up");
        assert_eq!(popup.names, ["Trick 3", "Trick 7"]);
        assert_eq!(popup.mult, Some(2));
        let left = lines.iter().map(|l| l.rect[0]).fold(f32::MAX, f32::min);
        let right = lines.iter().map(|l| l.rect[2]).fold(f32::MIN, f32::max);
        assert!(near((left + right) / 2.0, w / 2.0, 8.0 * s), "{at}: the pop-up spans {left}..{right}");
        for l in lines {
            assert!(l.rect[1] > h * 0.7 && l.rect[3] < h, "{at}: {:?} at {:?} is not low", l.text, l.rect);
        }
        // The machine switch's prompt, low and small in the middle.
        assert_eq!(d.prompts.len(), 1);
        let p = &d.prompts[0];
        assert_eq!(p.text, vehicle::offer_text(vehicle::Machine::Biplane));
        assert!(p.rect[0] > w * 0.2 && p.rect[2] < w / 2.0 + w * 0.1, "{at}: prompt across {:?}", p.rect);
        assert!(p.rect[1] > h * 0.88 && p.rect[3] < h, "{at}: prompt at {:?}", p.rect);
        assert!(p.rect[3] - p.rect[1] < 16.0 * s, "{at}: the prompt is not small: {:?}", p.rect);
        // The marker menu, top right.
        let mk = d.markers.as_ref().expect("the marker menu");
        assert!(near(mk.rect[0] + mk.rect[2], w - 3.2 * 448.0 / 45.0 * s, 0.5), "{at}: menu {:?}", mk.rect);
        assert!(mk.rect[1] < h * 0.1 && mk.rect[0] > w / 2.0, "{at}: menu {:?}", mk.rect);
    }
}

/// A split view's HUD is that view's: the same pieces at the view's own scale and corners.
#[test]
fn a_split_view_draws_its_own_hud() {
    let (w, h) = (1920.0, 1080.0);
    let g = gauges(Speedometer::Digital);
    let t = Tracker::default();
    let now = Instant::now();
    for view in [[0.0, 0.0, w / 2.0, h], [w / 2.0, 0.0, w / 2.0, h], [0.0, h / 2.0, w, h / 2.0]] {
        let mut out = Vec::new();
        let d = ride_hud::sprites(&mut out, 0, view, &shown(&g, &[], &[], None), &t, now);
        let s = ride_hud::scale_of(view[2], view[3], 1.0);
        assert!(near(d.score_bar[0], view[0] + 14.0 * s, 0.01) && near(d.score_bar[1], view[1] + 25.0 * s, 0.01), "{view:?}: {:?}", d.score_bar);
        let (c, _) = d.radar.unwrap();
        assert!(near(c[0], view[0] + 87.0 * s, 0.01) && near(c[1], view[1] + view[3] - 85.0 * s, 0.01), "{view:?}: radar {c:?}");
        let m = d.meter.unwrap();
        assert!(near(m[0] + m[2], view[0] + view[2] - 21.0 * s, 0.01), "{view:?}: meter {m:?}");
        let b = bounds(out.iter());
        assert!(b[0] >= view[0] - 1.0 && b[2] <= view[0] + view[2] + 1.0, "{view:?}: drawn across {b:?}");
        assert!(b[1] >= view[1] - 1.0 && b[3] <= view[1] + view[3] + 1.0, "{view:?}: drawn down {b:?}");
    }
}

/// How many of `frame`'s pixels in `r` are `colour`'s, near enough.
fn count(frame: &Frame, r: [f32; 4], colour: [u8; 4]) -> usize {
    let (x0, y0) = (r[0].max(0.0) as usize, r[1].max(0.0) as usize);
    let (x1, y1) = ((r[2] as usize).min(frame.width), (r[3] as usize).min(frame.height));
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| 3 * (y * frame.width + x)))
        .filter(|&i| (0..3).all(|k| frame.color[i + k].abs_diff(colour[k]) <= 12))
        .count()
}

/// A landed chain plays out in the land colour with its banked points, a bailed one in the bail
/// colour with its points as they were; neither says LANDED or BAILED, and both go when their
/// play-out is through. The original plays both out in the orange they were (research/oracle.txt).
#[test]
fn a_landed_and_a_bailed_chain_play_out_in_their_colours_without_a_word() {
    let (w, h) = (1920usize, 1080usize);
    let g = gauges(Speedometer::Digital);
    for landed in [true, false] {
        let t0 = Instant::now();
        let mut t = combo_up(t0);
        let end = t0 + Duration::from_millis(800);
        t.update(end, &card(&[], false, if landed { 500.0 } else { 0.0 }), name);
        let colour = if landed { ride_hud::LAND_COLOUR } else { ride_hud::BAIL_COLOUR };
        let p = t.popup(end).expect("the pop-up is still up as it ends");
        assert_eq!(p.outcome, Some(landed));
        assert_eq!(p.colour, colour);
        assert_eq!(p.points, if landed { "+500" } else { "500" });
        assert_eq!(p.mult, None);
        let mut frame = Frame::new(w, h, [0, 0, 0]);
        let mut shown = shown(&g, &[], &[], None);
        shown.offer = None;
        let d = ride_hud::draw(&mut frame, &shown, &t, end);
        let (_, lines) = d.trick.expect("drawn");
        for l in &lines {
            let upper = l.text.to_uppercase();
            assert!(!upper.contains("LANDED") && !upper.contains("BAILED"), "{landed}: says {:?}", l.text);
            assert_eq!(l.colour, colour, "{landed}: {:?}", l.text);
        }
        // The names drawn in that colour.
        let names = lines.iter().find(|l| l.text == "Trick 7").expect("the newest name");
        let n = count(&frame, names.rect, colour);
        assert!(n > 50, "{landed}: only {n} pixels of the colour in {:?}", names.rect);
        // A landing holds its points a second, then plays out; a bail plays out at once.
        let held = end + Duration::from_millis(900);
        assert_eq!(t.popup(held).is_some_and(|p| p.alpha > 0.9 && p.scale < 1.1), landed, "{landed}: held");
        let mut later = t.clone();
        later.update(end + ride_hud::TIMES[1] + ride_hud::TIMES[2] + Duration::from_millis(50), &card(&[], false, if landed { 500.0 } else { 0.0 }), name);
        assert!(later.popup(end + ride_hud::TIMES[1] + ride_hud::TIMES[2] + Duration::from_millis(50)).is_none(), "{landed}: still up");
        // On its way out it grows toward twice its size, fading.
        let going = end + if landed { ride_hud::TIMES[1] } else { Duration::ZERO } + ride_hud::TIMES[2] / 2;
        let p = t.popup(going).unwrap();
        assert!(near(p.scale, 1.5, 0.05) && near(p.alpha, 0.5, 0.05), "{landed}: {p:?}");
    }
    // A bank flashes the score.
    let t0 = Instant::now();
    let mut t = combo_up(t0);
    t.update(t0 + Duration::from_millis(800), &card(&[], false, 500.0), name);
    let mut out = Vec::new();
    let d = ride_hud::sprites(&mut out, 0, [0.0, 0.0, 1920.0, 1080.0], &shown(&g, &[], &[], None), &t, t0 + Duration::from_millis(900));
    assert!(d.flash);
    let d = ride_hud::sprites(&mut out, 0, [0.0, 0.0, 1920.0, 1080.0], &shown(&g, &[], &[], None), &t, t0 + Duration::from_millis(800) + ride_hud::TIMES[3]);
    assert!(!d.flash && d.score.colour == ride_hud::GREY);
}

/// The new trick pops in from twice its size.
#[test]
fn a_new_trick_pops_in() {
    let t0 = Instant::now();
    let t = combo_up(t0);
    let at = t0 + Duration::from_millis(400);
    let p = t.popup(at).unwrap();
    assert!(near(p.newest.0, 2.0, 0.01) && p.newest.1 < 0.2, "{p:?}");
    let p = t.popup(at + ride_hud::TIMES[0]).unwrap();
    assert!(near(p.newest.0, 1.0, 0.01) && near(p.newest.1, 1.0, 0.01), "{p:?}");
}

/// The speedometer: Digital, the default, is the DSEG7 number left of the meter; Analog is the dial
/// with the odometer under it, and no number.
#[test]
fn digital_is_the_dseg7_number_left_of_the_meter_and_analog_the_dial() {
    assert_eq!(Gauges::default().speedometer, Speedometer::Digital);
    let t = Tracker::default();
    let now = Instant::now();
    let base = 7;
    for (w, h) in SIZES {
        let g = gauges(Speedometer::Digital);
        let mut out = Vec::new();
        let d = ride_hud::sprites(&mut out, base, [0.0, 0.0, w, h], &shown(&g, &[], &[], None), &t, now);
        let m = d.meter.unwrap();
        // The digits atlas is the second picture; only the readout uses it.
        let digits: Vec<&Sprite> = out.iter().filter(|s| s.image == Some(base + 1)).collect();
        assert!(digits.len() >= overlay::speed_text(20.0).len(), "{w}x{h}: {} DSEG7 glyphs", digits.len());
        let b = bounds(digits.into_iter());
        assert!(b[2] < m[0], "{w}x{h}: the DSEG7 glyphs {b:?} reach the meter {m:?}");
        assert!(d.dial.is_none());
        let g = gauges(Speedometer::Analog);
        let mut out = Vec::new();
        let d = ride_hud::sprites(&mut out, base, [0.0, 0.0, w, h], &shown(&g, &[], &[], None), &t, now);
        assert!(d.speed.is_none());
        assert!(out.iter().all(|s| s.image != Some(base + 1)), "{w}x{h}: the dial drew DSEG7");
        let (pivot, degrees, odometer) = d.dial.expect("the dial");
        let s = d.scale;
        assert!(near(pivot[0], w - (640.0 - 536.0) * s, 0.01) && near(pivot[1], h - (448.0 - 357.0) * s, 0.01), "{w}x{h}: pivot {pivot:?}");
        assert_eq!(degrees, overlay::needle_degrees(20.0));
        assert_eq!(odometer.as_deref(), Some(overlay::odometer_text(1234.5).as_str()));
        assert!(d.meter.is_some(), "the meter stands by the dial too");
    }
    // The plane has no meter and no odometer.
    let g = Gauges { bike: false, ..gauges(Speedometer::Analog) };
    let mut out = Vec::new();
    let d = ride_hud::sprites(&mut out, 0, [0.0, 0.0, 1920.0, 1080.0], &shown(&g, &[], &[], None), &t, now);
    assert!(d.meter.is_none() && d.dial.unwrap().2.is_none());
}

/// The marker menu follows the selection through every slot and every other player as Left and
/// Right step it, and its hold bar fills over HOLD_MS.
#[test]
fn the_marker_menu_follows_every_slot_and_player_and_its_hold_fills() {
    let riding = [true, true, true];
    let others = Markers::others(0, riding.len(), &riding);
    assert_eq!(others, vec![1, 2]);
    let named: Vec<(usize, Option<String>)> = others.iter().map(|&s| (s, (s == 1).then(|| "Ann".to_string()))).collect();
    let g = gauges(Speedometer::Digital);
    let t = Tracker::default();
    let now = Instant::now();
    let mut seen = Vec::new();
    for at in 0..SLOTS + others.len() {
        let m = Markers { at: at as u8, ..Markers::default() };
        let pick = m.pick_among(0, &riding);
        seen.push(pick);
        let mut out = Vec::new();
        let d = ride_hud::sprites(&mut out, 0, [0.0, 0.0, 3840.0, 2160.0], &shown(&g, &[], &[], Some(panel(pick, named.clone(), 0))), &t, now);
        let p = d.markers.unwrap();
        assert_eq!(p.slots.len(), SLOTS);
        assert_eq!(p.players.iter().map(|q| q.0).collect::<Vec<_>>(), others);
        let lit: Vec<Pick> =
            p.slots.iter().enumerate().filter(|s| s.1.2).map(|s| Pick::Slot(s.0)).chain(p.players.iter().filter(|q| q.1).map(|q| Pick::Player(q.0))).collect();
        assert_eq!(lit, vec![pick], "at {at}");
        let filled: Vec<bool> = p.slots.iter().map(|s| s.3).collect();
        assert_eq!(filled, [true, false, true, false, false]);
        let want = match pick {
            Pick::Slot(s) => format!("Marker {} {}", s + 1, if [0, 2].contains(&s) { "set" } else { "empty" }),
            Pick::Player(1) => "to Ann P2".into(),
            Pick::Player(s) => format!("to P{}", s + 1),
        };
        assert_eq!(p.label, want);
        // The selected slot is grown.
        if let Pick::Slot(s) = pick {
            let r = p.slots[s].1;
            assert!(p.slots.iter().enumerate().all(|(i, o)| i == s || o.1 < r));
        }
    }
    let want: Vec<Pick> = (0..SLOTS).map(Pick::Slot).chain(others.iter().map(|&s| Pick::Player(s))).collect();
    assert_eq!(seen, want, "every slot and player, in order");
    for (holding, k, fired) in [(0, 0.0, false), (HOLD_MS / 4, 0.25, false), (HOLD_MS / 2, 0.5, false), (HOLD_MS, 1.0, true), (HOLD_MS * 3, 1.0, true)] {
        let mut out = Vec::new();
        let d = ride_hud::sprites(&mut out, 0, [0.0, 0.0, 1920.0, 1080.0], &shown(&g, &[], &[], Some(panel(Pick::Slot(0), vec![], holding))), &t, now);
        let p = d.markers.unwrap();
        assert!(near(p.hold, k, 1e-6) && p.fired == fired, "{holding} ms: {} {}", p.hold, p.fired);
    }
}

/// The camera toast shows a change of preset, not the ride's first.
#[test]
fn a_camera_change_toasts() {
    let g = gauges(Speedometer::Digital);
    let mut t = Tracker::default();
    let t0 = Instant::now();
    t.note_camera(t0, "Chase");
    let mut out = Vec::new();
    assert!(ride_hud::sprites(&mut out, 0, [0.0, 0.0, 1920.0, 1080.0], &shown(&g, &[], &[], None), &t, t0).camera.is_none());
    t.note_camera(t0, "Original");
    let d = ride_hud::sprites(&mut out, 0, [0.0, 0.0, 1920.0, 1080.0], &shown(&g, &[], &[], None), &t, t0);
    let c = d.camera.expect("the toast");
    assert_eq!(c.text, "Original");
    assert!(c.rect[0] > 960.0 && c.rect[1] < 200.0, "{:?}", c.rect);
    assert!(ride_hud::sprites(&mut out, 0, [0.0, 0.0, 1920.0, 1080.0], &shown(&g, &[], &[], None), &t, t0 + ride_hud::TIMES[4]).camera.is_none());
}

/// The overlay's default is the ride HUD, first of four; a file from before P121 that says
/// `original` takes the ride HUD once, and one written since keeps the original's.
#[test]
fn the_ride_hud_is_the_default_and_old_files_move_to_it_once() {
    assert_eq!(Overlay::default(), Overlay::Hud);
    assert_eq!(Overlay::ALL, [Overlay::Hud, Overlay::Original, Overlay::Debug, Overlay::None]);
    assert_eq!(Overlay::Hud.word(), "hud");
    assert_eq!(Settings::default().overlay, Overlay::Hud);
    assert_eq!(Settings::parse("name = Rider\noverlay = original\n").overlay, Overlay::Hud);
    for o in Overlay::ALL {
        let s = Settings { overlay: o, ..Settings::default() };
        assert_eq!(Settings::parse(&s.text()).overlay, o, "{o:?} written since");
        // Saved again, it stays.
        assert_eq!(Settings::parse(&Settings::parse(&s.text()).text()).overlay, o);
    }
    for (word, o) in [("debug", Overlay::Debug), ("none", Overlay::None), ("hud", Overlay::Hud)] {
        assert_eq!(Settings::parse(&format!("overlay = {word}\n")).overlay, o);
    }
}

/// How soft the edges of `colour`'s type on black in `frame` within `r` are: of the pixels part way
/// between, the share with no near-full and no near-empty pixel both among their eight neighbours
/// and themselves (tests/menu.rs's measure). An edge a pixel wide has nearly none.
fn soft_share(frame: &Frame, r: [f32; 4], full: u8) -> (f64, usize) {
    let at = |x: usize, y: usize| frame.color[(y * frame.width + x) * 3 + 1];
    let (hi, lo) = ((full as f32 * 0.9) as u8, (full as f32 * 0.1) as u8);
    let (x0, y0, x1, y1) = (r[0] as usize - 2, r[1] as usize - 2, r[2] as usize + 2, r[3] as usize + 2);
    let (mut partial, mut soft) = (0usize, 0usize);
    for y in y0..y1 {
        for x in x0..x1 {
            let v = at(x, y);
            if v <= lo || v >= hi {
                continue;
            }
            partial += 1;
            let (mut f, mut e) = (false, false);
            for dy in 0..3 {
                for dx in 0..3 {
                    let n = at(x + dx - 1, y + dy - 1);
                    f |= n >= hi;
                    e |= n <= lo;
                }
            }
            if !(f && e) {
                soft += 1;
            }
        }
    }
    (soft as f64 / partial.max(1) as f64, partial)
}

/// The type is sharp at 4K: the score's number and the DSEG7 readout step from the ground to
/// their ink within a pixel, on the software path that rasters the GPU's own sprites.
#[test]
fn text_edges_are_a_pixel_at_4k() {
    let g = Gauges { preload: None, bike: false, ..gauges(Speedometer::Digital) };
    let mut frame = Frame::new(3840, 2160, [0, 0, 0]);
    let mut shown = shown(&g, &[], &[], None);
    (shown.minimap, shown.offer, shown.speed) = (false, None, 40.0);
    let d = ride_hud::draw(&mut frame, &shown, &Tracker::default(), Instant::now());
    for (what, r, full) in [("score", d.score.rect, ride_hud::GREY[1]), ("readout", d.speed.unwrap().rect, 255)] {
        let (share, partial) = soft_share(&frame, r, full);
        assert!(partial > 20, "{what}: {partial} edge pixels in {r:?}");
        assert!(share < 0.1, "{what}: {:.0}% of {partial} edge pixels are more than a pixel from ink and ground", share * 100.0);
    }
}

/// A PNG the shell wrote, back to its RGB pixels: its rows are filter 0 in zlib stored blocks
/// (tests/hud.rs's reader).
fn read_png(path: &Path) -> (usize, usize, Vec<u8>) {
    let png = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let be = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
    let (mut at, mut size, mut idat) = (8, (0, 0), Vec::new());
    while at + 8 <= png.len() {
        let (len, tag) = (be(at), &png[at + 4..at + 8]);
        let data = &png[at + 8..at + 8 + len];
        match tag {
            b"IHDR" => size = (be(at + 8), be(at + 12)),
            b"IDAT" => idat.extend_from_slice(data),
            _ => {}
        }
        at += 12 + len;
    }
    let mut raw = Vec::new();
    let mut i = 2;
    loop {
        let last = idat[i] & 1 != 0;
        let n = u16::from_le_bytes([idat[i + 1], idat[i + 2]]) as usize;
        raw.extend_from_slice(&idat[i + 5..i + 5 + n]);
        i += 5 + n;
        if last {
            break;
        }
    }
    let (w, h) = size;
    let rgb = (0..h).flat_map(|y| raw[y * (1 + 3 * w) + 1..(y + 1) * (1 + 3 * w)].to_vec()).collect();
    (w, h, rgb)
}

/// The shell's first frame of a ride with these arguments, or `None` with no disc.
fn first_frame(name: &str, args: &[&str]) -> Option<Vec<u8>> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let shot: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("p121-{name}.png"));
    let _ = std::fs::remove_file(&shot);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--size", "640x448", "--frames", "2"])
        .args(args)
        .arg("--shot")
        .arg(&shot)
        .output()
        .expect("the shell runs");
    assert!(out.status.success(), "{name}: the shell failed: {}", String::from_utf8_lossy(&out.stderr));
    let (w, h, rgb) = read_png(&shot);
    assert_eq!((w, h), (640, 448), "{name}");
    Some(rgb)
}

/// How many of a 640x448 box's pixels the two frames have apart.
fn apart(a: &[u8], b: &[u8], (x0, y0, x1, y1): (usize, usize, usize, usize)) -> usize {
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| 3 * (y * 640 + x)))
        .filter(|&i| (0..3).any(|k| a[i + k].abs_diff(b[i + k]) > 24))
        .count()
}

/// The shell draws each `--overlay` as its own on either renderer: the ride HUD's score bar, the
/// original's, the debug HUD's text, and nothing for `none` — the frame the others are told from.
#[test]
fn each_overlay_draws_its_own() {
    const SCORE: (usize, usize, usize, usize) = (14, 25, 182, 46);
    const METER: (usize, usize, usize, usize) = (599, 314, 619, 399);
    for renderer in ["software", "gpu"] {
        let run = |o: &str| first_frame(&format!("{renderer}-{o}"), &["--renderer", renderer, "--overlay", o]);
        let Some(none) = run("none") else { return };
        let hud = run("hud").unwrap();
        let original = run("original").unwrap();
        let debug = run("debug").unwrap();
        let whole = (0, 0, 640, 448);
        for (name, f) in [("hud", &hud), ("original", &original), ("debug", &debug)] {
            assert!(apart(f, &none, whole) > 500, "{renderer}: --overlay {name} drew nothing");
        }
        assert!(apart(&hud, &original, whole) > 500, "{renderer}: the ride HUD is the original's");
        assert!(apart(&hud, &debug, whole) > 500, "{renderer}: the ride HUD is the debug HUD");
        // The ride HUD's score bar and meter where the layout puts them.
        let area = |b: (usize, usize, usize, usize)| (b.2 - b.0) * (b.3 - b.1);
        for (part, b) in [("score bar", SCORE), ("meter", METER)] {
            let n = apart(&hud, &none, b);
            assert!(n * 4 > area(b), "{renderer}: only {n} of the {part}'s {} pixels drawn", area(b));
        }
    }
}
