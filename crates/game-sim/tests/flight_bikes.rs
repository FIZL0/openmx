//! The other riders' bikes in the plane's world (S95), against biplane.p2m2's save state and the disc.
//!
//! 1. **the collidables** — the three bikes among the plane's candidates in the state are what
//!    [`flight_search::world_among`] builds from `bikef.col` on each one's own transform: the category,
//!    the flags, the reach, the radius and the centre bit for bit, one listener each, the struck
//!    listener, on a bike whose mask no plane's probe meets;
//! 2. **the listener** — the original's struck listener run on the state keeps the striker on the
//!    bike, and the port's keeps the striker's category;
//! 3. **the ride** — a plane flown level into a wall of bikes strikes one, does not crash on it and
//!    does not fly through; with the bikes out of the world it flies through;
//! 4. **the records** (B9) — each bike is its collision record's, and the plane's rays on a bike
//!    moved into it name that record and file their mirrors on it as the port's search does;
//! 5. **the bikes ride on** (B9a) — the plane's node collides its record before the AI bikes', so a
//!    struck bike's list is the plane's mirrors ahead of its own, and one-player `Sim::step` steps a
//!    bike the plane pushed.

use game_render::disc::Disc;
use game_sim::archive::{Psr, Res};
use game_sim::biplane::CRASHED;
use game_sim::flight::Flight;
use game_sim::flight_search::{self, BIKE, BIKE_STRUCK_MASK, BIKES, Bike, Bikes, FIRST, PROBE, Rays};
use game_sim::math::Mat4;
use game_sim::phy::{self, Plane};
use game_sim::track;
use game_sim::world::Flat;
use game_sim::world_search::{Class, flags};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const PLANE: u32 = 0x00c4_4a40;
const PLANE_COLLISION: u32 = 0x170;
const NEARBY: u32 = 0x10c;
const RAY_PROBE: u32 = 0x1f0;
const SWEEP_PROBE: u32 = 0x1f4;
/// The collision node the plane's record is on (B9a).
const NODE: u32 = 0x0060_b200;

/// A collidable's fields (S44/S45's offsets, and the bike's: S95).
mod at {
    pub const CLASS: u32 = 0x02;
    pub const NEARBY: u32 = 0x31;
    pub const CENTRE: u32 = 0x40;
    pub const RADIUS: u32 = 0x60;
    pub const CATEGORY: u32 = 0x84;
    pub const COLLIDES_WITH: u32 = 0xcc;
    pub const LISTENER_COUNT: u32 = 0xf4;
    pub const LISTENERS: u32 = 0xfc;
    pub const FLAGS: u32 = 0x116;
    /// The chassis node's world transform the bike's collidable is placed on.
    pub const TRANSFORM: u32 = 0x120;
}

/// The bike's own fields: its listener sits `LISTENER` into it; what struck it, and the mask.
mod bike_at {
    pub const LISTENER: u32 = 0x80;
    pub const STRUCK: u32 = 0x7d8;
    pub const STRUCK_MASK: u32 = 0x398;
}

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of biplane.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// `bikef.col` out of the first archive on the disc that carries it.
fn bike_mesh() -> Option<track::Mesh> {
    let mut disc = Disc::open_configured().map_err(|w| skip(&w)).ok()?;
    let names: Vec<String> =
        disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    for n in &names {
        let Ok(res) = disc.res(n) else { continue };
        if let Ok(bytes) = res.read(flight_search::BIKE_COLLISION) {
            return Some(track::Mesh::parse(&bytes).expect("bikef.col parses"));
        }
    }
    panic!("no archive on the disc carries {}", flight_search::BIKE_COLLISION);
}

fn quad(ee: &Ee, at: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(at + k)))
}

fn matrix(ee: &Ee, at: u32) -> Mat4 {
    Mat4([0, 1, 2, 3].map(|r| quad(ee, at + 16 * r)))
}

/// The plane's candidates that are bikes, in the state's order.
fn bikes_in(ee: &Ee) -> Vec<u32> {
    let search = ee.u32_at(ee.u32_at(PLANE + PLANE_COLLISION) + NEARBY);
    let (found, count) = (ee.u32_at(search), ee.u32_at(search + 4));
    (0..count).map(|k| ee.u32_at(found + 8 * k)).filter(|&o| ee.u32_at(o + at::CATEGORY) == BIKE).collect()
}

