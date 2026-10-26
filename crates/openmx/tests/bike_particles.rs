//! P74: the bike's particles — the roost, the dust off the rear tire, the exhaust, the wreck's dust and
//! chunks — out of the track's `mxparticles.dat`, driven by particles.rs's manager the way the
//! original's drives them.
//!
//! The disc's list for free09 is read and checked entry by entry, and then the manager is run over
//! the original's own: `context/capture/p74/run.csv` is S109's animandpump played back in PCSX2 with
//! the human's bike's particle manager read every frame (journal `bike-particles`, P74) — each entry's
//! flag byte, its emitter's switch byte and carry and where it is, the sprays' fade clock and side
//! velocity, and the words the bike hands the manager. Each row's state is the manager's after it
//! read the row before's words; the port is stepped on those words and must agree with every entry
//! on every row it can be sure of, across the launch (rows 2001 to 2007), the landing on the bare
//! surface (2738 to 2744), the wreck (3719) and getting back on (3913, 3914).
//!
//! The capture and the disc are not in the repo: without them the tests say so and pass.

#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;

use std::collections::HashMap;
use std::path::PathBuf;

use game_render::Disc;
use game_render::biplane;
use game_render::ini;
use game_render::mat::{M4, V3, v3};
use particles::{Atlas, Disable, Entry, Kind, PlaneFrame, PlaneFx, SprayKeys, TireFrame, Zone};

const TRACK: &str = "free09";

/// The rows the recorder read across two frames' writes (journal P74): a flag goes to 0 for one row,
/// or the snapshot and the flags change in the same row. Not compared, and the port is put back on
/// the capture after each.
const TORN: [u32; 3] = [2996, 3156, 3924];

fn context() -> PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context"),
    }
}

/// free09's bike particles off the disc: its list, the `.emt`s and zones beside it, and its sheet.
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

fn capture() -> Option<Vec<HashMap<String, String>>> {
    let path = context().join("capture/p74/run.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: no {}", path.display());
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(str::to_string)).collect()).collect())
}

