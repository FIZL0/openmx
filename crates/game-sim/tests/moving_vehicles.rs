//! **A bike that meets a moving vehicle is pushed or carried, not thrown at once** (S216).
//!
//! The riding bike's body rays strike the path cars where they stand (S137, `mover_contact`), and a
//! head ray whose end is inside a car with nothing struck — the train running into a bike beside the
//! line, every face a back face to it — came off as scenery struck. That fallback read the car's
//! root box alone, and free06's dump truck's box is 28 ft wide round a body that narrows under its
//! bed and between its wheels: a bike riding up beside it, its head inside the box and no face of the
//! truck near it, came off at once with nothing struck, which is what the user rode into on Dallas.
//!
//! Here each car is the disc's own collision mesh, driven straight along a flat yard at its path's
//! speed, as the traffic hands it to the bike's rays (its pose and its [`Moving`] each tick), and the
//! bike meets its side slowly: free06's dump truck passing a bike beside it, a train wagon's side, and
//! free09's trailer's. None throws the rider; each is pushed or carried. The train's own wrecks, its
//! nose into a bike on the rails and a rider found inside a car, are `crates/openmx/tests/train.rs`'s.

use game_render::disc::Disc;
use game_sim::bike_col::{self, BikeCol};
use game_sim::collide::{Bundle, Record};
use game_sim::contact_search::Heightfield;
use game_sim::math::Mat4;
use game_sim::phy::Phy;
use game_sim::ride::Controls;
use game_sim::rider::{Rider, Tables};
use game_sim::track::Mesh;
use game_sim::traffic::Moving;
use game_sim::trick::Set;
use game_sim::world::{Flat, Prop, Props, World};
use game_sim::Sim;
use ps2_float::{F, Vf};
use ps2_oracle::skip;
use std::sync::Arc;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);
/// Three seconds of riding beside or into the car.
const TICKS: usize = 90;
/// Ticks the bike stands braked before the car comes (`Rider`'s clutch ramp is a second and a half).
const SETTLE: usize = 60;

/// A flat floor with the one car on it, handed over as the shell's world hands its cars.
struct Yard {
    floor: Flat,
    props: Props,
}

impl World for Yard {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.floor.ground(at)
    }
    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.props.sweep(bundle, record)
    }
    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        Some(Box::new(self.floor))
    }
    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
        (Some(Box::new(self.floor)), Some(&self.props))
    }
    fn moving(&mut self) -> Option<&mut Props> {
        Some(&mut self.props)
    }
}

/// What the disc gives: the 125's numbers, the bike's collision files, and the named car meshes.
struct Parts {
    phy: Phy,
    col: Arc<BikeCol>,
    disc: Disc,
}

fn parts(tables: &Tables) -> Option<Parts> {
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
    Some(Parts { phy: Phy::read_bytes(&phy, &tables.ride.curve), col: Arc::new(col), disc })
}

impl Parts {
    /// A car's collision mesh, from the track's archive or its second one.
    fn mesh(&mut self, track: &str, file: &str) -> Mesh {
        let second = track.to_ascii_lowercase().replace(".res", "_2.res");
        let mut names = vec![track.to_string()];
        names.extend(self.disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase() == second));
        let bytes = names
            .iter()
            .find_map(|n| self.disc.res(n).ok()?.read_suffix(file).ok())
            .unwrap_or_else(|| panic!("{file} in {track}"));
        Mesh::parse(&bytes).unwrap_or_else(|e| panic!("{file}: {e:?}"))
    }
}

/// A car at `x, z` on the floor, facing `(dx, dz)` (a unit vector).
fn pose(x: f32, z: f32, (dx, dz): (f32, f32)) -> Mat4 {
    Mat4([Vf::new(dz, 0.0, -dx, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0), Vf::new(dx, 0.0, dz, 0.0), Vf::new(x, 0.0, z, 1.0)])
}

/// What a ride beside a car came to.
#[derive(Debug)]
struct Met {
    /// The first wreck, `(tick, reason)`.
    wreck: Option<(usize, u8)>,
    /// Ticks the bike's head ray's end was inside the car's root box: the fallback's question.
    in_box: usize,
    /// Ticks the bike's body rays struck the car.
    struck: usize,
    /// The bike's velocity at the end, and the car's.
    bike: [f32; 3],
    car: [f32; 3],
}

