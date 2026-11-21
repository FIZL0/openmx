//! R52: the fliers the original parks in play, free06's helicopter and free10's biplane, read off
//! RAM images booted with the session's first machine slot as recording2's (research/r52/boot.py),
//! and free07's baja rolling off its spot.
//!
//! The original's machine placement (`0x0020f6e0`) walks the event's machine slots (`+0x11c` the
//! machine kind, 6 none; `+0x120` the machine-racer flag). With the flag set it parks the
//! `[Machine<n>]` whose `MachineRacer = TRUE` (`[Machine1]` on every free map); without it, the
//! slot kind's own `[<Kind><n>]` bay. recording2 and first_recording (free09) hold kind 4 with the
//! flag: the truck. S133's free10/free11 images hold no slot (kind 6), C34's free06 ones kind 3
//! without the flag (a buggy at `[Buggy1]`), so none of them shows the flier.

use game_render::disc::Disc;
use game_sim::phy::Ini;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};

/// A parked machine in one of R52's images: its hull object, the event slot's kind, the hull file
/// and the hull's category.
struct Flier {
    state: &'static str,
    track: &'static str,
    hull: u32,
    kind: u32,
    file: &'static str,
    collidable: u32,
}

const FLIERS: [Flier; 2] = [
    Flier { state: "r52-free06-heli.ee.bin", track: "free06", hull: 0x0149_1070, kind: 2, file: game_sim::kiowa::KIOWA_HULL, collidable: game_sim::kiowa::COLLIDABLE },
    Flier { state: "r52-free10-biplane.ee.bin", track: "free10", hull: 0x0061_1880, kind: 1, file: game_sim::flight::HULL, collidable: game_sim::flight::COLLIDABLE },
];

/// The session's event (the settings' `+0xc`, the settings at `[0x0041f770] + 0x3d4`).
fn event(ee: &Ee) -> u32 {
    ee.u32_at(ee.u32_at(ee.u32_at(0x0041_f770) + 0x3d4) + 0xc)
}

fn quad(ee: &Ee, at: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|i| ee.u32_at(at + i)))
}

/// The machine's body (its `+0x160`), as [`game_sim::ai::parked_hull`] reads it.
fn body(ee: &Ee, b: u32) -> game_sim::biplane::Body {
    let mut body = game_sim::biplane::Body::default();
    for (v, o) in [(&mut body.last_position, 0x50), (&mut body.position, 0x60), (&mut body.last_quat, 0x150), (&mut body.quat, 0x170), (&mut body.cofg, 0x180)] {
        *v = quad(ee, b + o);
    }
    body
}

