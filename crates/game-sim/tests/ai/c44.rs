// ---------------------------------------------------------------------------------------------------
// C44 — player 1's plane dropped in front of a riding AI bike, through one-player Sim::step's plane lane
// ---------------------------------------------------------------------------------------------------
//
// The capture: biplane.p2m2's state (free06, not free09: the biplane state is free06's, and its AI
// riders are there) with the plane put half a second ahead of AI rider a1's bike, too late for it to
// steer round (research/c44/drop.py; free06's riders swerve round a plane parked in their way). The
// state is the copy's slot 31 saved after the put-back (context/states/c44-drop.eeMemory.bin, from
// `python3 -B research/c44/slots.py ram drop`), and watch/c44-plane-bike.toml recorded from it into
// context/capture/c44/drop-a.csv (two more recordings, try2 and drop-b, agree with it on every column
// of their 149 common rows). a1 brakes and backs off from 11134, its record lists the plane's record
// from 11145 (the plane's kind-1 mirrors first, then its own kind-0 entries), the plane is pushed
// from 11146 and a1 wrecks (reason 7) at 11183.
//
// The replay seeds a one-player Sim from the state: player 1 in the plane (its body, tires, engine,
// noise and controls off RAM, as S252 seeds it), the human's parked bike as player 1's, the three AI
// riders' bikes (C33's seed) as players 2-4 by their seats, their brains off RAM on a lane over
// free06's seeker world, and free06's terrain, props and plants. Each step into a row rides the levers
// the capture's row before it holds for each AI rider (the brain's levers, `+0x628`...`+0x648`),
// and the bikes are compared with the row: the chassis centre, velocity, rates and orientation.
//
// What it finds: a1 is bit for bit (every chassis word compared) through 11145, a0 and a3 within
// 0.001 ft, the plane where the original's is. From 11135 a1's brain backs up (`Recovery::back_up`:
// the reverse lever alone), and the bike is pushed back from 77 ft/s, as the original's levers go
// straight into the rider's (S493). On 11145 a1 lists the plane's mirror, as the original's; on
// 11146 the original's a1 also lists two of its own body rays on the plane's hull and ours none, and
// a1 and the plane both part there.

use super::race4::{Bikes, c33_bike, c44_human_bike};
use super::sim::{S12eLists, s12e_bike, s12e_racer, s12e_u64};
use super::*;
use game_render::disc::{Disc, Res};
use game_sim::Sim;
use game_sim::ai::{Lane, Levers, Pilot, Seat};
use game_sim::eco::{Trees, Wooded};
use game_sim::flight::Flight;
use game_sim::flight_search;
use game_sim::phy::{self, Phy, Plane};
use game_sim::contact::Contact;
use game_sim::rider::{Rider, Tables};
use game_sim::rider_contact;
use game_sim::track::{self, Scene, Terrain};
use game_sim::world::{Ground, Place, Props};
use ps2_oracle::{Ee, Kit};
use std::collections::HashMap;
use std::sync::Arc;

const STATE: &str = "states/c44-drop.eeMemory.bin";
const CSV: &str = "capture/c44/drop-a.csv";
/// The plane (MXBiplane) and its body's, engine's and tires' slots (`tests/replay.rs`'s `plane_at`).
const PLANE: u32 = 0x00c4_4a40;
const PLANE_BODY: u32 = 0x160;
const PLANE_ENGINE: u32 = 0x400;
const PLANE_TIRES: [u32; 3] = [0x408, 0x40c, 0x410];
/// The riders in the actor list's order (research/c33/riders.py): the rider, its brain (0: the
/// human, who flies) and the capture's prefix.
const RIDERS: [(u32, u32, &str); 4] =
    [(0x018f_7290, 0x0193_4ec0, "a0"), (0x019a_44b0, 0x0188_c530, "a1"), (0x0189_0cd0, 0, "human"), (0x0188_1b10, 0x0085_0910, "a3")];