/// **The state's bikes are the port's**: every bike collidable among the plane's candidates is an
/// object, enabled, reached from anywhere, category [`BIKE`], striking anything, with one listener —
/// the bike's struck listener, sitting [`bike_at::LISTENER`] into a bike whose mask is
/// [`BIKE_STRUCK_MASK`] — and its radius and centre are, bit for bit, `bikef.col`'s root box through
/// the transform the collidable holds, as [`flight_search::world_among`] makes them. Neither probe's
/// category wrecks a bike it strikes.
#[test]
fn the_states_bikes_are_the_ports() {
    let Some(ee) = image() else { return };
    let Some(mesh) = bike_mesh() else { return };
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]].map(|r: [f32; 3]| r.map(F));
    assert_eq!(mesh.transform, identity, "bikef.col carries a transform of its own");
    let found = bikes_in(&ee);
    assert_eq!(found.len(), 3, "the state's plane has three bikes among its candidates");
    let thunk = addr("bike_struck_thunk").unwrap();
    let bikes: Vec<Bike> = found.iter().map(|&o| Bike { transform: matrix(&ee, o + at::TRANSFORM), in_world: true }).collect();
    let (broad, objects) = flight_search::world_among(&[], Some(&Bikes { mesh: &mesh, bikes: &bikes }), None, Vf::new(0.0, 0.0, 0.0, 1.0), F(100.0));
    for (k, &o) in found.iter().enumerate() {
        let port = &objects[FIRST + k];
        assert_eq!(ee.u8_at(o + at::CLASS), 2, "bike {k}: an object");
        assert_eq!(port.class, Class::Object);
        assert_eq!(ee.u8_at(o + at::FLAGS) & !flags::SEARCHED, flags::ENABLED, "bike {k}: its flags");
        assert_eq!(port.flags, flags::ENABLED);
        assert_eq!(ee.u8_at(o + at::NEARBY) & 1, 1, "bike {k}: reached from anywhere");
        assert!(port.nearby);
        assert_eq!(port.category, BIKE);
        assert_eq!(ee.u32_at(o + at::COLLIDES_WITH), port.collides_with, "bike {k}: what it strikes");
        assert_eq!(ee.u32_at(o + at::LISTENER_COUNT), 1, "bike {k}: one listener");
        assert_eq!(port.listeners, vec![k]);
        let listener = ee.u32_at(ee.u32_at(o + at::LISTENERS));
        assert_eq!(ee.u32_at(ee.u32_at(listener) + 0xc), thunk, "bike {k}: the struck listener");
        let bike = listener - bike_at::LISTENER;
        assert_eq!(ee.u32_at(bike + bike_at::STRUCK_MASK), BIKE_STRUCK_MASK, "bike {k}: its struck mask");
        assert_eq!(ee.f32_at(o + at::RADIUS).to_bits(), port.radius.0.to_bits(), "bike {k}: the radius");
        let centre = broad.places[FIRST + k].centre;
        for l in 0..4 {
            assert_eq!(ee.f32_at(o + at::CENTRE + 4 * l as u32).to_bits(), centre[l].0.to_bits(), "bike {k}: centre[{l}]");
        }
    }
    let rec = ee.u32_at(PLANE + PLANE_COLLISION);
    for probe in [RAY_PROBE, SWEEP_PROBE] {
        let category = ee.u32_at(ee.u32_at(rec + probe) + at::CATEGORY);
        assert_eq!(category, PROBE, "the plane's probe {probe:#x}");
        assert_eq!(flight_search::struck_wreck(Some(category), BIKE_STRUCK_MASK), None);
    }
    assert_eq!(flight_search::struck_wreck(Some(BIKE_STRUCK_MASK), BIKE_STRUCK_MASK), Some(game_sim::air::Wreck::Flat));
    assert_eq!(flight_search::struck_wreck(None, BIKE_STRUCK_MASK), None);
}

