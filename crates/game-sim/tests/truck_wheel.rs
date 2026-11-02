//! S208 — a riding bike's wheels on a woken `HasPhysics` truck ([`truck_wheel`]).
//!
//! S168 and S208 proved what the original does against RAM (`tests/bike_col.rs`
//! `a_physics_trucks_mesh_is_what_a_wheel_could_own`, `a_woken_trucks_mesh_is_in_the_wheels_reach`):
//! the wake stands the car's path mesh down and its physics object's `f.col` up into the wheel's
//! search, owned by the object's body with no chassis. No capture of a wheel on a woken truck was
//! taken (the S207/S219 bikes are thrown off it, and part on the strike tick's push, S251), so these
//! drive the port: free06's company truck off the disc on a path over free06's terrain, its `f.col`
//! standing in as its path mesh, and race125p's bike dropped onto its roof once it is woken.
//!
//! 1. the wheel's contact names the truck's body ([`truck_wheel::handle`]) and no vehicle, so it is
//!    the `f.col`'s and not the path mesh's (which the wake stood down);
//! 2. what the wheel stands on is the body: the same drop onto the truck sliding leaves the bike going
//!    otherwise than onto it still;
//! 3. asleep (not struck), a wheel on the truck is on its path mesh: a vehicle and no owner;
//! 4. the physics objects and the path mesh are back where they were after the bike's frame.
//!
//! Skipped with a line where the disc is not there.

use std::sync::Arc;

use game_render::disc::Disc;
use game_sim::bike_col::{self, BikeCol};
use game_sim::phy::Phy;
use game_sim::physics_car::{self, PhysicsCar};
use game_sim::ride::{Controls, Tables};
use game_sim::rider::Rider;
use game_sim::track::{Mesh, Scene, Terrain};
use game_sim::traffic::{Moving, Spec, Traffic};
use game_sim::truck_wheel;
use game_sim::world::{Ground, Place, Props, World};
use game_sim::Sim;
use ps2_float::{F, Vf};
use ps2_oracle::skip;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// What the disc gives: free06's scene and terrain bytes, the truck's files, race125p's phy and the
/// bike's collision files.
struct Files {
    scene: Scene,
    trn: Vec<u8>,
    pco: Vec<u8>,
    psg: Vec<u8>,
    rays: Vec<u8>,
    shape: Vec<u8>,
    phy: Phy,
    col: Arc<BikeCol>,
}

fn files() -> Option<Files> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let parts: Vec<_> = ["Tracks\\free06.res", "Tracks\\free06_2.res"].iter().filter_map(|n| disc.res(n).ok()).collect();
    let read = |want: &str| parts.iter().find_map(|r| r.read_suffix(want).ok()).unwrap_or_else(|| panic!("{want} in free06"));
    let scene = Scene::parse(&read("free06.scn")).expect("free06's scene");
    let trn = read(&scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn"));
    let model = "ctruck.psg";
    let pco = read(&physics_car::pco_file(model));
    let psg = read(&game_sim::props::model_file("ctruck"));
    let rays = read(&game_sim::props::rays_file(model));
    let shape = read(&game_sim::props::shape_file(model));
    let split = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let bread = |name: String| split.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = BikeCol::load(&bread(bike_col::rays_file(bike_col::MODEL)), &bread(bike_col::shape_file(bike_col::MODEL)))
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
    let phy = bikes.iter().find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok()).expect("race125p.phy in a bike archive");
    Some(Files { scene, trn, pco, psg, rays, shape, phy: Phy::read_bytes(&phy, &Tables::build().curve), col: Arc::new(col) })
}

