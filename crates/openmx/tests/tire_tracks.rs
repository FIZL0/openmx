//! V40 — the bike's tire tracks: a strip behind each wheel on the ground, as the original's
//! `TireTrack` entries lay them (`src/tire_tracks.rs`, journal `bike-tracks`, `research/oracle.txt`
//! V40).
//!
//! `src/tire_tracks.rs` is compiled in by path, as the other tests take the shell's files. The
//! disc's half reads free09's (and every freeride's) `mxparticles.dat`, its zones and the bike's
//! tread, and skips without the disc, saying so. The rest rides: the stand-in bike
//! ([`stock::BIKE`]) launched flat out over a floor with a kicker on it, a tape of hands ridden twice,
//! the tracks stepped after every tick off the wheels as `src/main.rs`'s `tire_bike` reads them.
//!
//! What it holds:
//! - every row a strip lays is under its own wheel, on a tick that wheel touched the ground, and none
//!   is laid while the wheel is in the air — and the jump puts both wheels in the air;
//! - the marks are the ride's own: the same tape lays the same strips, and riding with the tracks
//!   leaves the ride bit for bit as it is without them;
//! - old marks go the original's way: not with age (a stopped bike's marks stay as they are), but by
//!   the ring coming round, oldest batch first, and never more than its batches of points;
//! - the fades and the rules: no mark under the speed, off on a zone the descriptor turns off and
//!   while wrecking, a fade out over its distance, a new segment past the break angle.

#[allow(dead_code)]
#[path = "../src/tire_tracks.rs"]
mod tire_tracks;

use std::sync::Arc;

use game_render::Disc;
use game_render::ini;
use game_render::mat::{V3, v3};
use game_sim::collide::{Bundle, Record};
use game_sim::phy::Phy;
use game_sim::ride::{Controls, Wheel};
use game_sim::rider::{Rider, Tables};
use game_sim::trick::Set;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use tire_tracks::{BATCH, Bike, Params, Point, Strip, Tire, Tracks};

/// The shell's fixed update's frame.
const DT: F = F(1.0 / 60.0);

/// A list like the disc's: its two entries' numbers, but marking at any compression, so the stand-in
/// bike's (which is not the game's) is past the threshold on every loaded tick and a row is laid on
/// every tick a wheel is down at speed. The zones file turns surface 0 off.
const LIST: &str = "[Front Tire Track]\nType = TireTrack\nZoneDescriptor = TireTrack\nAttachedToTire = 0\n\
Disable = NoContact | ObjectContact | Wrecking\nMaterial = ttrack\nNumberOfStrips = 32\nHeightAboveGround = 0.3\n\
Width = 0.260\nBreakAngle = 10.0\nMinimumSpeedThreshold = 3.0\nMinimumCompressionThreshold = 0.0\n\
MaximumCompressionThreshold = 1.00\nBasePercentAlpha = 0.4\nFadeInDistance = 1.0\nFadeOutDistance = 10.0\n\
[Rear Tire Track]\nType = TireTrack\nZoneDescriptor = TireTrack\nAttachedToTire = 1\n\
Disable = NoContact | ObjectContact | Wrecking\nMaterial = ttrack\nNumberOfStrips = 32\nHeightAboveGround = 0.3\n\
Width = 0.275\nBreakAngle = 10.0\nMinimumSpeedThreshold = 3.0\nMinimumCompressionThreshold = 0.0\n\
MaximumCompressionThreshold = 1.00\nBasePercentAlpha = 0.4\nFadeInDistance = 1.0\nFadeOutDistance = 10.0\n\
[Dust]\nType = Dust\nAttachedToTire = 1\n";
const ZONES: &str = "[TireTrack]\nZone1 = None\n";

/// The ramp: how far down +z it starts, its rise over its run, and its run, in feet — `tests/ride.rs`'s
/// `RAMP`, a lip of 4.5 ft. Past it the floor is back at 0.
const RAMP: (f32, f32, f32) = (250.0, 0.3, 15.0);