/// a1, the bike the plane is dropped before.
const STRUCK: usize = 1;
/// The bikes' class: the 250s on the A file, which the AI riders' chassis mass is.
const CLASS: (track::Engine, track::Tuning) = (track::Engine::Cc250, track::Tuning::A);
/// The ticks the capture's a1 lists the plane's record on, and the one it wrecks on.
const FIRST_LISTED: u64 = 11145;
const WRECKED: u64 = 11183;

/// A row of the capture, by column name.
struct Row(HashMap<String, String>);

impl Row {
    fn f(&self, name: &str) -> f32 {
        let s = self.0.get(name).unwrap_or_else(|| panic!("no column {name}"));
        s.parse::<f64>().unwrap_or_else(|_| panic!("{name} '{s}' is not a number")) as f32
    }
    fn n(&self, name: &str) -> u64 {
        self.f(name) as u64
    }
    fn vf(&self, name: &str) -> Vf {
        Vf::new(self.f(&format!("{name}.x")), self.f(&format!("{name}.y")), self.f(&format!("{name}.z")), self.f(&format!("{name}.w")))
    }
    fn levers(&self, pre: &str) -> Levers {
        let l = |k: &str| F(self.f(&format!("{pre}_{k}")));
        Levers { steer: l("steer"), lean: l("lean"), gas: l("gas"), brake: l("brake"), reverse: l("reverse") }
    }
}

fn rows(text: &str) -> Vec<Row> {
    let mut lines = text.lines();
    let head: Vec<String> = lines.next().unwrap_or_default().split(',').map(str::to_string).collect();
    lines.filter(|l| !l.is_empty()).map(|l| Row(head.iter().cloned().zip(l.split(',').map(str::to_string)).collect())).collect()
}

fn quad(ee: &Ee, at: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(at + k)))
}

/// free06's archives (`Tracks\free06*.res`) off the disc.
fn free06_parts(disc: &mut Disc) -> Vec<Res> {
    let names: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("tracks\\free06") && n.ends_with(".res")
        })
        .collect();
    names.iter().filter_map(|n| disc.res(n).ok()).collect()
}

fn read_any(parts: &[Res], suffix: &str) -> Option<Vec<u8>> {
    parts.iter().find_map(|r| r.read_suffix(suffix).ok())
}

