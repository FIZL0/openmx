//! V51 — the activities' start zones drawn in the world as the original draws them, a disc with an
//! arrow, a column of light and the activity's word, and the session markers as small copies of the
//! same in their slots' colours, the selected one brighter, with no post.
//!
//! The markers are put down the way the shell puts them down (`tests/markers.rs`'s ride: the stand-in
//! bike on a flat floor, the d-pad through `markers::frame`). The portals' pictures and models are the
//! disc's (`free09_2.res`); the tests that need them skip without it.

// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
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
#[path = "../src/portal.rs"]
#[allow(dead_code)]
mod portal;
#[allow(dead_code)]
#[path = "../src/net/mod.rs"]
mod net;

use game_render::Disc;
use game_render::glow::Glow;
use game_sim::input::{Control, PADS, Report};
use game_sim::markers::{self, Pick, SLOTS, Spot};
use game_sim::ride::Controls;
use game_sim::world::Flat;
use game_sim::Sim;
use ps2_float::F;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);
const GAS: u16 = 0x0040; // Cross, the shipped preset's gas

fn pads(buttons: u16) -> [Report; PADS] {
    let mut r = [Report::missing(); PADS];
    r[0] = Report::from_buttons(buttons);
    r
}

/// One tick of every rider, player 1 on `buttons`, then player 1's markers.
fn tick(sim: &mut Sim, buttons: u16) {
    let spots: Vec<Spot> = (0..sim.players()).map(|i| Spot::of(sim.rider_of(i))).collect();
    let mut hands: Vec<Controls> = sim.poll_all(DT, &pads(buttons)).into_iter().map(Controls::stick_alone).collect();
    hands.resize(sim.players(), Controls::default());
    sim.step_all(DT, &hands, &mut Flat::default());
    markers::frame(sim, 0, &spots, &hands[0]);
}

/// `control` pressed on top of the gas — Left and Right for a tick, Down and Up held the eight ticks
/// that cross `markers::HOLD_MS` at 30 Hz (P106) — then let go for one.
fn press(sim: &mut Sim, control: Control) {
    let ticks = if matches!(control, Control::Up | Control::Down) { 8 } else { 1 };
    for _ in 0..ticks {
        tick(sim, GAS | control.mask());
    }
    tick(sim, GAS);
}

/// A ride with markers in slots 1 and 3 (0 and 2), put down 15 ticks apart, the selection on the
/// second.
fn two_markers() -> Sim {
    let mut sim = Sim::new(0x88);
    for _ in 0..45 {
        tick(&mut sim, GAS);
    }
    press(&mut sim, Control::Down);
    press(&mut sim, Control::Right);
    press(&mut sim, Control::Right);
    for _ in 0..15 {
        tick(&mut sim, GAS);
    }
    press(&mut sim, Control::Down);
    assert!(sim.markers.holds(0) && sim.markers.holds(2) && !sim.markers.holds(1));
    assert_eq!(sim.markers.pick(0, 1), Pick::Slot(2));
    sim
}

/// The portals' art off the disc, or `None` without one.
fn art() -> Option<(portal::Art, String)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("no disc, the art's checks skipped: {e}");
            return None;
        }
    };
    let res = disc.res("Tracks\\free09_2.res").expect("free09's second archive");
    let art = portal::Art::load(|name| res.read_suffix(name).ok()).expect("the portal's art");
    let hit = String::from_utf8_lossy(&res.read_suffix("free09\\hit.dat").expect("free09's hit.dat")).into_owned();
    Some((art, hit))
}

/// Every corner of `glow`.
fn corners(glow: &Glow) -> impl Iterator<Item = &game_render::glow::Vertex> {
    glow.tris.iter().flatten()
}

/// The farthest any corner of `glow` is from `at` along the ground, and its lowest and highest.
fn reach(glow: &Glow, at: [f32; 3]) -> (f32, f32, f32) {
    let flat = |p: [f32; 3]| ((p[0] - at[0]).powi(2) + (p[2] - at[2]).powi(2)).sqrt();
    corners(glow).fold((0.0f32, f32::MAX, f32::MIN), |(r, lo, hi), v| (r.max(flat(v.at)), lo.min(v.at[1]), hi.max(v.at[1])))
}

