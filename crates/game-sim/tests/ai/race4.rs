// ---------------------------------------------------------------------------------------------------
// S12e — free09's four racers through the Sim: C33's capture replayed
// ---------------------------------------------------------------------------------------------------
//
// C33 saved free09 with four racers at frame_counter 2, all four bikes still on the spawn line, and
// recorded 1800 ticks from there: every rider's body and the generator, three times at three emulator
// speeds, with the rows two of the three agree on kept (`free09-ai4-agreed.csv`). This seeds a Sim off
// that image: each of the four riders built as the shell builds one and seeded as S174 seeds the 2p
// riders (chassis, wheels, drivetrain, balance, air, levers), the three AI brains read out of RAM into
// pilots as S323 reads recording2's, and free09 off the disc as the world. Then `Sim::step_all` runs
// the four, each AI rider's brain thinking after its bike's frame (the bike riding the levers of the
// tick before, `Sim::think`, the others read off their cards, S330), and every agreed row's generator
// and AI bodies are held against the port's.
//
// The AI bodies part from the original early (`C33_AGREES_THROUGH`); the test holds the agreement it
// has reached, so that a fix shows as a raised mark and a regression as a failure. The human, with no
// pad but the stick its image was saved holding on the first step, agrees on every row (S329).

use super::lane::s323_free09;
use super::sim::{s12e_bike, s12e_racer, s12e_seek_world, s12e_u64};
use super::*;
use game_render::disc::Disc;
use game_sim::Sim;
use game_sim::ai::{Lane, Pilot, Seat};
use game_sim::bike_col::{self, BikeCol};
use game_sim::eco::Wooded;
use game_sim::phy::Phy;
use game_sim::ride::{Controls, Ride, Wheel};
use game_sim::rider::{Rider as Bike, Tables};
use game_sim::track::{self, Terrain};
use game_sim::world::{Ground, Place};
use ps2_oracle::{Ee, Kit};
use std::sync::Arc;

pub(crate) const C33_STATE: &str = "capture/c33/free09-ai4.ee.bin";
const C33_CSV: &str = "capture/c33/free09-ai4-agreed.csv";
/// The image's frame counter: its RAM holds tick 2, and the first row is tick 3.
const C33_FROM: u64 = 2;
const HOLD_N: usize = 2;

/// The four riders in the actor list's order (C33's `free09-ai4.riders.json`): rider, body, brain (0
/// for the human's, which has no brain).
pub(crate) const C33_RIDERS: [(u32, u32, u32); 4] = [
    (0x005f_f210, 0x0089_9860, 0x008a_8800),
    (0x008c_7b10, 0x008c_f9b0, 0x0088_8360),
    (0x0088_8770, 0x0088_cfc0, 0),
    (0x0149_c910, 0x0090_14b0, 0x0094_53b0),
];
/// The capture's column prefix for each rider of [`C33_RIDERS`].
pub(crate) const C33_NAMES: [&str; 4] = ["ai0", "ai1", "human", "ai2"];

/// The last tick each of [`C33_LANES`] agrees with the original's, bit for bit, on every agreed row up
/// to it but the torn ones: the regression mark.
const C33_AGREES_THROUGH: [u64; 5] = [28, 28, 1802, 206, 6];

/// One rider's words off an image, at the offsets `tests/two_p_rider.rs` (S174) seeds a rider from.
struct Image<'a> {
    ee: &'a Ee,
    rider: u32,
}

