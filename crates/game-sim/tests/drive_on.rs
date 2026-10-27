//! P145 — the material under the rear wheel, which the drive-on sound plays: a prop's surface byte
//! while the wheel rolls on it, 0 on the ground.
//!
//! No disc: a flat floor and a plank lying on it, the plank's mesh carrying a surface word whose low
//! byte is the material (the high bytes are the file's, and not the material's).

use game_sim::collide::{Bundle, Record};
use game_sim::phy::Phy;
use game_sim::ride::{Controls, Ride, Tables};
use game_sim::track::{Mesh, Node, Placement, Points, Triangle};
use game_sim::contact_search::Heightfield;
use game_sim::world::{Flat, Prop, Props, World};
use ps2_float::{F, Vf};

const BIKE: &str = include_str!("freeride-bike.phy.txt");
const DT: F = F(1.0 / 60.0);

/// A floor with props on it, handed to the wheels' search as the scene's.
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
    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
        (None, Some(&self.props))
    }
}

/// A plank 20 ft wide and `z1 − z0` long, its top `y` above the floor, of surface word `surface`.
fn plank(y: f32, z0: f32, z1: f32, surface: Option<u32>) -> Prop {
    let v = |x: f32, z: f32| [F(x), F(y), F(z)];
    let mesh = Mesh {
        version: 2,
        surface,
        transform: [
            [F::ONE, F::ZERO, F::ZERO],
            [F::ZERO, F::ONE, F::ZERO],
            [F::ZERO, F::ZERO, F::ONE],
            [F::ZERO, F::ZERO, F::ZERO],
        ],
        vertices: Points::Wide(vec![v(-10.0, z0), v(10.0, z0), v(10.0, z1), v(-10.0, z1)]),
        normals: None,
        nodes: vec![Node {
            volume: F::ZERO,
            centre: [F::ZERO, F(y), F((z0 + z1) / 2.0)],
            half_extent: [F(10.0), F(0.5), F((z1 - z0) / 2.0)],
            children: [0x8000, 0x8001],
        }],
        triangles: vec![
            // Wound so the face points up, at the wheel coming down onto it.
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 2, 1] },
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 3, 2] },
        ],
    };
    Prop::place(mesh, &Placement::default())
}

/// The rear wheel's material each tick of a bike dropped onto `yard` and thrown along +z.
fn materials(yard: &mut Yard, ticks: usize) -> Vec<(f32, u8)> {
    let tables = Tables::build();
    let p = Phy::read_bytes(BIKE.as_bytes(), &tables.curve);
    let mut ride = Ride::new(&p, game_sim::stock::MOUNTS, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0));
    ride.chassis.vel = Vf::new(0.0, 0.0, 25.0, 0.0);
    (0..ticks)
        .map(|_| {
            ride.frame(&p, &tables, &Controls::default(), DT, yard);
            (ride.position().z(), ride.rear_material)
        })
        .collect()
}

#[test]
fn the_rear_wheel_reads_the_planks_material_and_the_ground_gives_none() {
    let mut yard = Yard {
        floor: Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) },
        props: Props { props: vec![plank(0.5, -20.0, 25.0, Some(0x0001_0002))] },
    };
    let rode = materials(&mut yard, 180);
    let on: Vec<_> = rode.iter().filter(|(_, m)| *m != 0).collect();
    assert!(!on.is_empty(), "the rear wheel never read the plank: {rode:?}");
    assert!(on.iter().all(|(_, m)| *m == 2), "the material is the surface word's low byte: {on:?}");
    let (z, last) = *rode.last().unwrap();
    assert!(z > 40.0, "the bike never left the plank: z = {z}");
    assert_eq!(last, 0, "off the plank the ground gives no material");

    // A mesh too old to carry a surface word is material 0, as is bare ground all the way.
    let mut old = Yard { props: Props { props: vec![plank(0.5, -20.0, 25.0, None)] }, ..yard };
    assert!(materials(&mut old, 60).iter().all(|(_, m)| *m == 0));
}
