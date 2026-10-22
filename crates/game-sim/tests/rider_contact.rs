//! S78 — rider against rider.
//!
//! - `the_collider_is_the_originals`: on S93's split-screen RAM images, both riding bikes' collision
//!   objects (the vehicle's `+0x7d0`) against [`rider_contact::collider`] off the disc's
//!   `race125p.phy`, which both players ride: every tuning word the resolution reads, bit for bit,
//!   and the chassis mass the springs are scaled by.
//! - the rest drive the port: two riders side by side push apart where riders without the files pass
//!   through; the push is a pure function of the state (a clone, an encode and a decode ride on the
//!   same); a ghost is pushed by nobody and pushes nobody; one rider with the files hashes as one
//!   without; and online seats, one Sim each, find the same contacts.

use game_render::disc::Disc;
use game_sim::bike_col::{self, BikeCol};
use game_sim::phy::Phy;
use game_sim::ride::{Controls, Tables};
use game_sim::rider::Rider;
use game_sim::rider_contact;
use game_sim::trick::Set;
use game_sim::world::Flat;
use game_sim::{Sim, snapshot};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::path::PathBuf;
use std::sync::Arc;

const IMAGES: [&str; 3] = ["splitscreen", "s93-short-5997", "s93-long-3402"];
/// The two players' vehicles, the same in every image.
const PLAYERS: [u32; 2] = [0x00c1_f810, 0x012e_3ad0];
/// The vehicle's collision object.
const RECORD: u32 = 0x7d0;
/// The chassis' mass (`+0x210`).
const CHASSIS_MASS: u32 = 0x210;

/// The collision object's fields (see tests/contact.rs for the resolution's own reading of them).
mod rec {
    pub const DAMP: u32 = 0x50;
    pub const DAMP_LINEAR: u32 = 0x58;
    pub const DAMP_ANGULAR: u32 = 0x60;
    pub const MASS: u32 = 0x64;
    pub const INV_MASS: u32 = 0x68;
    pub const INV_INERTIA: u32 = 0x80;
    pub const COFG: u32 = 0x90;
    pub const CHASSIS: u32 = 0xf0;
    pub const BODY: u32 = 0xf4;
    pub const CAPACITY: u32 = 0x104;
    pub const ACTORS: u32 = 0x110;
    pub const COLLIDES: u32 = 0x114;
    pub const MAX_DEPTH: u32 = 0x118;
    pub const SCENERY: u32 = 0x124;
    pub const ACTOR: u32 = 0x13c;
    pub const ANGULAR: u32 = 0x148;
    pub const ACTOR_ANGULAR: u32 = 0x14c;
    pub const SPRING_MASK: u32 = 0x150;
    pub const FRICTION_MASK: u32 = 0x160;
    pub const ACTOR_SPRING_MASK: u32 = 0x170;
    pub const ACTOR_FRICTION_MASK: u32 = 0x180;
    pub const FRICTION: u32 = 0x190;
    pub const ACTOR_FRICTION: u32 = 0x194;
    pub const SURFACES: u32 = 0x198;
    pub const FRAME: u32 = 0x1f8;
    /// The frame collidable's category.
    pub const CATEGORY: u32 = 0x84;
}

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    path.exists().then_some(path)
}

/// One of S93's images, from `states/` or `states/s93/`.
fn image(name: &str) -> Option<Ee> {
    let Some(path) = ctx(&format!("states/s93/{name}.ram")).or_else(|| ctx(&format!("states/{name}.ram"))) else {
        skip(&format!("{name}.ram is not there: S93's split-screen RAM images (context/states, or states/s93)"));
        return None;
    };
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// The disc's `race125p.phy` and the bike's two collision files.
fn disc(tables: &Tables) -> Option<(Phy, BikeCol)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
        .expect("the bike's collision files");
    let bikes: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let phy = bikes
        .iter()
        .find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok())
        .expect("race125p.phy in a bike archive");
    Some((Phy::read_bytes(&phy, &tables.curve), col))
}

