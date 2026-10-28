//! S131 — a track's knockable props in a ride: loaded into the simulation's state off the disc,
//! ticked with the track's cars, hashed and carried by a rollback snapshot.
//!
//! - **The load** against the first_recording state: free09's 62 collections, made by
//!   [`PropSet::load`] off the disc's `.toy`, `.scn`, `.pco` and `.psg` files alone, hold what the
//!   original's collections hold: the world's numbers, every object's loader fields and its edge
//!   object's bounds (the terrain's extent), and every body's numbers bit for bit (S80's checks, now
//!   through the ride's loader rather than the test's).
//! - **The tick in a ride**: a woken prop in a [`Sim`]'s traffic goes exactly as its collection
//!   ticked alone does (S130's, proved against the original), its swing moves the state's hash, and
//!   a snapshot taken mid-swing — copied or sent as bytes — carries on to the same hashes.
//!
//! Skipped with a line where the disc, the kit or the state is not there.

use std::path::PathBuf;

use game_render::disc::Disc;
use game_sim::biplane::Area;
use game_sim::phy::Ini;
use game_sim::prop_set::PropSet;
use game_sim::track::{Placement, Scene};
use game_sim::world::Flat;
use game_sim::{Sim, hash, snapshot};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";

/// The `.toy` actor's collection list in the state (tests/props.rs).
const LIST: u32 = 0x011b_43f4;

/// The collection's fields.
mod coll {
    pub const FLAGS: u32 = 0x20;
    pub const ACTIVE: u32 = 2;
    pub const ITERATIONS: u32 = 0x250;
    pub const RATE: u32 = 0x254;
    pub const BODIES: u32 = 0x320; // count, then the array's pointer at +8
    pub const COUNTDOWN: u32 = 0x338;
    pub const RESET_TIME: u32 = 0x33c;
}

/// The physics object's.
mod obj {
    pub const COLLISION: u32 = 0x54;
    pub const DETECT: u32 = 0x6c;
    pub const OFF_EDGE: u32 = 0x70;
    pub const PLACE: u32 = 0x90;
    pub const GRAVITY: u32 = 0xf0;
    pub const EDGE: u32 = 0x100;
    pub const BODY: u32 = 0x110;
    pub const COUNTDOWN: u32 = 0x1bc;
    pub const RESET_TIME: u32 = 0x1c0;
    pub const THRESHOLDS: u32 = 0x1c4;
}

/// The body's.
mod body_at {
    pub const ENABLED: u32 = 0x00;
    pub const INV_MASS: u32 = 0x10;
    pub const DAMPING: u32 = 0x20;
    pub const POSITION: u32 = 0x60;
    pub const INV_INERTIA: u32 = 0x120;
    pub const ANG_DAMPING: u32 = 0x130;
    pub const QUAT: u32 = 0x170;
}

/// The edge object's bounds.
const EDGE_MIN: u32 = 0x30;
const EDGE_MAX: u32 = 0x40;

/// free09's terrain: three root cells of 256 samples, nine feet apart, both ways.
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);

fn image() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/states").join(STATE);
    let path = if here.exists() { here } else { kit.ctx("states").join(STATE) };
    match Ee::from_ram(&path) {
        Ok(ee) => Some(ee),
        Err(why) => {
            skip(&format!("{why} — `./kit state ram context/recordings/first_recording.p2m2_SaveState.p2s` writes it"));
            None
        }
    }
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(a + k)))
}

fn bits(what: &str, port: Vf, game: Vf) {
    assert_eq!(port.to_bits(), game.to_bits(), "{what}: the original's {:?}, the port's {:?}", game.0, port.0);
}

/// free09's props, loaded the way the shell loads them.
fn free09() -> Option<(PropSet, Vec<String>)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let member = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\free09.res")).expect("free09.res").name.clone();
    let res = disc.res(&member).expect("free09's archive");
    let toy = Ini::parse_bytes(&res.read_suffix("free09.toy").expect("free09.toy"));
    let scene = Scene::parse(&res.read_suffix("free09.scn").expect("free09.scn")).expect("free09.scn parses");
    let (x, z, sx, sz) = FREE09;
    let area = Area::new(F(x), F(z), F(sx), F(sz));
    Some(PropSet::load(
        &toy,
        "Tracks\\Free09\\",
        &scene.models,
        &mut |p| res.read(p).ok(),
        &mut |p| res.read_suffix(p).ok(),
        Some(area),
    ))
}

