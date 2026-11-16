// --- MD135: MD107's five drives, the port's frame beside the interpreter's ----------------------------

use super::whole::{NOISE_G1, NOISE_PERM, NOISE_UNBUILT, Oasis, terrain_file};
use super::*;
use game_sim::handling::record;
use game_sim::{atv_contact, atv_ground};
use game_sim::noise::SmoothNoise;
use std::collections::BTreeMap;

#[path = "../mxvatv_car_replay/drive.rs"]
#[allow(dead_code)]
mod drive;
use drive::Drive;

/// The calls the port's frame still hands out on a drive (what [`Oasis`] lists): the sort's place
/// and search (the search answered by the original's, [`sort_answer`]), a struck object's listeners,
/// the database's moves and the gather, the node's rebound, the respawn, and the sound, pad and look
/// calls (the shift's sound and the preload's cue among them).
pub(crate) const HANDED_OUT: [&str; 17] = [
    "SortPlace",
    "SortSearch",
    "Struck",
    "SpatialMove",
    "PartMove",
    "Gather",
    "NodeRebound",
    "Respawn",
    "Cue",
    "Rumble",
    "WreckLook",
    "LandingSound",
    "ShiftSound",
    "PreloadCue",
    "LeaveWorld",
    "RidingLetGo",
    "ReleaseRecord",
];

/// The sort probe's search as the original's left it in RAM (the struck flag, and each hit's ray,
/// point, normal and record: MD127's [`hook_sort`] the other way round), for the port's
/// [`Call::SortSearch`].
pub(crate) fn sort_answer(ee: &Ee, p: &Ptrs) -> SortAnswer {
    let mut a = SortAnswer { found: ee.u8_at(p.sort_probe + sort::FLAGS) & 0x20 != 0, ..SortAnswer::default() };
    if !a.found {
        return a;
    }
    let list = ee.u32_at(p.sort_probe + sort::LIST);
    a.count = ee.u32_at(list + sort::COUNT) as usize;
    assert!(a.count <= SORT_HITS, "the sort struck {} times", a.count);
    let (froms, normals, points, records) =
        (ee.u32_at(list + sort::LIST_FROMS), ee.u32_at(list + sort::LIST_NORMALS), ee.u32_at(list + sort::LIST_POINTS), ee.u32_at(list + sort::LIST_RECORDS));
    for (i, h) in a.hits.iter_mut().enumerate().take(a.count) {
        let record = ee.u32_at(records + 4 * i as u32);
        let mover = ee.u32_at(record + sort::RECORD_MOVER);
        *h = atv_car::SortHit {
            from: get_v(ee, froms + 0x20 * i as u32),
            point: get_v(ee, points + 0x10 * i as u32),
            normal: get_v(ee, normals + 0x10 * i as u32),
            object: ee.u32_at(record + sort::RECORD_OBJECT),
            mover,
            mover_kind: if mover != 0 { ee.u32_at(mover + sort::MOVER_KIND) } else { 0 },
            record,
        };
    }
    a
}

/// [`Oasis`] with the sort's search answered as the original's frame answered it.
struct Beside<'t> {
    oasis: Oasis<'t>,
    sort: SortAnswer,
}

impl Steps for Beside<'_> {
    fn call(&mut self, c: Call, car: &mut Car, body: &mut Body) {
        if c == Call::SortSearch {
            car.sort = self.sort;
        }
        self.oasis.call(c, car, body);
    }
}

/// What a drive came to: the frames the port stepped beside the interpreter, the words compared
/// each, and how often each call was handed out.
#[derive(Default)]
struct Seen {
    frames: usize,
    words: usize,
    handed: BTreeMap<String, usize>,
    sort_struck: usize,
    /// Frames whose record held contacts, and the other worlds stepped.
    touching: usize,
    other_worlds: usize,
    wrecked: usize,
    /// The first wrecked frame and its state, and the car's up row's `y` at the last frame.
    first_wreck: Option<(usize, u32)>,
    last_up: f32,
    /// The frame whose respawn ended a run on its side.
    respawned: Option<usize>,
}