fn quad(ee: &Ee, at: u32) -> [u32; 4] {
    [0, 4, 8, 12].map(|k| ee.u32_at(at + k))
}

fn bits(v: Vf) -> [u32; 4] {
    v.to_bits()
}

#[test]
fn the_collider_is_the_originals() {
    let tables = Tables::build();
    let Some((phy, _)) = disc(&tables) else { return };
    let mut seen = 0;
    for name in IMAGES {
        let Some(ee) = image(name) else { continue };
        for (player, v) in PLAYERS.into_iter().enumerate() {
            let at = |field: u32| ee.u32_at(ee.u32_at(v + RECORD) + field);
            let r = ee.u32_at(v + RECORD);
            let chassis = at(rec::CHASSIS);
            let who = format!("{name} P{}", player + 1);
            assert_eq!(at(rec::BODY), 0, "{who}: the object is on a chassis, not a body");
            assert_eq!(at(rec::CAPACITY) as usize, rider_contact::CAPACITY, "{who}: the list's capacity");
            let mass = ee.f32_at(chassis + CHASSIS_MASS);
            assert_eq!(mass.to_bits(), phy.mass.mass.0.to_bits(), "{who}: the chassis' mass is the file's");
            let c = rider_contact::collider(&phy.collisions, phy.mass.mass);
            let f = |field: u32| at(field);
            assert_eq!(f(rec::DAMP) != 0, c.damp, "{who}: damp");
            assert_eq!(f(rec::DAMP_LINEAR), c.damp_linear.0.to_bits(), "{who}: damp linear");
            assert_eq!(f(rec::DAMP_ANGULAR), c.damp_angular.0.to_bits(), "{who}: damp angular");
            assert_eq!(f(rec::MASS), c.mass.0.to_bits(), "{who}: mass");
            assert_eq!(f(rec::INV_MASS), c.inv_mass.0.to_bits(), "{who}: inverse mass");
            assert_eq!(quad(&ee, r + rec::INV_INERTIA), bits(c.inv_inertia), "{who}: inverse inertia");
            assert_eq!(quad(&ee, r + rec::COFG), bits(c.cofg), "{who}: centre of mass");
            assert_eq!(f(rec::ACTORS), c.actors, "{who}: actors");
            assert_eq!(f(rec::COLLIDES), c.collides_with, "{who}: collides with");
            assert_eq!(Some(ee.u32_at(f(rec::FRAME) + rec::CATEGORY)), c.collidable, "{who}: the frame's category");
            assert_eq!(f(rec::MAX_DEPTH), c.max_depth.0.to_bits(), "{who}: max depth");
            for (base, resp, what) in [(rec::SCENERY, c.scenery, "scenery"), (rec::ACTOR, c.actor, "actor")] {
                let got = [f(base), f(base + 4), f(base + 8)];
                let want = [resp.stiffness, resp.damping, resp.restitution].map(|x| x.0.to_bits());
                assert_eq!(got, want, "{who}: the {what} spring and restitution");
            }
            assert_eq!(f(rec::ANGULAR) != 0, c.scenery.angular, "{who}: angular");
            assert_eq!(f(rec::ACTOR_ANGULAR) != 0, c.actor.angular, "{who}: actor angular");
            for (field, want, what) in [
                (rec::SPRING_MASK, c.scenery.spring_mask, "spring mask"),
                (rec::FRICTION_MASK, c.scenery.friction_mask, "friction mask"),
                (rec::ACTOR_SPRING_MASK, c.actor.spring_mask, "actor spring mask"),
                (rec::ACTOR_FRICTION_MASK, c.actor.friction_mask, "actor friction mask"),
            ] {
                assert_eq!(quad(&ee, r + field), bits(want), "{who}: {what}");
            }
            assert_eq!(f(rec::FRICTION), c.scenery.friction.0.to_bits(), "{who}: friction");
            assert_eq!(f(rec::ACTOR_FRICTION), c.actor.friction.0.to_bits(), "{who}: actor friction");
            let surfaces: Vec<u32> = (0..16).map(|k| f(rec::SURFACES + 4 * k)).collect();
            let want: Vec<u32> = c.surfaces.iter().map(|s| s.0.to_bits()).collect();
            assert_eq!(surfaces, want, "{who}: the surfaces' friction");
            seen += 1;
        }
    }
    if seen == 0 {
        skip("no S93 image");
    }
}