/// **The struck listener keeps the striker**: the original's, run through its thunk on each bike's
/// listener with the plane's ray probe as the striker, stores the probe on the bike over what was
/// there; the port's keeps the probe's category, which is all the wreck test reads of it.
#[test]
fn the_struck_listener_keeps_the_striker() {
    let Some(mut ee) = image() else { return };
    let thunk = addr("bike_struck_thunk").unwrap();
    let ray = ee.u32_at(ee.u32_at(PLANE + PLANE_COLLISION) + RAY_PROBE);
    for (k, o) in bikes_in(&ee).into_iter().enumerate() {
        let listener = ee.u32_at(ee.u32_at(o + at::LISTENERS));
        let bike = listener - bike_at::LISTENER;
        ee.set_u32(bike + bike_at::STRUCK, 0xdead_beef);
        ee.call(thunk, &[u64::from(listener), u64::from(o), u64::from(ray)], &[], 1_000);
        assert_eq!(ee.u32_at(bike + bike_at::STRUCK), ray, "bike {k}: the striker");
        let mut mark = Some(BIKE_STRUCK_MASK);
        flight_search::bike_struck(&mut mark, ee.u32_at(ray + at::CATEGORY));
        assert_eq!(mark, Some(PROBE));
    }
}

/// `machine.res` out of the disc's `MX.PSR`.
fn the_plane() -> Option<(Flight, track::Mesh)> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let member = |name: &str| {
        let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("machine.res has {name}"));
        res.read(&bytes, e).expect("its bytes")
    };
    let plane = Plane::read_bytes(&member(phy::BIPLANE_PHY));
    let rays = track::Rays::parse(&member(flight_search::BIPLANE_RAYS)).expect("biplanem.col");
    let mesh = bike_mesh()?;
    Some((Flight::new(&plane).with_rays(Rays::of(&rays)).with_bike(mesh.clone()), mesh))
}

/// A wall of bikes standing across the plane's way at `z` 60, turned side on, stacked from below its
/// height to above it.
fn wall(in_world: bool) -> Vec<Bike> {
    (0..8)
        .map(|k| Bike {
            transform: Mat4([
                Vf::new(0.0, 0.0, -1.0, 0.0),
                Vf::new(0.0, 1.0, 0.0, 0.0),
                Vf::new(1.0, 0.0, 0.0, 0.0),
                Vf::new(0.0, 188.0 + 3.0 * k as f32, 60.0, 1.0),
            ]),
            in_world,
        })
        .collect()
}