/// Runs the original's world step from inside its hook, on the world and tick it was handed, and
/// leaves the machine as a return from it would.
fn world_step_inside(ee: &mut Ee) -> (u32, F) {
    let (world, dt) = (ee.r(4) as u32, F(ee.f(12)));
    let (sp, ra, stack) = (ee.r(29), ee.r(31), ee.stack);
    ee.stack = (sp as u32 - 0x100) & !0xf;
    ee.call(WORLD_STEP, &[u64::from(world)], &[dt.0], drive::BUDGET);
    (ee.stack, ee.pc) = (stack, WORLD_STEP);
    ee.set(29, sp);
    ee.set(31, ra);
    (world, dt)
}

/// Replays drive `d` in the interpreter as MD107's test does (the image, then SLUS-21104's sim
/// frame, the input step's words off the tape through the port's input step), and at each frame's
/// world step reads the car, its body, engine and tires, the pieces' keys and state and the
/// vehicle's words off the machine, runs the original's step, then [`atv_car::frame`] on what it
/// read ([`Pieces`] and [`atv_car::Own`] answering what they port, [`Oasis`] the ground queries on
/// Desert Oasis and the smooth noise, the sort's search the original's): every word the port keeps
/// must equal the original's after it, every frame.
fn beside(d: &Drive, through: Through) -> Option<Seen> {
    beside_posed(d, through, None)
}