/// free06's scene and its terrain file.
fn free06_scene(parts: &[Res]) -> (Scene, Vec<u8>) {
    let scene = Scene::parse(&read_any(parts, ".scn").expect("free06's scene")).expect("free06's scene parses");
    let terrain_bytes = read_any(parts, &scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn")).expect("free06's terrain");
    (scene, terrain_bytes)
}

/// free06 as the Sim's world: its terrain, its props across its archives, and its plants on the
/// image's curve table, as [`super::lane::s323_free09`] makes free09's.
fn free06_world<'a>(parts: &[Res], ee: &Ee, scene: &Scene, terrain: &'a Terrain<'a>) -> Wooded<Place<'a>> {
    let ground = Ground::new(terrain, &scene.terrain);
    let trees = match read_any(parts, ".esb") {
        Some(esb) => {
            let field = ground.field();
            let mut eco = game_sim::eco::Ecosystem::parse(&esb).expect("free06.esb parses");
            let layout = &scene.terrain;
            let extent = F::from(layout.width.max(layout.breadth) as i32 * game_sim::world::SAMPLES_PER_WIDTH as i32) * terrain.scale;
            eco.stand(extent, |p| F(game_sim::contact_search::ground(&field, p).point.y()));
            let table = ee.u32_at(addr("curve_table_ptr").unwrap());
            let entries: Vec<[f32; 2]> = (0..game_sim::camera::CURVE_ENTRIES as u32)
                .map(|i| [f32::from_bits(ee.u32_at(table + 8 * i)), f32::from_bits(ee.u32_at(table + 8 * i + 4))])
                .collect();
            let (trees, missed) = Trees::new(&eco, game_sim::eco::cell_size(extent), &CurveTable::from_entries(&entries));
            if !missed.is_empty() {
                println!("free06's solids the port does not build: {missed:?}");
            }
            trees
        }
        None => Trees::default(),
    };
    let (props, missing) = Props::load(scene, |want| parts.iter().find_map(|r| r.read_whole(want).ok()).ok_or_else(|| "no archive of free06 has it".to_string()));
    if !missing.is_empty() {
        println!("free06's props the port leaves out: {missing:?}");
    }
    Wooded { world: Place { ground, props }, trees }
}

/// free06's seeker world, as [`super::sim::s12e_seek_world`] loads free09's: its `hit.dat` and the
/// databases it names, the image's sets' owners and its slots. An empty world where free06 has no
/// `hit.dat`. And the image's lists.
fn free06_seek(parts: &[Res], ee: &Ee, place: &mut Place) -> (sk::World, S12eLists) {
    let root = ee.u32_at(addr("ai_seek_sequence_root").unwrap());
    let set_items = ee.u32_at(root + 0x38);
    let set_list: Vec<u32> = (0..ee.u32_at(root + 0x30)).map(|i| ee.u32_at(set_items + 4 * i)).collect();
    let zones = ee.u32_at(root + 0x44);
    let manager = ee.u32_at(ee.u32_at(zones + 0x50) + 0x48);
    let target_items = ee.u32_at(manager + 0x38);
    let targets = (0..ee.u32_at(manager + 0x30)).map(|i| ee.u32_at(target_items + 4 * i)).collect();
    let lists = S12eLists { sets: set_list.clone(), targets };
    let Some(hit_dat) = read_any(parts, "\\hit.dat").map(|b| Ini::parse_bytes(&b)) else {
        println!("free06 has no hit.dat: the brains seek in an empty world");
        return (sk::World::default(), lists);
    };
    let (hits, sets) = sk::databases(&hit_dat);
    let db = |name: Option<&str>| name.and_then(|n| read_any(parts, &format!("\\{}", n.to_ascii_lowercase()))).map(|b| Ini::parse_bytes(&b));
    let (Some(hits), Some(hit_sets)) = (db(hits.as_deref()), db(sets.as_deref())) else {
        println!("free06's hit.dat names no databases its archives hold: the brains seek in an empty world");
        return (sk::World::default(), lists);
    };
    let owners: Vec<sk::Owner> = set_list
        .iter()
        .map(|&s| {
            let o = ee.u32_at(s + 0x54);
            let node = ee.u32_at(ee.u32_at(ee.u32_at(o + 0x3c)) + 0x18);
            sk::Owner { kind: ee.u32_at(o + 0x34) as i32, player: ee.u32_at(o + 0x30) as usize, position: quad(ee, node + 0x70), hit_freeride: false }
        })
        .collect();
    let slots = ee.u32_at(manager + 0x48) as usize;
    let open = |player: usize, zone: sk::Zone| player == 0 && zone.place == Some(0);
    (sk::load(&hit_dat, &hits, &hit_sets, &owners, slots, &open, place), lists)
}

/// The AI brain `b` on rider `r`, read out of RAM as C33's replay reads its brains.
fn pilot(ee: &Ee, lists: &S12eLists, b: u32, r: u32) -> Pilot {
    let filler = ai::spawn(&mut Rng::from_seed(0), 0, None, 2);
    let bike = s12e_bike(ee, b, r);
    Pilot {
        racer: s12e_racer(ee, lists, b, r),
        character: filler.character,
        wander: filler.wander,
        levers: bike.levers,
        recovery: bike.recovery,
        card: bike.card,
        seat: Seat {
            speed_scale: bike.speed_scale,
            slow_radius: bike.slow_radius,
            slow_speed: bike.slow_speed,
            player: bike.player,
            air_cone: bike.air_cone,
            look_ahead: bike.rider.look_ahead,
            bump_chance: bike.rider.bump_chance,
            race_phase: bike.race_phase,
            path_net: bike.path_net,
            live: bike.live,
        },
        ignore: Vec::new(),
    }
}

/// The plane as the state holds it, as `tests/replay.rs`'s S252 seeds it: `machine.res`'s plane and
/// rays, the bikes' collision mesh, and its body, tires, engine clock and prop, respawn, noise
/// tables and controls off RAM.
fn plane(disc: &mut Disc, ee: &Ee, tables: &Tables) -> Option<Flight> {
    let machine = disc.res("machine.res").map_err(|w| skip(&w)).ok()?;
    let plane = Plane::read_bytes(&machine.read_suffix(phy::BIPLANE_PHY).expect("machine.res's plane"));
    let rays = track::Rays::parse(&machine.read_suffix(flight_search::BIPLANE_RAYS).expect("machine.res's rays")).expect("biplanem.col");
    let names: Vec<String> = disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let mesh = names
        .iter()
        .find_map(|n| disc.res(n).ok()?.read(flight_search::BIKE_COLLISION).ok())
        .map(|b| track::Mesh::parse(&b).expect("bikef.col parses"))
        .expect("an archive carries bikef.col");
    let mut f = Flight::new(&plane).with_rays(flight_search::Rays::of(&rays)).with_bike(mesh);
    let word = |at: u32| F(ee.f32_at(at));
    let b = ee.u32_at(PLANE + PLANE_BODY);
    let e = ee.u32_at(PLANE + PLANE_ENGINE);
    let body = &mut f.body;
    body.enabled = ee.u32_at(b) != 0;
    body.inv_mass = word(b + 0x10);
    for (v, o) in [
        (&mut body.damping, 0x20),
        (&mut body.last_position, 0x50),
        (&mut body.position, 0x60),
        (&mut body.velocity, 0x70),
        (&mut body.last_force, 0x80),
        (&mut body.force, 0x90),
        (&mut body.accel, 0xa0),
        (&mut body.shock_peak, 0xb0),
        (&mut body.shock, 0xc0),
        (&mut body.step, 0xf0),
        (&mut body.inv_inertia, 0x120),
        (&mut body.ang_damping, 0x130),
        (&mut body.last_quat, 0x150),
        (&mut body.quat, 0x170),
        (&mut body.cofg, 0x180),
        (&mut body.moment, 0x190),
        (&mut body.last_moment, 0x1a0),
        (&mut body.rates, 0x1b0),
        (&mut body.ang_accel, 0x1c0),
        (&mut body.twist_peak, 0x1d0),
        (&mut body.twist, 0x1e0),
    ] {
        *v = quad(ee, b + o);
    }
    for (tire, at) in f.tires.iter_mut().zip(PLANE_TIRES) {
        let t = ee.u32_at(PLANE + at);
        let flag = |o: u32| ee.u32_at(t + o) != 0;
        (tire.below, tire.held, tire.touching) = (flag(0x80), flag(0x84), flag(0x88));
        (tire.compression, tire.last_compression, tire.spring_force) = (word(t + 0x8c), word(t + 0x90), word(t + 0x94));
        (tire.angle, tire.spin) = (word(t + 0x120), word(t + 0x124));
        for (v, o) in [
            (&mut tire.offset, 0x50),
            (&mut tire.bottom, 0xa0),
            (&mut tire.ground, 0xc0),
            (&mut tire.normal, 0xd0),
            (&mut tire.contact, 0xe0),
            (&mut tire.hub, 0xf0),
            (&mut tire.low, 0x100),
            (&mut tire.contact_velocity, 0x110),
            (&mut tire.last_normal, 0x140),
            (&mut tire.slip, 0x170),
            (&mut tire.friction, 0x180),
        ] {
            *v = quad(ee, t + o);
        }
        tire.frame = [0x190, 0x1a0, 0x1b0].map(|o| quad(ee, t + o));
    }
    f.engine.prop_angle = word(e + 0x48);
    f.engine.clock = word(e + 0x74);
    f.spawn = (quad(ee, PLANE + 0xe10), quad(ee, PLANE + 0xe20));
    let (perm, g1) = (addr("smooth_noise_perm").unwrap(), addr("smooth_noise_g1").unwrap());
    assert_eq!(ee.u32_at(addr("smooth_noise_unbuilt").unwrap()), 0, "the noise tables are built in the state");
    f.noise = Some(game_sim::noise::SmoothNoise::from_tables(
        &std::array::from_fn(|k| ee.u32_at(perm + 4 * k as u32)),
        &std::array::from_fn(|k| F(ee.f32_at(g1 + 4 * k as u32))),
    ));
    let w = |o: u32| word(PLANE + o);
    f.attitude.flown = ee.u32_at(PLANE + 0x334) != 0;
    let p = &mut f.pad;
    (p.forward, p.back, p.elevator, p.aileron, p.rudder) = (w(0x338), w(0x33c), w(0x340), w(0x344), w(0x348));
    let s = &mut f.surfaces;
    (s.throttle, s.brake, s.elevator, s.elevator_command) = (w(0x34c), w(0x350), w(0x354), w(0x358));
    (s.aileron, s.rudder, s.steer, s.pitch_integral) = (w(0x35c), w(0x360), w(0x364), w(0x584));
    f.remeasure(&tables.ride.acos);
    Some(f)
}

/// Each AI bike's gaps to a row: its chassis centre (ft), velocity (ft/s), rates and orientation.
fn gaps(sim: &Sim, player: usize, row: &Row, pre: &str) -> [f32; 4] {
    let ch = &sim.rider_of(player).ride.chassis;
    let gap = |a: Vf, b: Vf| (0..3).map(|c| (a[c] - b[c]).powi(2)).sum::<f32>().sqrt();
    let quat = (0..4).map(|c| (ch.quat[c] - row.vf(&format!("{pre}_ch_quat"))[c]).abs()).fold(0.0f32, f32::max);
    [
        gap(ch.cofg_world, row.vf(&format!("{pre}_ch_centre"))),
        gap(ch.vel, row.vf(&format!("{pre}_ch_vel"))),
        gap(ch.ang_vel, row.vf(&format!("{pre}_ch_rates"))),
        quat,
    ]
}

/// **C44: a plane dropped in front of a riding AI bike, through one-player `Sim::step`'s plane lane**
/// (see this file's head). Every row from the state's to a1's wreck is stepped, each AI bike on the
/// levers the capture's row before holds; a1's chassis is compared with every row.
#[test]
fn c44_a_plane_dropped_before_an_ai_bike_meets_it_as_the_original() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let (state, csv) = (kit.ctx(STATE), kit.ctx(CSV));
    if !state.exists() || !csv.exists() {
        return skip(&format!("{} or {} is not there: research/c44's capture (C44)", state.display(), csv.display()));
    }
    let mut ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    for &(r, b, pre) in &RIDERS {
        if b != 0 {
            assert_eq!(ee.u32_at(r + 0x10d0), b, "{pre}'s brain");
        }
    }
    let rows = rows(&std::fs::read_to_string(&csv).unwrap());
    let frame = u64::from(ee.u32_at(addr("frame_counter").unwrap()));
    // The image's RAM holds the bodies of the row after its frame counter, as C33's does.
    let a1 = RIDERS[STRUCK];
    let centre = quad(&ee, ee.u32_at(a1.0 + 0x7a4) + 0x60);
    let xyz = |v: Vf| [v.x(), v.y(), v.z()].map(f32::to_bits);
    assert!(rows.iter().all(|r| xyz(r.vf("a1_ch_centre")) != xyz(centre)), "the state's a1 is on a capture row");
    let dt = F(ee.f32_at(addr("frame_dt").unwrap()));
    assert!(rows.iter().all(|r| r.f("dt").to_bits() == dt.to_bits()), "every row's dt is the state's");
    println!("the state's frame counter {frame}; the capture's first row is {}", rows[0].n("frame_counter"));

    let Ok(mut disc) = Disc::open_configured().map_err(|w| skip(&w)) else { return };
    let parts = free06_parts(&mut disc);
    assert!(!parts.is_empty(), "the disc has free06's archives");
    let (scene, terrain_bytes) = free06_scene(&parts);
    let terrain = Terrain::parse(&terrain_bytes).expect("free06's terrain parses");
    let mut world = free06_world(&parts, &ee, &scene, &terrain);
    let (seek, lists) = free06_seek(&parts, &ee, &mut world.world);

    let tables = Arc::new(Tables::build());
    let Some(mut bikes) = Bikes::open() else { return };
    let name = track::physics_file(CLASS.0, false, CLASS.1);
    let phy: Phy = bikes.phy(&name, &tables);
    for &(r, b, pre) in &RIDERS {
        if b != 0 {
            let c = ee.u32_at(r + 0x7a4);
            assert_eq!(F(ee.f32_at(c + 0x210)).to_bits(), phy.mass.mass.to_bits(), "{pre}'s chassis mass is {name}'s");
        }
    }
    // Player 1 flies; the AI riders are players by their seats.
    let human = RIDERS[2].0;
    let mut sim = Sim::with_rider(0, Arc::clone(&tables), phy.clone(), c44_human_bike(&ee, human, &phy, &name, &bikes.col));
    let mut seated: Vec<(usize, usize)> = RIDERS.iter().enumerate().filter(|(_, (_, b, _))| *b != 0).map(|(k, &(r, b, _))| (s12e_bike(&ee, b, r).player, k)).collect();
    seated.sort();
    assert_eq!(seated.iter().map(|s| s.0).collect::<Vec<_>>(), [1, 2, 3], "the AI riders' seats");
    for &(_, k) in &seated {
        sim.add_player(phy.clone(), c33_bike(&ee, RIDERS[k].0, &phy, &name, &bikes.col));
    }
    let player_of = |k: usize| seated.iter().find(|s| s.1 == k).unwrap().0;
    let pilots: Vec<(usize, Pilot)> = seated.iter().map(|&(p, k)| (p, pilot(&ee, &lists, RIDERS[k].1, RIDERS[k].0))).collect();
    let gen0 = s12e_u64(&ee, addr("rand_state").unwrap());
    sim.ai = Some(Box::new(Lane { seek, tables: Arc::new(controls::tables(&ee)), rng: Rng::from_state(gen0), pilots, net: None, cards: Vec::new(), edge: None }));
    let Some(f) = plane(&mut disc, &ee, &tables) else { return };
    sim.plane = Some(f);

    // Each step into a row rides the levers the row before holds (the brain's think comes after the
    // ride in the original's frame). The capture starts a tick after the image's bodies, so the steps
    // into its first rows, which have no row before, ride their own row's levers (the brains held the
    // same levers into them: a1 is bit for bit there either way).
    let ais: Vec<usize> = seated.iter().map(|s| s.1).collect();
    let (mut parted, mut worst) = (None, [0.0f32; 4]);
    let mut agreed: Vec<(u64, [f32; 4])> = Vec::new();
    let by_tick: HashMap<u64, &Row> = rows.iter().map(|r| (r.n("frame_counter"), r)).collect();
    assert_eq!(by_tick[&(FIRST_LISTED - 1)].n("a1_rec_count"), 0, "a1 lists nothing before {FIRST_LISTED}");
    assert!(by_tick[&FIRST_LISTED].n("a1_rec_count") > 0, "a1 lists the plane at {FIRST_LISTED}");
    // a1's own entries on the plane's hull (kind 0) against the row's, and the plane against the
    // row's, from the first tick a1 lists the plane: (tick, own entries ours/theirs, the worst point
    // and normal gaps, the plane's gap).
    let mut met: Vec<(u64, usize, usize, f32, f32, f32)> = Vec::new();
    for tick in frame + 1 + C44_HELD..=WRECKED {
        if let Some(last) = by_tick.get(&(tick - 1)).or_else(|| by_tick.get(&tick)) {
            let lane = sim.ai.as_deref_mut().unwrap();
            for &k in &ais {
                let p = player_of(k);
                lane.pilots.iter_mut().find(|(q, _)| *q == p).unwrap().1.levers = last.levers(RIDERS[k].2);
            }
        }
        // The Sim gathers the bikes' lists from where the last tick left them, before anyone is
        // stepped, with the plane's hull in their world (`rider_contact::contacts_met`); the same
        // gather here is a1's own entries the step resolves. The plane's mirrors (kind 1) are its
        // frame's, which the Sim does not hand out.
        let own: Vec<Contact> = {
            let hulls: Vec<_> = sim.plane.as_ref().and_then(|p| p.hull(0)).into_iter().collect();
            let seated: Vec<(&Rider, bool)> = std::iter::once((&sim.rider, false)).chain(sim.others.iter().map(|p| (&p.rider, true))).collect();
            rider_contact::contacts_met(&seated, &hulls, dt, &world).map_or_else(Vec::new, |(l, _)| l[player_of(STRUCK)].clone())
        };
        sim.step(dt, &Default::default(), &mut world);
        let Some(row) = by_tick.get(&tick) else {
            continue;
        };
        if tick == rows[0].n("frame_counter") {
            let near: Vec<(u64, f32)> =
                (tick - 2..=tick + 2).filter_map(|t| by_tick.get(&t).map(|r| (t, gaps(&sim, player_of(STRUCK), r, "a1")[0]))).collect();
            println!("the first row's a1 against the rows about it: {near:?}");
        }
        let g = gaps(&sim, player_of(STRUCK), row, "a1");
        let plane = sim.plane.as_ref().unwrap().body.position;
        let plane_gap = (0..3).map(|c| (plane[c] - row.vf("plane_pos")[c]).powi(2)).sum::<f32>().sqrt();
        let others: Vec<String> = ais.iter().filter(|&&k| k != STRUCK).map(|&k| format!("{} {:.4}", RIDERS[k].2, gaps(&sim, player_of(k), row, RIDERS[k].2)[0])).collect();
        let r = sim.rider_of(player_of(STRUCK));
        if tick >= FIRST_LISTED {
            let xyz3 = |name: String| Vf::new(row.f(&format!("{name}.x")), row.f(&format!("{name}.y")), row.f(&format!("{name}.z")), 0.0);
            let theirs: Vec<(Vf, Vf)> = (0..row.n("a1_rec_count"))
                .filter(|k| row.n(&format!("a1_c{k}.kind")) == 0)
                .map(|k| (xyz3(format!("a1_c{k}.point")), xyz3(format!("a1_c{k}.normal"))))
                .collect();
            let ours: Vec<&Contact> = own.iter().filter(|c| c.kind == 0).collect();
            let gap = |a: Vf, b: Vf| (0..3).map(|c| (a[c] - b[c]).powi(2)).sum::<f32>().sqrt();
            let (mut point, mut normal) = (0.0f32, 0.0f32);
            for (o, t) in ours.iter().zip(&theirs) {
                point = point.max(gap(o.point, t.0));
                normal = normal.max(gap(o.normal, t.1));
            }
            println!("    own entries ours {} theirs {} (point {point:.5} normal {normal:.5})", ours.len(), theirs.len());
            met.push((tick, ours.len(), theirs.len(), point, normal, plane_gap));
        }
        if std::env::var_os("C44_SHOW").is_some() {
            let l = row.levers("a1");
            let (v, w) = (r.ride.chassis.vel, row.vf("a1_ch_vel"));
            println!(
                "    vel ours ({:.3} {:.3} {:.3}) theirs ({:.3} {:.3} {:.3}) | levers s {} l {} g {} b {} r {}",
                v.x(), v.y(), v.z(), w.x(), w.y(), w.z(), l.steer.0, l.lean.0, l.gas.0, l.brake.0, l.reverse.0
            );
        }
        println!(
            "{tick}: a1 centre {:.5} vel {:.4} rates {:.4} quat {:.5} | listed {} wrecked {:?}/{} | plane {:.5} | {}",
            g[0], g[1], g[2], g[3], row.n("a1_rec_count"), r.state.wreck.is_some(), row.n("a1_wreck_reason"), plane_gap, others.join(" ")
        );
        if parted.is_none() && (g[0] > 1e-3 || g[1] > 1e-2) {
            parted = Some(tick);
        }
        if parted.is_none() {
            agreed.push((tick, g));
            for c in 0..4 {
                worst[c] = worst[c].max(g[c]);
            }
        }
    }
    println!("a1 agrees through {:?} (worst {worst:?}); parts at {parted:?}", agreed.last().map(|a| a.0));
    let through = agreed.last().map_or(0, |a| a.0);
    assert!(through >= C44_AGREES_THROUGH, "a1 parts at {parted:?}, before {C44_AGREES_THROUGH}");
    assert_eq!(parted, C44_PARTS, "where a1 parts from the original (a regression mark: move it when it moves)");
    assert!(worst.iter().all(|&w| w == 0.0), "a1 is bit for bit until it parts: {worst:?}");
    // The list and the plane's push (regression marks: move them when they move). a1's own entries
    // on the hull are the row's, in number and (within 0.001) in place, until the strike: on 11146
    // the original's a1 lists two of its body rays on the plane's hull and ours none, though our a1
    // is still bit for bit going into it, so its resolve has only the plane's mirror and both bodies
    // part there.
    let list_parts = met.iter().find(|m| m.1 != m.2 || m.3 >= 1e-3 || m.4 >= 1e-3).map(|m| m.0);
    println!("a1's own entries part at {list_parts:?}: {:?}", met.iter().find(|m| Some(m.0) == list_parts));
    assert_eq!(list_parts, C44_LIST_PARTS, "where a1's own entries on the plane's hull part from the original's");
    let plane_parts = met.iter().find(|m| m.5 > 1e-3).map(|m| m.0);
    assert_eq!(plane_parts, C44_PLANE_PARTS, "where the plane's push parts from the original's");
}

/// The image's RAM holds the bodies of the row this many ticks past its frame counter.
const C44_HELD: u64 = 1;

/// How far a1 rides as the original's (centre within 0.001 ft, velocity within 0.01 ft/s), and the
/// tick it parts on. Its brain backs up from 11135 (the reverse lever alone), and the ride pushes it
/// back as the original's does (S493: the brain's levers skip the pad's reverse rule, and the push
/// turns with the chassis every substep), so a1 is bit for bit through 11145, its first tick listing
/// the plane, and parts on 11146, the strike's first resolve, where a1 and the plane both leave the
/// original's.
const C44_AGREES_THROUGH: u64 = 11145;
const C44_PARTS: Option<u64> = Some(11146);
/// The first tick a1's own entries on the plane's hull (kind 0) are not the row's: ours finds none
/// where the original's body rays strike the hull twice.
const C44_LIST_PARTS: Option<u64> = Some(11146);
/// The first tick the plane is off the row's (by more than 0.001 ft): the strike's.
const C44_PLANE_PARTS: Option<u64> = Some(11146);