/// A one-player Sim with the truck on a straight path along +z from `at`, its `f.col` its path mesh,
/// race125p's `[Collisions]` and the bike's files, wrecks off; the truck's mesh attached to `props`.
fn sim(f: &Files, at: Vf, props: &mut Props) -> Sim {
    let points: Vec<(F, Vf)> = (0..3).map(|k| (F(10.0 * k as f32), Vf::new(at.x(), at.y(), at.z() + 300.0 * k as f32, 1.0))).collect();
    let mesh = Mesh::parse(&f.shape).expect("the f.col");
    let spec = Spec::new(&points, F(0.5), F(0.5), F(0.5), F::ZERO, Some(mesh)).expect("a path");
    let truck = PhysicsCar::load(&f.pco, &f.psg, Some(&f.rays), Some(&f.shape), None).expect("the truck");
    let mut sim = Sim::new(0x5208);
    Arc::make_mut(&mut sim.phy).collisions = f.phy.collisions;
    sim.rider.sense.wrecks_enabled = false;
    sim.rider.carries_bike_col(Arc::clone(&f.col));
    sim.traffic = Traffic::new(vec![spec]).with_physics(vec![Some(truck)]);
    sim.traffic.attach(props);
    sim
}

/// The owner and vehicle words a bike's two wheels' contacts left, front then rear.
fn words(r: &Rider) -> [(u32, u32); 2] {
    [&r.ride.front, &r.ride.rear].map(|w| (w.mv.fresh.scalars[2], w.mv.fresh.scalars[3]))
}

/// Two steps of a copy of `s` with its bike placed at `at`, the truck's mesh stood where its car is and
/// moving as `s`'s world left it (`moving`):
/// the bike's kept nearby search ([`game_sim::ride::Ride::nearby`]) is gathered where the first left
/// it, so the second's scene has the truck's path mesh in it.
fn drop(s: &Sim, at: Vf, world: &mut Place<'_>, moving: &[Option<Moving>]) -> Sim {
    let mut s = s.clone();
    s.traffic.place(world);
    for (p, m) in world.props.props.iter_mut().zip(moving) {
        p.moving = *m;
    }
    s.rider.place(at, facing());
    for _ in 0..2 {
        s.step(DT, &Controls::default(), world);
    }
    s
}

fn facing() -> Vf {
    Vf::new(0.0, 0.0, 1.0, 0.0)
}

