//! P153: the wreck's dust and chunks — `Vehicle Dust` and `Vehicle Chunks` off the crash body's
//! contacts, `Rider Dust` and `Rider Chunks` off the ragdoll's spheres — against the original, row
//! by row.
//!
//! `context/capture/p153/wreck.csv` is S109's animandpump played back in PCSX2 from frame 3700 over
//! the wreck (journal `wreck-particle-contacts`, P153): each of the five wreck entries' flag byte and
//! its emitter's switch byte, carry and view-cull rate scale, the crash body's ground and object
//! words, velocity and contact list, and the rider's nine collision spheres (contact words, zone,
//! point and bone velocity). The port is fed each row's contacts and must leave every entry switched
//! as the original's is, with the dusts' carry to the bit's neighbourhood (their rate is the distance
//! and speed, held at its ends) and the chunks' carry moved by as many calls as the port made (their
//! rate is random).
//!
//! The capture and the disc are not in the repo: without them the test says so and passes.

#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;

use std::collections::HashMap;
use std::path::PathBuf;

use game_render::Disc;
use game_render::biplane;
use game_render::ini;
use game_render::mat::{V3, v3};
use particles::{Atlas, ConstraintFrame, Kind, PlaneFrame, PlaneFx, Zone};

const TRACK: &str = "free09";

/// The capture's spheres and contacts (its make_spec.py).
const SPHERES: usize = 9;
const CONTACTS: usize = 12;

/// The one row whose ragdoll spheres are not the ones the original's manager read a row before
/// (journal P153): its Rider Dust ran no sphere where the row before has four touching. The port is
/// put back on the capture after it.
const RAGDOLL_PHASE: [u32; 1] = [3850];

/// The respawn's row (journal P153): the manager still ran on the wreck (Rider Dust's carry moves as
/// the port's), and the respawn then switched the wreck's entries off before the row was read, so
/// its switches are not compared. The port is put back on the capture after it.
const RESPAWN: [u32; 1] = [3918];

type Row = HashMap<String, String>;

fn context() -> PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context"),
    }
}

/// free09's bike particles off the disc (as tests/bike_particles.rs reads them).
fn disc_fx() -> Option<PlaneFx> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("skipped: {why}");
            return None;
        }
    };
    let names = disc.entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    let member = |end: &str| names.iter().filter(|n| n.to_ascii_lowercase().ends_with(end)).min_by_key(|n| n.len()).cloned().unwrap();
    let archives = [disc.res(&member(&format!("{TRACK}.res"))).unwrap(), disc.res(&member(&format!("{TRACK}_2.res"))).unwrap()];
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let texture = game_render::pst::Texture::parse(&read(&particles::sheet_texture(TRACK)).unwrap()).unwrap();
    let atlas = Atlas::parse(&ini::text(&read(&particles::sheet(TRACK)).unwrap()), texture.width, texture.height);
    let list = biplane::emitters(&ini::text(&read(particles::BIKE_PARTICLES).unwrap()));
    Some(PlaneFx::new(&list, atlas, read))
}

fn capture() -> Option<Vec<Row>> {
    let path = context().join("capture/p153/wreck.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: no {}", path.display());
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(str::to_string)).collect()).collect())
}

fn num(r: &Row, k: &str) -> f32 {
    r[k].parse().unwrap_or_else(|_| panic!("{k} = {:?}", r[k]))
}

fn word(r: &Row, k: &str) -> u32 {
    r[k].parse().unwrap_or_else(|_| panic!("{k} = {:?}", r[k]))
}

fn vector(r: &Row, k: &str) -> V3 {
    v3(num(r, &format!("{k}.x")), num(r, &format!("{k}.y")), num(r, &format!("{k}.z")))
}

/// What the manager read for the frame between rows `a` and `b`: the wreck off `a`'s snapshot, the
/// crash body as `b` has it (it is moved before the manager runs), the spheres as `a` has them (the
/// ragdoll is stepped after it, so `b`'s are the next frame's).
fn feed(a: &Row, b: &Row) -> PlaneFrame {
    let n = (word(b, "body_n") as usize).min(CONTACTS);
    let points: Vec<V3> = (0..n).map(|c| vector(b, &format!("c{c}_at"))).collect();
    let mut frame = PlaneFrame { wrecked: word(a, "s868") != 0, contact_vel: Some(vector(b, "body_vel")), ..PlaneFrame::default() };
    if word(b, "body_ground") != 0 {
        frame.ground_contacts = points.clone();
        frame.ground_zones = (0..n).map(|c| word(b, &format!("c{c}_zone")) as u8).collect();
    }
    if word(b, "body_object") != 0 {
        frame.object_contacts = points;
    }
    frame.constraints = (0..SPHERES.min(word(a, "spheres_n") as usize))
        .map(|s| ConstraintFrame {
            ground: word(a, &format!("r{s}_ground")) != 0,
            object: word(a, &format!("r{s}_object")) != 0,
            at: vector(a, &format!("r{s}_at")),
            vel: vector(a, &format!("r{s}_vel")),
            zone: word(a, &format!("r{s}_zone")) as u8,
        })
        .collect();
    frame
}

