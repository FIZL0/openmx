//! P162: where the bike's roost is born. The rear tire's dirt spray (P74's `Rear Tire Dirt Spray`,
//! on `RearTire_Particle01`) must leave the rear tire where the original's does, and be drawn there.
//!
//! The original's point is P74's capture (`context/capture/p74/run.csv`, S109's animandpump played
//! back): at the launch the rear spray switches on at frame 2006 and first emits at 2007, and its
//! emitter there, taken into the bike's own frame by the row's chassis matrix, is [`ORIGINAL`] at a
//! rear compression of [`COMPRESSION`]. The port's node is posed by the chassis drives at that same
//! compression, placed by the ride's placement, and the manager is stepped over it across a launch:
//! the first particles are born on the node's path over the tick, no further off it than the
//! `.emt`'s own extent, and the node is within [`WITHIN`] of the original's point.
//!
//! The picture is blended between the last two ticks (P52) and the particles with it: a particle
//! born on the last tick is drawn on the blended node, so the roost leaves the drawn tire.
//!
//! Needs the disc; the capture's cross-check needs `context/capture/p74/`. Without them the tests
//! say so and pass.

#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;

use std::collections::HashMap;
use std::path::PathBuf;

use game_render::biplane;
use game_render::disc::{Disc, Res};
use game_render::ini;
use game_render::mat::{M4, V3, v3};
use game_render::scene::Vehicle;
use game_render::vehicle::{Chassis, Drive, REAR, params};
use game_sim::trick::{Set as Stunts24, Stunts};
use game_sim::{phy::Phy, rider};
use ps2_float::Vf;
use particles::{Atlas, PlaneFrame, PlaneFx, TireFrame};

const TRACK: &str = "free09";
/// The capture's bike has two exhaust emitters, as the 250's pipe does (the 125's has one).
const BIKE: &str = "250sx";
const NODE: &str = "RearTire_Particle01";
const ROOST: &str = "Rear Tire Dirt Spray";

/// The original's rear spray at its first emitting frame of the launch (2007), in the bike's frame
/// (x right, y up, z forward, feet), and the rear compression on that row.
const ORIGINAL: V3 = V3 { x: -0.001, y: 0.609, z: -3.184 };
const COMPRESSION: f32 = 0.4711;
/// How close the port's node must be to [`ORIGINAL`], feet.
const WITHIN: f32 = 0.05;

fn context() -> PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context"),
    }
}

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            eprintln!("skipped: {why}");
            None
        }
    }
}

/// The shortest member of the disc whose name ends this way.
fn member(disc: &Disc, end: &str) -> String {
    let end = end.to_ascii_lowercase();
    disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().ends_with(&end)).min_by_key(|n| n.len()).unwrap()
}

/// free09's bike particles, as the shell loads them.
fn fx(disc: &mut Disc) -> PlaneFx {
    let archives = [disc.res(&member(disc, &format!("{TRACK}.res"))).unwrap(), disc.res(&member(disc, &format!("{TRACK}_2.res"))).unwrap()];
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let texture = game_render::pst::Texture::parse(&read(&particles::sheet_texture(TRACK)).unwrap()).unwrap();
    let atlas = Atlas::parse(&ini::text(&read(&particles::sheet(TRACK)).unwrap()), texture.width, texture.height);
    let list = biplane::emitters(&ini::text(&read(particles::BIKE_PARTICLES).unwrap()));
    PlaneFx::new(&list, atlas, read)
}

fn bike_res(disc: &mut Disc) -> Res {
    let name = member(disc, &format!("{BIKE}.res"));
    disc.res(&name).unwrap()
}

/// The bike's chassis with its clips, and a rider standing on it at the origin facing +z, the
/// wheel mounts the chassis model's own as the shell hands them over.
fn chassis_and_rider(disc: &mut Disc) -> (Chassis, rider::Rider) {
    let app = disc.res(&member(disc, "appstart.res")).unwrap();
    let set = Stunts24::new(Stunts::parse(&app.read_suffix("rider\\anim\\stunts.ini").unwrap()).unwrap().selection(0));
    let tables = rider::Tables::build();
    let phy = Phy::read_bytes(&bike_res(disc).read_suffix(".phy").unwrap(), &tables.ride.curve);
    let sub = Vehicle::parse(&ini::text(&bike_res(disc).read_suffix(&format!("{BIKE}.sub")).unwrap()));
    let c = Chassis::load(vec![bike_res(disc)], &sub.mcf, "bike_", &set.stunts, params()).unwrap();
    let at = |name: &str| c.node_rest(name).map(|v| Vf::new(v.x, v.y, v.z, 1.0)).unwrap();
    let r = rider::Rider::new(&phy, (at(game_render::vehicle::FRONT), at(REAR)), Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), set);
    (c, r)
}

