// ---------------------------------------------------------------------------------------------------
// R63 — R22 with the seats swapped: player 1 in the buggy, player 2's bike into it
// ---------------------------------------------------------------------------------------------------

use super::draws::{through, whole_frame};
use super::*;
use ps2_oracle::{Ee, Kit};

/// R63's RAM image: split-screen free09 from the front end's Freestyle setup as R22's (Machine 1 the
/// monster truck, Machine 2 the Dune Buggy), player 1 switched into the buggy from the pause menu, the
/// buggy put on player 2's way (research/r63/place_save.sh) and held on its brake, player 2 on the gas
/// into it (research/r63/p1_brake.pad, p2_ride.pad). Saved at frame 9729 by research/r63/save_pre.sh.
const STATE: &str = "states/r63/pre.ram";
/// The same ride from the state before it, one row a tick (research/r63/r63-split-car.toml,
/// research/r22/capture.sh). Its row 9729 is the image.
const CAPTURE: &str = "capture/r63/strike.csv";

/// The level's collision node and the records it collides, in its order: the parked monster truck,
/// player 1's buggy, one among player 1's parked bike's structures (player 2's bike knocked it from
/// frame 9716, and the node's world resolves it beside the car's), player 2's bike.
const NODE: u32 = 0x01a6_af50;
const TRUCK_REC: u32 = 0x00c6_b110;
const CAR_REC: u32 = 0x017e_57d0;
const PARKED_BIKE_REC: u32 = 0x0136_2d30;
const BIKE_REC: u32 = 0x00da_3110;
/// The node's dynamics world (node `+0x78`) and the bike world.
const CAR_WORLD: u32 = 0x01a6_afc8;
const BIKE_WORLD: u32 = 0x007c_1d70;
/// The truck's and the buggy's actors (their collision records at `+0x170`), the buggy's body.
const TRUCK: u32 = 0x01a6_8e00;
const CAR: u32 = 0x017e_a7e0;
const CAR_BODY: u32 = 0x0139_4710;
/// Player 1's parked bike and player 2's ridden one (player `+0x30` 1), its chassis.
const PARKED_BIKE: u32 = 0x0189_fb90;
const BIKE: u32 = 0x00db_5a50;
const CHASSIS: u32 = 0x00df_edd0;
/// The tick the bike strikes the car.
const STRIKE: u32 = 9735;

/// The functions logged, by their first argument (as tests/ai/split_car.rs's).
const LOGGED: [(&str, u32); 6] = [
    ("collide", 0x0027_08a0),  // collision_record_collide (record)
    ("world", 0x001f_29a0),    // dynamics_world_step (world)
    ("preforce", 0x0039_7500), // car_preforce (car)
    ("resolve", 0x0027_0dc0),  // collision_resolve (record)
    ("measure", 0x0039_8150),  // car_measure (car), the end of its step
    ("player", 0x0027_c890),   // the player update (rider)
];

/// The image's pad reports as the tape left them, stamped fresh: port 0's two halves hold player 1's
/// brake (Square down), port 1's (0x400 on) player 2's gas (Cross down), the sticks centred.
fn held_pads(ee: &mut Ee) {
    for port in [0x0047_a980u32, 0x0047_ad80] {
        for half in [0u32, 0x80] {
            ee.set_u32(port + half + 0x58, ee.u32_at(port + half + 0x58) + 1);
        }
    }
}

/// A record's contacts as (kind, other record), and its four struck words `+0x40..+0x4c`.
fn record(ee: &Ee, rec: u32) -> (Vec<(u32, u32)>, [u32; 4]) {
    let list = ee.u32_at(rec + 0x108);
    let entries = (0..ee.u32_at(rec + 0x100).min(11)).map(|k| (ee.u32_at(list + 0x80 * k), ee.u32_at(list + 0x80 * k + 0x74))).collect();
    (entries, [0x40, 0x44, 0x48, 0x4c].map(|o| ee.u32_at(rec + o)))
}