#[test]
fn free09s_props_load_into_the_ride_as_the_original_made_them() {
    let Some(ee) = image() else { return };
    let Some((set, said)) = free09() else { return };
    assert!(said.is_empty(), "free09 loads whole: {said:?}");

    let mut game = Vec::new();
    let mut cell = ee.u32_at(LIST + 4);
    while cell != 0 {
        game.push(ee.u32_at(cell));
        cell = ee.u32_at(cell + 8);
    }
    assert_eq!(set.collections.len(), 62, "free09's collections");
    assert_eq!(set.collections.len(), game.len(), "the collection count");
    assert_eq!(set.models.iter().map(Vec::len).sum::<usize>(), 62, "one body a collection");

    let mut turned = 0;
    for (k, (port, &c)) in set.collections.iter().zip(&game).enumerate() {
        let at = |what: &str| format!("collection {k}: {what}");
        assert_eq!(port.world.iterations, ee.i32_at(c + coll::ITERATIONS), "{}", at("iterations"));
        assert_eq!(port.world.rate.to_bits(), ee.u32_at(c + coll::RATE), "{}", at("rate"));
        assert_eq!(port.active, ee.u32_at(c + coll::FLAGS) & coll::ACTIVE != 0, "{}", at("active"));
        assert_eq!(port.reset_time.to_bits(), ee.u32_at(c + coll::RESET_TIME), "{}", at("ResetTime"));
        assert_eq!(port.countdown.to_bits(), ee.u32_at(c + coll::COUNTDOWN), "{}", at("countdown"));
        assert!(port.listeners.is_empty(), "{}", at("nothing listening"));
        assert_eq!(port.objects.len() as u32, ee.u32_at(c + coll::BODIES), "{}", at("bodies"));
        for (j, ob) in port.objects.iter().enumerate() {
            let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8) + 4 * j as u32);
            let b = ee.u32_at(o + obj::BODY);
            let at = |what: &str| format!("collection {k} body {j}: {what}");
            bits(&at("gravity"), ob.gravity, quad(&ee, o + obj::GRAVITY));
            bits(&at("place"), ob.place, quad(&ee, o + obj::PLACE));
            assert_eq!(ob.collision_enabled, ee.u32_at(o + obj::COLLISION) != 0, "{}", at("CollisionEnabled"));
            assert_eq!(ob.detect_when_disabled, ee.u32_at(o + obj::DETECT) != 0, "{}", at("DetectCollisionsWhenDisabled"));
            assert_eq!(ob.reset_off_edge, ee.u32_at(o + obj::OFF_EDGE) != 0, "{}", at("ResetOffEdgeOfWorld"));
            assert_eq!(ob.reset_time.to_bits(), ee.u32_at(o + obj::RESET_TIME), "{}", at("ResetTime"));
            assert_eq!(ob.countdown.to_bits(), ee.u32_at(o + obj::COUNTDOWN), "{}", at("countdown"));
            for t in 0..6 {
                assert_eq!(ob.thresholds[t].to_bits(), ee.u32_at(o + obj::THRESHOLDS + 4 * t as u32), "{}", at("threshold"));
            }
            // The edge object: the terrain's extent, bit for bit, in both lanes the check reads.
            let edge = ee.u32_at(o + obj::EDGE);
            assert_ne!(edge, 0, "{}", at("an edge object"));
            let e = port.edge.expect("the area's bounds");
            let lane = |a: u32, l: u32| ee.u32_at(edge + a + 4 * l);
            assert_eq!(
                [e.min[0], e.min[1], e.max[0], e.max[1]].map(F::to_bits),
                [lane(EDGE_MIN, 0), lane(EDGE_MIN, 2), lane(EDGE_MAX, 0), lane(EDGE_MAX, 2)],
                "{}",
                at("the edge's bounds")
            );
            let body = &port.bones[j].body;
            assert_eq!(body.enabled, ee.u32_at(b + body_at::ENABLED) != 0, "{}", at("asleep"));
            assert_eq!(body.inv_mass.to_bits(), ee.u32_at(b + body_at::INV_MASS), "{}", at("inverse mass"));
            bits(&at("damping"), body.damping, quad(&ee, b + body_at::DAMPING));
            bits(&at("inverse inertia"), body.inv_inertia, quad(&ee, b + body_at::INV_INERTIA));
            bits(&at("angular damping"), body.ang_damping, quad(&ee, b + body_at::ANG_DAMPING));
            bits(&at("position"), body.position, quad(&ee, b + body_at::POSITION));
            // The turn by value: the state's nodes were posed by their world once (S80's note),
            // and a turned prop's frame has drifted an ulp or two off the scene's.
            let q = quad(&ee, b + body_at::QUAT);
            let off = body.quat.0.iter().zip(q.0).map(|(p, g)| (p - g).abs()).fold(0.0f32, f32::max);
            assert!(off < 1e-6, "{}: the original's {:?}, the port's {:?}", at("turn"), q.0, body.quat.0);
            if body.quat.to_bits() != q.to_bits() {
                turned += 1;
            }
        }
    }
    assert!(turned <= 12, "{turned} turns off by an ulp: S80 found the twelve rotated props drift at most");
}

