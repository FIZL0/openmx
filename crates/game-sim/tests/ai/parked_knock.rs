// ---------------------------------------------------------------------------------------------------
// C45 — player 1's parked bike in the level node's dynamics world, before and while player 2's bike
// knocks it
// ---------------------------------------------------------------------------------------------------

use super::draws::{through, whole_frame};
use super::*;
use ps2_oracle::{Ee, Kit};

/// R63's image before its tape (slot 34, research/r63/place_save.sh): split-screen free09, player 1 in
/// the buggy they switched to from their bike, the buggy put on player 2's way, neither pad held yet.
/// Frame 9656.
const STATE: &str = "states/r63/s34.ram";
/// The ride on R63's tape from that state, one row a tick (R63's), first row 9657.
const CAPTURE: &str = "capture/r63/strike.csv";
/// R22's image (tests/ai/split_car.rs): player 2's bike parked long before, far from the ride.
const R22_STATE: &str = "states/r22/pre.ram";

/// The level's collision node and the records it collides, as R63's (tests/ai/split_seats.rs).
const NODE: u32 = 0x01a6_af50;
const TRUCK_REC: u32 = 0x00c6_b110;
const CAR_REC: u32 = 0x017e_57d0;
const PARKED_REC: u32 = 0x0136_2d30;
const BIKE_REC: u32 = 0x00da_3110;
/// The node's dynamics world, its bodies at `+0x50`, and the bike world.
const NODE_WORLD: u32 = 0x01a6_afc8;
const BIKE_WORLD: u32 = 0x007c_1d70;
/// Player 1's parked bike's physics object (the body wrapper of the props and the wreck bike: its
/// interface vtables at `+0x40..`), its body at `+0x1f0` (`+0` enabled, `+0x70` velocity). Its
/// collision record is [`PARKED_REC`].
const PARKED_OBJECT: u32 = 0x0136_13a0;
const PARKED_BODY: u32 = 0x0136_1590;
/// The truck's and the buggy's actors, the buggy's body, player 2's ridden bike and its chassis,
/// player 1's parked bike.
const TRUCK: u32 = 0x01a6_8e00;
const CAR: u32 = 0x017e_a7e0;
const CAR_BODY: u32 = 0x0139_4710;
const BIKE: u32 = 0x00db_5a50;
const CHASSIS: u32 = 0x00df_edd0;
const PARKED_BIKE: u32 = 0x0189_fb90;
/// The first frame player 2's bike lists the parked bike's record (through 9728).
const KNOCK: u32 = 9716;
/// The last frame run: R63's image's (states/r63/pre.ram).
const LAST: u32 = 9729;
/// The first frames the tape's pads are read held, found by matching the capture: player 1's
/// Square (the buggy's brake) and player 2's Cross (the bike's gas).
const BRAKE: u32 = 9658;
const GAS: u32 = 9688;
/// R22's parked bike: its record (the node's fourth) and its physics object's body.
const R22_PARKED_REC: u32 = 0x00df_8410;
const R22_PARKED_BODY: u32 = 0x00df_73d0;

/// The functions logged, by their first argument (as R63's).
const LOGGED: [(&str, u32); 6] = [
    ("collide", 0x0027_08a0),  // collision_record_collide (record)
    ("world", 0x001f_29a0),    // dynamics_world_step (world)
    ("preforce", 0x0039_7500), // car_preforce (car)
    ("resolve", 0x0027_0dc0),  // collision_resolve (record)
    ("measure", 0x0039_8150),  // car_measure (car), the end of its step
    ("player", 0x0027_c890),   // the player update (rider)
];