/// A floor at 0 with the ramp on it, along +z, nothing solid: surface 0, the stand-in tire's one
/// friction (so the ride takes the list without the zones file, every surface on).
struct Kicker;

impl World for Kicker {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let (start, slope, run) = RAMP;
        let t = at.z() - start;
        if !(0.0..=run).contains(&t) {
            return (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0));
        }
        let n = 1.0 / (1.0 + slope * slope).sqrt();
        (Vf::new(at.x(), t * slope, at.z(), 1.0), Vf::new(0.0, n, -slope * n, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// `tests/ride.rs`'s wreck limits, so the landing is not a wreck.
const RIDE_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 60
BackWreckSpeedLimit = 60
SideWreckSpeedLimit = 40
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 40
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 120
AirbornePitchUpWreckAngle = 120
AirborneRollWreckAngle = 100
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

/// The stand-in bike as `tests/ride.rs` rides it.
fn sim() -> Sim {
    let mut bike = stock::BIKE.to_string();
    for (key, with) in [
        (
            "XAxisRateLimit = 6\n",
            "XAxisRateLimit = 6\nSteerScale1 = 1.0\nSteerScale2 = 0.05\nSteerSpeed1 = 5\nSteerSpeed2 = 50\n",
        ),
        ("Drag = 0.5", "Drag = 0.037"),
    ] {
        assert!(bike.contains(key), "game_sim::stock::BIKE no longer has `{}`", key.trim_end());
        bike = bike.replace(key, with);
    }
    let tables = Arc::new(Tables::build());
    let phy = Phy::read_bytes(format!("{bike}{RIDE_LIMITS}").as_bytes(), &tables.ride.curve);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let rider = Rider::new(&phy, stock::mounts(&geom), stock::SPAWN, stock::FACING, Set::default());
    Sim::with_rider(0, tables, phy, rider)
}

/// The tape: flat out straight down +z, over the kicker, then off the gas and braked to a stop.
fn tape() -> Vec<Controls> {
    (0..900)
        .map(|i| Controls {
            gas: F(if i < 420 { 1.0 } else { 0.0 }),
            brake: F(if i >= 600 { 1.0 } else { 0.0 }),
            ..Controls::default()
        })
        .collect()
}

fn vector(v: Vf) -> V3 {
    v3(v.x(), v.y(), v.z())
}

/// A wheel as the shell's `tire_bike` reads it.
fn tire(w: &Wheel) -> Tire {
    Tire {
        at: vector(w.w.load_point),
        normal: vector(w.w.contact_normal),
        ground: w.w.ray_hit != 0,
        object: w.w.swept_contact != 0,
        compression: w.w.compression.0,
        spinning: w.slip.spinning,
        zone: w.surface,
    }
}

fn bike(sim: &Sim) -> Bike {
    let r = &sim.rider;
    Bike { tires: vec![tire(&r.ride.front), tire(&r.ride.rear)], speed: r.ride.chassis.speed.0, wrecking: r.crash.is_some() }
}

fn list() -> Vec<Params> {
    tire_tracks::list(LIST, Some(ZONES))
}

/// Every point of a strip's ring, by batch and place.
fn points(s: &Strip) -> Vec<((usize, usize), Point)> {
    let mut out = Vec::new();
    for (b, batch) in s.ring.batches.iter().enumerate() {
        for (i, p) in batch.iter().enumerate() {
            if let Some(p) = p {
                out.push(((b, i), *p));
            }
        }
    }
    out
}

/// The points written to `after` at ground `before` did not have anywhere: a batch begun mid-strip
/// starts with the last row again, which is old ground and not a new mark.
fn new_points(before: &Strip, after: &Strip) -> Vec<Point> {
    let was = points(before);
    points(after).into_iter().filter(|(_, p)| !was.iter().any(|(_, q)| q.at == p.at)).map(|(_, p)| p).collect()
}

/// One tick of the ride: what each wheel was and which points each strip laid.
struct Tick {
    bike: Bike,
    laid: Vec<Vec<Point>>,
    state: Vec<u32>,
}

/// The ride's state as bits, to hold the tracks to reading it only.
fn state(sim: &Sim) -> Vec<u32> {
    let ride = &sim.rider.ride;
    let m = ride.transform();
    let mut out: Vec<u32> = (0..4).flat_map(|r| (0..4).map(move |c| (r, c))).map(|(r, c)| m.0[r].0[c].to_bits()).collect();
    out.extend((0..4).map(|c| ride.chassis.vel.0[c].to_bits()));
    out.push(ride.chassis.speed.0.to_bits());
    out
}

/// The ride's list: no zones file (the kicker is all surface 0), and a compression threshold the
/// stand-in bike's suspension crosses, about 0.23 rolling along and 0.68 landing, so it has light
/// ticks as the game's bike does against the disc's 0.75 and 0.85. A light tick is what brings an
/// entry back after the air turned it off: its fade-out sets the fade-in left to what the fade-out
/// has, which an off made 0.
fn ride_list() -> Vec<Params> {
    let mut l = tire_tracks::list(LIST, None);
    for p in &mut l {
        p.min_compression = RIDE_MIN_COMPRESSION;
    }
    l
}

const RIDE_MIN_COMPRESSION: f32 = 0.2;

/// The tape on the kicker, the tracks stepped after each tick when `tracks` says so.
fn ride(tracks: bool) -> (Vec<Tick>, Tracks) {
    let mut sim = sim();
    let mut marks = Tracks::new(&ride_list(), 1);
    let mut out = Vec::new();
    for hands in tape() {
        sim.step(DT, &hands, &mut Kicker);
        let b = bike(&sim);
        let before = marks.clone();
        if tracks {
            marks.step(&b);
        }
        let laid = before.tracks.iter().zip(&marks.tracks).map(|(a, z)| new_points(&a.strip, &z.strip)).collect();
        out.push(Tick { bike: b, laid, state: state(&sim) });
    }
    (out, marks)
}

#[test]
fn the_list_takes_the_tire_track_entries() {
    let l = list();
    assert_eq!(l.len(), 2, "the dust is not a track");
    assert_eq!((l[0].name.as_str(), l[0].tire, l[0].width), ("Front Tire Track", 0, 0.26));
    assert_eq!((l[1].name.as_str(), l[1].tire, l[1].width), ("Rear Tire Track", 1, 0.275));
    assert_eq!((l[1].strips, l[1].height, l[1].min_speed, l[1].fade_in, l[1].fade_out), (32, 0.3, 3.0, 1.0, 10.0));
    assert!((l[1].break_angle - 10f32.to_radians()).abs() < 1e-6);
    assert_eq!(l[1].disable, tire_tracks::Disable::NO_CONTACT | tire_tracks::Disable::OBJECT_CONTACT | tire_tracks::Disable::WRECKING);
    assert!(!l[0].zones[0] && l[0].zones[1..].iter().all(|z| *z), "Zone1 = None is surface 0 off, the rest on");
    // The constructor's numbers where a file says nothing.
    let bare = tire_tracks::list("[T]\nType = TireTrack\n", None);
    assert_eq!((bare[0].min_speed, bare[0].min_compression, bare[0].base_alpha, bare[0].fade_out), (10.0, 250.0, 0.7, 5.0));
}

#[test]
fn a_replayed_ride_lays_marks_under_each_wheel_in_contact_and_none_in_the_air() {
    let (ticks, marks) = ride(true);
    let l = ride_list();
    let mut laid = [0usize; 2];
    let mut air = 0;
    for (i, t) in ticks.iter().enumerate() {
        let both_up = t.bike.tires.iter().all(|w| !w.ground && !w.object);
        air += usize::from(both_up);
        for (k, (p, new)) in l.iter().zip(&t.laid).enumerate() {
            let w = &t.bike.tires[p.tire];
            if new.is_empty() {
                continue;
            }
            assert!(w.ground && !w.object, "tick {i}: {} laid with its wheel off the ground", p.name);
            laid[k] += new.len();
            // Each point is the wheel's point, up the normal by the height, out by the width: within
            // the two of the wheel (and the noise).
            let reach = (p.width * p.width + (p.height + 0.005) * (p.height + 0.005)).sqrt() + 1e-3;
            for q in new {
                let d = q.at.sub(w.at).len();
                assert!(d <= reach, "tick {i}: {} laid {d:.3} ft from its wheel (reach {reach:.3})", p.name);
                assert!(q.at.sub(w.at).dot(w.normal) > 0.0, "tick {i}: {} under the ground", p.name);
            }
        }
    }
    assert!(air >= 10, "the kicker puts both wheels in the air: {air} ticks");
    assert!(laid.iter().all(|n| *n > 64), "both wheels mark: {laid:?}");
    // And it marked on both sides of the jump: before the ramp and after the landing.
    let lip = RAMP.0 + RAMP.2;
    let after: usize = ticks.iter().filter(|t| t.bike.tires[1].at.z > lip + 30.0).map(|t| t.laid[1].len()).sum();
    let before: usize = ticks.iter().filter(|t| t.bike.tires[1].at.z < RAMP.0).map(|t| t.laid[1].len()).sum();
    assert!(before > 0 && after > 0, "marks before the ramp ({before}) and after the landing ({after})");
    // Nothing is drawn past the ring's size.
    for t in &marks.tracks {
        assert!(points(&t.strip).len() <= t.params.strips * BATCH);
    }
}

#[test]
fn the_marks_are_the_rides_own_and_touch_nothing() {
    let (with, a) = ride(true);
    let (without, _) = ride(false);
    assert!(with.iter().zip(&without).all(|(x, y)| x.state == y.state), "the tracks wrote to the ride");
    let (_, b) = ride(true);
    assert_eq!(a, b, "the same tape laid other marks");
}

/// A tire on flat dirt at `z` on x = 0, down and loaded, surface 1.
fn on(z: f32) -> Tire {
    Tire { at: v3(0.0, 0.0, z), normal: v3(0.0, 1.0, 0.0), ground: true, object: false, compression: 0.5, spinning: false, zone: 1 }
}

/// A one-entry set on tire 0 of `strips` batches, the disc's rear numbers, marking at compression 0.
fn one(strips: usize) -> Tracks {
    let text = LIST.replace("NumberOfStrips = 32", &format!("NumberOfStrips = {strips}"));
    let mut l = tire_tracks::list(&text, Some(ZONES));
    l.truncate(1);
    Tracks::new(&l, 7)
}

fn step(t: &mut Tracks, tire: Tire, speed: f32) {
    t.step(&Bike { tires: vec![tire], speed, wrecking: false });
}

#[test]
fn old_marks_go_by_the_ring_and_not_with_age() {
    let mut t = one(4);
    // 3 ft a tick down a straight: a row a tick, a segment every few rows.
    for i in 0..20 {
        step(&mut t, on(3.0 * i as f32), 60.0);
    }
    let laid = points(&t.tracks[0].strip);
    assert!(!laid.is_empty());
    // Stopped, the marks stay exactly as they are: nothing fades with time.
    let held = t.clone();
    for _ in 0..600 {
        step(&mut t, on(57.0), 0.0);
    }
    assert_eq!(points(&t.tracks[0].strip), points(&held.tracks[0].strip), "a still bike's marks changed");
    // Riding on, the ring comes round: the first rows laid are overwritten and the ring never holds
    // more than its batches' points.
    let first = laid.iter().find(|((b, i), _)| *b == 0 && *i == 0).map(|(_, p)| p.at).expect("batch 0 begun first");
    for i in 20..200 {
        step(&mut t, on(3.0 * i as f32), 60.0);
        assert!(points(&t.tracks[0].strip).len() <= 4 * BATCH);
    }
    let now = points(&t.tracks[0].strip);
    assert!(now.iter().all(|(_, p)| p.at.z > 100.0), "the oldest marks are still there");
    assert!(!now.iter().any(|(_, p)| p.at == first));
    // And each batch draws as one strip: a batch's triangles are its points in order, a restart
    // starting a new run.
    let tris = t.tracks[0].strip.ring.triangles();
    assert!(!tris.is_empty() && tris.len() <= 4 * (BATCH - 2));
}

#[test]
fn a_tire_under_the_speed_or_off_the_zones_or_wrecking_lays_nothing() {
    let rows = |f: &dyn Fn(usize) -> (Tire, f32, bool)| {
        let mut t = one(8);
        for i in 0..40 {
            let (tire, speed, wrecking) = f(i);
            t.step(&Bike { tires: vec![tire], speed, wrecking });
        }
        points(&t.tracks[0].strip).len()
    };
    assert!(rows(&|i| (on(3.0 * i as f32), 60.0, false)) > 0, "the control");
    // At the speed threshold the entry is off but for the constructor's 5 ft of fade-out left, which
    // a fresh entry spends as the original's does: rows over the first 5 ft and none after.
    let mut t = one(8);
    for i in 0..40 {
        step(&mut t, on(3.0 * i as f32), 3.0);
    }
    let slow = points(&t.tracks[0].strip);
    assert!(slow.iter().all(|(_, p)| p.at.z <= 5.0 + 3.0), "at the speed threshold: {slow:?}");
    assert_eq!(rows(&|i| (Tire { zone: 0, ..on(3.0 * i as f32) }, 60.0, false)), 0, "on a zone turned off");
    assert_eq!(rows(&|i| (on(3.0 * i as f32), 60.0, true)), 0, "wrecking");
    assert_eq!(rows(&|i| (Tire { object: true, ..on(3.0 * i as f32) }, 60.0, false)), 0, "on an object");
    assert_eq!(rows(&|i| (Tire { ground: false, ..on(3.0 * i as f32) }, 60.0, false)), 0, "in the air");
    // A wheel neither spinning nor past the compression threshold does not mark.
    let mut l = tire_tracks::list(LIST, Some(ZONES));
    l[0].min_compression = 0.85;
    let mut t = Tracks::new(&l[..1], 3);
    for i in 0..40 {
        step(&mut t, on(3.0 * i as f32), 60.0);
    }
    let light = points(&t.tracks[0].strip);
    assert!(light.iter().all(|(_, p)| p.at.z <= 5.0 + 3.0), "a light wheel marked past the first fade: {light:?}");
    let n = light.len();
    for i in 40..80 {
        step(&mut t, Tire { spinning: true, ..on(3.0 * i as f32) }, 60.0);
    }
    assert!(points(&t.tracks[0].strip).len() > n, "a spinning wheel marks");
}

#[test]
fn a_mark_fades_out_over_its_distance_a_segment_at_a_time() {
    let mut l = tire_tracks::list(LIST, Some(ZONES));
    l[0].min_compression = 0.85;
    let mut t = Tracks::new(&l[..1], 5);
    // Spinning for 60 ft, then not: the fade-out runs 10 ft, and its rows carry their segment's alpha,
    // the base 0.4 of 0x80 at most, stepping down.
    for i in 0..20 {
        step(&mut t, Tire { spinning: true, ..on(3.0 * i as f32) }, 60.0);
    }
    for i in 20..40 {
        step(&mut t, on(3.0 * i as f32), 60.0);
    }
    let pts = points(&t.tracks[0].strip);
    assert!(pts.iter().all(|(_, p)| p.alpha <= 51 || p.alpha == 0x80), "past the base alpha");
    let last = pts.iter().map(|(_, p)| p.at.z).fold(0.0, f32::max);
    assert!(last > 60.0 && last <= 60.0 + 10.0 + 3.0, "faded out 10 ft past the spin, ended at {last}");
    assert!(t.tracks[0].strip.ended, "the strip ends when the alpha is gone");
}

#[test]
fn a_turn_past_the_break_angle_starts_a_segment() {
    let mut s = Strip::new(8, 1);
    let up = v3(0.0, 1.0, 0.0);
    for i in 0..4 {
        s.add(0.25, 0.3, 10f32.to_radians(), v3(0.0, 0.0, 2.5 * i as f32), up, 0.4);
    }
    let count = s.count;
    // Straight on, short of the segment's length: no new segment.
    s.add(0.25, 0.3, 10f32.to_radians(), v3(0.0, 0.0, 10.0), up, 0.4);
    assert_eq!(s.count, count);
    // Off at 45 degrees: one.
    s.add(0.25, 0.3, 10f32.to_radians(), v3(3.0, 0.0, 13.0), up, 0.4);
    assert_eq!(s.count, count + 2);
    assert_eq!(s.u, -8.0, "a new segment starts its texture at -8");
}

// ---------------------------------------------------------------------------------------------------
// the disc
// ---------------------------------------------------------------------------------------------------

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("SKIP: no disc ({e})");
            None
        }
    }
}

