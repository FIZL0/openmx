// ---------------------------------------------------------------------------------------------------
// R22 — a split-screen bike into the other player's parked buggy: the original's frame whole
// ---------------------------------------------------------------------------------------------------

use super::draws::{through, whole_frame};
use super::*;
use ps2_oracle::{Ee, Kit};

/// R22's RAM image: split-screen free09 from the front end's Freestyle setup (two players, Machine 1
/// the monster truck, Machine 2 the Dune Buggy), player 2 switched into the buggy, the buggy put 30 ft
/// up player 1's way (research/c34/place.py) and held on its brake, player 1 on the gas into its tail
/// (research/r22/p1_ride.pad, p2_brake.pad). Saved at frame 12006 by research/r22/save_at.py.
const STATE: &str = "states/r22/pre.ram";
/// The same ride from the state before it, one row a tick (watch/r22-split-car.toml,
/// research/r22/capture.sh). Its row 12006 is the image.
const CAPTURE: &str = "capture/r22/strike.csv";

/// The level's collision node and the records it collides, in its order: player 1's parked monster
/// truck, player 2's buggy, player 1's bike, and one of player 2's parked bike.
const NODE: u32 = 0x01a6_af50;
const TRUCK_REC: u32 = 0x00c6_b590;
const CAR_REC: u32 = 0x017e_5c90;
const BIKE_REC: u32 = 0x0135_9240;
const PARKED_BIKE_REC: u32 = 0x00df_8410;
/// The node's dynamics world (node `+0x78`) and the bike world.
const CAR_WORLD: u32 = 0x01a6_afc8;
const BIKE_WORLD: u32 = 0x00db_cde0;
/// The truck's and the buggy's actors (their collision records at `+0x170`), the buggy's body.
const TRUCK: u32 = 0x01a6_8e00;
const CAR: u32 = 0x017e_a820;
const CAR_BODY: u32 = 0x0139_4700;
/// Player 2's parked bike and player 1's ridden one (player `+0x30` 0), its chassis.
const PARKED_BIKE: u32 = 0x0137_b2d0;
const BIKE: u32 = 0x0187_f6d0;
const CHASSIS: u32 = 0x0135_6440;
/// The tick the bike strikes the car's tail.
const STRIKE: u32 = 12012;
/// The car's body words compared across the riders' updates (its place, velocity, rates, pushes).
const BODY_WORDS: u32 = 0x300 / 4;

/// The functions logged, by their first argument.
const LOGGED: [(&str, u32); 6] = [
    ("collide", 0x0027_08a0),  // collision_record_collide (record)
    ("world", 0x001f_29a0),    // dynamics_world_step (world)
    ("preforce", 0x0039_7500), // car_preforce (car)
    ("resolve", 0x0027_0dc0),  // collision_resolve (record)
    ("measure", 0x0039_8150),  // car_measure (car), the end of its step
    ("player", 0x0027_c890),   // the player update (rider)
];

/// The image's pad reports as the tape left them, stamped fresh: port 0's two halves hold player 1's
/// gas (Cross down, its pressure 0xff, the sticks centred), as research/r22/p1_ride.pad holds it.
fn held_pad(ee: &mut Ee) {
    const PAD: u32 = 0x0047_a980;
    for half in [0u32, 0x80] {
        ee.set_u32(PAD + half + 0x58, ee.u32_at(PAD + half + 0x58) + 1);
    }
}

/// A record's contacts as (kind, other record), and its four struck words `+0x40..+0x4c`.
fn record(ee: &Ee, rec: u32) -> (Vec<(u32, u32)>, [u32; 4]) {
    let list = ee.u32_at(rec + 0x108);
    let entries = (0..ee.u32_at(rec + 0x100).min(11)).map(|k| (ee.u32_at(list + 0x80 * k), ee.u32_at(list + 0x80 * k + 0x74))).collect();
    (entries, [0x40, 0x44, 0x48, 0x4c].map(|o| ee.u32_at(rec + o)))
}