/// A sign on a pin, as free09's `sign_.pco` makes it, stood at `(100, 0, 100)`: what the tick test
/// needs, off no disc.
const PCO: &str = "[PhysicsParts]
NumberOfPhysicsParts = 1
NumberOfConstraints = 1
NumberOfIterations = 2
SimulationHz = 60
[PhysicsPart1]
Type = RigidBody
Name = Sign
Weight = 20
GravityScale = 1
ResetTime = 5
SpeedThreshold = 0.5
AngularRateThreshold = 0.5
[Constraint1]
Type = Pin
Name = Sign
PinPosition = 0, 4, 0
BodyPinPosition = 0, 4, 0
ConstrainAngles = 1
MinAngles = -60, -5, -60
MaxAngles = 60, 5, 60
";

const TOY: &str = "[PhysicsCollections]
NumberOfPhysicsCollections = 1
[Collection1]
Name = Sign
";

/// A one-node `.psg` with a box of half extent `(1, 2, 0.1)` about `(0, 2, 0)`.
fn psg() -> Vec<u8> {
    let mut b = vec![0u8; 12 + 224];
    b[..4].copy_from_slice(b"psg\0");
    b[8..12].copy_from_slice(&1u32.to_le_bytes());
    for (k, v) in [0.0f32, 2.0, 0.0, 1.0, 2.0, 0.1].iter().enumerate() {
        let at = 12 + 192 + 4 * k;
        b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    b
}

fn sign() -> PropSet {
    let model = Placement { file: "Sign".into(), position: [F(100.0), F(0.0), F(100.0)], ..Placement::default() };
    let (set, said) = PropSet::load(
        &Ini::parse(TOY),
        "Tracks\\Test\\",
        &[model],
        &mut |p| (p == "Tracks\\Test\\Sign.pco").then(|| PCO.as_bytes().to_vec()),
        &mut |p| (p == "Sign.psg").then(psg),
        Some(Area::new(F(1.0), F(1.0), F(1.0), F(1.0))),
    );
    assert!(said.is_empty(), "{said:?}");
    assert_eq!(set.collections.len(), 1, "the sign's collection");
    set
}

#[test]
fn a_woken_prop_ticks_with_the_ride_and_rolls_back() {
    let dt = F(1.0 / 30.0);
    let mut sim = Sim::new(7);
    let mut world = Flat::default();
    let quiet = hash::of(&sim);
    sim.traffic.props = sign();
    assert_ne!(hash::of(&sim), quiet, "the props are in the walk");

    // At rest, nothing moves.
    let mut alone = sim.traffic.props.collections[0].clone();
    let rest = alone.clone();
    for _ in 0..10 {
        sim.step(dt, &Default::default(), &mut world);
    }
    assert_eq!(sim.traffic.props.collections[0], rest, "a prop nothing woke stays as it was made");

    // Woken and spun, as a strike would; the ride's tick is the collection's.
    sim.traffic.props.wake(0, 0);
    sim.traffic.props.collections[0].bones[0].body.rates = Vf::new(2.5, 0.0, -3.0, 1.0);
    alone.activate();
    alone.enable(0);
    alone.bones[0].body.rates = Vf::new(2.5, 0.0, -3.0, 1.0);
    let tables = game_sim::ragdoll::Tables::build();
    let mut hashes = Vec::new();
    let mut saved = None;
    let mut bytes = Vec::new();
    for t in 0..200 {
        if t == 20 {
            saved = Some(snapshot::save(&sim));
            bytes = snapshot::encode(&sim);
        }
        let before = hash::of(&sim);
        sim.step(dt, &Default::default(), &mut world);
        alone.tick(dt, &tables);
        assert_eq!(sim.traffic.props.collections[0], alone, "tick {t}: the ride's prop and the collection alone");
        if t < 5 {
            assert_ne!(hash::of(&sim), before, "tick {t}: a swinging prop moves the hash");
        }
        hashes.push(hash::of(&sim));
    }
    let c = &sim.traffic.props.collections[0];
    assert!(!c.active && c.listeners.is_empty(), "the sign slept, counted down and went back inside 200 ticks");
    assert_eq!(c.objects[0].node, rest.objects[0].node, "put back where it was made");

    // Rolled back to tick 20, by copy and by bytes: the same ticks after.
    let mut again = Sim::new(7);
    again.traffic.props = sign();
    snapshot::restore(&mut again, saved.as_ref().expect("saved"));
    let mut sent = Sim::new(7);
    sent.traffic.props = sign();
    snapshot::decode(&mut sent, &bytes).expect("the bytes decode");
    assert_eq!(hash::of(&sent), hash::of(&again), "sent and copied agree");
    for (t, &h) in hashes.iter().enumerate().skip(20) {
        again.step(dt, &Default::default(), &mut world);
        sent.step(dt, &Default::default(), &mut world);
        assert_eq!(hash::of(&again), h, "tick {t}: the copied snapshot");
        assert_eq!(hash::of(&sent), h, "tick {t}: the sent one");
    }

    // A machine without the props refuses the bytes rather than misreading them.
    let mut bare = Sim::new(7);
    assert!(snapshot::decode(&mut bare, &bytes).is_err(), "props in the state, none here");
}