/// The plane flown level at the wall for two seconds, or until a few frames after it first touches
/// something (the knock pitches it over, and upside down it later crashes by the other rule): where it
/// got to, what its record struck, what the bikes' struck listeners kept, and whether it crashed.
fn fly_at(bikes: &[Bike]) -> Option<(F, Option<usize>, Vec<Option<u32>>, bool)> {
    let (f, _) = the_plane()?;
    let tables = game_sim::ride::Tables::build();
    let mut f = f;
    let mut ground = Flat::default();
    f.attitude.flown = true;
    f.place(Vf::new(0.0, 200.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
    f.body.velocity = Vf::new(0.0, 0.0, 120.0, 1.0);
    let dt = F(1.0 / 30.0);
    let gas = game_sim::biplane::Pad { forward: F::ONE, ..Default::default() };
    let (mut struck, mut marks, mut touched) = (None, vec![None; bikes.len()], None);
    for tick in 0..60 {
        if touched.is_some_and(|t| tick > t + 5) {
            break;
        }
        let kept = f.frame_among(dt, gas, &mut ground, &tables, bikes);
        for (m, k) in marks.iter_mut().zip(kept) {
            *m = m.or(k);
        }
        struck = struck.or(f.record.struck);
        if f.record.touching {
            touched = touched.or(Some(tick));
        }
    }
    Some((F(f.body.position.z()), struck, marks, f.crash.reason == CRASHED))
}

/// **The plane meets the ride's bikes**: flown level into a wall of them, its probes strike a bike,
/// the record keeps that bike ([`BIKES`]), the bike's struck listener keeps the plane's probe, the
/// crash test does not wreck it — a bike is a vehicle, a mesh is not — and the contact holds it short
/// of the wall.
/// The same wall out of the world is not there: the plane flies through it untouched.
#[test]
fn a_plane_flown_into_a_bike_strikes_it() {
    let Some((z, struck, marks, crashed)) = fly_at(&wall(true)) else { return };
    let k = struck.and_then(|s| s.checked_sub(BIKES)).unwrap_or_else(|| panic!("the record struck {struck:?}, not a bike"));
    assert!(k < marks.len(), "bike {k} is not one of the wall's");
    assert!(marks.iter().any(|m| *m == Some(PROBE)), "no bike's struck listener heard the plane: {marks:?}");
    assert!(marks.iter().all(|m| m.is_none_or(|c| c == PROBE)), "a bike kept something else: {marks:?}");
    assert!(!crashed, "the plane crashed on a bike");
    assert_ne!(flight_search::category(BIKES + k) & flight_search::VEHICLE, 0, "a bike is not a vehicle to the crash test");
    assert_eq!(flight_search::category(FIRST) & flight_search::VEHICLE, 0, "a mesh is a vehicle to the crash test");
    assert!(z < 60.0, "the plane flew through the bikes to z {z:?}");

    let Some((z, struck, marks, _)) = fly_at(&wall(false)) else { return };
    assert_eq!(struck, None, "a bike out of the world was struck");
    assert!(marks.iter().all(Option::is_none));
    assert!(z > 70.0, "the plane stopped at z {z:?} with no bike in its way");
}


/// **The plane pushes the bikes it strikes** (N22b): with the wall's bikes named as the riders' collision
/// objects ([`game_sim::flight::Met::bikes`]), the frame the plane's rays first strike one its entries
/// name that bike, its resolve impulses the bike's chassis along the plane's way, and the bike's list
/// takes the mirrors naming the plane. Unnamed (S95's world), nothing moves the bikes.
#[test]
fn a_plane_flown_into_named_bikes_pushes_them() {
    let Some((f, _)) = the_plane() else { return };
    let tables = game_sim::ride::Tables::build();
    let bikes = wall(true);
    let sim = game_sim::Sim::new(0x5354);
    let met_of = |named: bool| game_sim::flight::Met {
        colliders: vec![game_sim::rider_contact::collider(&sim.phy.collisions, sim.phy.mass.mass); bikes.len()],
        vehicles: bikes
            .iter()
            .map(|b| game_sim::contact::Vehicle {
                node: game_sim::contact::Node { rows: [0, 1, 2].map(|r| b.transform.0[r]), origin: b.transform.0[3] },
                mass: sim.phy.mass.mass,
                inv_mass: F::ONE / sim.phy.mass.mass,
                inv_inertia: [F(0.01); 3],
                ..Default::default()
            })
            .collect(),
        bikes: if named { (0..bikes.len()).collect() } else { Vec::new() },
        named: named.then_some(bikes.len()),
        ..Default::default()
    };
    let fly = |named: bool| {
        let mut f = f.clone();
        let mut ground = Flat::default();
        f.attitude.flown = true;
        f.place(Vf::new(0.0, 200.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
        f.body.velocity = Vf::new(0.0, 0.0, 120.0, 1.0);
        let gas = game_sim::biplane::Pad { forward: F::ONE, ..Default::default() };
        let mut met = met_of(named);
        for _ in 0..60 {
            f.frame_met(F(1.0 / 30.0), gas, &mut ground, &tables, &bikes, Some(&mut met));
            if f.record.touching {
                return (f, met);
            }
        }
        panic!("the plane never touched the wall");
    };
    let (f, met) = fly(true);
    let k = met.pushed.iter().position(|&p| p).unwrap_or_else(|| panic!("no bike named: {:?}", f.record.contacts));
    assert!(f.record.contacts.iter().any(|c| c.kind == 0 && c.other == Some(k)), "the plane's entries name bike {k}");
    assert!(!met.struck[k].is_empty() && met.struck[k].iter().all(|c| c.kind == 1 && c.other == Some(bikes.len())), "bike {k}'s mirrors {:?}", met.struck[k]);
    assert!(met.vehicles[k].velocity.z() > 0.0, "bike {k} was not pushed the plane's way: {:?}", met.vehicles[k].velocity);
    let (f, met) = fly(false);
    assert!(f.record.contacts.iter().all(|c| c.other.is_none()), "unnamed, the plane's entries name no bike");
    assert!(met.struck.is_empty() && met.pushed.is_empty() && met.vehicles.iter().all(|v| v.velocity == Vf::ZERO), "unnamed bikes moved");
}

/// **B9a: the AI riders ride on while player 1 flies**: one-player `Sim::step` with the plane up and
/// another rider's bike in its way. The frame the plane's rays strike the bike, its resolve pushes the
/// bike's chassis the plane's way, and the bike — stepped after the plane, as the original's bike world
/// is (S99's clock), its list the plane's mirrors — moves on that push; before B9a it stood still for
/// as long as the plane flew.
#[test]
fn the_bike_a_one_player_plane_strikes_rides_on() {
    let Some((f, _)) = the_plane() else { return };
    let mut sim = game_sim::Sim::new(0x5354);
    let mounts = game_sim::stock::mounts(&sim.phy);
    let start = Vf::new(0.0, 197.0, 30.0, 1.0);
    let mut other = game_sim::rider::Rider::new(&sim.phy, mounts, start, Vf::new(1.0, 0.0, 0.0, 0.0), game_sim::trick::Set::default());
    // Kept up on its bike: this is about the push, not the wreck it may lead to.
    other.sense.wrecks_enabled = false;
    other.sense.air_wrecks = false;
    let phy = (*sim.phy).clone();
    sim.add_player(phy, other);
    let mut f = f;
    f.attitude.flown = true;
    f.place(Vf::new(0.0, 198.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &sim.tables.ride.acos);
    f.body.velocity = Vf::new(0.0, 0.0, 120.0, 1.0);
    sim.plane = Some(f);
    let mut ground = Flat { y: F(196.0), ..Default::default() };
    let dt = F(1.0 / 30.0);
    let mut pushed = None;
    for tick in 0..40 {
        let before = sim.others[0].rider.ride.position();
        sim.step(dt, &Default::default(), &mut ground);
        let r = &sim.others[0].rider;
        assert_ne!(r.ride.position().to_bits(), before.to_bits(), "tick {tick}: the other rider was not stepped");
        if pushed.is_none() && r.ride.chassis.vel.z() > 1.0 {
            pushed = Some(tick);
        }
    }
    let t = pushed.expect("the plane never pushed the bike");
    let z = sim.others[0].rider.ride.position().z();
    assert!(z > start.z() + 1.0, "the pushed bike (tick {t}) did not ride on: z {z:?}");
}

/// A collision record's list (count `+0x100`, entries of 0x80 at `+0x108`), each entry's other side
/// named by its place in `recs` (99: something else, `None`: nothing).
fn list_at(ee: &Ee, r: u32, recs: &[u32]) -> Vec<game_sim::contact::Contact> {
    let (n, at) = (ee.u32_at(r + 0x100), ee.u32_at(r + 0x108));
    (0..n)
        .map(|k| {
            let e = at + 0x80 * k;
            let other = ee.u32_at(e + 0x74);
            game_sim::contact::Contact {
                kind: ee.u32_at(e),
                from: quad(ee, e + 0x10),
                on_body: quad(ee, e + 0x20),
                normal: quad(ee, e + 0x30),
                point: quad(ee, e + 0x40),
                arm: quad(ee, e + 0x50),
                other_arm: quad(ee, e + 0x60),
                surface: ee.u32_at(e + 0x70) as i32,
                other: (other != 0).then(|| recs.iter().position(|&o| o == other).unwrap_or(99)),
                mover: (ee.u32_at(e + 0x78) != 0).then_some(99),
            }
        })
        .collect()
}

/// Every field of two entries the same, bit for bit.
fn same_entry(port: &game_sim::contact::Contact, orig: &game_sim::contact::Contact) -> bool {
    let q = |c: &game_sim::contact::Contact| [c.from, c.on_body, c.normal, c.point, c.arm, c.other_arm].map(Vf::to_bits);
    q(port) == q(orig) && (port.kind, port.surface, port.other, port.mover) == (orig.kind, orig.surface, orig.other, orig.mover)
}

/// A plane's body off the state (S48's offsets).
fn body_of(ee: &Ee, b: u32) -> game_sim::biplane::Body {
    game_sim::biplane::Body {
        enabled: ee.u32_at(b) != 0,
        inv_mass: F(ee.f32_at(b + 0x10)),
        damping: quad(ee, b + 0x20),
        last_position: quad(ee, b + 0x50),
        position: quad(ee, b + 0x60),
        velocity: quad(ee, b + 0x70),
        last_force: quad(ee, b + 0x80),
        force: quad(ee, b + 0x90),
        accel: quad(ee, b + 0xa0),
        shock_peak: quad(ee, b + 0xb0),
        shock: quad(ee, b + 0xc0),
        step: quad(ee, b + 0xf0),
        inv_inertia: quad(ee, b + 0x120),
        ang_damping: quad(ee, b + 0x130),
        last_quat: quad(ee, b + 0x150),
        quat: quad(ee, b + 0x170),
        cofg: quad(ee, b + 0x180),
        moment: quad(ee, b + 0x190),
        last_moment: quad(ee, b + 0x1a0),
        rates: quad(ee, b + 0x1b0),
        ang_accel: quad(ee, b + 0x1c0),
        twist_peak: quad(ee, b + 0x1d0),
        twist: quad(ee, b + 0x1e0),
    }
}

/// A bike's chassis as a vehicle off its collision record (N22a's offsets: `+0xf0` the chassis,
/// `+0xf8` its node).
fn vehicle_at(ee: &Ee, rec: u32) -> game_sim::contact::Vehicle {
    let (v, n) = (ee.u32_at(rec + 0xf0), ee.u32_at(rec + 0xf8));
    game_sim::contact::Vehicle {
        node: game_sim::contact::Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 16 * k)), origin: quad(ee, n + 0xb0) },
        cofg: quad(ee, v + 0x190),
        mass: F(ee.f32_at(v + 0x210)),
        inv_mass: F(ee.f32_at(v + 0x214)),
        inv_inertia: [0xf0, 0x104, 0x118].map(|at| F(ee.f32_at(v + at))),
        velocity: quad(ee, v + 0x260),
        rates: quad(ee, v + 0x290),
        shock: quad(ee, v + 0x170),
        twist: quad(ee, v + 0x180),
    }
}

/// **B9: the one-player plane's rays name the AI riders' bikes**. In the state each bike's collidable
/// among the plane's candidates is its collision record's frame collidable (record `+0x1f8`, owner
/// `+0x80` the record, capacity [`game_sim::rider_contact::CAPACITY`]), as split's are (N22b). With
/// one bike moved into the plane (its transform's origin and its box's centre rebuilt by the
/// original's bounds, the plane's search refilled by its own refill), the original's collide of the
/// plane's record files kind-0 entries naming that bike's record and kind-1 mirrors on it; the port's
/// search with the bikes named ([`game_sim::flight::Met::bikes`], as `Sim::step` builds it) gives the
/// same entries and the same mirrors, every field, in order, at every pose in [`B9_POSES`].
#[test]
fn the_one_player_planes_rays_name_the_states_bikes() {
    let Some(ee) = image() else { return };
    let Some((plane, mesh)) = the_plane() else { return };
    let found = bikes_in(&ee);
    let recs: Vec<u32> = found.iter().map(|&o| ee.u32_at(o + 0x80)).collect();
    for (k, (&o, &r)) in found.iter().zip(&recs).enumerate() {
        assert_eq!(ee.u32_at(r + 0x1f8), o, "bike {k}: the record's frame collidable");
        assert_eq!(ee.u32_at(r + 0x104) as usize, game_sim::rider_contact::CAPACITY, "bike {k}: its record's capacity");
    }
    let (mut entries, mut hit) = (0, 0);
    for (pose, &(moved, offset)) in B9_POSES.iter().enumerate() {
        let Some(mut ee) = image() else { return };
        let mut plane = plane.clone();
        let rec = ee.u32_at(PLANE + PLANE_COLLISION);
        plane.body = body_of(&ee, ee.u32_at(rec + 0xf4));
        let dt = ee.f32_at(addr("frame_dt").unwrap());
        let p = plane.body.position;
        let at = Vf::new(p.x() + offset[0], p.y() + offset[1], p.z() + offset[2], 1.0);
        let o = found[moved];
        for (l, x) in at.0.iter().enumerate() {
            ee.set_f32(o + at::TRANSFORM + 0x30 + 4 * l as u32, *x);
        }
        // Its box's centre (`+0x40`) rebuilt; the half extent goes to spare stack (the gather reads
        // the radius).
        let spare = (ee.stack - 0x8000) & !0xf;
        ee.call(addr("mesh_collidable_bounds").unwrap(), &[u64::from(o), u64::from(o + at::CENTRE), u64::from(spare)], &[], 1_000_000);
        // The plane's nearby search refilled where it stands, as its post does every tick (S99), so
        // the moved bike's distance is fresh.
        ee.call(addr("plane_refresh_search").unwrap(), &[u64::from(PLANE + 0x50)], &[], 50_000_000);

        let bikes: Vec<Bike> = found.iter().map(|&o| Bike { transform: matrix(&ee, o + at::TRANSFORM), in_world: true }).collect();
        let sim = game_sim::Sim::new(0x5354);
        let mut met = game_sim::flight::Met {
            colliders: vec![game_sim::rider_contact::collider(&sim.phy.collisions, sim.phy.mass.mass); bikes.len()],
            vehicles: recs.iter().map(|&r| vehicle_at(&ee, r)).collect(),
            bikes: (0..bikes.len()).collect(),
            named: Some(bikes.len()),
            ..Default::default()
        };
        let mut ground = Flat { y: F(-1.0e6), ..Default::default() };
        plane.search_alone(F(dt), &mut ground, &mesh, &bikes, Some(&mut met));

        // the original: the plane's and the bikes' lists emptied, then the plane's collide
        let mut named = recs.clone();
        named.push(rec);
        for &r in &named {
            ee.call(ee.u32_at(ee.u32_at(r + 8) + 0x74), &[u64::from(r)], &[], 1_000_000);
        }
        ee.call(addr("collision_record_collide").unwrap(), &[u64::from(rec)], &[dt], 50_000_000);
        let all = list_at(&ee, rec, &named);
        assert!(all.len() < plane.record.capacity, "pose {pose}: the plane's record filled");
        let orig: Vec<_> = all.iter().filter(|c| c.other.is_some()).cloned().collect();
        assert!(orig.iter().all(|c| c.kind == 0 && c.other == Some(moved)), "pose {pose}: the plane's entries name bike {moved}'s record: {orig:?}");
        let port: Vec<_> = plane.record.contacts.iter().filter(|c| c.other.is_some()).collect();
        assert_eq!(port.len(), orig.len(), "pose {pose}: the plane's entries on the bike\nport {port:?}\noriginal {orig:?}");
        for (k, (a, b)) in port.iter().zip(&orig).enumerate() {
            assert!(same_entry(a, b), "pose {pose}: plane entry {k}\nport {a:?}\noriginal {b:?}");
        }
        let pushed: Vec<bool> = (0..bikes.len()).map(|k| k == moved && !orig.is_empty()).collect();
        assert_eq!(met.pushed, pushed, "pose {pose}: the bikes the plane's entries name");
        for (k, &r) in recs.iter().enumerate() {
            let orig = list_at(&ee, r, &named);
            assert_eq!(met.struck[k].len(), orig.len(), "pose {pose}: bike {k}'s mirrors\nport {:?}\noriginal {orig:?}", met.struck[k]);
            for (j, (a, b)) in met.struck[k].iter().zip(&orig).enumerate() {
                assert!(same_entry(a, b), "pose {pose}: bike {k} mirror {j}\nport {a:?}\noriginal {b:?}");
            }
        }
        entries += orig.len();
        hit += usize::from(!orig.is_empty());
    }
    // pose 1 (four feet under the wheels) is the one the plane's rays miss
    assert_eq!((hit, entries), (7, 36), "the poses the plane's rays struck the bike in, and its entries");
}

/// The bike moved and its origin's offset from the plane's body, per pose: under the wheels, into
/// the nose, the tail and either wing, each of the three bikes.
const B9_POSES: [(usize, [f32; 3]); 8] = [
    (0, [0.0, -2.0, 0.0]),
    (0, [0.0, -4.0, 0.0]),
    (1, [0.0, 0.0, 4.0]),
    (1, [0.0, -1.0, -6.0]),
    (2, [5.0, -1.0, 0.0]),
    (2, [-5.0, -1.0, 0.0]),
    (0, [2.0, -3.0, 3.0]),
    (1, [-1.5, -2.5, -2.0]),
];

/// [`B9_POSES`]' pose `pose` on a fresh image: the bike moved into the plane, its box's centre rebuilt
/// and the plane's search refilled, as [`the_one_player_planes_rays_name_the_states_bikes`] sets it.
fn posed(pose: usize, found: &[u32]) -> Option<Ee> {
    let (moved, offset) = B9_POSES[pose];
    let mut ee = image()?;
    let body = body_of(&ee, ee.u32_at(ee.u32_at(PLANE + PLANE_COLLISION) + 0xf4));
    let p = body.position;
    let at = Vf::new(p.x() + offset[0], p.y() + offset[1], p.z() + offset[2], 1.0);
    let o = found[moved];
    for (l, x) in at.0.iter().enumerate() {
        ee.set_f32(o + at::TRANSFORM + 0x30 + 4 * l as u32, *x);
    }
    let spare = (ee.stack - 0x8000) & !0xf;
    ee.call(addr("mesh_collidable_bounds").unwrap(), &[u64::from(o), u64::from(o + at::CENTRE), u64::from(spare)], &[], 1_000_000);
    ee.call(addr("plane_refresh_search").unwrap(), &[u64::from(PLANE + 0x50)], &[], 50_000_000);
    Some(ee)
}

/// **B9a: a bike's list is the plane's mirrors, then its own**. The original's collision node empties
/// every record's list, then searches the plane's record before the bikes' (S99's clock): the bike
/// moved into the plane resolves, in the bike world's step after the plane's post, a list of the
/// plane's kind-1 mirrors first and its own search's entries after, up to its record's capacity.
/// At every pose in [`B9_POSES`], the moved bike's list after the plane's collide and then its own
/// is, entry for entry and bit for bit, [`game_sim::rider_contact::struck_first`] of the mirrors the
/// plane's collide alone left on it and the list the bike's collide alone files on a second image —
/// what `Sim::step`'s plane lane hands each AI rider's frame. The state's own collision node collides
/// the plane's record first and the AI riders' bikes after it.
#[test]
fn a_bike_the_plane_struck_lists_its_mirrors_ahead_of_its_own() {
    let Some(ee) = image() else { return };
    let found = bikes_in(&ee);
    let recs: Vec<u32> = found.iter().map(|&o| ee.u32_at(o + 0x80)).collect();
    let rec = ee.u32_at(PLANE + PLANE_COLLISION);
    let mut named = recs.clone();
    named.push(rec);
    // The plane's collision node (its dynamics world at +0x78 is S99's): count +0x30,
    // records +0x38, collided in that order. The plane's record leads, the AI riders' bikes follow.
    let node: Vec<u32> = (0..ee.u32_at(NODE + 0x30)).map(|k| ee.u32_at(ee.u32_at(NODE + 0x38) + 4 * k)).collect();
    assert_eq!(node.first(), Some(&rec), "the node's first record is the plane's: {node:x?}");
    for &r in &recs[1..] {
        assert!(node.contains(&r), "bike record {r:#x} is not on the plane's node: {node:x?}");
    }
    let dt = ee.f32_at(addr("frame_dt").unwrap());
    let empty_all = |ee: &mut Ee| {
        for &r in &named {
            ee.call(ee.u32_at(ee.u32_at(r + 8) + 0x74), &[u64::from(r)], &[], 1_000_000);
        }
    };
    let collide = |ee: &mut Ee, r: u32| {
        ee.call(addr("collision_record_collide").unwrap(), &[u64::from(r)], &[dt], 50_000_000);
    };
    let (mut struck, mut both, mut full) = (0, 0, 0);
    for pose in 0..B9_POSES.len() {
        let moved = B9_POSES[pose].0;
        let r = recs[moved];
        // the tick: lists emptied, the plane's collide, then the bike's
        let Some(mut ee) = posed(pose, &found) else { return };
        empty_all(&mut ee);
        collide(&mut ee, rec);
        let mirrors = list_at(&ee, r, &named);
        assert!(mirrors.iter().all(|c| c.kind == 1 && c.other == Some(recs.len())), "pose {pose}: the plane's mirrors on bike {moved}: {mirrors:?}");
        collide(&mut ee, r);
        let all = list_at(&ee, r, &named);
        // the bike's collide alone, on the same pose
        let Some(mut alone) = posed(pose, &found) else { return };
        empty_all(&mut alone);
        collide(&mut alone, r);
        let own = list_at(&alone, r, &named);
        let port = game_sim::rider_contact::struck_first(std::slice::from_ref(&mirrors), own.clone(), Vf::ZERO, &Flat::default());
        assert_eq!(port.len(), all.len(), "pose {pose}: bike {moved}'s list\nport {port:?}\noriginal {all:?}");
        for (k, (a, b)) in port.iter().zip(&all).enumerate() {
            assert!(same_entry(a, b), "pose {pose}: bike {moved} entry {k}\nport {a:?}\noriginal {b:?}");
        }
        struck += usize::from(!mirrors.is_empty());
        both += usize::from(!mirrors.is_empty() && !own.is_empty());
        full += usize::from(mirrors.len() + own.len() > game_sim::rider_contact::CAPACITY);
        eprintln!("B9a pose {pose}: {} mirrors, {} own, {} listed", mirrors.len(), own.len(), all.len());
    }
    eprintln!("B9a: {struck} poses struck, {both} with the bike's own entries too, {full} past the record's room");
    // The bike's own rays stand where its chassis was (only its collidable moved), and strike
    // nothing there: its own list is empty at every pose. Pose 7's mirrors fill its record.
    assert_eq!((struck, both, full), (7, 0, 0), "the poses the plane struck the bike in");
}