// --- the port -------------------------------------------------------------------------------------

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// Side by side, close enough for each bike's bar-end rays to reach into the other's frame from
/// outside it. Closer, the rays start inside the other's frame, whose faces only stop a ray coming
/// in, and nothing is struck (a bike well inside another strikes nothing, in the original too).
const SIDE: f32 = 1.5;

/// Two riders on the stand-in bike with race125p's `[Collisions]`, `apart` feet along x, wrecks off;
/// with `col`, both carry the bike's collision files.
fn two(apart: f32, phy: &Phy, col: Option<&Arc<BikeCol>>) -> Sim {
    let mut sim = Sim::new(0x5378);
    Arc::make_mut(&mut sim.phy).collisions = phy.collisions;
    let mounts = game_sim::stock::mounts(&sim.phy);
    let s = game_sim::stock::SPAWN;
    let at = Vf::new(s.x() + apart, s.y(), s.z(), 1.0);
    let mut r = Rider::new(&sim.phy, mounts, at, game_sim::stock::FACING, Set::default());
    r.sense.wrecks_enabled = false;
    let p = (*sim.phy).clone();
    sim.add_player(p, r);
    if let Some(col) = col {
        sim.rider.carries_bike_col(Arc::clone(col));
        sim.others[0].rider.carries_bike_col(Arc::clone(col));
    }
    sim
}

fn ride(sim: &mut Sim, ticks: usize) -> Vec<u64> {
    let still = vec![Controls::default(); 2];
    (0..ticks)
        .map(|_| {
            sim.step_all(DT, &still, &mut Flat::default());
            sim.hash()
        })
        .collect()
}

fn gap(sim: &Sim) -> f32 {
    sim.rider_of(1).ride.position().x() - sim.rider_of(0).ride.position().x()
}

fn port() -> Option<(Phy, Arc<BikeCol>)> {
    let tables = Tables::build();
    disc(&tables).map(|(p, c)| (p, Arc::new(c)))
}

/// Side by side, each one's body rays strike the other's frame: both lists hold a
/// striker's contact and the mirror of the other's, and the two riders come apart, where two without
/// the files stay where they stood.
#[test]
fn side_by_side_riders_push_apart() {
    let Some((phy, col)) = port() else { return };
    let mut sim = two(SIDE, &phy, Some(&col));
    let lists = sim.bike_contacts(DT).expect("both carry the files");
    for (i, l) in lists.iter().enumerate() {
        assert!(l.iter().any(|c| c.kind == 0), "player {}: its rays struck nothing", i + 1);
        assert!(l.iter().any(|c| c.kind == 1), "player {}: nobody's rays struck it", i + 1);
        assert!(l.iter().all(|c| c.other == Some(1 - i)), "player {}: a contact not with the other", i + 1);
    }
    let mut apart = two(SIDE, &phy, None);
    assert_eq!(apart.bike_contacts(DT), None, "no files, no list");
    let (g0, g1) = (gap(&sim), gap(&apart));
    ride(&mut sim, 30);
    ride(&mut apart, 30);
    let (with, without) = (gap(&sim), gap(&apart));
    assert!((without - g1).abs() < 0.01, "without the files the gap moved: {g1} to {without}");
    assert!(with > g0 + 0.25, "with the files the gap went from {g0} to {with} only");
    // Far apart there is nothing to strike.
    let far = two(20.0, &phy, Some(&col));
    assert!(far.bike_contacts(DT).unwrap().iter().all(Vec::is_empty));
}