impl Image<'_> {
    fn word(&self, a: u32) -> u32 {
        self.ee.u32_at(a)
    }
    fn float(&self, a: u32) -> F {
        F(self.ee.f32_at(a))
    }
    fn flag(&self, a: u32) -> bool {
        self.word(a) != 0
    }
    fn vf(&self, a: u32) -> Vf {
        Vf([0, 4, 8, 12].map(|k| self.ee.f32_at(a + k)))
    }
    fn at(&self, off: u32) -> u32 {
        self.word(self.rider + off)
    }

    fn mounts(&self) -> (Vf, Vf) {
        (self.vf(self.at(0x7b0) + 0xa0), self.vf(self.at(0x7b4) + 0xa0))
    }

    fn chassis(&self, ride: &mut Ride) {
        let c = self.at(0x7a4);
        let node = self.word(c + 0x254);
        let ch = &mut ride.chassis;
        ch.speed = self.float(c + 0x34);
        ch.cofg_world = self.vf(c + 0x60);
        ch.accel = self.vf(c + 0x70);
        ch.ang_accel = self.vf(c + 0x80);
        ch.euler = self.vf(c + 0x90);
        ch.force = self.vf(c + 0x130);
        ch.torque = self.vf(c + 0x140);
        ch.impulse = self.vf(c + 0x150);
        ch.correction = self.vf(c + 0x160);
        ch.cofg = self.vf(c + 0x190);
        ch.quat = self.vf(c + 0x1a0);
        ch.awake = self.word(c + 0x1b4) as i32;
        ch.contacts = self.word(c + 0x1e0) as i32;
        ch.vel = self.vf(c + 0x260);
        ch.local_vel = self.vf(c + 0x270);
        ch.world_ang_vel = self.vf(c + 0x280);
        ch.ang_vel = self.vf(c + 0x290);
        for k in 0..3 {
            ch.free[k] = self.word(c + 0x2a0 + 4 * k as u32) as i32;
            ch.ang_free[k] = self.word(c + 0x2ac + 4 * k as u32) as i32;
        }
        let three = |at: u32| [self.float(at), self.float(at + 4), self.float(at + 8)];
        ch.pos_hist.dx = three(c + 0x2b8);
        ch.pos_hist.prev = three(c + 0x2c4);
        ch.vel_hist.dx = three(c + 0x2d0);
        ch.vel_hist.prev = three(c + 0x2dc);
        for k in 0..4 {
            ch.quat_hist.dx[k] = self.float(c + 0x2f0 + 4 * k as u32);
            ch.quat_hist.prev[k] = self.float(c + 0x300 + 4 * k as u32);
        }
        ch.ang_hist.dx = three(c + 0x310);
        ch.ang_hist.prev = three(c + 0x31c);
        ride.node = [self.vf(node + 0x80), self.vf(node + 0x90), self.vf(node + 0xa0)];
        ride.origin = self.vf(node + 0xb0);
    }

    fn wheel(&self, which: usize, wh: &mut Wheel) {
        let a = self.at([0x7b0, 0x7b4][which]);
        let s = &mut wh.w;
        s.mount_world = self.vf(a + 0x150);
        s.probe_end = self.vf(a + 0x100);
        s.probe_end_world = self.vf(a + 0x160);
        s.ground_normal = self.vf(a + 0x200);
        s.ray_hit = self.word(a + 0xd0) as i32;
        s.swept_contact = self.word(a + 0xd4) as i32;
        s.terrain_wins = self.word(a + 0xd8) as i32;
        s.contact = self.flag(a + 0xdc);
        s.ray_steps = self.word(a + 0xec) as i32;
        s.compression = self.float(a + 0xf0);
        s.last_compression = self.float(a + 0xf4);
        s.spring_force = self.float(a + 0xf8);
        s.contact_point = self.vf(a + 0x120);
        s.contact_normal = self.vf(a + 0x130);
        s.load_point = self.vf(a + 0x140);
        s.contact_velocity = self.vf(a + 0x170);
        s.chassis_velocity = self.vf(a + 0x180);
        s.balancing = self.flag(a + 0x3c);
        s.steer_torque = self.float(a + 0x6c);
        s.steer_angle = self.float(a + 0x8ac);
        s.steer_rate = self.float(a + 0x8b0);
        let m = &mut wh.mv;
        m.spin = self.float(a + 0x1d4);
        m.contact = s.contact;
        m.contact_normal = s.contact_normal;
        m.drive_torque = self.float(a + 0x210);
        m.balancing = s.balancing;
        for (k, o) in [0xd0, 0xd4, 0x1f0, 0x1f4, 0xf0, 0xf8, 0x1a0].into_iter().enumerate() {
            m.fresh.scalars[k] = self.word(a + o);
        }
        for (k, o) in [0x170, 0x1b0, 0x1c0].into_iter().enumerate() {
            m.fresh.vectors[k] = self.vf(a + o);
        }
        wh.surface = self.word(a + 0x110) as u8;
        wh.angle = self.float(a + 0x1d0);
        wh.chassis_local_velocity = self.vf(a + 0x190);
        wh.local_velocity = self.vf(a + 0x840);
        wh.local_force = self.vf(a + 0x850);
        for r in 0..4 {
            wh.frame[r] = self.vf(a + 0x8d0 + 16 * r as u32);
        }
        wh.side_per_load = self.float(a + 0x234);
        wh.steer_axis = self.vf(a + 0x8c0);
        wh.slip = game_sim::tire::Slip { spinning: self.flag(a + 0xe0), locked: self.flag(a + 0xe4), sliding: self.flag(a + 0xe8) };
        wh.mv.spin_from_torque = wh.slip.spinning;
    }

    fn rest(&self, ride: &mut Ride) {
        let (r, e) = (self.rider, self.at(0x7b8));
        ride.engine.torque = self.float(e + 0xa0);
        ride.engine.wheel_torque = self.float(e + 0xa4);
        ride.engine.road_rpm = self.float(e + 0xac);
        ride.mv.clutch_timer = self.float(r + 0x6c0);
        ride.mv.clutch_scale_start = self.float(r + 0x6c4);
        ride.mv.clutch_cofg_weight = self.float(r + 0x6cc);
        ride.held.gas = self.float(r + 0x658);
        ride.held.clutch = self.float(r + 0x65c);
        ride.mv.clutch_in = ride.held.clutch;
        game_sim::r#move::set_gear(&mut ride.engine, self.word(e + 0x68) as i32);
        ride.engine.clutch_locked = self.flag(e + 0x64);
        ride.engine.clutch_torque_scale = self.float(e + 0x88);
        ride.engine.curve = game_sim::r#move::Curve::at(self.word(e + 0x8c), self.word(e + 0x90), self.float(e + 0x94));
        ride.engine.rpm = self.float(e + 0xa8);
        let mv = &mut ride.mv;
        mv.g_peak = self.vf(r + 0x550);
        mv.balance_inertia = self.float(r + 0xa4c);
        mv.balance_command = self.float(r + 0xa60);
        mv.balance_point = self.float(r + 0xa64);
        mv.balance_noise_time = self.float(r + 0xa6c);
        mv.pitch_integral = self.float(r + 0x724);
        mv.stoppie = self.flag(r + 0x728);
        let air = &mut ride.air;
        air.on_ground = self.flag(r + 0x574);
        air.both_wheels = self.flag(r + 0x578);
        air.airborne = self.flag(r + 0x57c);
        air.was_airborne = self.flag(r + 0x580);
        air.hang_time = self.float(r + 0x4f0);
        air.contact_vel = self.vf(r + 0x430);
        air.crossup = self.float(r + 0x600);
        let h = &mut ride.held;
        h.stick = (self.float(r + 0x628), self.float(r + 0x62c));
        h.flick = self.float(r + 0x630);
        h.front_brake = self.float(r + 0x650);
        h.brake = h.front_brake;
        h.rear_brake = self.float(r + 0x654);
        h.lean = (self.float(r + 0x670), self.float(r + 0x674));
    }
}