#[test]
fn a_wheel_stands_on_a_woken_truck() {
    let Some(f) = files() else { return };
    let terrain = Terrain::parse(&f.trn).expect("free06's terrain");
    let mut world = Place { ground: Ground::new(&terrain, &f.scene.terrain), props: Props::default() };
    let area = world.area().expect("free06's extent");
    let (x, z) = (area.width.0 * 0.5, area.depth.0 * 0.5);
    let (ground, _) = world.ground(Vf::new(x, 0.0, z, 1.0));
    let start = Vf::new(x, ground.y(), z, 1.0);
    let mut base = sim(&f, start, &mut world.props);
    // The bike well off the truck's path: from the spawn its rays strike the car before its first update.
    let (park, _) = world.ground(Vf::new(x - 200.0, 0.0, z, 1.0));
    base.rider.place(Vf::new(park.x(), park.y() + 2.0, park.z(), 1.0), facing());
    for _ in 0..10 {
        base.step(DT, &Controls::default(), &mut world);
    }
    eprintln!("start {start:?}; before the strike: physical {:?}, car at {:?}", base.traffic.physics[0].as_ref().map(|p| p.physical), base.traffic.cars[0].pose.0[3]);
    let asleep = base.clone();
    let asleep_moved: Vec<Option<Moving>> = world.props.props.iter().map(|p| p.moving).collect();
    assert!(base.traffic.strike(0, DT, &mut world), "the strike wakes the truck");
    // Let it slide to rest on the ground: its body off, lying out its ResetTime.
    let truck = |s: &Sim| s.traffic.physics[0].clone().expect("the truck's physics");
    let mut rested = None;
    for t in 0..240 {
        base.step(DT, &Controls::default(), &mut world);
        if !truck(&base).body().enabled {
            rested = Some(t);
            break;
        }
    }
    eprintln!("the truck rested on tick {rested:?} after the strike");
    assert!(truck(&base).physical, "still physical, lying out its ResetTime");
    let top = truck(&base).pose().0[3];
    let moved: Vec<Option<Moving>> = world.props.props.iter().map(|p| p.moving).collect();
    let handle = truck_wheel::handle(0);

    // A spot over the roof where one step leaves a wheel's contact on the truck.
    let mut spot = None;
    'scan: for dy in (4..=48).map(|k| k as f32 * 0.25) {
        for dz in (-6..=6).map(|k| k as f32) {
            let at = Vf::new(top.x(), top.y() + dy, top.z() + dz, 1.0);
            if words(&drop(&base, at, &mut world, &moved).rider).iter().any(|&(o, _)| o == handle) {
                spot = Some(at);
                break 'scan;
            }
        }
    }
    let at = spot.expect("some spot over the woken truck where a wheel stands on it");
    eprintln!("the truck at {top:?}, the bike dropped at {at:?}");

    let on = drop(&base, at, &mut world, &moved);
    for (o, v) in words(&on.rider) {
        assert!(o == 0 || o == handle, "a wheel's owner is the truck's or nothing: {o:#x}");
        if o == handle {
            assert_eq!(v, 0, "a wheel on the woken truck's f.col names no vehicle: the path mesh is stood down");
        }
    }
    assert!(on.rider.ride.props.is_none(), "the trucks are back in the traffic");
    assert!(on.traffic.physics[0].as_ref().is_some_and(|p| p.physical), "the truck's physics object is back");
    let mesh = &world.props.props[on.traffic.span().expect("attached")[0]];
    assert!(!mesh.mesh.triangles.is_empty() && mesh.radius > F::ZERO, "the path mesh is stood back up after the frame");

    // What the wheel stands on is the body (S169's owner branch): the body lies off, so the world's
    // step leaves it where it is whatever its velocity, and only what the wheel reads of it moves.
    assert!(!truck(&base).body().enabled, "the truck lies at rest, its body off");
    let mut read = |v: Vf| {
        let mut s = base.clone();
        s.traffic.physics[0].as_mut().unwrap().set.bones[0].body.velocity = v;
        let s = drop(&s, at, &mut world, &moved);
        assert!(words(&s.rider).iter().any(|&(o, _)| o == handle), "a wheel on the truck going {v:?}");
        assert_eq!(truck(&s).pose(), truck(&base).pose(), "the truck did not move under the bike");
        s.rider.ride.chassis.vel
    };
    let (still, going) = (read(Vf::new(0.0, 0.0, 0.0, 0.0)), read(Vf::new(0.0, 0.0, 8.0, 0.0)));
    assert_ne!(still.to_bits(), going.to_bits(), "the truck's body reaches the bike: {still:?} both ways");

    // Asleep (never struck), a wheel on the truck stands on its path mesh: a vehicle and no owner, as
    // before S208, and nothing is lent.
    let car = asleep.traffic.cars[0].pose.0[3];
    let mut found = None;
    'asleep: for dy in (4..=48).map(|k| k as f32 * 0.25) {
        for dz in (-6..=6).map(|k| k as f32) {
            let s = drop(&asleep, Vf::new(car.x(), car.y() + dy, car.z() + dz, 1.0), &mut world, &asleep_moved);
            if !truck(&s).physical && words(&s.rider).iter().any(|&(_, v)| v != 0) {
                found = Some(s);
                break 'asleep;
            }
        }
    }
    let s = found.expect("some spot over the sleeping truck where a wheel stands on its path mesh");
    assert!(words(&s.rider).iter().all(|&(o, _)| o == 0), "asleep, no wheel has an owner: {:x?}", words(&s.rider));
    assert!(s.rider.ride.props.is_none() && s.traffic.physics[0].is_some(), "the physics objects stay in the traffic");
}