/// The chassis posed at a rear compression of `rear`, on the ground.
fn pose(c: &mut Chassis, r: &rider::Rider, rear: f32) {
    let drive = Drive { rear_compression: rear, on_ground: true, ..Drive::of(&r.ride) };
    c.frame(ps2_float::F(1.0 / 60.0), Chassis::want(&r.ride, &r.tricks), &Chassis::request(&r.ride, &r.tricks), drive);
}

/// A point of the world in placement `m`'s own frame (its rows the axes, orthonormal).
fn local(m: &M4, p: V3) -> V3 {
    let row = |i: usize| v3(m.0[i][0], m.0[i][1], m.0[i][2]);
    let d = p.sub(row(3));
    v3(d.dot(row(0)), d.dot(row(1)), d.dot(row(2)))
}

/// How far `p` is from the segment `a`–`b`.
fn off_segment(p: V3, a: V3, b: V3) -> f32 {
    let ab = b.sub(a);
    let t = if ab.dot(ab) > 0.0 { (p.sub(a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0) } else { 0.0 };
    p.sub(a.add(ab.scale(t))).len()
}

/// A launch on the posed chassis: the bike placed along +z at `speed` feet a second from frame to
/// frame, throttle open, the rear tire down and spinning. Returns the manager, each frame's node in
/// the world, and the bike's placement on each frame.
fn launch(fx: &mut PlaneFx, node: M4, frames: usize, speed: f32) -> (Vec<V3>, Vec<M4>) {
    let dt = 1.0 / 60.0;
    let mut points = Vec::new();
    let mut placements = Vec::new();
    for k in 0..frames {
        // A placement of its own: off the origin and turned, so a local-for-world mix-up shows.
        let heading = 0.6f32;
        let forward = v3(heading.sin(), 0.0, heading.cos());
        let at = v3(120.0, 30.0, -40.0).add(forward.scale(speed * dt * k as f32));
        let place = M4::placement(at, forward, v3(0.0, 1.0, 0.0));
        let m = node.mul(place);
        points.push(m.point(v3(0.0, 0.0, 0.0)));
        placements.push(place);
        // On a surface whose zone draws dirt: free09's first zone, the road, draws none.
        let rear = TireFrame { ground: true, spinning: true, compression: COMPRESSION, zone: 1, ..TireFrame::default() };
        let frame = PlaneFrame {
            nodes: fx.entries.iter().filter_map(|e| e.node.clone()).map(|n| (n, m)).collect(),
            throttle: 1.0,
            airspeed: speed,
            vel: forward.scale(speed),
            tires: vec![TireFrame { ground: true, zone: 1, ..TireFrame::default() }, rear],
            ..PlaneFrame::default()
        };
        fx.step(&frame, dt);
    }
    (points, placements)
}

/// The capture's rows, by column.
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

/// [`ORIGINAL`] is the capture's: the rear spray's emitter on the launch's first emitting row, in
/// that row's chassis frame — and the matrix is this row's, not the row before's, which is where the
/// original reads its node.
#[test]
fn the_originals_roost_point_is_the_captures() {
    let Some(rows) = capture() else { return };
    let num = |r: &HashMap<String, String>, k: &str| r[k].parse::<f32>().unwrap();
    let matrix = |r: &HashMap<String, String>| {
        let mut m = M4::IDENTITY;
        for i in 0..4 {
            for j in 0..4 {
                m.0[i][j] = num(r, &format!("bike_xform.m{i}{j}"));
            }
        }
        m
    };
    // The rear dirt spray is the capture's entry 9.
    let at = |r: &HashMap<String, String>| v3(num(r, "e9_at.x"), num(r, "e9_at.y"), num(r, "e9_at.z"));
    let k = rows.iter().position(|r| r["frame_counter"] == "2007").expect("the launch row");
    let (row, before) = (&rows[k], &rows[k - 1]);
    let got = local(&matrix(row), at(row));
    assert!(got.sub(ORIGINAL).len() < 2e-3, "the capture has {got:?}");
    assert!((num(row, "r_comp") - COMPRESSION).abs() < 1e-4);
    // The row before's matrix puts it a tick's travel forward of the tire: not the original's.
    let stale = local(&matrix(before), at(row));
    assert!(stale.sub(ORIGINAL).len() > 0.05, "{stale:?}");
}

/// The roost's first particles at a launch are born at the original's point under the rear wheel.
#[test]
fn the_roost_is_born_at_the_originals_point() {
    let Some(mut disc) = disc() else { return };
    let mut fx = fx(&mut disc);
    let (mut c, r) = chassis_and_rider(&mut disc);
    pose(&mut c, &r, COMPRESSION);
    let node = c.node(NODE).expect("the chassis has the rear spray's node");
    let at = node.point(v3(0.0, 0.0, 0.0));
    // Under the rear axle, and within WITHIN of where the original's is.
    let axle = c.node(REAR).unwrap().point(v3(0.0, 0.0, 0.0));
    assert!(at.y < axle.y - 0.5, "the node {at:?} is not under the axle {axle:?}");
    assert!(at.sub(ORIGINAL).len() < WITHIN, "the port's node {at:?}, the original's {ORIGINAL:?}");

    let roost = fx.entries.iter().position(|e| e.name == ROOST).unwrap();
    let extent = fx.entries[roost].emt.extent;
    let reach = v3(extent.0.x.abs().max(extent.1.x.abs()), extent.0.y.abs().max(extent.1.y.abs()), extent.0.z.abs().max(extent.1.z.abs())).len();
    // On at the first frame, emitting from the second: its first particles are the second frame's.
    let (points, placements) = launch(&mut fx, node, 2, 6.0);
    let born: Vec<V3> = fx.particles.iter().filter(|p| p.emitter == roost).map(|p| p.at).collect();
    assert!(!born.is_empty(), "the roost emitted nothing on its second frame");
    for p in &born {
        // On the node's path over the tick, the .emt's extent off it at most…
        let off = off_segment(*p, points[0], points[1]);
        assert!(off <= reach + 1e-4, "a particle born {off} ft off the node's path (extent {reach})");
        // …which in the bike's frame is at the original's point.
        let there = local(&placements[1], *p);
        assert!(there.sub(ORIGINAL).len() < WITHIN + reach + 6.0 / 60.0, "born at {there:?}");
    }
    eprintln!("{} roost particles, node {:.3} ft from the original's, extent {reach:.3}", born.len(), at.sub(ORIGINAL).len());
}

/// Drawn between two ticks, a particle born on the last one sits on the node blended the same way:
/// at the tick before's node with the picture there, and at its birth point on the last tick.
#[test]
fn the_roost_is_drawn_off_the_blended_tire() {
    let Some(mut disc) = disc() else { return };
    let mut fx = fx(&mut disc);
    let (mut c, r) = chassis_and_rider(&mut disc);
    pose(&mut c, &r, COMPRESSION);
    let node = c.node(NODE).unwrap();
    let roost = fx.entries.iter().position(|e| e.name == ROOST).unwrap();
    let (points, _) = launch(&mut fx, node, 2, 30.0);
    let travel = points[1].sub(points[0]);
    assert!(travel.len() > 0.4);
    let made: Vec<usize> = fx.particles.iter().enumerate().filter(|(_, p)| p.emitter == roost).map(|(i, _)| i).collect();
    assert!(!made.is_empty());
    for by in [0.0f32, 0.25, 0.5, 1.0] {
        let drawn = fx.drawn_at(by);
        for &i in &made {
            // As far behind its birth point as the blended node is behind the last tick's.
            let want = fx.particles[i].at.sub(travel.scale(1.0 - by));
            assert!(drawn[i].at.sub(want).len() < 1e-4, "at {by}: {:?} for {want:?}", drawn[i].at);
        }
    }
    // A tick on, it is drawn from where it was born to where it went, with no jump at the tick.
    let before: Vec<V3> = made.iter().map(|&i| fx.drawn_at(1.0)[i].at).collect();
    launch(&mut fx, node, 1, 30.0);
    let after = fx.drawn_at(0.0);
    for (k, &i) in made.iter().enumerate() {
        assert!(after[i].at.sub(before[k]).len() < 1e-4);
    }
}