/// A bike's physics file and the bike's collision files, off the disc.
struct Bikes {
    disc: Disc,
    col: Arc<BikeCol>,
}

impl Bikes {
    fn open() -> Option<Bikes> {
        let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
        let res = disc.res("Tracks\\free09_2.res").expect("free09's second archive");
        let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
        let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
            .expect("the bike's collision files");
        Some(Bikes { disc, col: Arc::new(col) })
    }

    /// `name` (`Bike\Race250A.phy`) out of whichever bike archive carries it.
    fn phy(&mut self, name: &str, tables: &Tables) -> Phy {
        let leaf = name.rsplit('\\').next().unwrap().to_ascii_lowercase();
        let bikes: Vec<String> = self
            .disc
            .entries()
            .iter()
            .map(|e| e.name.clone())
            .filter(|n| {
                let n = n.to_ascii_lowercase();
                (n.starts_with("bike\\") && n.ends_with(".res")) || n.starts_with("tracks\\free09")
            })
            .collect();
        let bytes = bikes
            .iter()
            .find_map(|b| self.disc.res(b).ok()?.read_suffix(&leaf).ok())
            .unwrap_or_else(|| panic!("{leaf} in no archive"));
        Phy::read_bytes(&bytes, &tables.ride.curve)
    }
}

/// The rider at `r`, built on `phy` (named `name`) and seeded off the image.
fn c33_bike(ee: &Ee, r: u32, phy: &Phy, name: &str, col: &Arc<BikeCol>) -> Bike {
    let im = Image { ee, rider: r };
    let c = im.at(0x7a4);
    let mut bike = Bike::new(phy, im.mounts(), im.vf(c + 0x60), Vf::new(0.0, 0.0, 1.0, 0.0), game_sim::trick::Set::default());
    bike.no_start_line();
    bike.carries_bike_col(Arc::clone(col));
    bike.fifty = track::fifty_reach(name);
    im.chassis(&mut bike.ride);
    im.wheel(0, &mut bike.ride.front);
    im.wheel(1, &mut bike.ride.rear);
    im.rest(&mut bike.ride);
    bike
}