/// The push is a function of the state and nothing else: a clone ridden on, and a state encoded,
/// ridden past and decoded back, ride the same hashes the first run did (rollback, N1).
#[test]
fn the_push_rides_the_same_after_a_rollback() {
    let Some((phy, col)) = port() else { return };
    let mut sim = two(SIDE, &phy, Some(&col));
    let first = ride(&mut sim.clone(), 40);
    let head = ride(&mut sim, 10);
    assert_eq!(head, first[..10]);
    let saved = snapshot::encode(&sim);
    let fork = sim.clone();
    ride(&mut sim, 7);
    snapshot::decode(&mut sim, &saved).expect("the state decodes");
    assert_eq!(ride(&mut sim, 30), first[10..], "decoded and ridden again");
    assert_eq!(ride(&mut fork.clone(), 30), first[10..], "a clone ridden on");
}

/// A ghost is pushed by nobody and pushes nobody: either rider a ghost, both lists are empty and the
/// ride is the one without the files.
#[test]
fn a_ghost_is_pushed_by_no_one() {
    let Some((phy, col)) = port() else { return };
    for ghost in [0, 1] {
        let start = |sim: &mut Sim| {
            if ghost == 0 { sim.rider.ghost.start() } else { sim.others[0].rider.ghost.start() }
        };
        let mut with = two(SIDE, &phy, Some(&col));
        start(&mut with);
        assert!(
            with.bike_contacts(DT).unwrap().iter().all(Vec::is_empty),
            "player {} a ghost: a contact",
            ghost + 1
        );
        let mut without = two(SIDE, &phy, None);
        start(&mut without);
        assert_eq!(ride(&mut with, 20), ride(&mut without, 20), "player {} a ghost", ghost + 1);
    }
}

/// One rider with the files hashes as one without: there is nobody to strike.
#[test]
fn one_rider_rides_as_before() {
    let Some((_, col)) = port() else { return };
    let still = Controls::default();
    let mut with = Sim::new(7);
    with.rider.carries_bike_col(Arc::clone(&col));
    let mut without = Sim::new(7);
    for k in 0..30 {
        with.step(DT, &still, &mut Flat::default());
        without.step(DT, &still, &mut Flat::default());
        assert_eq!(with.hash(), without.hash(), "tick {k}");
    }
}

/// Online every seat is a Sim of its own: the same two riders as two one-player Sims find the same
/// lists the two-player Sim does, a seat nobody sits in is in none, and stepping each seat with its
/// own and writing the other back rides the two-player Sim's first tick.
#[test]
fn online_seats_find_the_same_contacts() {
    let Some((phy, col)) = port() else { return };
    let both = two(SIDE, &phy, Some(&col));
    let seat = |i: usize| {
        let mut s = Sim::new(0x5378);
        s.phy = Arc::new((*both.phy).clone());
        s.rider = both.rider_of(i).clone();
        s
    };
    let mut sims = vec![seat(0), seat(1)];
    let lists = rider_contact::among(&sims, &[true, true], DT).unwrap();
    assert_eq!(Some(lists.clone()), both.bike_contacts(DT));
    assert!(rider_contact::among(&sims, &[true, false], DT).unwrap().iter().all(Vec::is_empty));
    let mut both = both;
    ride(&mut both, 1);
    let still = Controls::default();
    for (i, l) in lists.into_iter().enumerate() {
        let mut bump = rider_contact::bump_among(&sims, i, l);
        sims[i].step_bumped(DT, &still, &mut Flat::default(), Some(&mut bump));
        rider_contact::bumped_among(&mut sims, &bump);
    }
    for i in 0..2 {
        assert_eq!(sims[i].rider.ride.chassis, both.rider_of(i).ride.chassis, "seat {}", i + 1);
    }
}