/// The car's collision record, its actor and its record's first four entries, word by word.
fn car_words(ee: &Ee) -> Vec<(u32, u32)> {
    [(CAR_REC, 0x200), (CAR, 0x800), (ee.u32_at(CAR_REC + 0x108), 0x80 * 4)]
        .into_iter()
        .flat_map(|(base, n)| (0..n).step_by(4).map(move |o| base + o))
        .map(|a| (a, ee.u32_at(a)))
        .collect()
}

fn v3(ee: &Ee, at: u32) -> [f32; 3] {
    [ee.f32_at(at), ee.f32_at(at + 4), ee.f32_at(at + 8)]
}

/// **A split-screen bike into the other player's parked buggy, in the original** (R22): R22's image
/// run whole in the interpreter (S328's recipe) nine ticks on player 1's held gas, every tick against
/// the capture of the same ride bit for bit — the bike's chassis centre and velocity, the car's body
/// position and velocity, both records' lists and struck words — and what ran:
///
/// - every record of the level's node collides first, the car's before the bike's; then the node's
///   dynamics world steps the parked truck and the buggy (four resolves of the car's record), the
///   car's step ending in its measure; then the riders (player 2's parked bike, then player 1's), the
///   bike's record resolved twice; then the bike world;
/// - on the strike (frame 12012) the bike's list is the car's three rays' mirrors (kind 1, naming the
///   car's record) at its head, then its own one body ray (kind 0) on the car; the car's list is its
///   own three rays on the bike (kind 0, naming the bike's record), then the bike ray's mirror (kind
///   1) after them. Both records' struck words +0x40, +0x44 and +0x4c are 1;
/// - the car's resolve is what meets: its first, in its own step, takes the bike's chassis from
///   46.2 ft/s to 4.4 and the car to 3.4 ft/s along the bike's way; its step leaves the car at
///   2.35 ft/s. The riders' updates after (the bike's two resolves) write no word of the car's body
///   (of its record, actor and entries, the record's `+0xa0` vector alone) and throw the bike back
///   off its tail.
#[test]
fn r22_a_split_bike_strikes_the_other_players_parked_buggy() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let (state, capture) = (kit.ctx(STATE), kit.ctx(CAPTURE));
    for f in [&state, &capture] {
        if !f.exists() {
            return skip(&format!("{} is not there: research/r22/capture.sh and `./kit state ram` of its save", f.display()));
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
    assert_eq!(ee.u32_at(0x0047_9d04), 12006, "the image's frame");
    let records: Vec<u32> = (0..ee.u32_at(NODE + 0x30)).map(|k| ee.u32_at(ee.u32_at(NODE + 0x38) + 4 * k)).collect();
    assert_eq!(records, [TRUCK_REC, CAR_REC, BIKE_REC, PARKED_BIKE_REC], "the node's records");
    assert_eq!((ee.u32_at(TRUCK + 0x170), ee.u32_at(CAR + 0x170), ee.u32_at(BIKE + 0x7d0)), (TRUCK_REC, CAR_REC, BIKE_REC));
    assert_eq!((ee.u32_at(CAR + 0x160), ee.u32_at(BIKE + 0x7a4), ee.u32_at(ee.u32_at(BIKE + 0x20) + 0x30)), (CAR_BODY, CHASSIS, 0));
    let app = ee.u32_at(addr("app_pointer").unwrap());
    let dt = F(ee.f32_at(app + 0x344));
    // Each logged call as it returned, with the bike's chassis velocity and the car's whole body then.
    type Seen = (&'static str, u32, [f32; 3], Vec<u32>);
    let log: Rc<RefCell<Vec<Seen>>> = Rc::default();
    // The car's words player 1's update wrote on the strike (its record, actor and list).
    let wrote: Rc<RefCell<Vec<u32>>> = Rc::default();
    for (name, at) in LOGGED {
        let l = Rc::clone(&log);
        let w = Rc::clone(&wrote);
        ee.hooks.insert(
            at,
            Box::new(move |ee: &mut Ee| {
                let a0 = ee.r(4) as u32;
                let pre = car_words(ee);
                through(ee);
                if name == "player" && a0 == BIKE && ee.u32_at(0x0047_9d04) == STRIKE {
                    let changed: Vec<u32> = pre.iter().zip(car_words(ee)).filter(|(b, n)| b.1 != n.1).map(|(b, _)| b.0).collect();
                    w.borrow_mut().extend(changed);
                }
                let body = (0..BODY_WORDS).map(|k| ee.u32_at(CAR_BODY + 4 * k)).collect();
                l.borrow_mut().push((name, a0, v3(ee, CHASSIS + 0x260), body));
            }),
        );
    }
    let mut want: Vec<(&str, u32)> = records.iter().map(|&r| ("collide", r)).collect();
    want.extend([("preforce", TRUCK), ("preforce", CAR)]);
    want.extend([("resolve", CAR_REC); 4]);
    want.extend([("measure", CAR), ("world", CAR_WORLD), ("player", PARKED_BIKE)]);
    want.extend([("resolve", BIKE_REC); 2]);
    want.extend([("player", BIKE), ("world", BIKE_WORLD)]);
    let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
    for tick in 0..9 {
        held_pad(&mut ee);
        whole_frame(&mut ee, dt, &mut clock);
        let frame = ee.u32_at(0x0047_9d04);
        let got = std::mem::take(&mut *log.borrow_mut());
        // Returns are logged as each call ends: the world steps after what they call.
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
        let speed = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if frame < STRIKE {
            assert!(bike.0.is_empty() && car.0.is_empty(), "frame {frame}: a contact before the strike");
        }
        if frame == STRIKE {
            assert_eq!(bike.0, [(1, CAR_REC), (1, CAR_REC), (1, CAR_REC), (0, CAR_REC)], "the bike's list on the strike");
            assert_eq!(car.0, [(0, BIKE_REC), (0, BIKE_REC), (0, BIKE_REC), (1, BIKE_REC)], "the car's list on the strike");
            assert_eq!((bike.1, car.1), ([1, 1, 0, 1], [1, 1, 0, 1]), "the struck words");
            let was = f3(row(STRIKE - 1), "ch_vel");
            let vel = |body: &[u32]| [0, 1, 2].map(|k| f32::from_bits(body[0x70 / 4 + k]));
            let along = |v: [f32; 3]| (v[0] * was[0] + v[2] * was[2]) / speed(was);
            // The car's first resolve, in its own step: the bike's chassis all but stopped, the car on.
            let first = got.iter().find(|s| s.0 == "resolve" && s.1 == CAR_REC).expect("the car's resolve");
            println!(
                "the bike at {} ft/s; after the car's first resolve the bike at {} ft/s, the car at {} ft/s",
                speed(was),
                speed(first.2),
                speed(vel(&first.3))
            );
            assert!(speed(was) > 46.0 && speed(first.2) < 5.0, "the car's resolve did not stop the bike");
            assert!(along(vel(&first.3)) > 3.0, "the car's resolve did not push it on along the bike's way");
            // The car's step done: what it is left with is what it keeps. The riders' updates, the
            // bike's two resolves in player 1's, write no word of its body.
            let stepped = &got.iter().find(|s| s.0 == "measure").expect("the car's step").3;
            let ridden = &got.iter().find(|s| s.0 == "player" && s.1 == BIKE).expect("player 1's update").3;
            assert_eq!(ridden, stepped, "the bike's resolve wrote the car's body");
            // Of the car's record, actor and entries it writes one vector alone: the record's +0xa0.
            assert_eq!(*wrote.borrow(), [CAR_REC + 0xa0, CAR_REC + 0xa4, CAR_REC + 0xa8], "what player 1's update wrote of the car");
            assert_eq!(vel(ridden), v3(&ee, CAR_BODY + 0x70));
            assert!((2.35..2.36).contains(&speed(vel(ridden))), "the car left at {} ft/s", speed(vel(ridden)));
            // The bike's own resolves throw it back off the tail.
            assert!(along(v3(&ee, CHASSIS + 0x260)) < -1.0, "the bike went on");
        }
    }
}

// --- F76: the port's car among the bikes, set against R22's strike ---------------------------------

use game_sim::biplane::Body;
use game_sim::contact::{Collider, Contact, Node, Response, Vehicle};

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

/// A dynamics body in RAM (`tests/biplane.rs`'s field offsets).
fn body_of(ee: &Ee, b: u32) -> Body {
    Body {
        enabled: ee.u32_at(b) != 0,
        inv_mass: F(ee.f32_at(b + 0x10)),
        damping: quad(ee, b + 0x20),
        last_position: quad(ee, b + 0x50),
        position: quad(ee, b + 0x60),
        velocity: quad(ee, b + 0x70),
        last_force: quad(ee, b + 0x80),
        force: quad(ee, b + 0x90),
        accel: quad(ee, b + 0xa0),
        shock_peak: quad(ee, b + 0xb0),
        shock: quad(ee, b + 0xc0),
        step: quad(ee, b + 0xf0),
        inv_inertia: quad(ee, b + 0x120),
        ang_damping: quad(ee, b + 0x130),
        last_quat: quad(ee, b + 0x150),
        quat: quad(ee, b + 0x170),
        cofg: quad(ee, b + 0x180),
        moment: quad(ee, b + 0x190),
        last_moment: quad(ee, b + 0x1a0),
        rates: quad(ee, b + 0x1b0),
        ang_accel: quad(ee, b + 0x1c0),
        twist_peak: quad(ee, b + 0x1d0),
        twist: quad(ee, b + 0x1e0),
    }
}

fn node_at(ee: &Ee, n: u32) -> Node {
    Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 16 * k)), origin: quad(ee, n + 0xb0) }
}