/// The disc's list for free09, as the port reads it: fifteen entries, the two tire tracks not ported.
#[test]
fn the_discs_bike_particles() {
    let Some(fx) = disc_fx() else { return };
    let got: Vec<(&str, &Kind, Option<&str>, Option<usize>, u32)> =
        fx.entries.iter().map(|e| (e.name.as_str(), &e.kind, e.node.as_deref(), e.tire, e.disable)).collect();
    const NC: u32 = Disable::NO_CONTACT;
    const OC: u32 = Disable::OBJECT_CONTACT;
    const W: u32 = Disable::WRECKING;
    const NW: u32 = Disable::NOT_WRECKING;
    let spray = |side_threshold: f32, side_scale: f32, max_y: f32| Kind::Spray(SprayKeys { side_scale, side_threshold, min_y: 0.0, max_y, ramp: 0.0 });
    let want: Vec<(&str, Kind, Option<&str>, Option<usize>, u32)> = vec![
        ("Vehicle Dust", Kind::CollisionModel, None, None, NC | OC | NW),
        ("Vehicle Chunks", Kind::CollisionModel, None, None, NC | OC | NW),
        ("Rider Dust", Kind::Constraint { mask: 0x1fc }, None, None, NC | OC | NW),
        ("Rider Chunks", Kind::Constraint { mask: 0x1fc }, None, None, NC | OC | NW),
        ("Foot Dust", Kind::Constraint { mask: 0x3 }, None, None, NC | OC | W),
        ("Right Exhuast", Kind::Exhaust { max_speed: 15.0 }, Some("250_Exhaust_Particle01"), Some(1), W),
        ("Left Exhuast", Kind::Exhaust { max_speed: 15.0 }, Some("250_Exhaust_Particle02"), Some(1), W),
        ("Rear Tire Dust Spray", Kind::Generic, Some("RearTire_Particle01"), Some(1), OC | W | NC),
        ("Front Tire Dirt Spray", spray(8.0, 1.5, 2.0), Some("FrontTire_Particle01"), Some(0), NC | OC | W),
        ("Rear Tire Dirt Spray", spray(5.0, 1.0, 5.0), Some("RearTire_Particle01"), Some(1), NC | OC | W),
        ("Front Tire Chunks", spray(4.0, 1.75, 10.0), Some("FrontTire_Particle01"), Some(0), NC | OC | W),
        ("Rear Tire Chunks", spray(5.0, 4.0, 20.0), Some("RearTire_Particle01"), Some(1), NC | OC | W),
        ("Rear Tire Collision Chunks", spray(3.0, 4.0, 10.0), Some("RearTire_Particle01"), Some(1), NC | OC | W),
    ];
    let want: Vec<(&str, &Kind, Option<&str>, Option<usize>, u32)> = want.iter().map(|(a, b, c, d, e)| (*a, b, *c, *d, *e)).collect();
    assert_eq!(got, want);
    let skipped: Vec<&str> = fx.skipped.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(skipped, ["Front Tire Track", "Rear Tire Track"]);
    // The chunks that say `HasCollision` meet the ground; the rest do not.
    let collide: Vec<&str> = fx.entries.iter().filter(|e| e.emt.collides).map(|e| e.name.as_str()).collect();
    assert_eq!(collide, ["Vehicle Chunks", "Rider Chunks", "Rear Tire Collision Chunks"]);
    // The exhaust blows back along the pipe, 30 to 50 a second (P74's capture: rate and velocity).
    let exhaust = &fx.entries[5];
    assert_eq!((exhaust.file.velocity, exhaust.file.rate), (v3(0.0, 1.0, -4.5), (30.0, 50.0)));
    // The roost: 400 to 600 a second, gated at nothing; the rear dust spray at 5 ft/s.
    assert_eq!(fx.entries[9].file.rate, (400.0, 600.0));
    assert_eq!(fx.entries[7].file.min_speed, 5.0);
    // The zones: the first surface draws no dust, dirt or chunks; the dirt spray's thirteenth is grass.
    for e in fx.entries.iter().filter(|e| e.tire.is_some() && !matches!(e.kind, Kind::Exhaust { .. })) {
        assert_eq!(e.zones[0], Zone::Off, "{}", e.name);
    }
    let grass = fx.atlas.group("GrassSpray").unwrap();
    assert_eq!(fx.entries[8].zones[12], Zone::Group(grass.0, grass.1));
    assert_eq!(fx.entries[8].zones[13], Zone::Group(fx.entries[8].frames.0, fx.entries[8].frames.1));
}

fn num(r: &HashMap<String, String>, k: &str) -> f32 {
    r[k].parse().unwrap_or_else(|_| panic!("{k} = {:?}", r[k]))
}

fn word(r: &HashMap<String, String>, k: &str) -> u32 {
    r[k].parse().unwrap_or_else(|_| panic!("{k} = {:?}", r[k]))
}

/// The words the bike handed its manager on row `r` (the snapshot the manager reads), as the port's
/// frame: the throttle, the clutch, the speed, the wreck, and the two tires.
fn feed(r: &HashMap<String, String>) -> PlaneFrame {
    let tire = |g: &str, o: &str, spin: &str, side: &str, zone: &str| TireFrame {
        ground: word(r, g) != 0,
        object: word(r, o) != 0,
        spinning: word(r, spin) != 0,
        side: num(r, side),
        zone: word(r, zone) as u8,
        ..TireFrame::default()
    };
    PlaneFrame {
        throttle: num(r, "s888"),
        clutch: num(r, "s88c"),
        airspeed: num(r, "s860"),
        wrecked: word(r, "s868") != 0,
        tires: vec![tire("s914", "s918", "s91c", "s870", "s910"), tire("s928", "s92c", "s930", "s880", "s911")],
        ..PlaneFrame::default()
    }
}

/// Entry `i`'s emitter where row `r` has it, as its node: the capture reads the point and not the
/// turn, which only turns what it emits.
fn at(r: &HashMap<String, String>, i: usize) -> V3 {
    v3(num(r, &format!("e{i}_at.x")), num(r, &format!("e{i}_at.y")), num(r, &format!("e{i}_at.z")))
}

fn node(at: V3) -> M4 {
    let mut m = M4::IDENTITY;
    m.0[3] = [at.x, at.y, at.z, 1.0];
    m
}