/// The AI brain `b` on rider `r`, read out of RAM as S323's lane test reads recording2's.
fn c33_pilot(ee: &Ee, lists: &super::sim::S12eLists, b: u32, r: u32) -> Pilot {
    let filler = ai::spawn(&mut Rng::from_seed(0), 0, None, 2);
    let bike = s12e_bike(ee, b, r);
    let ignore = ee.u32_at(b + 0x150);
    assert_eq!(ignore, 0, "brain {b:#x}: an ignore list, which this replay does not map onto the port's props");
    Pilot {
        racer: s12e_racer(ee, lists, b, r),
        character: filler.character,
        wander: filler.wander,
        levers: bike.levers,
        recovery: bike.recovery,
        card: bike.card,
        seat: Seat {
            speed_scale: bike.speed_scale,
            slow_radius: bike.slow_radius,
            slow_speed: bike.slow_speed,
            player: bike.player,
            air_cone: bike.air_cone,
            look_ahead: bike.rider.look_ahead,
            bump_chance: bike.rider.bump_chance,
            race_phase: bike.race_phase,
            path_net: bike.path_net,
            live: bike.live,
        },
        ignore: Vec::new(),
    }
}

/// C33's agreed rows: each one's frame counter, dt, generator, whether two recordings agree on it, and
/// each rider's raw body (its first 0x2c0 bytes).
struct Row {
    frame: u64,
    dt: F,
    rng: u64,
    agreed: bool,
    bodies: [Vec<u8>; 4],
}

impl Row {
    fn word(&self, k: usize, at: u32) -> u32 {
        let b = &self.bodies[k][at as usize..at as usize + 4];
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }
    fn quad(&self, k: usize, at: u32) -> [u32; 4] {
        [0, 4, 8, 12].map(|o| self.word(k, at + o))
    }
}