/// A record's chassis as the other side of a contact (`tests/plane_contact_capture.rs`'s).
fn vehicle_at(ee: &Ee, rec: u32) -> Vehicle {
    let v = ee.u32_at(rec + 0xf0);
    Vehicle {
        node: node_at(ee, ee.u32_at(rec + 0xf8)),
        cofg: quad(ee, v + 0x190),
        mass: F(ee.f32_at(v + 0x210)),
        inv_mass: F(ee.f32_at(v + 0x214)),
        inv_inertia: [0xf0, 0x104, 0x118].map(|at| F(ee.f32_at(v + at))),
        velocity: quad(ee, v + 0x260),
        rates: quad(ee, v + 0x290),
        shock: quad(ee, v + 0x170),
        twist: quad(ee, v + 0x180),
    }
}

fn response(ee: &Ee, r: u32, at: u32, angular: u32, spring: u32, friction_mask: u32, friction: u32) -> Response {
    Response {
        stiffness: F(ee.f32_at(r + at)),
        damping: F(ee.f32_at(r + at + 4)),
        restitution: F(ee.f32_at(r + at + 8)),
        angular: ee.u32_at(r + angular) != 0,
        friction: F(ee.f32_at(r + friction)),
        spring_mask: quad(ee, r + spring),
        friction_mask: quad(ee, r + friction_mask),
    }
}