/// [`beside`], with the car first put on its side when `side` is given ([`put_on_side`]): as many
/// frames as [`SIDE_FRAMES`] then, not the capture's rows.
fn beside_posed(d: &Drive, through: Through, side: Option<f32>) -> Option<Seen> {
    let mut pre = drive::image(d.name)?;
    let ticks = drive::tape()?;
    let rows = drive::rows(d.name)?;
    let trn = terrain_file()?;
    assert_eq!(pre.u32_at(d.car + 4), d.vtable, "{}: no car at {:#x}", d.name, d.car);
    assert_eq!(pre.u32_at(NOISE_UNBUILT), 0, "{}: the noise's tables are built", d.name);
    THE_CAR.with(|c| c.set(d.car));
    let trn: &'static [u8] = Box::leak(trn.into_boxed_slice());
    let map: &'static game_sim::track::Terrain<'static> = Box::leak(Box::new(game_sim::track::Terrain::parse(trn).unwrap()));
    let terrain: &'static atv_ground::Terrain<'static> = Box::leak(Box::new(atv_ground::Terrain::of(map)));
    let noise = SmoothNoise::from_tables(&std::array::from_fn(|k| pre.u32_at(NOISE_PERM + 4 * k as u32)), &std::array::from_fn(|k| F(pre.f32_at(NOISE_G1 + 4 * k as u32))));
    let table: &'static AcosTable = Box::leak(Box::new(AcosTable::build()));
    if let Some(roll) = side {
        put_on_side(&mut pre, d.car, roll);
    }
    let s = drive::sim(&pre);
    let mut ee = drive::hooked(pre);
    let seen: Shared<Seen> = Rc::default();
    let frame_no: Shared<usize> = Rc::default();
    let (keep, at_frame, name) = (seen.clone(), frame_no.clone(), d.name);
    let ride: Shared<Option<atv_car::Ride>> = Rc::default();
    ee.hooks.insert(WORLD_STEP, Box::new(move |ee: &mut Ee| {
        let k = *at_frame.borrow();
        let p = ptrs(ee);
        let o = own_ptrs(ee, &p);
        let w = ee.r(4) as u32;
        let listeners: Vec<u32> = (0..ee.u32_at(w + 0x98)).map(|i| ee.u32_at(ee.u32_at(w + 0xa0) + 4 * i)).collect();
        if !listeners.contains(&(car_at() + 0x30)) {
            // Another world (the golf cart's image steps a second, with no listeners).
            world_step_inside(ee);
            keep.borrow_mut().other_worlds += 1;
            return;
        }
        let touching = ee.u32_at(p.record + 0x34);
        let mut car = read_car(ee, &p);
        let mut body = game_sim::handling::seed::body(&|a| ee.u32_at(a), p.body);
        let mut v = read_vehicle(ee, &p, &o);
        // The record's contacts as the pass filed them (the reader holds only an empty list): none
        // held when one is with another record or a mover, and the gravity's call goes on.
        let contacts = ee.u32_at(p.record + 0xf0);
        if contacts != 0 {
            v.held = record::held(&|a| ee.u32_at(a), p.record);
        }
        let m = read_mx(ee, &p);
        let (_, dt) = world_step_inside(ee);
        let world = World { iterations: 1, rate: F(115.0) };
        let oasis = Oasis { terrain, noise: noise.clone(), knot: None, asked: Vec::new(), rest: Vec::new() };
        let sort = sort_answer(ee, &p);
        let mut pieces = Pieces { keys: m.keys(), state: m.state, rest: Beside { oasis, sort } };
        match through {
            Through::Frame => atv_car::frame(&world, &mut car, &mut body, dt, table, &mut atv_car::Own { v: &mut v, rest: &mut pieces }),
            Through::Ride => {
                assert_eq!(world, atv_car::WORLD, "the Ride's world is the frame's");
                let mut r = ride.borrow_mut();
                let r = r.get_or_insert_with(|| ride_of(table));
                (r.keys, r.state, r.car, r.body, r.vehicle) = (std::mem::take(&mut pieces.keys), pieces.state, car, body, v);
                let rest = std::mem::replace(&mut pieces.rest, Beside { oasis: Oasis { terrain, noise: noise.clone(), knot: None, asked: Vec::new(), rest: Vec::new() }, sort });
                pieces.rest = r.frame_with(dt, rest);
                (pieces.keys, pieces.state) = (std::mem::take(&mut r.keys), r.state);
                (car, body) = (r.car.clone(), r.body.clone());
                v = r.vehicle.clone();
            }
        }
        // On its side, the original's respawn (which the port hands out) ends the comparison.
        if side.is_some() && pieces.rest.oasis.rest.iter().any(|c| matches!(c, Call::Respawn)) {
            keep.borrow_mut().respawned = Some(k);
            return;
        }
        let pm = Mx { keys: Keys { tires: pieces.keys.tires.clone(), ..m.keys.clone() }, state: pieces.state, ..m };
        // What the resolution writes of the record and its list, when it holds contacts (the body is
        // the frame's, compared whole beside).
        let resolved: Vec<(u32, u32)> = v.held.as_ref().filter(|_| contacts != 0).map_or(Vec::new(), |h| record::held_words(&atv_contact::Held { body: None, ..h.clone() }, p.record, &|a| ee.u32_at(a)));
        let parts = [car_words(&car, &p), game_sim::handling::seed::body_words(&body, p.body), mx_words(&pm, &p), vehicle_words(ee, &v, &p, &o), resolved];
        let counts = parts.clone().map(|ws| ws.iter().filter(|&&(a, x)| ee.u32_at(a) != x).count());
        let after: Vec<(u32, u32)> = parts.concat();
        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .take(40)
            .map(|&(a, x)| format!("  {a:#x} (car +{:#x}): port {x:#010x} ({}), original {:#010x} ({})", a.wrapping_sub(car_at()), f32::from_bits(x), ee.u32_at(a), ee.f32_at(a)))
            .collect();
        assert!(
            bad.is_empty(),
            "{name} frame {k}: words of the port's frame differ from the original's (car, body, pieces, vehicle, record: {counts:?}; touching {touching}, {contacts} contacts, held {}, sort struck {}, state {}, handed out {:?}):\n{}",
            v.held.is_some(),
            sort.found,
            car.state,
            pieces.rest.oasis.rest,
            bad.join("\n")
        );
        let mut s = keep.borrow_mut();
        for c in &pieces.rest.oasis.rest {
            let n = format!("{c:?}").split([' ', '(', '{']).next().unwrap().to_string();
            assert!(HANDED_OUT.contains(&n.as_str()), "{name} frame {k}: the port's frame handed out {c:?}");
            *s.handed.entry(n).or_default() += 1;
        }
        s.frames += 1;
        s.words += after.len();
        s.sort_struck += usize::from(sort.found);
        s.touching += usize::from(contacts != 0);
        s.wrecked += usize::from(car.state != atv_car::RIDING);
        if s.first_wreck.is_none() && car.state != atv_car::RIDING {
            s.first_wreck = Some((k, car.state));
        }
        s.last_up = car.up.y();
    }));
    // As many frames as the capture has rows (MD107's test steps one past the row the last vsync
    // names); every frame's input two tape ticks back, as there.
    let mut j = 0;
    for _ in 0..side.map_or(rows.len(), |_| SIDE_FRAMES) {
        *frame_no.borrow_mut() = j + 1;
        drive::step(&mut ee, s, d, &ticks, &mut j, 2);
        if seen.borrow().respawned.is_some() {
            break;
        }
    }
    let seen = std::mem::take(&mut *seen.borrow_mut());
    let want = seen.respawned.map_or(side.map_or(rows.len(), |_| SIDE_FRAMES), |k| k - 1);
    assert_eq!(seen.frames, want, "{}: the world step ran once a frame", d.name);
    eprintln!(
        "{} {}: {} frames, {} words in all equal to the original's; handed out {:?}; the sort struck on {} frames, the record held contacts on {}; wrecked on {}; {} other world steps",
        match through {
            Through::Frame => "MD135",
            Through::Ride => "MD149a (through Ride)",
        },
        d.name, seen.frames, seen.words, seen.handed, seen.sort_struck, seen.touching, seen.wrecked, seen.other_worlds
    );
    Some(seen)
}