/// How many game frames row `b` is past row `a`: 0 for a repeat, else by how far the bike moved
/// against its speed, and whether that is sure — it is not under 5 ft/s or while wrecked.
fn frames(a: &HashMap<String, String>, b: &HashMap<String, String>) -> (u32, bool) {
    if ["s860", "s888", "e5_at.x", "e9_at.x", "e9_carry"].iter().all(|k| a[*k] == b[*k]) {
        return (0, true);
    }
    let v = num(a, "s860");
    if v <= 5.0 || word(a, "s868") != 0 {
        return (1, false);
    }
    let p = |r: &HashMap<String, String>| v3(num(r, "bike_xform.m30"), num(r, "bike_xform.m31"), num(r, "bike_xform.m32"));
    let ratio = p(b).sub(p(a)).len() / (v / 60.0);
    match ratio {
        r if (0.8..=1.5).contains(&r) => (1, true),
        r if (1.8..=2.5).contains(&r) => (2, false),
        r => ((r.round() as u32).max(1), false),
    }
}

/// What the capture says of entry `i` on row `r`, and the port's of it: the flag byte, the emitter
/// on (switch byte bit 2), and for a spray its clock to 2 ms and its side velocity to a thousandth.
#[derive(Debug, PartialEq)]
struct Seen {
    flag: u8,
    on: Option<bool>,
    clock: Option<i32>,
    vx: Option<i32>,
}

fn seen(r: &HashMap<String, String>, i: usize) -> Seen {
    let spray = (8..=12).contains(&i);
    Seen {
        flag: word(r, &format!("e{i}_flag")) as u8,
        on: (5..=12).contains(&i).then(|| word(r, &format!("e{i}_on")) & 4 != 0),
        clock: spray.then(|| (num(r, &format!("e{i}_clock")) * 500.0).round() as i32),
        vx: spray.then(|| (num(r, &format!("e{i}_vx")) * 1000.0).round() as i32),
    }
}

fn port(e: &Entry, i: usize) -> Seen {
    let spray = (8..=12).contains(&i);
    Seen {
        flag: e.flag,
        on: (5..=12).contains(&i).then_some(e.lit),
        clock: spray.then(|| (e.clock * 500.0).round() as i32),
        vx: spray.then(|| (e.emt.velocity.x * 1000.0).round() as i32),
    }
}

/// Puts the port's entry `i` where the capture's row `r` has it.
fn sync(e: &mut Entry, r: &HashMap<String, String>, i: usize) {
    e.flag = word(r, &format!("e{i}_flag")) as u8;
    if (5..=12).contains(&i) {
        e.lit = word(r, &format!("e{i}_on")) & 4 != 0;
        e.sources.resize_with(1, Default::default);
        e.sources[0].last = e.lit.then(|| at(r, i));
    }
    if (8..=12).contains(&i) {
        e.clock = num(r, &format!("e{i}_clock"));
        e.emt.velocity.x = num(r, &format!("e{i}_vx"));
    }
}