/// The emitter's switch byte as the port has it: on (4), and just switched on (2, the next point it
/// is run at only becomes where it was).
fn switched(e: &particles::Entry) -> u8 {
    let last = e.sources.first().and_then(|s| s.last);
    if e.lit { 4 | if last.is_none() { 2 } else { 0 } } else { 0 }
}

fn carry(e: &particles::Entry) -> f32 {
    e.sources.first().map_or(0.0, |s| s.carry)
}

/// Puts the port's entry `i` where row `r` has it.
fn sync(e: &mut particles::Entry, r: &Row, i: usize) {
    let on = word(r, &format!("e{i}_on"));
    e.lit = on & 4 != 0;
    e.sources.resize_with(1, Default::default);
    e.sources[0].carry = num(r, &format!("e{i}_carry"));
    e.sources[0].last = (on & 6 == 4).then(|| vector(r, &format!("e{i}_at")));
}

/// The disc's five wreck entries as the original's RAM has them (S109's start state, journal P153):
/// the two dusts' rate goes by distance and speed, and every zone but the first draws.
#[test]
fn the_wreck_entries_read_as_the_original() {
    let Some(fx) = disc_fx() else { return };
    let names: Vec<&str> = fx.entries[..5].iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Vehicle Dust", "Vehicle Chunks", "Rider Dust", "Rider Chunks", "Foot Dust"]);
    for (i, e) in fx.entries[..5].iter().enumerate() {
        let dust = i == 0 || i == 2;
        assert_eq!((e.emt.distance_ratio, e.emt.speed_ratio, e.emt.min_movement_speed), if dust { (1.0, 10.0, 0.0) } else { (0.0, 0.0, 0.0) }, "{}", e.name);
        assert_eq!(e.emt.min_speed, [3.0, 15.0, 3.0, 15.0, 30.0][i], "{}", e.name);
        assert_eq!(e.emt.rate, [(10.0, 20.0), (100.0, 150.0), (10.0, 20.0), (10.0, 20.0), (100.0, 150.0)][i], "{}", e.name);
        assert_eq!(e.zones.len(), 16, "{}", e.name);
        assert_eq!(e.zones[0], Zone::Off, "{}", e.name);
        assert!(e.zones[1..].iter().all(|z| matches!(z, Zone::Group(..))), "{}: {:?}", e.name, e.zones);
        assert_eq!(matches!(e.kind, Kind::CollisionModel), i < 2, "{}", e.name);
    }
}