/// A collision object's tuning in RAM (`tests/contact.rs`'s offsets).
fn collider_of(ee: &Ee, r: u32) -> Collider {
    let c = ee.u32_at(r + 0x1f8);
    Collider {
        damp: ee.u32_at(r + 0x50) != 0,
        damp_linear: F(ee.f32_at(r + 0x58)),
        damp_angular: F(ee.f32_at(r + 0x60)),
        mass: F(ee.f32_at(r + 0x64)),
        inv_mass: F(ee.f32_at(r + 0x68)),
        inv_inertia: quad(ee, r + 0x80),
        cofg: quad(ee, r + 0x90),
        cofg_world: quad(ee, r + 0xa0),
        actors: ee.u32_at(r + 0x110),
        collides_with: ee.u32_at(r + 0x114),
        collidable: (c != 0).then(|| ee.u32_at(c + 0x84)),
        max_depth: F(ee.f32_at(r + 0x118)),
        scenery: response(ee, r, 0x124, 0x148, 0x150, 0x160, 0x190),
        actor: response(ee, r, 0x13c, 0x14c, 0x170, 0x180, 0x194),
        surfaces: std::array::from_fn(|k| F(ee.f32_at(r + 0x198 + 4 * k as u32))),
        normal_speed: F(ee.f32_at(r + 0x1d8)),
        relative: quad(ee, r + 0x1e0),
    }
}