/// The tape's pads for the frame about to run: player 1's Square from [`BRAKE`] on, player 2's Cross
/// from [`GAS`] on, both reports stamped fresh. Each port's two halves alike: buttons in the high half
/// of `+0` (active low: Square bit 15, Cross bit 14), pressures in `+0xc`, stamp `+0x58` (as
/// states/r63/pre.ram holds them).
fn tape_pads(ee: &mut Ee, frame: u32) {
    for (port, held, bit, pressure) in [(0x0047_a980u32, frame >= BRAKE, 15, 0xff00_0000u32), (0x0047_ad80, frame >= GAS, 14, 0x00ff_0000)] {
        for half in [0u32, 0x80] {
            let at = port + half;
            ee.set_u32(at, if held { 0xffff_7900 & !(1 << (16 + bit)) } else { 0xffff_7900 });
            ee.set_u32(at + 0xc, if held { pressure } else { 0 });
            ee.set_u32(at + 0x58, ee.u32_at(at + 0x58) + 1);
        }
    }
}

/// A list's members: count at `+0`, array at `+8`.
fn list(ee: &Ee, at: u32) -> Vec<u32> {
    (0..ee.u32_at(at)).map(|k| ee.u32_at(ee.u32_at(at + 8) + 4 * k)).collect()
}

/// A record's contacts as (kind, other record).
fn contacts(ee: &Ee, rec: u32) -> Vec<(u32, u32)> {
    let at = ee.u32_at(rec + 0x108);
    (0..ee.u32_at(rec + 0x100).min(11)).map(|k| (ee.u32_at(at + 0x80 * k), ee.u32_at(at + 0x80 * k + 0x74))).collect()
}

fn v3(ee: &Ee, at: u32) -> [f32; 3] {
    [ee.f32_at(at), ee.f32_at(at + 4), ee.f32_at(at + 8)]
}