fn c33_rows(text: &str) -> Vec<Row> {
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no column {name}"));
    let (frame, dt, rng, agree) = (col("frame_counter"), col("dt"), col("rng_state"), col("agree"));
    let raw = C33_NAMES.map(|n| col(&format!("{n}_body_raw")));
    let hex = |s: &str| (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect::<Vec<u8>>();
    lines
        .map(|l| {
            let cells: Vec<&str> = l.split(',').collect();
            Row {
                frame: cells[frame].parse().unwrap(),
                dt: F(cells[dt].parse::<f64>().unwrap() as f32),
                rng: cells[rng].parse().unwrap(),
                agreed: cells[agree].parse::<u32>().unwrap() >= 2,
                bodies: raw.map(|c| hex(cells[c])),
            }
        })
        .collect()
}

/// The body words the replay compares, by name and offset: the centre of gravity, the attitude, the
/// world velocity and the world spin.
const C33_WORDS: [(&str, u32); 4] = [("centre", 0x60), ("quat", 0x1a0), ("velocity", 0x260), ("spin", 0x280)];

fn c33_ours(bike: &Bike) -> [[u32; 4]; 4] {
    let ch = &bike.ride.chassis;
    [ch.cofg_world, ch.quat, ch.vel, ch.world_ang_vel].map(|v| v.to_bits())
}

/// The Sim's player for each of [`C33_RIDERS`]: `Sim::step_all` walks its players from the last to
/// the first, and the original walks the actor list from its head, so the actor list's head is the
/// Sim's last player.
fn c33_player(k: usize) -> usize {
    C33_RIDERS.len() - 1 - k
}

#[test]
fn s12e_free09s_four_racers_ride_as_the_capture() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let (state, csv) = (kit.ctx(C33_STATE), kit.ctx(C33_CSV));
    if !state.exists() || !csv.exists() {
        return skip(&format!("{} or {} is not there: C33's capture", state.display(), csv.display()));
    }
    let mut ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let Some(at) = named(&["rand_state", "frame_counter"]) else { return };
    let [rand_state, frame_counter] = at[..] else { unreachable!() };
    assert_eq!(u64::from(ee.u32_at(frame_counter)), C33_FROM, "the image's frame counter");
    for &(r, body, b) in &C33_RIDERS {
        assert_eq!(ee.u32_at(r + 0x7a4), body, "rider {r:#x}'s body");
        assert_eq!(ee.u32_at(r + 0x10d0), b, "rider {r:#x}'s brain");
    }
    let rows = c33_rows(&std::fs::read_to_string(&csv).unwrap());

    let Some((seek, lists)) = s12e_seek_world(&kit, &ee) else { return };
    let Some((terrain_bytes, scene, props, trees)) = s323_free09(&kit, &ee) else { return };
    let terrain = Terrain::parse(&terrain_bytes).expect("free09's terrain");
    let mut world = Wooded { world: Place { ground: Ground::new(&terrain, &scene.terrain), props }, trees };

    // The event's bike class is 2 (C33's boot): the 250s, the AI riders on the pair's A file and the
    // human on its P file.
    let tables = Arc::new(Tables::build());
    let Some(mut bikes) = Bikes::open() else { return };
    let names = C33_RIDERS.map(|(_, _, b)| track::physics_file(track::Engine::Cc250, false, if b == 0 { C33_HUMAN_TUNING } else { track::Tuning::A }));
    let phys: Vec<Phy> = names.iter().map(|n| bikes.phy(n, &tables)).collect();
    let mut order: Vec<usize> = (0..C33_RIDERS.len()).collect();
    order.sort_by_key(|&k| c33_player(k));
    let seeded = |k: usize| c33_bike(&ee, C33_RIDERS[k].0, &phys[k], &names[k], &bikes.col);
    let mut sim = Sim::with_rider(0, Arc::clone(&tables), phys[order[0]].clone(), seeded(order[0]));
    for &k in &order[1..] {
        sim.add_player(phys[k].clone(), seeded(k));
    }
    let pilots: Vec<(usize, Pilot)> = C33_RIDERS
        .iter()
        .enumerate()
        .filter(|(_, (_, _, b))| *b != 0)
        .map(|(k, &(r, _, b))| (c33_player(k), c33_pilot(&ee, &lists, b, r)))
        .collect();
    let gen0 = s12e_u64(&ee, rand_state);
    sim.ai = Some(Box::new(Lane { seek, tables: Arc::new(controls::tables(&ee)), rng: Rng::from_state(gen0), pilots, net: None, cards: Vec::new() }));

    // The image's RAM already holds the bodies of the first row: it is tick 3's, saved before its
    // frame counter moved on. Each step from it is the next row.
    assert_eq!(rows[0].frame, C33_FROM + 1, "the capture's first row");
    assert_eq!(rows[0].rng, gen0, "the first row's generator is the image's");
    for (k, &(_, body, _)) in C33_RIDERS.iter().enumerate() {
        for (name, at) in C33_WORDS {
            assert_eq!(rows[0].quad(k, at), got(&ee, body + at), "{}: the first row's {name} is the image's", C33_NAMES[k]);
        }
    }
    // How many rand() the original made between two agreed rows, over the ride's first second.
    let draws: Vec<(u64, Option<usize>)> = rows
        .windows(2)
        .take(30)
        .filter(|p| p[0].rng != p[1].rng && p[0].agreed && p[1].agreed)
        .map(|p| {
            let mut g = Rng::from_state(p[0].rng);
            (p[1].frame, (1..=64).find(|_| {
                g.rand();
                g.state() == p[1].rng
            }))
        })
        .collect();
    println!("the original's draws (tick, rand() calls) over the first second: {draws:?}");

    let mut lanes: [Lane5; 5] = Default::default();
    // The AI riders' levers are their brains'. The human has no pad, but the first step after a
    // load rides on the controls the state was saved holding (S117): the image's stick.
    let mut controls = vec![Controls::default(); C33_RIDERS.len()];
    controls[c33_player(2)] = c33_saved_controls(&ee, C33_RIDERS[2].0);
    for (n, pair) in rows.windows(2).enumerate() {
        let (last, row) = (&pair[0], &pair[1]);
        assert_eq!(row.frame, C33_FROM + 2 + n as u64, "the capture's rows run on from the image, one a tick");
        // The step into a row is the frame length the row before it read.
        sim.step_all(last.dt, &controls, &mut world);
        // The human's pad from the second step on: none, so nothing held.
        controls[c33_player(2)] = Controls::default();
        if !row.agreed {
            continue;
        }
        for (k, lane) in lanes[..4].iter_mut().enumerate() {
            let ours = c33_ours(sim.rider_of(c33_player(k)));
            let off: Vec<String> = C33_WORDS
                .iter()
                .zip(ours)
                .filter(|((_, at), ours)| row.quad(k, *at) != *ours)
                .map(|((name, at), ours)| format!("{name} {:?} ours {:?}", Vf::from_bits(row.quad(k, *at)), Vf::from_bits(ours)))
                .collect();
            lane.saw(row.frame, (!off.is_empty()).then(|| off.join("; ")));
        }
        let ours = sim.ai.as_ref().unwrap().rng.state();
        lanes[4].saw(row.frame, (ours != row.rng).then(|| format!("ours {ours:#x}, the original's {:#x}", row.rng)));
        if lanes.iter().all(|l| l.parted.is_some()) {
            break;
        }
    }
    for (i, lane) in lanes.iter_mut().enumerate() {
        if let Some(p) = lane.pending.take() {
            lane.parted = Some(p);
        }
        let what = C33_LANES[i];
        match &lane.parted {
            Some((t, why)) => println!("{what} agrees through tick {} (torn rows {:?}); parts at tick {t}: {why}", lane.through, lane.torn),
            None => println!("{what} agrees on every agreed row, through tick {} (torn rows {:?})", lane.through, lane.torn),
        }
    }
    for (i, lane) in lanes.iter().enumerate() {
        let what = C33_LANES[i];
        assert!(
            lane.through >= C33_AGREES_THROUGH[i],
            "{what} agrees only through tick {}, below C33_AGREES_THROUGH[{i}] = {}: a regression",
            lane.through,
            C33_AGREES_THROUGH[i]
        );
        if lane.through > C33_AGREES_THROUGH[i] {
            println!("{what} now agrees through tick {}: raise C33_AGREES_THROUGH[{i}]", lane.through);
        }
    }
}