/// Which way the port's frame is stepped beside the interpreter's: [`atv_car::frame`] with
/// [`atv_car::Own`] over [`Pieces`] built here (MD135), or through the shell's own type,
/// [`atv_car::Ride::frame_with`]: its chain of [`atv_car::Own`] over its put-back over [`Pieces`],
/// the keys and state taken out of the Ride and put back (MD149a).
#[derive(Clone, Copy)]
enum Through {
    Frame,
    Ride,
}

/// A Ride to carry each frame's words read off the machine: everything but its acos table is laid
/// over before each frame; nothing it holds besides those is read by [`atv_car::Ride::frame_with`]
/// (its put-back spot only by the respawn and the put-back, which these drives never hand out).
pub(crate) fn ride_of(table: &AcosTable) -> atv_car::Ride {
    atv_car::Ride {
        keys: Keys::default(),
        state: atv_car::State::default(),
        input_keys: atv_car::InputKeys::default(),
        input: atv_car::Input::default(),
        car: Car::default(),
        body: Body::default(),
        vehicle: atv_car::Vehicle::default(),
        update: atv_car::Update::default(),
        updated: atv_car::Updated::default(),
        table: table.clone(),
        noise: SmoothNoise::build(&mut game_sim::rng::Rng::default()),
        back: None,
        cache: None,
        spot: (Vf::ZERO, Vf::ZERO),
        recorded: None,
        rest: Vec::new(),
    }
}

/// **The buggy's drive is the port's**: every frame of MD107's replay, the port's car frame beside
/// the interpreter's world step, every word equal.
#[test]
fn the_buggys_drive_is_the_ports() {
    beside(&drive::BUGGY, Through::Frame);
}

/// **The baja's.**
#[test]
fn the_bajas_drive_is_the_ports() {
    beside(&drive::BAJA, Through::Frame);
}

/// **The sandrail's.**
#[test]
fn the_sandrails_drive_is_the_ports() {
    beside(&drive::SANDRAIL, Through::Frame);
}

/// **The golf cart's.**
#[test]
fn the_golf_carts_drive_is_the_ports() {
    beside(&drive::GOLF, Through::Frame);
}

/// **The monster truck's.**
#[test]
fn the_trucks_drive_is_the_ports() {
    beside(&drive::TRUCK, Through::Frame);
}

/// **MD149a: the buggy's drive through [`atv_car::Ride`]**: every frame of MD135's replay stepped by
/// the Ride's own chain ([`atv_car::Ride::frame_with`]) with the image's world answers in place of
/// [`atv_car::OnWorld`], every word equal to the interpreter's; no frame wrecks, so the Ride's
/// put-back never answers.
#[test]
fn the_buggys_drive_is_the_rides() {
    ridden(&drive::BUGGY);
}