fn speed(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// **A parked bike in the level node's dynamics world, knocked or not, in the original** (C45). R63's
/// image before its tape run whole in the interpreter on the tape's pads, frames 9657..9729, every one
/// against R63's capture bit for bit (the ridden bike's chassis centre and velocity and its record's
/// list, the buggy's place and velocity). Player 1's parked bike, left when they switched into the
/// buggy and still sliding at under 1 ft/s, is a physics object of the node's world: before every
/// frame its body is enabled and among the world's bodies, and each frame the world resolves its
/// record beside the buggy's in all four passes — sixty ticks before player 2's bike first lists it
/// (9716) as on and after. The knock throws it (over 10 ft/s) and changes nothing of what runs: it
/// does not join the world by being knocked, it was in it. R22's image shows the other case: a bike
/// parked long before, its body asleep (`+0` clear) and out of the world's bodies, its record still
/// among the node's (R22's world resolves the car's record alone).
#[test]
fn c45_the_parked_bike_is_in_the_node_world_before_the_knock() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let (state, capture, r22) = (kit.ctx(STATE), kit.ctx(CAPTURE), kit.ctx(R22_STATE));
    for f in [&state, &capture, &r22] {
        if !f.exists() {
            return skip(&format!("{} is not there: R63's slot 34 (`./kit state ram`), research/r22/capture.sh, R22's image", f.display()));
        }
    }
    let r22 = Ee::from_ram(&r22).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(list(&r22, NODE + 0x30)[3], R22_PARKED_REC, "R22's node's records");
    assert_eq!(r22.u32_at(R22_PARKED_BODY), 0, "R22's parked bike's body awake");
    assert_eq!(r22.u32_at(R22_PARKED_BODY - 0x1f0 + 0x40), 0x0044_e7e0, "R22's parked bike's object is a body wrapper");
    assert!(!list(&r22, NODE_WORLD + 0x50).contains(&R22_PARKED_BODY), "R22's parked body in the node's world");

    let text = std::fs::read_to_string(&capture).unwrap_or_else(|e| panic!("{e}"));
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let rows: Vec<Vec<&str>> = lines.map(|l| l.split(',').collect()).collect();
    let col = |name: &str| header.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no column {name}"));
    let row = |frame: u32| rows.iter().find(|r| r[col("frame_counter")] == frame.to_string()).unwrap_or_else(|| panic!("no row {frame}"));
    let f3 = |r: &[&str], name: &str| ["x", "y", "z"].map(|a| r[col(&format!("{name}.{a}"))].parse::<f32>().unwrap());
    let listed = |r: &[&str]| -> Vec<(u32, u32)> {
        (0..r[col("rec_count")].parse::<usize>().unwrap().min(11))
            .map(|k| (r[col(&format!("c{k}.kind"))].parse().unwrap(), r[col(&format!("c{k}.other"))].parse().unwrap()))
            .collect()
    };

    let mut ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(0x0047_9d04), 9656, "the image's frame");
    let records = list(&ee, NODE + 0x30);
    assert_eq!(records, [TRUCK_REC, CAR_REC, PARKED_REC, BIKE_REC], "the node's records");
    assert_eq!((ee.u32_at(CAR + 0x160), ee.u32_at(BIKE + 0x7a4), ee.u32_at(BIKE + 0x7d0)), (CAR_BODY, CHASSIS, BIKE_REC));
    assert_eq!(ee.u32_at(PARKED_OBJECT + 0x40), 0x0044_e7e0, "the parked bike's object is a body wrapper");
    let app = ee.u32_at(addr("app_pointer").unwrap());
    let dt = F(ee.f32_at(app + 0x344));
    let log: Rc<RefCell<Vec<(&'static str, u32)>>> = Rc::default();
    for (name, at) in LOGGED {
        let l = Rc::clone(&log);
        ee.hooks.insert(
            at,
            Box::new(move |ee: &mut Ee| {
                let a0 = ee.r(4) as u32;
                through(ee);
                l.borrow_mut().push((name, a0));
            }),
        );
    }
    // R63's order, every frame: the parked bike's record resolved after the car's in each pass.
    let mut want: Vec<(&str, u32)> = records.iter().map(|&r| ("collide", r)).collect();
    want.extend([("preforce", TRUCK), ("preforce", CAR)]);
    for _ in 0..4 {
        want.extend([("resolve", CAR_REC), ("resolve", PARKED_REC)]);
    }
    want.extend([("measure", CAR), ("world", NODE_WORLD)]);
    want.extend([("resolve", BIKE_REC); 2]);
    want.extend([("player", BIKE), ("player", PARKED_BIKE), ("world", BIKE_WORLD)]);
    let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
    for frame in 9657..=LAST {
        assert_eq!(ee.u32_at(PARKED_BODY), 1, "frame {frame}: the parked bike's body asleep");
        assert!(list(&ee, NODE_WORLD + 0x50).contains(&PARKED_BODY), "frame {frame}: the parked body out of the node's world");
        let was = v3(&ee, PARKED_BODY + 0x70);
        tape_pads(&mut ee, frame);
        whole_frame(&mut ee, dt, &mut clock);
        assert_eq!(ee.u32_at(0x0047_9d04), frame);
        let order = std::mem::take(&mut *log.borrow_mut());
        assert_eq!(order, want, "frame {frame}: what ran");
        let r = row(frame);
        assert_eq!(v3(&ee, CHASSIS + 0x60), f3(r, "ch_cofg"), "frame {frame}: the bike's centre");
        assert_eq!(v3(&ee, CHASSIS + 0x260), f3(r, "ch_vel"), "frame {frame}: the bike's velocity");
        assert_eq!(v3(&ee, CAR_BODY + 0x60), f3(r, "car_pos"), "frame {frame}: the car's place");
        assert_eq!(v3(&ee, CAR_BODY + 0x70), f3(r, "car_vel"), "frame {frame}: the car's velocity");
        let bike = contacts(&ee, BIKE_REC);
        assert_eq!(bike, listed(r), "frame {frame}: the bike's list");
        assert_eq!(bike.iter().any(|&(_, o)| o == PARKED_REC), (KNOCK..LAST).contains(&frame), "frame {frame}: the knock's frames");
        let now = v3(&ee, PARKED_BODY + 0x70);
        if frame < KNOCK {
            assert!(speed(now) < 1.0, "frame {frame}: the parked bike moved at {} ft/s before the knock", speed(now));
        }
        if frame == KNOCK {
            println!("the parked bike from {} to {} ft/s", speed(was), speed(now));
            assert!(speed(now) > 10.0, "the knock did not throw the parked bike");
        }
    }
}