/// **The fliers the original parks are its hull at its body, awake, beside `[Machine1]`'s spot.**
/// In each image the event's first slot is the machine-racer one, the hull object (a mesh named
/// after the flier's `f.col`, of its category, collidable) is its machine's (`+0x178`, the owner at
/// the hull's `+0x24` less 0x58, as the car's), the machine's stored spot (wrapper `+0x90`) is
/// `[Machine1]`'s `x`, `z` and nose, and [`game_sim::ai::parked_hull`] of its body is the object's
/// matrix, centre and reach, bit for bit. Neither flier's body sleeps (word 0 set) 300 frames in.
#[test]
fn the_parked_fliers_hulls_stand_at_their_bodies() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let Some(mut disc) = Disc::open_configured().map_err(|w| skip(&w)).ok() else { return };
    let res = disc.res("machine.res").expect("machine.res");
    let mut seen = 0;
    for f in &FLIERS {
        let path = kit.ctx("states").join(f.state);
        if !path.exists() {
            skip(&format!("{} is not there: research/r52/boot.py makes it", path.display()));
            continue;
        }
        let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        let ev = event(&ee);
        assert_eq!((ee.u32_at(ev + 0x11c), ee.u8_at(ev + 0x120)), (f.kind, 1), "{}: the first slot is the racer's", f.track);

        let o = f.hull;
        let name: Vec<u8> = (0..32).map(|i| ee.u8_at(o + 0x8a + i)).take_while(|&b| b != 0).collect();
        let base = f.file.rsplit('\\').next().unwrap();
        assert!(String::from_utf8_lossy(&name).eq_ignore_ascii_case(base), "{}: the hull is {base}", f.track);
        assert_eq!((ee.u8_at(o + 2), ee.u32_at(o + 0x68), ee.u32_at(o + 0x84)), (2, 0, f.collidable), "{}: a mesh of the flier's category", f.track);
        assert_eq!(ee.u8_at(o + 0x31) & 1, 1, "{}: collidable", f.track);
        let m = ee.u32_at(o + 0x24) - 0x58;
        assert_eq!(ee.u32_at(m + 0x178), o, "{}: the hull is its machine's", f.track);

        let txt = Ini::parse_bytes(&res.read_suffix(&format!("{}\\machine.txt", f.track)).expect("machine.txt"));
        let bay = txt.section("Machine1");
        let (spot, nose) = (bay.vector("Position", Vf::default()), bay.vector("Direction", Vf::default()));
        let (at, dir) = (quad(&ee, m + 0x50 + 0x90), quad(&ee, m + 0x50 + 0xa0));
        assert_eq!([at.x(), at.z()], [spot.x(), spot.z()], "{}: stored at [Machine1]'s spot", f.track);
        assert_eq!([dir.x(), dir.y(), dir.z()], [nose.x(), nose.y(), nose.z()], "{}: along its nose", f.track);

        let b = ee.u32_at(m + 0x160);
        assert_ne!(ee.u32_at(b), 0, "{}: the flier's body is awake", f.track);
        let body = body(&ee, b);
        let hull = game_sim::track::Mesh::parse(&res.read(f.file).expect("the hull file")).expect("the hull");
        let parked = game_sim::ai::parked_hull(hull, &body);
        let matrix = [0, 1, 2, 3].map(|k| quad(&ee, o + 0x120 + 0x10 * k));
        assert_eq!(parked.to_world.0.map(|v| v.to_bits()), matrix.map(|v| v.to_bits()), "{}: the hull's matrix", f.track);
        let (centre, _) = game_sim::flight_search::prop_box(&parked);
        assert_eq!(centre.map(|c| c.to_bits()), quad(&ee, o + 0x40).to_bits(), "{}: the hull's centre", f.track);
        assert_eq!(parked.radius.to_bits(), ee.u32_at(o + 0x60), "{}: the hull's reach", f.track);
        let (p, v) = (body.position, quad(&ee, b + 0x70));
        println!(
            "{}: spot {:?} stored y {} body at ({}, {}, {}) v ({}, {}, {}) up {:?}",
            f.track, [spot.x(), spot.y(), spot.z()], at.y(), p.x(), p.y(), p.z(), v.x(), v.y(), v.z(), [matrix[1].x(), matrix[1].y(), matrix[1].z()]
        );
        seen += 1;
    }
    if seen == 0 {
        skip("no R52 image");
    }
}

/// **free07's parked baja rolls off its spot and is still awake 1711 frames in.** The original
/// stores it at `[Machine1]` (3899.637, 141.32 = the ground plus the bounds' lift, 2438.25), it
/// rolls down the hill along +x (frame 303: 3916.3, 1.27 ft/s; frame 1711: 3941.3, 0.08 ft/s), and
/// its body never sleeps. B47: the port's matches it from its spawn on (car_spawn.rs's
/// `free07s_parked_baja_rolls_on_as_the_original`); its settle leaves it awake.
#[test]
fn free07s_parked_baja_rolls_on_past_the_ports_rest() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let mut seen = 0;
    for (state, x, moving) in [("r52-free07-baja.ee.bin", 3916.3367, 1.0), ("r52-free07-baja-later.ee.bin", 3941.2859, 0.05)] {
        let path = kit.ctx("states").join(state);
        if !path.exists() {
            skip(&format!("{} is not there: research/r52/boot.py makes it", path.display()));
            continue;
        }
        let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        let ev = event(&ee);
        assert_eq!((ee.u32_at(ev + 0x11c), ee.u8_at(ev + 0x120)), (5, 1), "the racer's slot");
        let m = ee.u32_at(0x012e_c030 + 0x24) - 0x58;
        assert_eq!(quad(&ee, m + 0x270).x(), F(3899.637), "spawned at [Machine1]");
        let b = ee.u32_at(m + 0x160);
        assert_ne!(ee.u32_at(b), 0, "awake");
        let (p, v) = (quad(&ee, b + 0x60), quad(&ee, b + 0x70));
        assert!((p.x() - F(x)).abs() < 0.01, "at {}", p.x());
        assert!(v.x() > F(moving), "still rolling: {}", v.x());
        seen += 1;
    }
    if seen == 0 {
        skip("no R52 image");
    }
}