/// A filled slot draws the start zone's portal where its bike was put down, a quarter of a zone's
/// size, facing the way the bike faced, in its slot's colour — the selected one more opaque — and
/// nothing else: no post standing over it, nothing outside its column. An empty slot draws nothing,
/// and neither does a view that shows no markers.
#[test]
fn a_filled_slot_draws_the_zone_portal_small_and_no_post() {
    let sim = two_markers();
    let mut built: Vec<Option<portal::Portal>> = Vec::new();
    let ground = |_: f32, _: f32, _: f32| None;
    portal::markers(&mut built, &sim.markers, Some(Pick::Slot(2)), ground);
    let filled: Vec<usize> = (0..SLOTS).filter(|&s| built[s].is_some()).collect();
    assert_eq!(filled, vec![0, 2], "a portal a filled slot");

    let art = art();
    let empty = portal::Art::default();
    let drawn_with = art.as_ref().map_or(&empty, |(a, _)| a);
    let mut opacity = Vec::new();
    for slot in [0, 2] {
        let p = built[slot].as_ref().unwrap();
        let ride = sim.markers.slots[slot].as_ref().unwrap();
        let (at, (forward, _)) = (ride.position(), ride.axes());
        assert_eq!(p.at, [at.x(), at.y(), at.z()], "slot {slot} stands where its bike was put down");
        assert_eq!(p.kind, portal::Kind::Marker { slot, selected: slot == 2 });
        assert_eq!(p.fraction, portal::MARKER_FRACTION);
        assert_eq!(p.size, [portal::ZONE_SIZE * portal::MARKER_FRACTION; 2], "a quarter of a zone's 40 ft");
        let l = (forward.x().powi(2) + forward.z().powi(2)).sqrt();
        assert!((p.look[0] - forward.x() / l).abs() < 1e-5 && (p.look[1] - forward.z() / l).abs() < 1e-5, "{:?} {forward:?}", p.look);
        let [r, g, b, _] = overlay::marker_colour(slot);
        assert_eq!(p.tint, [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0], "slot {slot}'s colour");
        assert_eq!(p.number, Some(slot + 1));

        let eye = [p.at[0] - 30.0, p.at[1] + 6.0, p.at[2] - 30.0];
        let glows = p.glows(drawn_with, eye, [1.0, -0.1, 1.0], 0.0);
        // The disc: round the spot, no wider than the portal, laid on the ground under it.
        let disc = &glows[0];
        assert_eq!(disc.texture, portal::DISC);
        assert!(!disc.additive && !disc.tris.is_empty());
        let (radius, lo, hi) = reach(disc, p.at);
        assert!((radius - p.size[0] / 2.0).abs() < 0.01, "slot {slot}'s disc is {radius} ft out");
        assert!(lo > p.at[1] && hi < p.at[1] + 1.0, "slot {slot}'s disc is laid at {lo}..{hi} over {}", p.at[1]);
        // Its arrow points the way the bike faced: the picture's top (v = 0) is ahead of the spot.
        let top = corners(disc).min_by(|a, b| a.uv[1].total_cmp(&b.uv[1])).unwrap();
        let ahead = (top.at[0] - p.at[0]) * p.look[0] + (top.at[2] - p.at[2]) * p.look[1];
        assert!(ahead > 0.45 * p.size[1], "the arrow's tip is {ahead} ft ahead");
        opacity.push(corners(disc).map(|v| v.tint[3]).fold(0.0, f32::max));
        assert!(corners(disc).all(|v| v.tint[..3] == p.tint[..]), "the disc is tinted the slot's colour");
        // Nothing stands outside the portal: no post over it, and the column and the number keep
        // within its width and under the column's top.
        let top = p.at[1] + 200.0 * portal::MARKER_FRACTION;
        for g in &glows {
            let (radius, _, hi) = reach(g, p.at);
            assert!(radius <= p.size[0] / 2.0 + 0.01 && hi <= top + 0.01, "slot {slot}: {radius} ft out, up to {hi}");
        }
        if art.is_some() {
            let kinds: Vec<(usize, bool)> = glows.iter().map(|g| (g.texture, g.additive)).collect();
            assert_eq!(kinds, [(portal::DISC, false), (portal::COLUMN, true), (portal::WORDS, false)], "a disc, a column and the number");
            let (_, lo, hi) = reach(&glows[1], p.at);
            assert!((hi - lo - 225.0 * portal::MARKER_FRACTION).abs() < 0.5, "the column is {} ft tall", hi - lo);
        }
    }
    assert!((opacity[0] - 0.6).abs() < 1e-5, "{opacity:?}");
    assert!(opacity[1] > opacity[0] + 0.2, "the selected marker's is brighter: {opacity:?}");

    // The selection moved off it: built again, no longer the brighter one. No markers shown: none.
    portal::markers(&mut built, &sim.markers, Some(Pick::Slot(1)), ground);
    assert_eq!(built[2].as_ref().unwrap().kind, portal::Kind::Marker { slot: 2, selected: false });
    portal::markers(&mut built, &sim.markers, None, ground);
    assert!(built.iter().all(Option::is_none));
}