/// The surfaces each freeride's `[TireTrack]` descriptor turns off (`ZoneN = None` is surface N-1):
/// free06 and free10 mark every surface, free11 a second one off besides 0.
fn zones_off(archive: &str) -> Vec<usize> {
    let a = archive.to_ascii_lowercase();
    let track = ["free06", "free07", "free09", "free10", "free11"].into_iter().find(|t| a.contains(t));
    match track {
        Some("free06" | "free10") => vec![],
        Some("free07" | "free09") => vec![0],
        Some("free11") => vec![0, 9],
        _ => panic!("{archive}: a freeride this test does not know"),
    }
}

#[test]
fn every_freeride_lays_the_discs_two_tracks_with_the_bikes_tread() {
    let Some(mut disc) = disc() else { return };
    let all: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).collect();
    let low = |n: &String| n.rsplit(['\\', '/']).next().unwrap_or(n).to_ascii_lowercase();
    let names: Vec<String> = all.iter().filter(|n| low(n).starts_with("free") && low(n).ends_with("_2.res") && !low(n).contains("_ss")).cloned().collect();
    assert!(names.len() >= 4, "the freerides' second archives: {names:?}");
    for name in names {
        let first = all.iter().find(|n| low(n) == low(&name).replace("_2.res", ".res")).expect(&name);
        let archives = [disc.res(first).expect(first), disc.res(&name).expect(&name)];
        let read = |suffix: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{suffix}")).ok());
        let list = read(tire_tracks::LIST).unwrap_or_else(|| panic!("{name}: no {}", tire_tracks::LIST));
        let zones = read(tire_tracks::ZONES).map(|z| ini::text(&z));
        let l = tire_tracks::list(&ini::text(&list), zones.as_deref());
        assert_eq!(l.len(), 2, "{name}: {l:?}");
        let (f, r) = (&l[0], &l[1]);
        assert_eq!((f.tire, f.width, f.min_compression), (0, 0.26, 0.85), "{name}");
        assert_eq!((r.tire, r.width, r.min_compression), (1, 0.275, 0.75), "{name}");
        for p in &l {
            assert_eq!((p.strips, p.height, p.min_speed, p.max_compression), (32, 0.3, 3.0, 1.0), "{name}");
            assert_eq!((p.base_alpha, p.fade_in, p.fade_out, p.material.as_str()), (0.4, 1.0, 10.0, "ttrack"), "{name}");
            let off: Vec<usize> = (0..tire_tracks::ZONE_COUNT).filter(|z| !p.zones[*z]).collect();
            assert_eq!(off, zones_off(&name), "{name}: the surfaces its descriptor turns off");
        }
        let tread = archives.iter().find_map(|a| a.read_suffix("bike\\tex\\ttrack.pst").ok()).unwrap_or_else(|| panic!("{name}: no tread"));
        let t = game_render::pst::Texture::parse(&tread).unwrap();
        assert_eq!((t.width, t.height), (32, 64), "{name}");
        assert!(t.rgba.chunks(4).all(|c| c[..3] == [0, 0, 0]), "{name}: the tread is black");
        assert!(t.rgba.chunks(4).any(|c| c[3] > 64), "{name}: a tread in its alpha");
    }
}