/// **The baja's.**
#[test]
fn the_bajas_drive_is_the_rides() {
    ridden(&drive::BAJA);
}

/// **The sandrail's.**
#[test]
fn the_sandrails_drive_is_the_rides() {
    ridden(&drive::SANDRAIL);
}

/// **The golf cart's.**
#[test]
fn the_golf_carts_drive_is_the_rides() {
    ridden(&drive::GOLF);
}

/// **The monster truck's.**
#[test]
fn the_trucks_drive_is_the_rides() {
    ridden(&drive::TRUCK);
}

fn ridden(d: &Drive) {
    let Some(seen) = beside(d, Through::Ride) else { return };
    assert_eq!(seen.wrecked, 0, "{}: a wrecked frame would reach the Ride's put-back, not the original's", d.name);
    assert!(!seen.handed.contains_key("Respawn"), "{}: the Ride answers the respawn itself", d.name);
}

// --- MD169: a car put on its side ------------------------------------------------------------------

/// How many frames a car put on its side is stepped.
const SIDE_FRAMES: usize = 150;

/// How far (ft) a car put on its side is lifted.
const SIDE_LIFT: f32 = 3.0;

/// The car in image `ee` put on its side where it stands: its body turned `roll` radians about its
/// own forward axis, lifted [`SIDE_LIFT`] ft, still (velocity, rates and the last substep's pose
/// with it).
fn put_on_side(ee: &mut Ee, car: u32, roll: f32) {
    let b = ee.u32_at(car + at::BODY);
    let q = get_v(ee, b + 0x190);
    let (s, c) = (roll / 2.0).sin_cos();
    let (x, y, z, w) = (q.x(), q.y(), q.z(), q.w());
    // q · (s about z, c), Hamilton's product.
    let r = Vf::new(x * c + y * s, y * c - x * s, z * c + w * s, w * c - z * s);
    let p = get_v(ee, b + 0x70);
    let p = Vf::new(p.x(), p.y() + SIDE_LIFT, p.z(), p.w());
    for (o, v) in [(0x170, r), (0x190, r), (0x60, p), (0x70, p), (0x80, Vf::ZERO), (0x1d0, Vf::ZERO)] {
        put_v(ee, b + o, v);
    }
}

/// What the original's car does put on its side at its image's spot ([`put_on_side`]): the frame its
/// tipped wreck starts, `None` when it stands back up on its wheels.
const ON_ITS_SIDE: [(&Drive, Option<usize>); 5] =
    [(&drive::BUGGY, None), (&drive::BAJA, Some(2)), (&drive::SANDRAIL, Some(2)), (&drive::GOLF, Some(11)), (&drive::TRUCK, Some(6))];

/// **MD169: each car put on its side is the port's**: MD135's frame beside the interpreter's, from
/// the image with the car turned a quarter about its forward axis and dropped [`SIDE_LIFT`] ft, every
/// word equal for [`SIDE_FRAMES`] frames or up to the original's respawn (which the port hands out).
/// The tires' probes ask the ground straight down on their side as upright (the heightfield query
/// keeps the asked `x`, `z`), and the original's buggy stands back up on its wheels while the other
/// four wreck tipped ([`ON_ITS_SIDE`]).
#[test]
fn each_car_put_on_its_side_is_the_ports() {
    for (d, wreck) in ON_ITS_SIDE {
        let Some(seen) = beside_posed(d, Through::Frame, Some(std::f32::consts::FRAC_PI_2)) else { return };
        eprintln!("MD169 {}: {} frames on its side equal to the original's; wrecked {:?}, respawned {:?}, up {}", d.name, seen.frames, seen.first_wreck, seen.respawned, seen.last_up);
        assert_eq!(seen.first_wreck, wreck.map(|k| (k, atv_car::WRECK_TIPPED)), "{}: the original's wreck on its side", d.name);
        if wreck.is_none() {
            assert_eq!(seen.frames, SIDE_FRAMES, "{}: respawned", d.name);
            assert!(seen.last_up > 0.9, "{}: not back on its wheels (up {})", d.name, seen.last_up);
        } else {
            assert!(seen.respawned.is_some(), "{}: never respawned after its tipped wreck", d.name);
        }
    }
}