/// The records the port names by index: player 1's bike and player 2's parked one (its players),
/// then the car, after them (`2 + 0`, the bikes' others' first after the players).
const NAMES: [u32; 3] = [BIKE_REC, PARKED_BIKE_REC, CAR_REC];

fn contact_at(ee: &Ee, e: u32) -> Contact {
    let other = ee.u32_at(e + 0x74);
    Contact {
        kind: ee.u32_at(e),
        from: quad(ee, e + 0x10),
        on_body: quad(ee, e + 0x20),
        normal: quad(ee, e + 0x30),
        point: quad(ee, e + 0x40),
        arm: quad(ee, e + 0x50),
        other_arm: quad(ee, e + 0x60),
        surface: ee.u32_at(e + 0x70) as i32,
        other: (other != 0).then(|| NAMES.iter().position(|&o| o == other).unwrap_or(99)),
        mover: (ee.u32_at(e + 0x78) != 0).then_some(99),
    }
}

fn list_at(ee: &Ee, r: u32) -> Vec<Contact> {
    let (n, at) = (ee.u32_at(r + 0x100), ee.u32_at(r + 0x108));
    (0..n).map(|k| contact_at(ee, at + 0x80 * k)).collect()
}

/// The fields of two entries that differ, bit for bit.
fn differ(port: &Contact, orig: &Contact) -> Vec<&'static str> {
    let quads = [
        ("from", port.from, orig.from),
        ("on_body", port.on_body, orig.on_body),
        ("normal", port.normal, orig.normal),
        ("point", port.point, orig.point),
        ("arm", port.arm, orig.arm),
        ("other_arm", port.other_arm, orig.other_arm),
    ];
    let mut bad: Vec<&'static str> = quads.iter().filter(|(_, p, o)| p.to_bits() != o.to_bits()).map(|(n, _, _)| *n).collect();
    if (port.kind, port.surface, port.other, port.mover) != (orig.kind, orig.surface, orig.other, orig.mover) {
        bad.push("kind, surface, other or mover");
    }
    bad
}

/// What the strike's frame hands the port, read off the interpreter as each call finds it.
#[derive(Default)]
struct Strike {
    /// At the car's collide: its body, the bike's frame collidable's transform, both bikes' chassis.
    body: Option<Body>,
    frame: Option<game_sim::math::Mat4>,
    vehicles: Vec<Vehicle>,
    /// After the bike's collide: both lists.
    car_list: Vec<Contact>,
    bike_list: Vec<Contact>,
    /// The car's first resolve: what it finds (its body, its collider, the bikes' colliders and
    /// chassis, its list), and what it leaves (its body, the bikes' chassis).
    resolve_in: Option<(Body, Collider, Vec<Collider>, Vec<Vehicle>, Vec<Contact>)>,
    resolve_out: Option<(Body, Vec<Vehicle>)>,
}