/// The controls rider `r`'s image was saved holding: its stick, read back from the limiter's
/// squared copy (`+0x628`/`+0x62c`). Only a stick at rest or at full throw is read back, which is
/// what C33's holds (the human's pulled fully forward, -1); the levers there are all let go.
fn c33_saved_controls(ee: &Ee, r: u32) -> Controls {
    let im = Image { ee, rider: r };
    let axis = |off: u32| {
        let v = im.float(r + off);
        assert!(v == F::ZERO || v.abs() == F::ONE, "rider {r:#x}'s saved stick +{off:#x} is {v:?}: not at rest or full throw");
        v
    };
    for off in [0x650, 0x654, 0x658] {
        assert_eq!(im.float(r + off), F::ZERO, "rider {r:#x}'s saved lever +{off:#x}");
    }
    Controls { stick: (axis(0x628), axis(0x62c)), ..Controls::default() }
}

/// The human's physics file: the shell's P tuning (the AI riders ride A). The capture tells them
/// apart (S329): on A the human parts at tick 6, on P it agrees on every row.
const C33_HUMAN_TUNING: track::Tuning = track::Tuning::P;

/// What the replay's comparisons read, in [`C33_AGREES_THROUGH`]'s order: each rider's body in the
/// actor list's order, then the generator.
const C33_LANES: [&str; 5] = ["ai0", "ai1", "the human", "ai2", "the generator"];

/// One compared thing's agreement with the capture. A row that differs and is followed by an agreed
/// row that matches again is taken as torn — read mid-tick the same way by two recordings (C33's
/// note) — and skipped; two differing agreed rows in a row part.
#[derive(Default)]
struct Lane5 {
    through: u64,
    pending: Option<(u64, String)>,
    parted: Option<(u64, String)>,
    torn: Vec<u64>,
}

