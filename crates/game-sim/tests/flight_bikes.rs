//! The other riders' bikes in the plane's world (S95), against biplane.p2m2's save state and the disc.
//!
//! 1. **the collidables** — the three bikes among the plane's candidates in the state are what
//!    [`flight_search::world_among`] builds from `bikef.col` on each one's own transform: the category,
//!    the flags, the reach, the radius and the centre bit for bit, one listener each, the struck
//!    listener, on a bike whose mask no plane's probe meets;
//! 2. **the listener** — the original's struck listener run on the state keeps the striker on the
//!    bike, and the port's keeps the striker's category;
//! 3. **the ride** — a plane flown level into a wall of bikes strikes one, does not crash on it and
//!    does not fly through; with the bikes out of the world it flies through.

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