/// The original's five wreck entries, row by row over the wreck: switched on and off on the rows its
/// are, the dusts' carry within a hair of its, and the chunks' carry moved by a share of as many calls
/// as the port ran.
#[test]
fn the_wreck_emits_as_the_original() {
    let (Some(mut fx), Some(rows)) = (disc_fx(), capture()) else { return };
    // The chunks' rate is random: the port's runs at the low end, so that its carry says how many
    // contacts it ran, and the original's must have moved by that many shares of its range.
    let chunk_rate: Vec<(f32, f32)> = fx.entries.iter().map(|e| e.emt.rate).collect();
    for i in [1, 3] {
        fx.entries[i].emt.rate = (chunk_rate[i].0, chunk_rate[i].0);
    }
    for i in 0..5 {
        sync(&mut fx.entries[i], &rows[0], i);
    }
    let (mut compared, mut skipped, mut missed) = (0, Vec::new(), Vec::new());
    let mut calls = [0u32; 5];
    let mut emitted = [0u32; 5];
    for (a, b) in rows.iter().zip(&rows[1..]) {
        let row = word(b, "frame_counter");
        assert_eq!(row, word(a, "frame_counter") + 1, "the capture has every frame");
        let dt = num(b, "dt");
        let before: Vec<f32> = (0..5).map(|i| carry(&fx.entries[i])).collect();
        // Only this frame's particles, so that what each entry let go can be counted.
        fx.particles.clear();
        fx.step(&feed(a, b), dt);
        if RAGDOLL_PHASE.contains(&row) {
            for i in 0..5 {
                sync(&mut fx.entries[i], b, i);
            }
            continue;
        }
        compared += 1;
        for i in 0..5 {
            let e = &fx.entries[i];
            // (An emitter switched off keeps its "just on" bit: it only counts while on.)
            let on = word(b, &format!("e{i}_on")) as u8;
            let (theirs, ours) = (if on & 4 != 0 { on & 6 } else { 0 }, switched(e));
            // The share the port's emitter took this frame: its carry's move and what it let go.
            let made = fx.particles.iter().filter(|p| p.emitter == i).count();
            let share = carry(e) - before[i] + made as f32;
            let (was, now) = (num(a, &format!("e{i}_carry")), num(b, &format!("e{i}_carry")));
            // The original's view cull scales the rate (+0x254): it is the camera's, not the
            // simulation's, and is 0 until the emitter has been drawn once. Rows it scales are not
            // held to the port's share.
            let scale = num(a, &format!("e{i}_scale"));
            let mut why = None;
            if theirs != ours && !RESPAWN.contains(&row) {
                why = Some(format!("switch {theirs} vs port {ours}"));
            } else if scale != 1.0 {
                if now != was {
                    skipped.push((row, i));
                }
            } else if i == 0 || i == 2 {
                if (carry(e) - now).abs() > 2e-5 {
                    why = Some(format!("carry {was} -> {now}, port {} -> {}", before[i], carry(e)));
                }
            } else if i == 1 || i == 3 {
                let (lo, hi) = (chunk_rate[i].0 * dt, chunk_rate[i].1 * dt);
                let k = (share / lo).round();
                // Some whole number of particles let go makes the original's move one of k shares.
                let fits = (0..=(k * hi).ceil() as i32 + 1).any(|whole| {
                    let total = now - was + whole as f32;
                    total >= k * lo - 1e-4 && total <= k * hi + 1e-4
                });
                if !fits {
                    why = Some(format!("{k} chunk calls, but carry {was} -> {now}"));
                }
                calls[i] += k as u32;
            } else if now != was {
                why = Some(format!("foot dust moved {was} -> {now}"));
            }
            if i == 0 || i == 2 {
                calls[i] += ((share / (chunk_rate[i].1 * dt)).round()) as u32;
            }
            if now != was {
                emitted[i] += 1;
            }
            if let Some(why) = why {
                missed.push(format!("{row} e{i}: {why}"));
            }
            sync(&mut fx.entries[i], b, i);
        }
    }
    println!("{compared} rows compared; rows each entry's carry moved on {emitted:?}; the port's calls {calls:?}");
    println!("rows the original's view cull scaled, not compared: {skipped:?}");
    for m in missed.iter().take(40) {
        println!("  {m}");
    }
    assert!(compared > 200, "the capture has 230 rows: {compared}");
    assert!(missed.is_empty(), "{} misses", missed.len());
    // Both vehicle and both rider entries emitted, and foot dust (off while wrecking) never did.
    assert!(emitted[..4].iter().all(|&n| n >= 5) && emitted[4] == 0, "{emitted:?}");
}

/// A capture's rows by frame counter, out of `context/capture/<rel>`.
fn rows_at(rel: &str) -> Option<HashMap<u32, Row>> {
    let path = context().join("capture").join(rel);
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: no {}", path.display());
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
    let rows = lines.map(|l| head.iter().cloned().zip(l.split(',').map(str::to_string)).collect::<Row>());
    Some(rows.map(|r| (word(&r, "frame_counter"), r)).collect())
}

/// One of the wreck dusts' particles out of a RAM image: its emitter's entry (0 or 2), age and life
/// in milliseconds, its size when it was made, and where it sits in the system's list.
struct Made {
    entry: usize,
    age: f32,
    life: f32,
    size: f32,
    slot: usize,
}

/// The frame counter of an EE RAM image, and the two dusts' live particles in it (P185): the wreck
/// entries the rider's particle manager holds, their emitter, its particle system's list, and each
/// particle that emitter made (+0x30 age, +0x34 life, +0x40 the size it was made with, +0x4c the
/// emitter; `research/oracle.txt` P58, P153 and P185).
fn dust_in(ram: &[u8]) -> Option<(u32, Vec<Made>)> {
    let (Ok(manager), Ok(counter)) = (ps2_oracle::addr("game_manager"), ps2_oracle::addr("frame_counter")) else {
        eprintln!("skipped: game_manager / frame_counter are not in research/oracle.txt");
        return None;
    };
    let u = |a: u32| u32::from_le_bytes(ram[(a & 0x01ff_ffff) as usize..][..4].try_into().unwrap());
    let f = |a: u32| f32::from_bits(u(a));
    let rider = u(u(u(u(u(manager) + 0x34) + 0x34) + 0x8));
    let emitter = |i: u32| u(u(u(u(rider + 0x1490) + 0x3c) + 4 * i) + 0x14);
    let (vehicle, rider_dust) = (emitter(0), emitter(2));
    let system = u(vehicle + 0xa0);
    let made = (0..u(system + 0x108))
        .filter_map(|k| {
            let p = u(u(system + 0x118) + 4 * k);
            let by = u(p + 0x4c);
            let entry = if by == vehicle { 0 } else if by == rider_dust { 2 } else { return None };
            Some(Made { entry, age: f(p + 0x30), life: f(p + 0x34), size: f(p + 0x40), slot: k as usize })
        })
        .collect();
    Some((u(counter), made))
}