fn v3(ee: &Ee, at: u32) -> [f32; 3] {
    [ee.f32_at(at), ee.f32_at(at + 4), ee.f32_at(at + 8)]
}

/// **R22 with the seats swapped, in the original** (R63): player 1 (seat 0) in the buggy, player 2
/// (seat 1) on a bike into it. R63's image run whole in the interpreter nine ticks on both players'
/// held pads, every tick against the capture of the same ride bit for bit — the bike's chassis centre
/// and velocity, the car's body position and velocity, both records' lists and struck words — and
/// what ran. The order is F77's (main.rs `pass_order`): the node's records collide, then the node's
/// dynamics world steps the truck and the buggy (its preforce, four resolves of the car's record, each
/// with the knocked record of player 1's parked bike, its measure), and only then the riders, last
/// seat first (player 2's ridden bike, its record resolved twice, then player 1's parked one), then
/// the bike world. The seat-0 car steps before the seat-1 bike, and on the strike (frame 9735) the
/// car's resolve, in its own step, is what stops it.
#[test]
fn r63_the_seat_0_buggy_steps_before_the_seat_1_bike_that_strikes_it() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let (state, capture) = (kit.ctx(STATE), kit.ctx(CAPTURE));
    for f in [&state, &capture] {
        if !f.exists() {
            return skip(&format!("{} is not there: research/r22/capture.sh and `./kit state ram` of research/r63/save_pre.sh's save", f.display()));
        }
    }
    let text = std::fs::read_to_string(&capture).unwrap_or_else(|e| panic!("{e}"));
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let rows: Vec<Vec<&str>> = lines.map(|l| l.split(',').collect()).collect();
    let col = |name: &str| header.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no column {name}"));
    let row = |frame: u32| rows.iter().find(|r| r[col("frame_counter")] == frame.to_string()).unwrap_or_else(|| panic!("no row {frame}"));
    let f3 = |r: &[&str], name: &str| ["x", "y", "z"].map(|a| r[col(&format!("{name}.{a}"))].parse::<f32>().unwrap());
    let list = |r: &[&str], count: &str, prefix: &str| -> Vec<(u32, u32)> {
        (0..r[col(count)].parse::<usize>().unwrap().min(11))
            .map(|k| (r[col(&format!("{prefix}{k}.kind"))].parse().unwrap(), r[col(&format!("{prefix}{k}.other"))].parse().unwrap()))
            .collect()
    };
    let struck = |r: &[&str], name: &str| -> [u32; 4] {
        let hex = r[col(name)];
        std::array::from_fn(|k| u32::from_le_bytes(std::array::from_fn(|b| u8::from_str_radix(&hex[8 * k + 2 * b..][..2], 16).unwrap())))
    };

    let mut ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(0x0047_9d04), 9729, "the image's frame");
    let records: Vec<u32> = (0..ee.u32_at(NODE + 0x30)).map(|k| ee.u32_at(ee.u32_at(NODE + 0x38) + 4 * k)).collect();
    assert_eq!(records, [TRUCK_REC, CAR_REC, PARKED_BIKE_REC, BIKE_REC], "the node's records");
    assert_eq!((ee.u32_at(TRUCK + 0x170), ee.u32_at(CAR + 0x170), ee.u32_at(BIKE + 0x7d0)), (TRUCK_REC, CAR_REC, BIKE_REC));
    assert_eq!((ee.u32_at(CAR + 0x160), ee.u32_at(BIKE + 0x7a4), ee.u32_at(ee.u32_at(BIKE + 0x20) + 0x30)), (CAR_BODY, CHASSIS, 1));
    let app = ee.u32_at(addr("app_pointer").unwrap());
    let dt = F(ee.f32_at(app + 0x344));
    // Each logged call as it returned, with the bike's chassis velocity and the car's velocity then.
    type Seen = (&'static str, u32, [f32; 3], [f32; 3]);
    let log: Rc<RefCell<Vec<Seen>>> = Rc::default();
    for (name, at) in LOGGED {
        let l = Rc::clone(&log);
        ee.hooks.insert(
            at,
            Box::new(move |ee: &mut Ee| {
                let a0 = ee.r(4) as u32;
                through(ee);
                l.borrow_mut().push((name, a0, v3(ee, CHASSIS + 0x260), v3(ee, CAR_BODY + 0x70)));
            }),
        );
    }
    let mut want: Vec<(&str, u32)> = records.iter().map(|&r| ("collide", r)).collect();
    want.extend([("preforce", TRUCK), ("preforce", CAR)]);
    for _ in 0..4 {
        want.extend([("resolve", CAR_REC), ("resolve", PARKED_BIKE_REC)]);
    }
    want.extend([("measure", CAR), ("world", CAR_WORLD)]);
    // Returns are logged as each call ends: the bike's two resolves are inside player 2's update.
    want.extend([("resolve", BIKE_REC); 2]);
    want.extend([("player", BIKE), ("player", PARKED_BIKE), ("world", BIKE_WORLD)]);
    let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
    for tick in 0..9 {
        held_pads(&mut ee);
        whole_frame(&mut ee, dt, &mut clock);
        let frame = ee.u32_at(0x0047_9d04);
        let got = std::mem::take(&mut *log.borrow_mut());
        let order: Vec<(&str, u32)> = got.iter().map(|s| (s.0, s.1)).collect();
        assert_eq!(order, want, "tick {tick} (frame {frame}): what ran");
        let r = row(frame);
        assert_eq!(v3(&ee, CHASSIS + 0x60), f3(r, "ch_cofg"), "frame {frame}: the bike's centre");
        assert_eq!(v3(&ee, CHASSIS + 0x260), f3(r, "ch_vel"), "frame {frame}: the bike's velocity");
        assert_eq!(v3(&ee, CAR_BODY + 0x60), f3(r, "car_pos"), "frame {frame}: the car's place");
        assert_eq!(v3(&ee, CAR_BODY + 0x70), f3(r, "car_vel"), "frame {frame}: the car's velocity");
        let (bike, car) = (record(&ee, BIKE_REC), record(&ee, CAR_REC));
        assert_eq!(bike, (list(r, "rec_count", "c"), struck(r, "rec_struck")), "frame {frame}: the bike's record");
        assert_eq!(car, (list(r, "car_rec_count", "k"), struck(r, "car_rec_struck")), "frame {frame}: the car's record");
        if frame < STRIKE {
            assert!(bike.0.is_empty() && car.0.is_empty(), "frame {frame}: a contact before the strike");
        }
        if frame == STRIKE {
            let speed = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            let was = f3(row(STRIKE - 1), "ch_vel");
            // The car's first resolve, in its own step before player 2's update: the bike all but stopped.
            let first = got.iter().position(|s| s.0 == "resolve" && s.1 == CAR_REC).expect("the car's resolve");
            let ridden = got.iter().position(|s| s.0 == "player" && s.1 == BIKE).expect("player 2's update");
            assert!(first < ridden);
            println!("the bike at {} ft/s; after the car's first resolve at {} ft/s, the car at {} ft/s", speed(was), speed(got[first].2), speed(got[first].3));
            assert!(speed(was) > 26.0 && speed(got[first].2) < 5.0, "the car's resolve did not stop the bike");
            assert_eq!(bike.0, [(1, CAR_REC), (1, CAR_REC), (0, CAR_REC)], "the bike's list on the strike");
            assert_eq!(car.0, [(0, BIKE_REC), (0, BIKE_REC), (1, BIKE_REC)], "the car's list on the strike");
            assert_eq!((bike.1, car.1), ([1, 1, 0, 1], [1, 1, 0, 1]), "the struck words");
        }
    }
}