/// The original's manager, row by row: which entries are live, zone-off, switched on, how long a
/// spray has left and how fast it throws sideways — and that a switched-on emitter emits from the
/// frame after it went on, on the ticks the original's does.
#[test]
fn the_bikes_emitters_follow_the_original() {
    let (Some(mut fx), Some(rows)) = (disc_fx(), capture()) else { return };
    // The tire tracks are not ported, so the port's entries are the capture's first thirteen.
    assert_eq!(fx.entries.len(), 13);
    for (i, e) in fx.entries.iter_mut().enumerate() {
        sync(e, &rows[0], i);
    }
    let (mut compared, mut unsure, mut missed, mut emitted) = (0, 0, Vec::new(), [0u32; 13]);
    let mut switched: Vec<(u32, usize, bool)> = Vec::new();
    let mut flagged: Vec<(u32, usize, u8)> = Vec::new();
    for (a, b) in rows.iter().zip(&rows[1..]) {
        let (k, sure) = frames(a, b);
        if k == 0 {
            continue;
        }
        let row = word(b, "frame_counter");
        let carry_before: Vec<f32> = fx.entries.iter().map(|e| e.sources.first().map_or(0.0, |s| s.carry)).collect();
        let lit_before: Vec<bool> = fx.entries.iter().map(|e| e.lit).collect();
        let flag_before: Vec<u8> = fx.entries.iter().map(|e| e.flag).collect();
        let mut frame = feed(a);
        for step in 1..=k {
            // A row two frames on is stepped twice, the emitters half way the first time.
            let t = step as f32 / k as f32;
            frame.nodes = fx
                .entries
                .iter()
                .enumerate()
                .filter_map(|(i, e)| Some((e.node.clone()?, node(at(a, i).add(at(b, i).sub(at(a, i)).scale(t))))))
                .collect();
            fx.step(&frame, num(b, "dt"));
        }
        if TORN.contains(&row) {
            for (i, e) in fx.entries.iter_mut().enumerate() {
                sync(e, b, i);
            }
            continue;
        }
        compared += 1;
        for i in 0..fx.entries.len() {
            let (want, got) = (seen(b, i), port(&fx.entries[i], i));
            // An emitter emits on a frame it was on before and is on after: the carry moves, if only
            // in its last bit (a gated or switched-off emitter leaves it exactly as it was). The
            // original's carry moves on the same rows (rows a single frame on only: two frames could
            // switch it on and emit).
            let moved = |before: f32, after: f32| after != before;
            let theirs = moved(num(a, &format!("e{i}_carry")), num(b, &format!("e{i}_carry")));
            let ours = moved(carry_before[i], fx.entries[i].sources.first().map_or(0.0, |s| s.carry));
            let carry_differs = (5..=12).contains(&i) && k == 1 && theirs != ours;
            if ours {
                emitted[i] += 1;
            }
            if want == got && !carry_differs {
                if lit_before[i] != got.on.unwrap_or(lit_before[i]) {
                    switched.push((row, i, got.on.unwrap()));
                }
                if flag_before[i] != got.flag {
                    flagged.push((row, i, got.flag));
                }
                continue;
            }
            if sure {
                missed.push(format!(
                    "{row} e{i}: port {got:?} carry {} -> {}, original {want:?} carry {} -> {}",
                    carry_before[i],
                    fx.entries[i].sources.first().map_or(0.0, |s| s.carry),
                    num(a, &format!("e{i}_carry")),
                    num(b, &format!("e{i}_carry"))
                ));
            } else {
                unsure += 1;
            }
            sync(&mut fx.entries[i], b, i);
        }
    }
    println!("{compared} rows compared, {} misses on sure rows, {unsure} on unsure rows", missed.len());
    println!("frames each entry emitted on: {emitted:?}");
    for m in missed.iter().take(40) {
        println!("  {m}");
    }
    assert!(compared > 2000, "the capture has 2400 rows: {compared}");
    assert!(missed.is_empty(), "{} misses on rows a single frame on", missed.len());
    // The launch: the exhaust on at 2001, the rear sprays at 2006 (the throttle and the spin a frame
    // before), the front's not until the front wheel throws.
    let on = |row: u32, i: usize| switched.iter().any(|s| *s == (row, i, true));
    assert!(on(2001, 5) && on(2001, 6) && on(2006, 9) && on(2006, 11) && on(2006, 12), "{:?}", &switched[..switched.len().min(20)]);
    // The landing: over the road the front's zone turns to 0 at 2738 (none for every descriptor
    // these entries name) and its entries go zone-off (flag 2), the rear's at 2744; the wreck at 3719
    // turns the riding entries off and the vehicle and rider ones live. The port's flags change on
    // those rows as the original's do.
    let flag = |row: u32, i: usize, f: u8| flagged.contains(&(row, i, f));
    assert!([8, 10].iter().all(|&i| flag(2738, i, 2)), "{flagged:?}");
    assert!([7, 9, 11, 12].iter().all(|&i| flag(2744, i, 2)), "{flagged:?}");
    // (The tire entries were off already, the bike in the air.) The respawn at 3914 turns the
    // riding ones on again.
    assert!((0..4).all(|i| flag(3719, i, 1)) && (4..7).all(|i| flag(3719, i, 0)), "{flagged:?}");
    assert!((4..7).all(|i| flag(3914, i, 1)), "{flagged:?}");
    // The exhausts and every tire entry but the front's dirt spray (8: it never throws on this ride)
    // emitted somewhere along it.
    for i in [5, 6, 7, 9, 10, 11, 12] {
        assert!(emitted[i] > 0, "entry {i} never emitted: {emitted:?}");
    }
}