/// `hit.dat`'s zones as the portals read them: where, which way, how big, and each one's number in
/// its progression — its `HitSetID`'s place in the hit or the run list.
#[test]
fn the_zones_read_their_number_from_the_progressions() {
    use overlay::Activity::*;
    let text = "[Progression HitSets]\nHitSetID=1\nHitSetID=2\n\n[Run HitSets]\nHitSetID=6\nHitSetID=7\n\n\
        [Activity Start Zones]\nASZCount = 4\n\n\
        [ASZ 1]\nPosition = 4105.0, 217.35, 3336.0, 1.0\nLookVector = 0.707, 0.0, 0.707, 1.0\nScale = 40.0, 0.0, 40.0\nHitSetID = 2\nType=Hit\n\n\
        [ASZ 2]\nPosition = 10.0, 20.0, 30.0, 1.0\nScale = 30.0, 0.0, 50.0\nHitSetID = 7\nType=Run\r\n\n\
        [ASZ 3]\nPosition = 1.0, 2.0, 3.0, 1.0\nHitSetID = 0\nType=Race\n\n\
        [ASZ 4]\nPosition = 1.0, 2.0, 3.0, 1.0\nType=Bowling\n";
    let spots = portal::spots(text);
    let got: Vec<_> = spots.iter().map(|s| (s.activity, s.at, s.look, s.size, s.number)).collect();
    assert_eq!(
        got,
        [
            (Hit, [4105.0, 217.35, 3336.0], [0.707, 0.707], [40.0, 40.0], Some(2)),
            (Run, [10.0, 20.0, 30.0], [0.0, 1.0], [30.0, 50.0], Some(2)),
            (Race, [1.0, 2.0, 3.0], [0.0, 1.0], [40.0, 40.0], None),
        ]
    );
    assert_eq!(portal::shown(&spots, true).len(), 2, "no race from a machine");
}

/// A track's start zones draw the portal at full size: the four the original shows a new player on
/// free09, each where `hit.dat` puts it, its disc the zone's 40 ft, its column 225 ft standing from
/// 25 ft under the zone, its word over it — the hit's and the run's with their number — in the
/// activity's colour, the radar icon's.
#[test]
fn a_tracks_zones_draw_the_portal_full_size() {
    use overlay::Activity::*;
    let Some((art, hit)) = art() else { return };
    let spots = portal::spots(&hit);
    assert_eq!(spots.len(), 12, "free09's twelve zones");
    let shown = portal::shown(&spots, false);
    assert_eq!(shown.iter().map(|s| s.activity).collect::<Vec<_>>(), [Hit, Run, Race, Stunt]);
    for spot in &shown {
        let p = portal::Portal::zone(spot, |_, _| Some(spot.at[1] - 2.0));
        assert_eq!((p.at, p.size, p.fraction), (spot.at, [40.0, 40.0], 1.0), "{:?}", spot.activity);
        let colour = overlay::activity_colour(spot.activity);
        assert_eq!(p.tint, [0, 1, 2].map(|i| colour[i] as f32 / 255.0));
        assert_eq!(p.number, matches!(spot.activity, Hit | Run).then_some(1), "the first of its progression");
        let glows = p.glows(&art, [spot.at[0] - 200.0, spot.at[1] + 30.0, spot.at[2]], [1.0, 0.0, 0.0], 1.0);
        let (radius, lo, hi) = reach(&glows[0], p.at);
        assert!((radius - 20.0).abs() < 0.01 && lo > spot.at[1] - 2.0 && hi < spot.at[1] - 1.0, "{radius} {lo} {hi}");
        let (radius, lo, hi) = reach(&glows[1], p.at);
        assert!(glows[1].additive && (radius - 19.5).abs() < 0.1, "the column is {radius} ft round");
        assert!((lo - (spot.at[1] - 25.0)).abs() < 0.5 && (hi - (spot.at[1] + 200.0)).abs() < 0.5, "the column: {lo}..{hi}");
        let expected = 2 + p.number.iter().count() + 1;
        assert_eq!(glows.len(), expected, "{:?}: the disc, the column, the number and the word", spot.activity);
        // The word, 40 ft to the model's unit, over the zone and higher than the number.
        let word = glows.last().unwrap();
        let (_, word_lo, _) = reach(word, p.at);
        assert!(word_lo > spot.at[1] + 15.0, "{:?}'s word at {word_lo}", spot.activity);
    }
    // And a marker's is a quarter of it, the column and all.
    let zone = portal::Portal::zone(&shown[0], |_, _| None);
    let marker = portal::Portal::marker(0, false, shown[0].at, shown[0].look, |_, _| None);
    let tall = |p: &portal::Portal| {
        let (_, lo, hi) = reach(&p.glows(&art, [0.0; 3], [0.0, 0.0, 1.0], 0.0)[1], p.at);
        hi - lo
    };
    assert!((tall(&marker) * 4.0 - tall(&zone)).abs() < 0.5, "{} against {}", tall(&marker), tall(&zone));
}
