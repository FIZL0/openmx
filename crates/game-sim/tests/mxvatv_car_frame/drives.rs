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
const HANDED_OUT: [&str; 17] = [
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
fn sort_answer(ee: &Ee, p: &Ptrs) -> SortAnswer {
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
fn beside(d: &Drive) -> Option<Seen> {
    let pre = drive::image(d.name)?;
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
    let s = drive::sim(&pre);
    let mut ee = drive::hooked(pre);
    let seen: Shared<Seen> = Rc::default();
    let frame_no: Shared<usize> = Rc::default();
    let (keep, at_frame, name) = (seen.clone(), frame_no.clone(), d.name);
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
        atv_car::frame(&world, &mut car, &mut body, dt, table, &mut atv_car::Own { v: &mut v, rest: &mut pieces });
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
    }));
    // As many frames as the capture has rows (MD107's test steps one past the row the last vsync
    // names); every frame's input two tape ticks back, as there.
    let mut j = 0;
    for _ in 0..rows.len() {
        *frame_no.borrow_mut() = j + 1;
        drive::step(&mut ee, s, d, &ticks, &mut j, 2);
    }
    let seen = std::mem::take(&mut *seen.borrow_mut());
    assert_eq!(seen.frames, rows.len(), "{}: the world step ran once a frame", d.name);
    eprintln!(
        "MD135 {}: {} frames, {} words in all equal to the original's; handed out {:?}; the sort struck on {} frames, the record held contacts on {}; wrecked on {}; {} other world steps",
        d.name, seen.frames, seen.words, seen.handed, seen.sort_struck, seen.touching, seen.wrecked, seen.other_worlds
    );
    Some(seen)
}

/// **The buggy's drive is the port's**: every frame of MD107's replay, the port's car frame beside
/// the interpreter's world step, every word equal.
#[test]
fn the_buggys_drive_is_the_ports() {
    beside(&drive::BUGGY);
}

/// **The baja's.**
#[test]
fn the_bajas_drive_is_the_ports() {
    beside(&drive::BAJA);
}

/// **The sandrail's.**
#[test]
fn the_sandrails_drive_is_the_ports() {
    beside(&drive::SANDRAIL);
}

/// **The golf cart's.**
#[test]
fn the_golf_carts_drive_is_the_ports() {
    beside(&drive::GOLF);
}

/// **The monster truck's.**
#[test]
fn the_trucks_drive_is_the_ports() {
    beside(&drive::TRUCK);
}
