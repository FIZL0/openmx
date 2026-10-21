//! S65 — what the ride's ragdoll reads of the original's: the settle gate's writer and the wreck
//! bike's sphere.
//!
//! The gate the ride machine latches its settle on (the vehicle's `+0xd3c`, S5/S26) is written by
//! the rider body actor: its tick ends by copying its touching flag (`+0x48c`) into the request it
//! shares with the bike (`[actor + 0x488] + 0x1bc`), and the request is the vehicle's `+0xb80`. The
//! off-bike test reads the collision object the request points at (`+0x1ac`): its centre at
//! `+0x40`, carried with the wreck bike, and its radius at `+0x60`, which is `wreckbikef.col`'s root
//! node.
//!
//! The captures and states are S56's (`tests/ragdoll_wrecks.rs` has how they were made).

use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::path::PathBuf;

/// The rider body actor in every s56 state.
const ACTOR: u32 = 0x017a_7700;
/// The actor's request pointer and touching flag.
const REQUEST: u32 = 0x488;
const TOUCHING: u32 = 0x48c;
/// The request's place in the vehicle, and the vehicle's settle gate.
const REQUEST_IN_BIKE: u32 = 0xb80;
const SETTLES: u32 = 0xd3c;
/// The request's collision object, and that object's centre and radius.
const COLLIDER: u32 = 0x1ac;
const CENTRE: u32 = 0x40;
const RADIUS: u32 = 0x60;
/// The request's frame: three rows and the translation.
const FRAME: u32 = 0x10;
/// The human's vehicle's class table in the s56 states, at the actor's `+0x48`.
const BIKE_VTABLE: u32 = 0x0045_4ef0;
/// The capture's `req_area` starts this far before the request.
const REQ_AREA: usize = 0x200;

const NAMES: [&str; 4] = ["front", "side", "back", "tip"];
const STATES: [&str; 10] = [
    "front-2600", "front-2601", "side-2604", "side-2605", "back-2475", "back-2476", "tip-2473", "tip-2474",
    "tip-2475", "tip-2476",
];

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there (S56's captures: research/s56_wreck.py)", path.display()));
        return None;
    }
    Some(path)
}

fn state(name: &str) -> Option<Ee> {
    let path = ctx(&format!("capture/s56/{name}.ee.bin"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(ACTOR + 0x2e0), 18, "{name}: no rider body actor at {ACTOR:#x}");
    Some(ee)
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn bits(v: Vf) -> [u32; 3] {
    [v.x().to_bits(), v.y().to_bits(), v.z().to_bits()]
}

/// The wreck bike's collision file off the disc, or `None` without one.
fn wreck_bike_col() -> Option<Vec<u8>> {
    use game_render::disc::Disc;
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let names: Vec<String> = disc
        .entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(".res"))
        .map(|e| e.name.clone())
        .collect();
    let col = names.iter().find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbikef.col").ok()));
    Some(col.expect("an archive carries wreckbikef.col"))
}

/// **The request is the bike's, and its touching word is the actor's** in every state: the
/// actor's request pointer is the human's vehicle plus `0xb80`, so the request's `+0x1bc` is the
/// vehicle's settle gate, and it holds the actor's touching flag.
#[test]
fn the_body_actors_request_is_the_bikes() {
    let mut seen = 0;
    for name in STATES {
        let Some(ee) = state(name) else { return };
        let request = ee.u32_at(ACTOR + REQUEST);
        let bike = request - REQUEST_IN_BIKE;
        assert_eq!(ee.u32_at(bike + 0x48), BIKE_VTABLE, "{name}: no vehicle at {bike:#x}");
        assert_eq!(ee.u32_at(bike + SETTLES), ee.u32_at(ACTOR + TOUCHING), "{name}: the gate is not the actor's flag");
        seen += 1;
    }
    assert_eq!(seen, STATES.len());
}

/// **The settle gate is the body's touching flag, on every tick of the four wrecks** (S65): the
/// gate in the captured request (`req_area`, at the vehicle's `+0xd3c`) against the actor's
/// `a_touching`, through the riding ticks before each entry, the wreck, and the rest after. It
/// comes on with the hands still holding: the gate is not "the hands off and on the ground".
#[test]
fn the_settle_gate_is_the_bodys_touching_flag() {
    let (mut rows, mut on, mut on_holding) = (0, 0, 0);
    for name in NAMES {
        let Some(path) = ctx(&format!("capture/s56/{name}.csv")) else { return };
        let text = std::fs::read_to_string(path).expect("read the capture");
        let mut lines = text.lines();
        let cols: Vec<&str> = lines.next().expect("a header").split(',').collect();
        let col = |c: &str| cols.iter().position(|&h| h == c).unwrap_or_else(|| panic!("no column {c}"));
        let (area, touching, hand_hold, frame) = (col("req_area"), col("a_touching"), col("a_hand_hold"), col("frame_counter"));
        let gate_at = REQ_AREA + (SETTLES - REQUEST_IN_BIKE) as usize;
        for line in lines {
            let r: Vec<&str> = line.split(',').collect();
            let hex = &r[area][2 * gate_at..2 * gate_at + 8];
            let b: Vec<u8> = (0..4).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex")).collect();
            let gate = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            let flag: u32 = r[touching].parse().expect("a flag");
            assert_eq!(gate, flag, "{name} at {}: the gate is not the actor's touching flag", r[frame]);
            rows += 1;
            if gate != 0 {
                on += 1;
                if r[hand_hold].parse::<f32>().expect("a clock") > 0.0 {
                    on_holding += 1;
                }
            }
        }
    }
    assert!(rows > 1500 && on > 500 && on_holding > 0, "{rows} rows, {on} with the gate on, {on_holding} of them holding");
}

/// **The wreck bike's sphere is its collision file's root node** (S65): the radius the collision
/// object holds, bit for bit, is the length of the root's half extent through the FPU, and the
/// centre it holds is the root's centre through the request's frame (the wreck bike's, written
/// that tick), translation first, in every state.
#[test]
fn the_bike_sphere_is_the_wreck_bikes_root_node() {
    let Some(col) = wreck_bike_col() else { return };
    let (centre, radius) = game_sim::ragdoll_ride::bike_sphere(&col).expect("the file reads as a mesh");
    assert_eq!(centre.w(), 1.0);
    let mut seen = 0;
    for name in STATES {
        let Some(ee) = state(name) else { return };
        let request = ee.u32_at(ACTOR + REQUEST);
        let object = ee.u32_at(request + COLLIDER);
        assert_eq!(F(ee.f32_at(object + RADIUS)).0.to_bits(), radius.0.to_bits(), "{name}: the radius");
        let frame = game_sim::math::Mat4(std::array::from_fn(|i| quad(&ee, request + FRAME + 16 * i as u32)));
        let world = game_sim::ragdoll_ride::bike_centre(&frame, centre);
        assert_eq!(bits(world), bits(quad(&ee, object + CENTRE)), "{name}: the centre");
        seen += 1;
    }
    assert_eq!(seen, STATES.len());
}
