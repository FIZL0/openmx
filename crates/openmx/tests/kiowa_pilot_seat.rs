//! P216b1: the Kiowa's pilot sits and leans where the original puts it.
//!
//! What the original does (research/journal/*-kiowa-pilot-seat):
//! - the helicopter's pilot request hands the body the rigid body's centre of mass and rotation as
//!   its base, and a pointer to `kiowa.phy`'s `PelvisBoxPosition` as its offset; the body puts its
//!   root at the base plus the turned offset. The model's origin is the centre of mass less the
//!   turned `CofG`, so in the model's space the root is `CofG + PelvisBoxPosition`, which is
//!   `game_render::kiowa::PILOT_SEAT`. The plane's measured seat is the same sum of its own file;
//! - the request writes the plane's two lean targets from the helicopter's pad, lateral for the
//!   aileron and longitudinal for the elevator, and never its holding flag nor its four limb
//!   targets, which keep the constructor's 0 and origin points: the clips are played unheld.
//!
//! Proved on the RAM of S148's and C14's flown helicopters (`context/states/s148-kiowa.ee.bin`,
//! `c14-kiowa.ee.bin`; each skips where the slot does not have it): the request's words, the pilot's
//! root node at the seat with the helicopter's rotation, and all 22 of the pilot's bones where the
//! port's unheld pose of the two clips puts them.

use game_render::disc::{Disc, Res};
use game_render::rider::{Body, Sex};
use game_sim::biplane_anim::Pilot;
use game_sim::trick::{Set as Stunts24, Stunts};
use game_sim::{phy::Phy, rider};
use ps2_float::F;
use ps2_oracle::{Kit, skip};

const STATES: [&str; 2] = ["s148-kiowa.ee.bin", "c14-kiowa.ee.bin"];
/// The MX helicopter in both states.
const HELI: usize = 0x007c_cae0;

/// The helicopter's words the proof reads.
mod at {
    /// The flown flag.
    pub const FLOWN: usize = 0x364;
    /// The model's root node; a node's world matrix is at its `+0x80` (rows x, y, z, position).
    pub const MODEL: usize = 0x18c;
    pub const NODE_WORLD: usize = 0x80;
    /// The pad as the input step leaves it: lateral, then longitudinal.
    pub const LATERAL: usize = 0x340;
    pub const LONGITUDINAL: usize = 0x344;
    /// The pilot request: its holding flag, its four limb targets (feet, then hands), the pointer to
    /// its offset, and the phase and mix targets.
    pub const HELD: usize = 0x55c;
    pub const LIMBS: usize = 0x600;
    pub const OFFSET: usize = 0x640;
    pub const PHASE: usize = 0x6c0;
    pub const MIX: usize = 0x6c4;
    /// `[Animation]` `PelvisBoxPosition` as the reader left it.
    pub const PELVIS_BOX: usize = 0xe60;
}

fn member(disc: &Disc, ends: &str) -> Option<String> {
    let want = ends.to_ascii_lowercase();
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
}

fn res(disc: &mut Disc, ends: &str) -> Res {
    let name = member(disc, ends).unwrap_or_else(|| panic!("the disc has no {ends}"));
    disc.res(&name).unwrap_or_else(|why| panic!("{name} does not open: {why}"))
}

fn u32_at(ram: &[u8], a: usize) -> u32 {
    u32::from_le_bytes(ram[a..a + 4].try_into().unwrap())
}

fn f32_at(ram: &[u8], a: usize) -> f32 {
    f32::from_bits(u32_at(ram, a))
}

/// Four rows of four floats at `a`.
fn matrix(ram: &[u8], a: usize) -> [[f64; 4]; 4] {
    std::array::from_fn(|r| std::array::from_fn(|c| f32_at(ram, a + 16 * r + 4 * c) as f64))
}

/// A world point in a matrix's own space (rows x, y, z, position; the rotation orthonormal).
fn local(m: &[[f64; 4]; 4], p: [f64; 3]) -> [f64; 3] {
    let d = [p[0] - m[3][0], p[1] - m[3][1], p[2] - m[3][2]];
    std::array::from_fn(|r| d[0] * m[r][0] + d[1] * m[r][1] + d[2] * m[r][2])
}

fn apart(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f64::max)
}

#[test]
fn the_seat_is_the_centre_of_mass_and_the_pelvis_box() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => return skip(&why),
    };
    let machines = res(&mut disc, "machine.res");
    let kiowa = game_sim::kiowa::Phy::read_bytes(&machines.read_suffix(game_sim::kiowa::KIOWA_PHY).expect("kiowa.phy"));
    let plane = game_sim::phy::Plane::read_bytes(&machines.read_suffix(game_sim::phy::BIPLANE_PHY).expect("biplane.phy"));
    let sum = |a: ps2_float::Vf, b: ps2_float::Vf| [a.x() + b.x(), a.y() + b.y(), a.z() + b.z()];
    let seat = game_render::kiowa::PILOT_SEAT;
    assert_eq!(sum(kiowa.mass_properties.cofg, kiowa.pelvis_box_position), [seat.x, seat.y, seat.z], "the Kiowa's seat");
    let seat = game_render::biplane::PILOT_SEAT;
    assert_eq!(sum(plane.mass_properties.cofg, plane.pelvis_box.position), [seat.x, seat.y, seat.z], "the plane's seat");
}