/// **F76: the port's car meets the bike as R22's buggy does.** R22's image run whole in the
/// interpreter to the strike (frame 12012), each call's inputs read off RAM as the call finds them:
///
/// - the car's search ([`game_sim::car::search_met`]) from its body as its collide finds it, the
///   bike's frame where its collidable stands, the bike named player 0 and the car `2`: its own
///   entries (kind 0, naming the bike) and their mirrors on the bike's list (kind 1, naming the
///   car) are the original's, field for field, bit for bit;
/// - the car's first resolve ([`game_sim::contact::resolve`] on its list against both players'
///   chassis, as the car's step resolves it with a met): the car's body and the bike's chassis it
///   leaves are the original's, bit for bit — the bike stopped from 46.2 ft/s, the car pushed on.
#[test]
fn f76_the_ports_car_strikes_and_stops_the_bike_as_r22s() {
    use game_render::disc::Disc;
    use game_sim::bike_col::{self, BikeCol};
    use game_sim::car::{Car, CarModel, Kind};
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let state = kit.ctx(STATE);
    if !state.exists() {
        return skip(&format!("{} is not there: research/r22/capture.sh and `./kit state ram` of its save", state.display()));
    }
    let Ok(mut disc) = Disc::open_configured() else { return skip("no disc configured") };
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL))).expect("the bike's collision files");
    let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res")).expect("machine.res").name.clone();
    let machines = disc.res(&name).expect("machine.res");
    let file = |n: &str| machines.read_suffix(n).unwrap_or_else(|e| panic!("{n}: {e}"));
    let car = Car::read_bytes(&file(Kind::Buggy.phy()), Kind::Buggy);
    let model = CarModel::read(&file("buggy\\anim\\buggy.psg")).expect("the buggy's model");
    let rays = game_sim::flight_search::Rays::of(&game_sim::track::Rays::parse(&file(Kind::Buggy.rays())).expect("the buggy's rays"));

    let mut ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!((ee.u32_at(CAR_REC + 0xf4), ee.u32_at(BIKE_REC + 0xf0)), (CAR_BODY, CHASSIS), "the car's body and the bike's chassis");
    let app = ee.u32_at(addr("app_pointer").unwrap());
    let dt = F(ee.f32_at(app + 0x344));
    let tick: Rc<std::cell::Cell<u32>> = Rc::default();
    let seen: Rc<RefCell<Strike>> = Rc::default();
    for at in [0x0027_08a0u32, 0x0027_0dc0] {
        let (tick, seen) = (Rc::clone(&tick), Rc::clone(&seen));
        ee.hooks.insert(
            at,
            Box::new(move |ee: &mut Ee| {
                let a0 = ee.r(4) as u32;
                let strike = tick.get() == STRIKE - 12007;
                let resolve = at == 0x0027_0dc0;
                let first = strike && resolve && a0 == CAR_REC && seen.borrow().resolve_in.is_none();
                if strike && !resolve && a0 == CAR_REC {
                    let mut s = seen.borrow_mut();
                    s.body = Some(body_of(ee, CAR_BODY));
                    let c = ee.u32_at(BIKE_REC + 0x1f8);
                    s.frame = Some(game_sim::math::Mat4([0, 1, 2, 3].map(|r| quad(ee, c + 0x120 + 16 * r))));
                    s.vehicles = [BIKE_REC, PARKED_BIKE_REC].map(|r| vehicle_at(ee, r)).into();
                }
                if first {
                    let colliders = [BIKE_REC, PARKED_BIKE_REC].map(|r| collider_of(ee, r)).into();
                    let vehicles = [BIKE_REC, PARKED_BIKE_REC].map(|r| vehicle_at(ee, r)).into();
                    seen.borrow_mut().resolve_in = Some((body_of(ee, CAR_BODY), collider_of(ee, CAR_REC), colliders, vehicles, list_at(ee, CAR_REC)));
                }
                through(ee);
                if strike && !resolve && a0 == BIKE_REC {
                    let mut s = seen.borrow_mut();
                    (s.car_list, s.bike_list) = (list_at(ee, CAR_REC), list_at(ee, BIKE_REC));
                }
                if first {
                    seen.borrow_mut().resolve_out = Some((body_of(ee, CAR_BODY), [BIKE_REC, PARKED_BIKE_REC].map(|r| vehicle_at(ee, r)).into()));
                }
            }),
        );
    }
    let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
    for t in 0..=STRIKE - 12007 {
        tick.set(t);
        held_pad(&mut ee);
        whole_frame(&mut ee, dt, &mut clock);
    }
    let s = seen.borrow();

    // The search: the car's own entries on the bike, and their mirrors at the head of its list.
    let mut body = s.body.expect("the car's collide on the strike");
    let mut collision = game_sim::car::collision_of_model(&car, &model, &body, rays);
    let mut met = game_sim::flight::Met {
        colliders: [BIKE_REC, PARKED_BIKE_REC].map(|r| collider_of(&ee, r)).into(),
        vehicles: s.vehicles.clone(),
        bikes: vec![0],
        named: Some(2),
        rides: vec![game_sim::flight_search::Bike { transform: s.frame.expect("the bike's frame"), in_world: true }],
        bike_col: Some(std::sync::Arc::new(col)),
        ..game_sim::flight::Met::default()
    };
    assert!(game_sim::car::strikes_bikes(&met));
    let mut ground = game_sim::world::Flat { y: F(-1.0e6), ..Default::default() };
    game_sim::car::search_met(&mut collision, &mut body, &mut ground, dt, &[], &mut met);
    let own: Vec<Contact> = s.car_list.iter().filter(|c| c.kind == 0).copied().collect();
    assert_eq!(s.car_list.iter().map(|c| (c.kind, c.other)).collect::<Vec<_>>(), [(0, Some(0)), (0, Some(0)), (0, Some(0)), (1, Some(0))], "the car's list");
    let port = &collision.record.contacts;
    assert_eq!(port.len(), own.len(), "the car's own entries\nport {port:?}\noriginal {own:?}");
    for (k, (a, b)) in port.iter().zip(&own).enumerate() {
        assert_eq!(differ(a, b), Vec::<&str>::new(), "car entry {k}\nport {a:?}\noriginal {b:?}");
    }
    assert_eq!(met.pushed, [true, false], "the players the car's entries name");
    assert!(met.struck[1].is_empty());
    let mirrors = &met.struck[0];
    assert_eq!(mirrors.len(), 3, "the mirrors on the bike {mirrors:?}");
    for (k, (a, b)) in mirrors.iter().zip(&s.bike_list).enumerate() {
        assert_eq!(differ(a, b), Vec::<&str>::new(), "bike entry {k}\nport {a:?}\noriginal {b:?}");
    }

    // The car's first resolve: its body and the bike's chassis as it leaves them.
    let (mut body, mut collider, mut colliders, mut vehicles, mut list) = s.resolve_in.clone().expect("the car's first resolve");
    assert!(list.iter().all(|c| c.mover.is_none() && c.other != Some(99)), "the car's list names nothing but the bike {list:?}");
    let was = vehicles[0].velocity;
    // The car's step weighs its resolve by its collision's mass (the body wrapper's), not +0x64.
    let mass = collision.mass;
    println!("the car's collision mass {:?}, its record's +0x64 {:?}", mass, collider.mass);
    let mut others: Vec<game_sim::contact::Other<'_>> = colliders
        .iter_mut()
        .zip(vehicles.iter_mut())
        .map(|(collider, v)| game_sim::contact::Other { collider, side: game_sim::contact::Side::Vehicle(v) })
        .collect();
    game_sim::contact::resolve(&mut collider, &mut game_sim::contact::Weighed { body: &mut body, mass }, &mut list, &mut others, &[], true);
    drop(others);
    let (want_body, want) = s.resolve_out.clone().expect("the car's first resolve");
    let speed = |v: Vf| (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt();
    println!("the bike at {} ft/s; after the car's first resolve {} ft/s (original {}), the car at {} ft/s", speed(was), speed(vehicles[0].velocity), speed(want[0].velocity), speed(body.velocity));
    for (name, a, b) in [
        ("velocity", body.velocity, want_body.velocity),
        ("shock", body.shock, want_body.shock),
        ("rates", body.rates, want_body.rates),
        ("twist", body.twist, want_body.twist),
        ("force", body.force, want_body.force),
        ("moment", body.moment, want_body.moment),
    ] {
        assert_eq!(a.to_bits(), b.to_bits(), "the car's {name}: port {a:?}, original {b:?}");
    }
    for (name, a, b) in [
        ("velocity", vehicles[0].velocity, want[0].velocity),
        ("rates", vehicles[0].rates, want[0].rates),
        ("shock", vehicles[0].shock, want[0].shock),
        ("twist", vehicles[0].twist, want[0].twist),
    ] {
        assert_eq!(a.to_bits(), b.to_bits(), "the bike's {name}: port {a:?}, original {b:?}");
    }
    assert!(speed(was) > 46.0 && speed(vehicles[0].velocity) < 5.0, "the car's resolve did not stop the bike");
}