impl Lane5 {
    fn saw(&mut self, tick: u64, off: Option<String>) {
        if self.parted.is_some() {
            return;
        }
        match off {
            None => {
                if let Some((t, _)) = self.pending.take() {
                    self.torn.push(t);
                }
                self.through = tick;
            }
            Some(why) => match self.pending.take() {
                Some((t, first)) => self.parted = Some((t, format!("{first}\n    and tick {tick}: {why}"))),
                None => self.pending = Some((tick, why)),
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// S331 — the AI lane at a map's load: `Sim::spawn_ai` against C33's image
// ---------------------------------------------------------------------------------------------------

/// C33's four riders in the game's player order (the interface's player word, the human 0): their
/// index into [`C33_RIDERS`].
fn c33_by_player(ee: &Ee) -> Vec<usize> {
    let player = |k: usize| {
        let (r, _, b) = C33_RIDERS[k];
        if b == 0 { 0 } else { ee.u32_at(r + 0x10d0 + 0x1e4) as usize }
    };
    let mut order: Vec<usize> = (0..C33_RIDERS.len()).collect();
    order.sort_by_key(|&k| player(k));
    assert_eq!(order.iter().map(|&k| player(k)).collect::<Vec<_>>(), [0, 1, 2, 3], "C33's player numbers");
    order
}

/// The lane `Sim::spawn_ai` builds at the load, against the one C33's image holds: free09's four
/// riders in a Sim in the game's player order (the human 0, then the AI riders as their interfaces
/// number them), the map's seeker world loaded over them (`Sim::load_seek`, the image's slot count),
/// and the roster's three AI riders (the image's skills, the event's difficulty and kind) spawned
/// with the retail character file. Held against the image:
/// - the seeker world, field for field, against the one built with the image's own owners;
/// - each pilot's seat: the constructors' speed scale, slow radius and speed, air cone and
///   look-ahead, the player number, and the bump chance the character file put at the difficulty;
/// - each brain's interface type and mode, its spawn flags, and every tuning word the spawn and the
///   reader write that the first ticks do not (the controller's, the avoidance's, the vehicle's, the
///   reach's least and most, the trick plan's, the steering filter's, the assist's weights, the least
///   speed, and the habits' numbers);
/// - the controllers' tables, the game's curve table.
#[test]
fn s331_the_lane_spawns_at_the_load_as_c33_holds_it() {
    use game_sim::ai::{AiRider, Entry, Roster, skill_range};
    use game_sim::world::Props;
    use std::io::Read;

    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let state = kit.ctx(C33_STATE);
    if !state.exists() {
        return skip(&format!("{} is not there: C33's capture", state.display()));
    }
    let Some(retail) = super::character::ch_retail() else {
        return skip("no MX.PSR (or no Bike\\250sx.res in it): the retail character file is needed");
    };
    let character = Ini::parse_bytes(&retail);
    let mut ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let Some(at) = named(&["app_pointer", "rand_state"]) else { return };
    let app = ee.u32_at(at[0]);
    let event = ee.u32_at(ee.u32_at(app + 0x3d4) + 0xc);
    let (difficulty, event_kind) = (ee.read(event + 0x117, 1) as u8, ee.u32_at(event) as i32);
    let Some((image_seek, lists)) = s12e_seek_world(&kit, &ee) else { return };

    // The Sim: the four bikes seeded off the image, in player order.
    let tables = Arc::new(Tables::build());
    let Some(mut bikes) = Bikes::open() else { return };
    let names = C33_RIDERS.map(|(_, _, b)| track::physics_file(track::Engine::Cc250, false, if b == 0 { C33_HUMAN_TUNING } else { track::Tuning::A }));
    let phys: Vec<Phy> = names.iter().map(|n| bikes.phy(n, &tables)).collect();
    let order = c33_by_player(&ee);
    let seeded = |k: usize| c33_bike(&ee, C33_RIDERS[k].0, &phys[k], &names[k], &bikes.col);
    let mut sim = Sim::with_rider(0, Arc::clone(&tables), phys[order[0]].clone(), seeded(order[0]));
    for &k in &order[1..] {
        sim.add_player(phys[k].clone(), seeded(k));
    }

    // The map's seeker world, as the shell loads it, with the image's slots and zones.
    let mut f = std::fs::File::open(kit.ctx("iso").join("MX.PSR")).expect("MX.PSR");
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).unwrap();
    let psr = game_sim::archive::Psr::parse(&front).unwrap();
    let track_res = super::seek_world::s313_member(&mut f, &psr, "Tracks\\free09.res");
    let scene = game_sim::track::Scene::parse(&super::seek_world::s313_file(&track_res, "tracks\\free09\\free09.scn")).unwrap();
    let terrain_bytes = super::seek_world::s313_file(&track_res, "tracks\\free09\\free09.ps2.trn");
    let terrain = Terrain::parse(&terrain_bytes).unwrap();
    let mut place = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    let data = super::seek_world::s313_member(&mut f, &psr, "Tracks\\free09_2.res");
    let hit_dat = Ini::parse_bytes(&super::seek_world::s313_file(&data, "tracks\\free09\\hit.dat"));
    let mut read = |name: &str| Some(super::seek_world::s313_file(&data, &format!("tracks\\free09\\{name}")));
    let slots = image_seek.targets[0].done.len();
    let open = |player: usize, zone: game_sim::ai::seek::Zone| player == 0 && zone.place == Some(0);
    assert!(sim.load_seek(&hit_dat, &mut read, slots, &open, &mut place), "free09's seeker world");

    // The roster: the image's AI riders in player order, at their skills.
    let skills: Vec<i32> = order[1..].iter().map(|&k| ee.u32_at(C33_RIDERS[k].0 + 0x10d0 + 0x104) as i32).collect();
    let (low, high) = skill_range(difficulty);
    assert!(skills.iter().all(|s| (low..=high).contains(s)), "skills {skills:?} outside the difficulty's {low}..={high}");
    let roster = Roster {
        humans: 1,
        ghost: false,
        ai: skills.iter().map(|&skill| AiRider { identity: game_sim::ai::FIRST_AI_IDENTITY, skill }).collect(),
        all_ai: None,
    };
    // The race's words as the image holds them on the first AI rider (all three agree).
    let pilots: Vec<Pilot> = order[1..].iter().map(|&k| c33_pilot(&ee, &lists, C33_RIDERS[k].2, C33_RIDERS[k].0)).collect();
    for p in &pilots[1..] {
        assert_eq!((p.seat.race_phase, p.seat.path_net, p.seat.live, p.card), (pilots[0].seat.race_phase, pilots[0].seat.path_net, pilots[0].seat.live, pilots[0].card));
    }
    let entry = Entry { race_phase: pilots[0].seat.race_phase, path_net: pilots[0].seat.path_net, live: pilots[0].seat.live, card: pilots[0].card };
    let before = Rng::from_state(s12e_u64(&ee, at[1]));
    sim.spawn_ai(&roster, Some(&character), difficulty, event_kind, &entry, before);
    assert_eq!(sim.seek, None, "the lane takes the seeker world");
    let lane = sim.ai.as_deref().expect("a lane");

    // The draws: ten for each brain and two for each reader.
    let mut twelve = before;
    (0..12 * pilots.len()).for_each(|_| _ = twelve.rand());
    assert_eq!(lane.rng.state(), twelve.state(), "the spawns' draws");

    // The seeker world, against the image's owners.
    let mut wrong = Vec::new();
    for (i, (ours, theirs)) in lane.seek.sets.iter().zip(&image_seek.sets).enumerate() {
        if ours.owner != theirs.owner {
            wrong.push(format!("set {i}'s owner: {:?}, the image's {:?}", ours.owner, theirs.owner));
        }
    }
    let owned = game_sim::ai::seek::World { sets: image_seek.sets.clone(), ..lane.seek.clone() };
    if owned != image_seek {
        wrong.push("the seeker world (owners aside) is not the image's".into());
    }

    // The pilots, against the image's.
    assert_eq!(lane.pilots.iter().map(|(p, _)| *p).collect::<Vec<_>>(), [1, 2, 3], "the AI players");
    let mut compared = 0;
    for ((player, ours), theirs) in lane.pilots.iter().zip(&pilots) {
        let who = format!("player {player}");
        let mut same = |what: &str, a: String, b: String| {
            compared += 1;
            if a != b {
                wrong.push(format!("{who}: {what} is {a}, the image's {b}"));
            }
        };
        same("seat", format!("{:?}", ours.seat), format!("{:?}", theirs.seat));
        let (o, t) = (&ours.racer, &theirs.racer);
        same("interface type", format!("{:?}", o.brain.kind), format!("{:?}", t.brain.kind));
        same("mode", format!("{}", o.brain.mode), format!("{}", t.brain.mode));
        same("spawn flags", format!("{:#x}", game_sim::ai::SPAWN_FLAGS), format!("{:#x}", t.brain.flags & game_sim::ai::SPAWN_FLAGS));
        same("tuning", format!("{:?}", o.tuning), format!("{:?}", t.tuning));
        same("avoidance", format!("{:?}", o.avoid), format!("{:?}", t.avoid));
        same("vehicle", format!("{:?}", o.vehicle), format!("{:?}", t.vehicle));
        same("reach", format!("{:?}", (o.reach.base, o.reach.max)), format!("{:?}", (t.reach.base, t.reach.max)));
        same("trick plan", format!("{:?}", (o.plan.cooldown, o.plan.chance)), format!("{:?}", (t.plan.cooldown, t.plan.chance)));
        same("filter", format!("{:?}", (o.filter.rate, o.filter.jitter, o.filter.gain)), format!("{:?}", (t.filter.rate, t.filter.jitter, t.filter.gain)));
        same("weights", format!("{:?}", o.weights), format!("{:?}", t.weights));
        same("least speed", format!("{:?}", o.min_speed), format!("{:?}", t.min_speed));
        let numbers = |h: &game_sim::ai::Habits| {
            format!(
                "{:?}",
                [
                    h.low_air, h.preload_per_speed, h.pull_back_amount, h.lag_hang_time, h.lag_chance, h.lag_time, h.horse_power,
                    h.brilliant_chance, h.brilliant_min, h.brilliant_random, h.cross_up_chance, h.cross_up_min, h.cross_up_max,
                    h.cross_up_height, h.low_air_bar,
                ]
            )
        };
        same("habits' numbers", numbers(&o.habits), numbers(&t.habits));
    }
    assert!(compared >= 3 * 13, "only {compared} words compared");
    assert_eq!(lane.tables.curve, controls::tables(&ee).curve, "the controllers' curve table");
    assert!(wrong.is_empty(), "the lane at the load is not C33's:\n{}", wrong.join("\n"));
}