/// The rider on the 125 at the origin facing +z at `speed`, coasting, beside `mesh` driven along `dir`
/// from `at` at `car` ft/s, for [`TICKS`] ticks.
fn ride(tables: &Arc<Tables>, p: &Parts, mesh: Mesh, at: (f32, f32), dir: (f32, f32), car: f32, speed: f32) -> Met {
    let mounts = game_sim::stock::mounts(&p.phy);
    let mut r = Rider::new(&p.phy, mounts, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    r.carries_bike_col(Arc::clone(&p.col));
    let mut sim = Sim::with_rider(0x5216, Arc::clone(tables), p.phy.clone(), r);
    sim.traffic.first = Some(0);
    // The car far off while the fresh bike settles on its wheels, braked: a new rider's first second
    // and a half runs its clutch ramp (three times the torque), which would launch it.
    let mut yard = Yard { floor: Flat::default(), props: Props { props: vec![Prop::on(mesh, &pose(1000.0, 1000.0, dir))] } };
    let braked = Controls { brake: F::ONE, ..Controls::default() };
    for _ in 0..SETTLE {
        sim.step(DT, &braked, &mut yard);
    }
    assert!(sim.rider.state.wreck.is_none(), "the bike went down settling");
    sim.rider.ride.chassis.vel = Vf::new(0.0, 0.0, speed, 0.0);
    let head = p.col.rays.segments.iter().map(|s| s.to).max_by(|a, b| a.y().total_cmp(&b.y())).unwrap();
    let (mut wreck, mut in_box, mut struck) = (None, 0, 0);
    let mut before = pose(at.0, at.1, dir);
    for tick in 0..TICKS {
        let gone = car * DT.0 * (tick + 1) as f32;
        let after = pose(at.0 + dir.0 * gone, at.1 + dir.1 * gone, dir);
        let prop = &mut yard.props.props[0];
        prop.stand(&after);
        prop.moving = Some(Moving::of(&before, &after, F::ONE / DT));
        before = after;

        let end = game_sim::math::transform_point(&sim.rider.ride.transform(), head);
        let local = game_sim::math::transform_point(&prop.from_world, end);
        let root = &prop.mesh.nodes[0];
        if (0..3).all(|i| (local.0[i] - root.centre[i].0).abs() < root.half_extent[i].0) {
            in_box += 1;
        }
        if !sim.car_contacts(0, &mut yard, DT).is_empty() {
            struck += 1;
        }
        sim.step(DT, &Controls::default(), &mut yard);
        if let Some(w) = sim.rider.state.wreck {
            wreck = Some((tick, w.0));
            break;
        }
    }
    let v = sim.rider.ride.chassis.vel;
    Met { wreck, in_box, struck, bike: [v.x(), v.y(), v.z()], car: [dir.0 * car, 0.0, dir.1 * car] }
}

/// The car's root box half extents and centre, for the ride's numbers.
fn root(mesh: &Mesh) -> ([f32; 3], [f32; 3]) {
    let n = &mesh.nodes[0];
    (n.centre.map(|c| c.0), n.half_extent.map(|h| h.0))
}

/// Free06's dump truck crawling past a bike rolling along beside it, the bike's head inside the
/// truck's box and clear of its body: the bike rides on. Before S216 it came off on the first tick.
#[test]
fn a_bike_beside_a_moving_dump_truck_rides_on() {
    let tables = Arc::new(Tables::build());
    let Some(mut p) = parts(&tables) else { return };
    let mesh = p.mesh("Tracks\\free06.res", "dumptruck.col");
    let (c, h) = root(&mesh);
    println!("dumptruck root centre {c:?} half {h:?}");
    // The truck's centre line 13.8 ft to the bike's right, inside its 14.2 ft half width.
    let met = ride(&tables, &p, mesh, (13.8 - c[0], -c[2]), (0.0, 1.0), 6.0, 5.0);
    println!("beside the dump truck: {met:?}");
    assert!(met.in_box > 30, "the bike's head was in the truck's box for only {} ticks: not the case", met.in_box);
    assert_eq!(met.wreck, None, "a bike beside the dump truck came off");
}

/// A train wagon at the train's 54 ft/s meeting a bike that rolls into its side at 8 ft/s: pushed
/// and dragged, not thrown. S129's `across` capture is the original's: the bike meets the wagon's
/// side, is pushed back and rides off.
#[test]
fn a_bike_rolling_into_a_moving_wagons_side_is_pushed() {
    let tables = Arc::new(Tables::build());
    let Some(mut p) = parts(&tables) else { return };
    let mesh = p.mesh("Tracks\\free06.res", "train_car_03.col");
    let (c, h) = root(&mesh);
    println!("train_car_03 root centre {c:?} half {h:?}");
    // Its side 4 ft ahead of the bike, its middle coming past as the bike gets there.
    let met = ride(&tables, &p, mesh, (-10.0, h[0] + 4.0), (1.0, 0.0), 54.0, 8.0);
    println!("into the wagon: {met:?}");
    assert!(met.struck > 0, "the bike never met the wagon: {met:?}");
    assert_eq!(met.wreck, None, "a bike rolling into the wagon's side came off");
}

/// Free09's trailer passing at 30 ft/s, met side-on by a bike rolling at 8 ft/s.
#[test]
fn a_bike_rolling_into_a_moving_trailers_side_is_pushed() {
    let tables = Arc::new(Tables::build());
    let Some(mut p) = parts(&tables) else { return };
    let mesh = p.mesh("Tracks\\free09.res", "free_trailera.col");
    let (c, h) = root(&mesh);
    println!("free_trailera root centre {c:?} half {h:?}");
    let met = ride(&tables, &p, mesh, (-10.0, h[0] + 4.0 + c[0]), (1.0, 0.0), 30.0, 8.0);
    println!("into the trailer: {met:?}");
    assert!(met.struck > 0, "the bike never met the trailer: {met:?}");
    assert_eq!(met.wreck, None, "a bike rolling into the trailer's side came off");
}