/// P185: Vehicle and Rider Dust's particles are as big and live as long as the emitter was fast,
/// against two RAM images of S109's animandpump wreck (`context/capture/p185/run-3850/3910.ee.bin`,
/// saved during the capture `run.csv` of P153's spec; journal P185).
///
/// - Every one of the two dusts' particles lies on the port's curve: its life is the low end, 1200 ms
///   (5 ms a foot a second reaches past it only at 240 ft/s), and its size is 0.1 ft a foot a second
///   from the 3 ft/s emission gate up to 1 ft — none of the original's random range below 0.3 ft or
///   over 1200 ms.
/// - The last particle each dust made in a frame was made at the row's own emitter point and the
///   point it came from: where that row is the frame it was made on, the port's size off that speed
///   is the original's to within a bit (the speed is the capture's two points over its dt, not the
///   game's own sum). A frame whose last contact or sphere emitted nothing
///   holds a point no particle came from; those are only held to the curve.
#[test]
fn the_wreck_dusts_grow_with_their_speed() {
    let (Some(fx), Some(rows)) = (disc_fx(), rows_at("p185/run.csv")) else { return };
    let mut matched = Vec::new();
    let mut groups = 0;
    let mut seen = 0;
    for image in ["p185/run-3850.ee.bin", "p185/run-3910.ee.bin"] {
        let path = context().join("capture").join(image);
        let Ok(ram) = std::fs::read(&path) else {
            eprintln!("skipped: no {}", path.display());
            return;
        };
        let Some((frame, made)) = dust_in(&ram) else { return };
        for p in &made {
            let emt = &fx.entries[p.entry].emt;
            assert_eq!((emt.size_ratio, emt.life_ratio, emt.min_speed), (0.1, 5.0, 3.0), "{}", fx.entries[p.entry].name);
            // The speed its size says, which the port's curve must give back.
            let speed = if p.size < emt.size.1 { p.size / emt.size_ratio } else { 20.0 };
            let (size, life) = emt.size_and_life(speed, true, 0.0, 0.0);
            assert!(speed >= emt.min_speed * (1.0 - 1e-6), "{image} slot {}: size {} is under the emission gate", p.slot, p.size);
            assert!((size - p.size).abs() <= f32::EPSILON * p.size, "{image} slot {}: size {} off the curve ({size})", p.slot, p.size);
            assert_eq!(life, p.life, "{image} slot {}: life", p.slot);
            seen += 1;
        }
        // The last of each frame's particles, by entry and age (one frame's are all of an age).
        let mut last: HashMap<(usize, u32), &Made> = HashMap::new();
        for p in &made {
            let at = last.entry((p.entry, p.age.to_bits())).or_insert(p);
            if p.slot > at.slot {
                *at = p;
            }
        }
        for p in last.values() {
            groups += 1;
            let emt = &fx.entries[p.entry].emt;
            // The tick it was made on, give or take one: its age is ticks of the row's dt.
            let born = frame as i64 - (p.age / (1000.0 / 60.0)).round() as i64;
            let hit = (born - 1..=born + 1).filter_map(|b| rows.get(&(b as u32))).find_map(|r| {
                let e = format!("e{}", p.entry);
                let speed = vector(r, &format!("{e}_at")).sub(vector(r, &format!("{e}_from"))).len() / num(r, "dt");
                let (size, life) = emt.size_and_life(speed, emt.min_movement_speed < speed, 0.0, 0.0);
                let ulps = (size.to_bits() as i64 - p.size.to_bits() as i64).abs();
                (ulps <= 1 && life == p.life).then_some((speed, ulps))
            });
            if let Some((speed, ulps)) = hit {
                matched.push((p.size, speed, ulps));
            }
        }
    }
    println!("{seen} particles on the curve; {} of {groups} frames' last ones at the row's speed: {matched:?}", matched.len());
    assert!(seen > 150, "{seen} particles");
    // A capped 1 ft is any speed past 10 ft/s, so only the sizes under it count: a random size in
    // 0.1..1 ft lands within a bit of the row's speed by chance about never, so twenty frames that
    // do, over a spread of sizes, are the speed's.
    let sizes: Vec<f32> = matched.iter().map(|m| m.0).filter(|&s| s < 1.0).collect();
    assert!(sizes.len() >= 15, "{} frames under the cap at the row's speed: {sizes:?}", sizes.len());
    assert!(sizes.iter().cloned().fold(1.0, f32::min) < 0.35 && sizes.iter().cloned().fold(0.0, f32::max) > 0.7, "{sizes:?}");
}