#[test]
fn the_original_seats_its_pilot_there_and_plays_the_clips_unheld() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => return skip(&why),
    };
    let app = res(&mut disc, "appstart.res");
    let table = app.read_suffix("rider\\anim\\stunts.ini").expect("the app archive has no stunt table");
    let set = Stunts24::new(Stunts::parse(&table).expect("the stunt table parses").selection(0));
    let tables = rider::Tables::build();
    let bike = res(&mut disc, "250sx.res");
    let p = Phy::read_bytes(&bike.read_suffix(".phy").expect("the bike has a .phy"), &tables.ride.curve);
    let mut body = Body::load(vec![res(&mut disc, "thq1.res")], "thq1", Sex::Male, "", &set.stunts, p.motion())
        .unwrap_or_else(|why| panic!("the body did not load: {why}"));
    let seat = game_render::kiowa::PILOT_SEAT;
    let seat = [seat.x as f64, seat.y as f64, seat.z as f64];

    let mut read = 0;
    for name in STATES {
        let Ok(ram) = std::fs::read(kit.ctx("states").join(name)) else {
            skip(&format!("no {name} in this slot's context/states"));
            continue;
        };
        read += 1;
        assert_eq!(u32_at(&ram, HELI + at::FLOWN), 1, "{name}: no flown helicopter");
        let model = matrix(&ram, (u32_at(&ram, HELI + at::MODEL) & 0x01ff_ffff) as usize + at::NODE_WORLD);

        // The request: the offset is the pelvis box, the limbs are left at the origin point, it is
        // never held, and its targets are the plane's formulas on the helicopter's pad.
        assert_eq!(u32_at(&ram, HELI + at::OFFSET) as usize, HELI + at::PELVIS_BOX, "{name}: the offset pointer");
        let pelvis: [f32; 4] = std::array::from_fn(|i| f32_at(&ram, HELI + at::PELVIS_BOX + 4 * i));
        assert_eq!(pelvis, [1.25, -2.0, 3.0, 1.0], "{name}: PelvisBoxPosition");
        for limb in 0..4 {
            let q: [f32; 4] = std::array::from_fn(|i| f32_at(&ram, HELI + at::LIMBS + 16 * limb + 4 * i));
            assert_eq!(q, [0.0, 0.0, 0.0, 1.0], "{name}: limb target {limb} was written");
        }
        assert_eq!(u32_at(&ram, HELI + at::HELD), 0, "{name}: the request is held");
        let pad = Pilot {
            elevator: F(f32_at(&ram, HELI + at::LONGITUDINAL)),
            aileron: F(f32_at(&ram, HELI + at::LATERAL)),
            ..Default::default()
        };
        let (phase, mix) = pad.axes();
        assert_eq!(phase.0.to_bits(), u32_at(&ram, HELI + at::PHASE), "{name}: the phase target");
        assert_eq!(mix.0.to_bits(), u32_at(&ram, HELI + at::MIX), "{name}: the mix target");

        // Every matrix in RAM with the helicopter's rotation and a point within 12 ft, in its space.
        let mut roots = Vec::new();
        let mut points = Vec::new();
        for a in (0..ram.len() - 64).step_by(16) {
            if f32_at(&ram, a + 60) != 1.0 || [12, 28, 44].iter().any(|o| f32_at(&ram, a + o) != 0.0) {
                continue;
            }
            let m = matrix(&ram, a);
            let at = local(&model, [m[3][0], m[3][1], m[3][2]]);
            if !at.iter().all(|v| v.is_finite() && v.abs() < 12.0) {
                continue;
            }
            points.push((a, at));
            let turned = (0..3).flat_map(|r| (0..3).map(move |c| (r, c))).map(|(r, c)| (m[r][c] - model[r][c]).abs()).fold(0.0, f64::max);
            if turned < 1e-4 && apart(at, seat) < 2e-3 {
                roots.push(a);
            }
        }
        assert!(!roots.is_empty(), "{name}: no node at the seat with the helicopter's rotation");

        // The pilot's bones, a run of 0x40 apart, where the port's unheld pose at the request's
        // targets puts them about the seat.
        let palette = game_render::biplane::pilot_palette(&mut body, &[None; 4], (phase.0, mix.0)).expect("the pilot's clips");
        let want: Vec<[f64; 3]> =
            palette.iter().map(|m| [seat[0] + m.0[3][0] as f64, seat[1] + m.0[3][1] as f64, seat[2] + m.0[3][2] as f64]).collect();
        let found = points.iter().find(|(a, _)| {
            want.iter().enumerate().all(|(k, w)| {
                points.binary_search_by_key(&(a + 0x40 * k), |(b, _)| *b).is_ok_and(|i| apart(points[i].1, *w) < 0.01)
            })
        });
        assert!(found.is_some(), "{name}: no run of {} bones where the unheld pose puts them", want.len());
        println!("{name}: root at the seat in {} matrices; the {} bones at {:#x}", roots.len(), want.len(), found.unwrap().0);
    }
    if read == 0 {
        skip("no Kiowa state in this slot");
    }
}
