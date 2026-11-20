// ---------------------------------------------------------------------------------------------------
// S12e — free09's four racers through the Sim: C33's capture replayed
// ---------------------------------------------------------------------------------------------------
//
// C33 saved free09 with four racers at frame_counter 2, all four bikes still on the spawn line, and
// recorded 1800 ticks from there: every rider's body and the generator, three times at three emulator
// speeds, with the rows two of the three agree on kept (`free09-ai4-agreed.csv`). This seeds a Sim off
// that image: each of the four riders built as the shell builds one and seeded as S174 seeds the 2p
// riders (chassis, wheels, drivetrain, balance, air, levers), the three AI brains read out of RAM into
// pilots as S323 reads recording2's, and free09 off the disc as the world. Then `Sim::step_all_posed` runs
// the four, each AI rider's brain thinking after its bike's frame (the bike riding the levers of the
// tick before, `Sim::think`, the others read off their cards, S330), and every agreed row's generator
// and AI bodies are held against the port's.
//
// The AI bodies part from the original early (`C33_AGREES_THROUGH`); the test holds the agreement it
// has reached, so that a fix shows as a raised mark and a regression as a failure. The human, with no
// pad but the stick its image was saved holding on the first step, agrees on every row (S329).

use super::draws::{C33_DUST, C33_NOTE_PICKS, C33_NOTES_ON, c33_dust, c33_looks, c33_tracks};
use super::lane::s323_free09;
use super::sim::{s12e_bike, s12e_racer, s12e_seek_world, s12e_u64};
use super::*;
use game_render::disc::Disc;
use game_sim::Sim;
use game_sim::ai::{Lane, Pilot, Seat};
use game_sim::bike_col::{self, BikeCol};
use game_sim::eco::Wooded;
use game_sim::{dust, engine_note, tire_track};
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
/// to it but the torn ones: the regression mark. S482: the free ride's jump watch took the generator
/// from 84 to 130 and ai0 from 296 to 318; ai2 went from 280 to 278, on the draws after the
/// generator parts at 131 (with the generator put back to the capture's each row, it agrees to 729).
/// S482a: an AI rider's note reads its brain's clutch, and the generator agrees through 131 (it parts
/// at 132 on ai0's foot dust, which needs the riding ragdoll this replay does not seed). The human
/// went from 1802 to 944: ai1, off the original's since 304, now rides into it there, where before
/// its own fiction of a path happened to miss it. S482b: each rider rides its riding body off the
/// image, posed each tick by the shell's body machine, and ai0's foot ball touches at 131 as the
/// original's: the generator agrees through 133 (it parts at 134), the human through 1802 again and
/// ai2 through 279. S347b: ai2's preload pop is not where it parts. With the generator put back to
/// the capture's each row, ai2 agrees through 729 and ai0 through 373. So all of ai2's early split,
/// and ai0's from 319 to 373, comes after the generator. The generator parts at 134 because ai0's
/// foot dust (entry 4) draws on 134. The original's foot ball has left the ground after 133 (bone 17's
/// y is 150.764 against our 150.744). That gap goes back to tick 4. There every image body is Held
/// and unplaced, and both sides switch to Riding and pose it. The original poses from its animated
/// skeleton (bones within ~0.1 ft of the image's). The Sim has no `rider.skeleton` before the first
/// pose, so it poses from the rig's fixed pose, ~1 ft off (all four riders, ~1.26 ft, decaying to
/// ~0.2 by tick 15). S347b1: each body is seeded with that skeleton (the actor's joint records) and
/// its parts' drawn matrices, and tick 4's foot is within 0.01 ft of the original's across the ground
/// (0.09 ft high, from ~0.7 ft off). The marks do not move: from tick 14 on both skeletons agree to
/// 0.005 ft, yet ai0's bones sit ~0.14 ft further along z than the original's from 15 through 133,
/// seeded or not. That steady gap, not the first pose, is what parts the foot dust at 134. R50: the
/// S347b1 seed put the parts' drawn matrices (at their centres of mass) where the riding entry reads
/// node places; seeded with the node matrices, tick 4's bones and velocities match to 1 ulp. Holds
/// are not the gap: handing the Sim the original's holds each tick leaves bone 17 0.15 ft off at
/// 133. The gap is the pose spring's target, the skeleton the render Body hands `rider.skeleton`:
/// its clip machine starts fresh, not from the image's, and is 0.3-0.5 ft off from tick 5. Fed the
/// original's skeleton instead (the actor's joint records after frame t, under the bike node from
/// before it), ai0's bones stay within 0.04 ft and are 0.004 ft off at 120. The marks do not move.
/// S488: each render Body is seeded from its actor's animation state (`c33_anim_seed`: machine,
/// clock, snapshot, and the bike driver's pose and fore-aft its axes chase) and keeps the drawn nodes
/// as the shell does (`c33_drawn`). Checked beside an interpreter copy stepped with `draws::whole_frame`,
/// ai0's joints now match the original's after every frame bit for bit, but for the two fades after
/// the restarts at 64 and 120 (0.034 and 0.097 in a quaternion, gone by 73 and 127). The marks do not
/// move: the skeleton is the original's one frame late. The Sim's riding step in frame t poses to the
/// joints `c33_pose` left after frame t-1; the original poses its body inside frame t, before its
/// ragdoll step. Fed the copy's joints of frame t (under its bike node from before it), bone 17 stays
/// within 0.06 ft through 131 where the seeded run is 0.29 ft off at 130. Either way ai0's bodies
/// stray up to 0.45 ft at 24-36 (part 7), the same with or without the seed. S490: the Sim's riding
/// step can take the frame's own skeleton (`Sim::step_all_posed`), and posed so, ai0's bones follow
/// the original's much closer (bone 17 0.02 ft at 14 where this replay is 0.13), but the marks would
/// fall: the foot ball touches at 130, a tick early, and the generator would part at 131. Part 7 is a
/// hand: this replay keeps the image's holds, so the grips never turn with the bars. With the
/// original's holds of each frame, every part stays within 0.08 ft through 130
/// (`s490_c33s_bodies_posed_in_the_frame_follow_the_original`); the early touch stays even then.
/// T9: each rider's chassis is drawn after its body as the shell's `ai_ride` draws it, and its grips
/// are the next step's holds (`c33_hold`). The marks do not move. Drawn in the frame, those holds
/// are the original's within 0.004 ft but for the chassis' fresh start (to 16) and each AI rider's
/// bars jump (S490_HOLDS_APART). Ridden in the frame, ai0's parts stay within 0.093 ft through 130
/// (part 7 within 0.01 at 24-36). This replay goes in the frame once B42 moves the early touch.
/// B42: the main bone is set with the last step's rest, and ridden in the frame ai0's parts stay
/// within 0.0066 ft through 134; the foot dust draws on 132 with the original's (T9a).
/// T9a: this replay poses each body in the frame, before its riding step, and draws its chassis after
/// it, whose grips are the holds that step rides (the first step's too, not the image's holds). The generator now agrees through 172 (from 133; it parts at 173) and ai2 through
/// 280 (from 279); ai0, ai1 and the human do not move. Ridden so, ai0's foot dust draws on 132 with
/// the original's, as `C33_DUST_THROUGH` holds. R56: the chassis starts on the image's pose
/// (`c33_chassis_seed`), not fresh; none of these move.
const C33_AGREES_THROUGH: [u64; 5] = [318, 303, 1802, 280, 172];

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
pub(crate) struct Bikes {
    disc: Disc,
    pub(crate) col: Arc<BikeCol>,
}

impl Bikes {
    pub(crate) fn open() -> Option<Bikes> {
        let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
        let res = disc.res("Tracks\\free09_2.res").expect("free09's second archive");
        let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
        let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
            .expect("the bike's collision files");
        Some(Bikes { disc, col: Arc::new(col) })
    }

    /// `name` (`Bike\Race250A.phy`) out of whichever bike archive carries it.
    pub(crate) fn phy(&mut self, name: &str, tables: &Tables) -> Phy {
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

    /// T9: the chassis `sub` (`250SX.sub`) names, out of whichever bike archive carries it, loaded
    /// as the shell loads an AI rider's bike (no stunts, as [`super::wreck_dust::s483_ragdoll_body`]'s
    /// body).
    pub(crate) fn chassis(&mut self, sub: &str) -> game_render::vehicle::Chassis {
        let bikes: Vec<String> = self
            .disc
            .entries()
            .iter()
            .map(|e| e.name.clone())
            .filter(|n| n.to_ascii_lowercase().starts_with("bike\\") && n.to_ascii_lowercase().ends_with(".res"))
            .collect();
        let res = bikes
            .iter()
            .find_map(|b| self.disc.res(b).ok().filter(|r| r.find_suffix(sub).is_some()))
            .unwrap_or_else(|| panic!("{sub} in no bike archive"));
        let vehicle = game_render::scene::Vehicle::parse(&game_render::ini::text(&res.read_suffix(sub).unwrap_or_else(|e| panic!("{e}"))));
        game_render::vehicle::Chassis::load(vec![res], &vehicle.mcf, "bike_", &[], game_render::vehicle::params())
            .unwrap_or_else(|e| panic!("{sub}'s chassis: {e}"))
    }
}

/// The rider at `r`, built on `phy` (named `name`) and seeded off the image.
pub(crate) fn c33_bike(ee: &Ee, r: u32, phy: &Phy, name: &str, col: &Arc<BikeCol>) -> Bike {
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
    // The body's rates, as the shell fills them (S328a: the grid's first threshold cancels the look
    // clock), and the image's body holds the same threshold.
    bike.body_rates = game_render::vehicle::params();
    assert_eq!(im.float(im.at(0xdb4) + 0xa14), bike.body_rates.band[0], "rider {r:#x}'s body's first band");
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

/// The Sim's player for each of [`C33_RIDERS`]: the game's own player number ([`c33_by_player`]: the
/// human 0, ai2 1, ai0 2, ai1 3). `Sim::step_all` walks its players from the last to the first, and
/// so does the original's frame: ai1, ai0, ai2, then the human, not the actor list's order (S338:
/// the order its collision resolves run in, in the interpreter). It shows where two riders touch:
/// at tick 29 ai0's push off ai1 reads ai1 as ai1's frame has just left it.
fn c33_player(ee: &Ee, k: usize) -> usize {
    c33_by_player(ee).iter().position(|&j| j == k).unwrap()
}

#[test]
fn s12e_free09s_four_racers_ride_as_the_capture() {
    c33_replay(false);
}

/// S490: the riding bodies posed inside the frame, as the original poses its body in frame t before
/// its ragdoll step ([`Sim::step_all_posed`]), and each rider's holds the frame's own (the original's
/// request after frame t, off an interpreter copy stepped beside the Sim, [`c33_frame_holds`]): ai0's
/// riding bones stay within [`S490_BONES_WITHIN`] of the copy's through [`S490_THROUGH`]. Posed
/// after the step instead (the replay above), they are 0.43 ft off at 128; with the image's holds
/// kept, part 7 (a hand) strays 0.42 ft at 24-36, the grips not turning with the bars. T9: each
/// rider's chassis is drawn in the frame too, and its holds are held to the original's
/// ([`S490_HOLDS_WITHIN`]).
#[test]
fn s490_c33s_bodies_posed_in_the_frame_follow_the_original() {
    c33_replay(true);
}

/// How far any of ai0's riding parts may be from the original's in [`c33_replay`]'s in-frame run, in
/// feet: 0.0066 is the worst seen (part 15 at 35). Before B42 it was 0.08 (0.04 to 0.05 at 127-131,
/// after the restart at 120), the main bone set with this frame's rest and not the last one's.
const S490_BONES_WITHIN: f32 = 0.01;
/// The last tick the in-frame run is held to: past the foot dust's 132 and the foot ball's lift
/// after 133. Before B42 ai0's foot ball (bone 17) was 0.02 ft low at 130 and touched a tick early,
/// its foot dust drawing on 131; now it draws on 132 with the original's. Held through 400, the
/// bones stay within [`S490_BONES_WITHIN`] through 320 (ai0's ride parts at 319); its drawn holds
/// jump at the take-off at 198 too (R56, [`S490_HOLDS_APART`]).
const S490_THROUGH: u64 = 134;

/// T9: how far each rider's drawn chassis holds ([`c33_hold`], drawn in the frame after its body)
/// may be from the original's request after the frame ([`c33_frame_holds`]), in feet: the root's
/// place and the four points, in the bike's frame. The run still rides the original's. 0.0041 is
/// the worst seen outside [`S490_HOLDS_APART`]. Held from the first row (R56): the chassis starts on
/// the image's pose ([`c33_chassis_seed`]); fresh, every rider's root was 0.07 ft off at 4 and closed
/// over the restart's fade to 16, the first restart fading out of the bind pose.
const S490_HOLDS_WITHIN: f32 = 0.006;
/// The ticks each of [`C33_RIDERS`]' drawn holds are let go: both an AI rider's grips (the bars) jump
/// from the original's at one tick (ai0 0.13 ft at 64, ai1 0.09 at 63, ai2 0.03 at 54) and close
/// linearly over about ten; the feet and the root stay within 0.003 ft. The human's never part.
/// R56: each is a take-off. Ours snaps, the original's ramps: the chassis' restart into the air row
/// fades from its shown pose, kept before the drives, so without the bars' steering turn, and in the
/// air nothing turns them; the original's fades from the turned bars. Writing the turn into the
/// shown pose closes every one (B45, which empties this).
const S490_HOLDS_APART: [std::ops::RangeInclusive<u64>; 4] = [64..=73, 63..=72, 1..=0, 54..=62];

/// The original's request after the frame, per rider of [`C33_RIDERS`]: the root node and the four
/// holds' world points (rider `+0xb80`: `+0x50`, `+0xb0..+0xe0`), as [`super::wreck_dust::s483_riding_of`]
/// reads them off the image.
fn c33_frame_holds(copy: &Ee) -> Vec<(game_sim::math::Mat4, [Vf; 4])> {
    use game_sim::math::Mat4;
    let quad = |at: u32| Vf::from_bits([0, 4, 8, 12].map(|o| copy.u32_at(at + o)));
    C33_RIDERS
        .iter()
        .map(|&(r, _, _)| {
            let req = copy.u32_at(super::wreck_dust::rider_actor(copy, r) + 0x488);
            (Mat4([0, 0x10, 0x20, 0x30].map(|o| quad(req + 0x50 + o))), [0xb0, 0xc0, 0xd0, 0xe0].map(|o| quad(req + o)))
        })
        .collect()
}

/// How far each of rider `k`'s riding parts' nodes are from the copy's (each part's node world,
/// part `+0xa0` → node `+0x80`), in feet.
fn c33_part_offs(copy: &Ee, sim: &Sim, k: usize) -> Vec<f32> {
    let rider = sim.rider_of(c33_player(copy, k));
    let (Some(r), Some(kit)) = (&rider.riding, &rider.ragdoll_kit) else { return Vec::new() };
    let parts = copy.u32_at(super::wreck_dust::rider_actor(copy, C33_RIDERS[k].0) + 0x2f4);
    r.nodes(kit)
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let w = copy.u32_at(parts + 0xb0 * i as u32 + 0xa0) + 0x80 + 0x30;
            (0..3).map(|c| (copy.f32_at(w + 4 * c as u32) - m.0[3][c]).powi(2)).sum::<f32>().sqrt()
        })
        .collect()
}

/// C33's replay. `in_frame`: S490's run, the bodies posed inside the frame on the copy's holds and
/// held to the copy's bones through [`S490_THROUGH`], where it stops; else S12e's, posed inside the
/// frame on its own drawn chassis holds (T9a), held to its marks.
fn c33_replay(in_frame: bool) {
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
    order.sort_by_key(|&k| c33_player(&ee, k));
    let seeded = |k: usize| c33_bike(&ee, C33_RIDERS[k].0, &phys[k], &names[k], &bikes.col);
    let mut sim = Sim::with_rider(0, Arc::clone(&tables), phys[order[0]].clone(), seeded(order[0]));
    for &k in &order[1..] {
        sim.add_player(phys[k].clone(), seeded(k));
    }
    let pilots: Vec<(usize, Pilot)> = C33_RIDERS
        .iter()
        .enumerate()
        .filter(|(_, (_, _, b))| *b != 0)
        .map(|(k, &(r, _, b))| (c33_player(&ee, k), c33_pilot(&ee, &lists, b, r)))
        .collect();
    let gen0 = s12e_u64(&ee, rand_state);
    sim.ai = Some(Box::new(Lane { seek, tables: Arc::new(controls::tables(&ee)), rng: Rng::from_state(gen0), pilots, net: None, cards: Vec::new(), edge: None }));
    // S328b: the four engine notes, as the image holds them, on the lane's generator.
    sim.notes = Some(Box::new(c33_notes(&ee)));
    // S482b: the riding bodies, as the image holds them, whose bone collisions the foot dust reads.
    // Each rider's body machine poses its skeleton inside the frame, before its riding step (T9a).
    let Some(loaded) = C33_RIDERS.iter().map(|_| super::wreck_dust::s483_ragdoll_body()).collect::<Option<Vec<_>>>() else { return };
    let mut kits = Vec::new();
    let mut bodies: Vec<_> = loaded.into_iter().map(|(body, doll, kit, _, _)| {
        kits.push(kit);
        (body, doll)
    }).collect();
    c33_riding_seed(&ee, &mut sim, &Arc::new(kits.swap_remove(0)), &mut bodies);
    // T9: each rider's bike as the shell draws it, whose grips are the holds of every step after the
    // first (that one rides the image's).
    let mut chassis: Vec<_> = (0..C33_RIDERS.len()).map(|k| bikes.chassis(S363_C33_MODELS[c33_player(&ee, k)])).collect();
    for (k, ch) in chassis.iter_mut().enumerate() {
        c33_chassis_seed(&ee, C33_RIDERS[k].0, ch);
    }
    // S328c: the bikes' emitters, as the image holds them, on the lane's generator.
    sim.dust = Some(Box::new(c33_dust_seed(&ee, &sim)));
    // S448: the tyre tracks, as the image holds them, and each one's entry in its rider's manager.
    let (tracks, track_entries) = c33_tracks_seed(&ee);
    sim.tracks = Some(Box::new(tracks));

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
    controls[c33_player(&ee, 2)] = c33_saved_controls(&ee, C33_RIDERS[2].0);
    // S490: the interpreter copy the in-frame run steps beside the Sim, and the worst part gap seen.
    let mut copy = in_frame.then(|| {
        let mut copy = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
        copy.set(28, u64::from(addr("boot_gp").unwrap()));
        let clock = f64::from(copy.u32_at(addr("wall_ticks").unwrap())) * 100.0;
        super::draws::render_setup(&mut copy);
        (copy, clock)
    });
    let mut worst = (0.0f32, 0, 0);
    let mut worst_hold = (0.0f32, 0, 0);
    let by_player: Vec<usize> = (0..C33_RIDERS.len()).map(|p| (0..C33_RIDERS.len()).find(|&k| c33_player(&ee, k) == p).unwrap()).collect();
    for (n, pair) in rows.windows(2).enumerate() {
        let (last, row) = (&pair[0], &pair[1]);
        assert_eq!(row.frame, C33_FROM + 2 + n as u64, "the capture's rows run on from the image, one a tick");
        if in_frame && row.frame > S490_THROUGH {
            break;
        }
        // The step into a row is the frame length the row before it read.
        let looks_were: Vec<F> = (0..C33_RIDERS.len()).map(|k| sim.rider_of(c33_player(&ee, k)).body.look).collect();
        let gen_was = sim.ai.as_ref().unwrap().rng.state();
        match copy.as_mut() {
            None => {
                // T9a: each body posed in the frame, before its riding step, on the levers its bike
                // rode, and its chassis drawn after it: its grips are the holds the step rides.
                sim.step_all_posed(last.dt, &controls, &mut world, &mut |p, rider, hands| {
                    let (body, doll) = &mut bodies[by_player[p]];
                    body.ride(last.dt, rider, hands, game_sim::rider::Gate::default());
                    rider.skeleton = Some(Arc::new(body.ragdoll_posed(doll)));
                    c33_hold(rider, &mut chassis[by_player[p]], body, last.dt);
                });
                for (k, (body, doll)) in bodies.iter_mut().enumerate() {
                    c33_drawn(sim.rider_of(c33_player(&ee, k)), body, doll);
                }
            }
            Some((copy, clock)) => {
                super::draws::neutral_pad(copy);
                super::draws::whole_frame(copy, last.dt, clock);
                super::draws::render(copy);
                let held = c33_frame_holds(copy);
                // Each body posed before its riding step, on the levers its bike rode and the
                // holds framed by the bike as its frame left it, as the riding step frames them.
                sim.step_all_posed(last.dt, &controls, &mut world, &mut |p, rider, hands| {
                    use game_sim::math;
                    let (root, at) = &held[by_player[p]];
                    let inv = math::inverse(&rider.ride.transform()).unwrap();
                    let local = |v: Vf| math::transform_point(&inv, v);
                    let theirs = game_sim::ragdoll_riding::Holds { root: math::mul(root, &inv), hands: [local(at[0]), local(at[1])], feet: [local(at[2]), local(at[3])] };
                    let (body, doll) = &mut bodies[by_player[p]];
                    body.ride(last.dt, rider, hands, game_sim::rider::Gate::default());
                    rider.skeleton = Some(Arc::new(body.ragdoll_posed(doll)));
                    // T9: the chassis drawn in the frame, its holds against the original's.
                    c33_hold(rider, &mut chassis[by_player[p]], body, last.dt);
                    let ours = rider.holds.as_deref().unwrap();
                    let gap = |a: Vf, b: Vf| (0..3).map(|c| (a[c] - b[c]).powi(2)).sum::<f32>().sqrt();
                    let off = [gap(ours.root.0[3], theirs.root.0[3]), gap(ours.hands[0], theirs.hands[0]), gap(ours.hands[1], theirs.hands[1]), gap(ours.feet[0], theirs.feet[0]), gap(ours.feet[1], theirs.feet[1])];
                    let off = off.into_iter().fold(0.0f32, f32::max);
                    let held = !S490_HOLDS_APART[by_player[p]].contains(&row.frame);
                    assert!(!held || off <= S490_HOLDS_WITHIN, "tick {}: {}'s drawn holds are {off} ft from the original's", row.frame, C33_NAMES[by_player[p]]);
                    if held && off > worst_hold.0 {
                        worst_hold = (off, row.frame, by_player[p]);
                    }
                    rider.holds = Some(Arc::new(theirs));
                });
                for (k, (body, doll)) in bodies.iter_mut().enumerate() {
                    c33_drawn(sim.rider_of(c33_player(&ee, k)), body, doll);
                }
                let offs = c33_part_offs(copy, &sim, 0);
                assert!(!offs.is_empty(), "tick {}: ai0 rides no riding body", row.frame);
                for (i, &off) in offs.iter().enumerate() {
                    assert!(off <= S490_BONES_WITHIN, "tick {}: ai0's part {i} is {off} ft from the original's", row.frame);
                    if off > worst.0 {
                        worst = (off, row.frame, i);
                    }
                }
            }
        }
        // S328b: the notes switch on and pick as the original's do (draws.rs's C33_NOTES_ON and
        // C33_NOTE_PICKS), through 16.
        if row.frame <= 16 {
            let notes = &sim.notes.as_ref().unwrap().riders;
            for (k, name) in C33_NAMES.iter().enumerate() {
                let note = &notes[c33_player(&ee, k)];
                if let Some(&(on, _)) = C33_NOTES_ON.iter().find(|(_, n)| n == name) {
                    assert_eq!(note.on, row.frame >= on, "tick {}: {name}'s note switched on", row.frame);
                }
                let (tick, picked, index) = C33_NOTE_PICKS;
                let want = match row.frame >= tick && picked.contains(name) {
                    true => (engine_note::Note::LowAccel, index),
                    false => (engine_note::Note::Idle, 0),
                };
                assert_eq!((note.brain.note, note.brain.current.index), want, "tick {}: {name}'s note", row.frame);
            }
        }
        // S328c: the emitters draw as the original's do (draws.rs's C33_DUST) while the riders' bodies
        // agree, each through its C33_AGREES_THROUGH and the table's window.
        if row.frame <= C33_DUST_THROUGH {
            let dust = &sim.dust.as_ref().unwrap().riders;
            let mut drew: Vec<(&'static str, usize)> = Vec::new();
            for (k, name) in C33_NAMES.iter().enumerate() {
                if row.frame > C33_AGREES_THROUGH[k] {
                    continue;
                }
                for (i, e) in dust[c33_player(&ee, k)].entries.iter().enumerate() {
                    if e.draws != 0 {
                        drew.push((name, i));
                    }
                }
            }
            drew.sort();
            let want: Vec<(&'static str, usize)> = c33_dust(row.frame).into_iter().filter(|(n, _)| row.frame <= C33_AGREES_THROUGH[C33_NAMES.iter().position(|m| m == n).unwrap()]).collect();
            assert_eq!(drew, want, "tick {}: the emitters that drew", row.frame);
            // From 17 through 28 nothing else draws in the original's sim half (draws.rs), so the
            // Sim's generator steps two a drawing emitter.
            // S448: and the tyre tracks' rows after the sim half (draws.rs's C33_TRACKS), one a row.
            let tracks = &sim.tracks.as_ref().unwrap().riders;
            let mut laid: Vec<(&'static str, usize)> = Vec::new();
            for (k, name) in C33_NAMES.iter().enumerate() {
                if row.frame > C33_AGREES_THROUGH[k] {
                    continue;
                }
                let p = c33_player(&ee, k);
                for (t, track) in tracks[p].tracks.iter().enumerate() {
                    assert!(track.draws <= 1, "tick {}: {name}'s track {} drew {}", row.frame, track_entries[p][t], track.draws);
                    if track.draws != 0 {
                        laid.push((name, track_entries[p][t]));
                    }
                }
            }
            laid.sort();
            let want_laid: Vec<(&'static str, usize)> = c33_tracks(row.frame).into_iter().filter(|(n, _)| row.frame <= C33_AGREES_THROUGH[C33_NAMES.iter().position(|m| m == n).unwrap()]).collect();
            assert_eq!(laid, want_laid, "tick {}: the tracks that laid a row", row.frame);
            if (17..=28).contains(&row.frame) {
                let mut g = Rng::from_state(gen_was);
                let steps = (0..=64).find(|_| g.state() == sim.ai.as_ref().unwrap().rng.state() || { g.rand(); false });
                assert_eq!(steps, Some(2 * want.len() + want_laid.len()), "tick {}: the Sim's draws", row.frame);
            }
        }
        // S328a: the bodies' look-around clocks re-arm on the generator as the original's do
        // (draws.rs's C33_LOOKS), through 14: ai0 and ai2 launch at 15.
        if row.frame < 15 && last.agreed {
            let rearmed: Vec<(&'static str, F)> = (0..C33_RIDERS.len())
                .filter(|&k| looks_were[k] <= F::ZERO && sim.rider_of(c33_player(&ee, k)).body.look > F::ZERO)
                .map(|k| (C33_NAMES[k], sim.rider_of(c33_player(&ee, k)).body.look))
                .collect();
            let mut want = c33_looks(row.frame, last.rng);
            want.sort_by_key(|l| C33_NAMES.iter().position(|n| *n == l.0));
            let bits = |v: &[(&'static str, F)]| v.iter().map(|&(n, l)| (n, l.0.to_bits())).collect::<Vec<_>>();
            assert_eq!(bits(&rearmed), bits(&want), "tick {}: the look-around clocks the Sim re-armed", row.frame);
        }
        // The human's pad from the second step on: none, so nothing held.
        controls[c33_player(&ee, 2)] = Controls::default();
        if !row.agreed {
            continue;
        }
        for (k, lane) in lanes[..4].iter_mut().enumerate() {
            let ours = c33_ours(sim.rider_of(c33_player(&ee, k)));
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
    if in_frame {
        println!("posed in the frame, ai0's riding parts are within {:.4} ft of the original's through tick {S490_THROUGH} (worst part {} at tick {})", worst.0, worst.2, worst.1);
        println!("the drawn chassis' holds are within {:.4} ft of the original's (worst {} at tick {})", worst_hold.0, C33_NAMES[worst_hold.2], worst_hold.1);
        return;
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

/// S328b: a rider's engine note (`+0x1494`) and the delay its update switches it on after
/// (`+0x14a4`, the loader's unit draw), the note's words as `tests/audio.rs`'s oracle reads them.
mod c33_note {
    /// The lists' counts and arrays, in `Note::slot` order.
    pub const LISTS: [u32; 8] = [0x204, 0x21c, 0x234, 0x264, 0x27c, 0x24c, 0x2ac, 0x294];
    pub const FAR: u32 = 0x8c;
    pub const AT: u32 = 0xb0;
    pub const SWITCHED: u32 = 0x29;
    pub const LAST: u32 = 0x2c4;
    pub const NOTE: u32 = 0x2d8;
    pub const NEXT: u32 = 0x2dc;
    pub const OFF_GAS: u32 = 0x2e0;
    pub const AIR_TO_DECEL: u32 = 0x2e4;
    pub const AIR: u32 = 0x2e8;
    pub const HOLD_OFF_GAS: u32 = 0x2ec;
    pub const AIR_REVS: u32 = 0x2f0;
    pub const WAS_GAS: u32 = 0x2f4;
    pub const HELD: u32 = 0x2f8;
    pub const MUTED: u32 = 0x2fc;
    pub const LOW_TORQUE: u32 = 0x300;
    pub const OVERLAP: u32 = 0x304;
    pub const VARIANCE: u32 = 0x308;
    pub const MIDPOINT: u32 = 0x30c;
    /// One sound's length in seconds and its looping flag.
    pub const LENGTH: u32 = 0x24;
    pub const LOOPING: u32 = 0xbc;
}

/// The image's four engine notes as the Sim's, by Sim player. Each is still the loader's: idle
/// queued, nothing picked, not yet switched on.
fn c33_notes(ee: &Ee) -> engine_note::Notes {
    use c33_note::*;
    use engine_note::{Note, NoteSounds, RiderNote};
    let mut riders: Vec<(usize, RiderNote)> = C33_RIDERS
        .iter()
        .enumerate()
        .map(|(k, &(r, _, _))| {
            let n = ee.u32_at(r + 0x1494);
            assert_eq!(ee.read(n + SWITCHED, 1) & 3, 0, "{}'s note is switched on already", C33_NAMES[k]);
            assert_eq!((ee.i32_at(n + NOTE), ee.i32_at(n + NEXT)), (Note::Idle as i32, Note::Off as i32), "{}'s note", C33_NAMES[k]);
            let lists = LISTS.map(|l| {
                let array = ee.u32_at(n + l + 8);
                (0..ee.i32_at(n + l).max(0) as u32)
                    .map(|i| ee.u32_at(array + 4 * i))
                    .map(|p| (ee.f32_at(p + LENGTH), ee.i32_at(p + LOOPING) != 0))
                    .collect()
            });
            let sounds = NoteSounds {
                lists,
                low_torque_speed: ee.f32_at(n + LOW_TORQUE),
                airborne_to_decel: ee.f32_at(n + AIR_TO_DECEL),
                rev_midpoint: ee.f32_at(n + MIDPOINT),
                overlap: ee.f32_at(n + OVERLAP),
                pitch_variance: ee.i32_at(n + VARIANCE),
                far: ee.f32_at(n + FAR),
            };
            let mut note = RiderNote::new(Arc::new(sounds), F(ee.f32_at(r + 0x14a4)));
            let b = &mut note.brain;
            b.off_gas = ee.f32_at(n + OFF_GAS);
            b.air = ee.f32_at(n + AIR);
            b.hold_off_gas = ee.i32_at(n + HOLD_OFF_GAS) != 0;
            b.air_revs = ee.i32_at(n + AIR_REVS) != 0;
            b.was_gas = ee.i32_at(n + WAS_GAS) != 0;
            b.held = ee.i32_at(n + HELD) != 0;
            b.muted = ee.i32_at(n + MUTED) != 0;
            b.last = [0, 4, 8, 12, 16].map(|o| ee.i32_at(n + LAST + o));
            note.at = Some(Vf([0, 4, 8, 12].map(|o| ee.f32_at(n + AT + o))));
            (c33_player(ee, k), note)
        })
        .collect();
    riders.sort_by_key(|r| r.0);
    engine_note::Notes { riders: riders.into_iter().map(|r| r.1).collect() }
}

/// S482b: each rider's riding body off the image (wreck_dust.rs's [`s483_riding_of`] on the rider's
/// own body actor), on the ragdoll's kit, with the holds of its last request and the clip it read.
///
/// [`s483_riding_of`]: super::wreck_dust::s483_riding_of
///
/// S347b1: with the skeleton the original's first pose reads ([`c33_posed`]) and its parts' drawn
/// matrices (part `+0x60`), so tick 4's Held-to-Riding pose is the original's and not the rig's.
fn c33_riding_seed(
    ee: &Ee,
    sim: &mut Sim,
    kit: &Arc<game_sim::ragdoll_ride::Kit>,
    bodies: &mut [(game_render::rider::Body, game_render::rider::Ragdolled)],
) {
    for (k, &(r, _, _)) in C33_RIDERS.iter().enumerate() {
        let p = c33_player(ee, k);
        let rider = if p == 0 { &mut sim.rider } else { &mut sim.others[p - 1].rider };
        rider.carries_ragdoll(Arc::clone(kit));
        let a = super::wreck_dust::rider_actor(ee, r);
        let m = rider.ride.transform();
        let (mut riding, holds) = super::wreck_dust::s483_riding_of(ee, kit, &m, a);
        let parts = ee.u32_at(a + 0x2f4);
        let quad = |at: u32| Vf::from_bits([0, 4, 8, 12].map(|o| ee.u32_at(at + o)));
        // The riding entry reads Riding.drawn as node places, so seed it from each part's node
        // world matrix (node+0x80), not the drawn matrix at part+0x60, which already sits at the
        // part's centre of mass and would add that offset twice (R50: the main bone sat 0.19 ft
        // low on tick 4 and its velocity 1.6 ft/s off).
        let nodes: Vec<_> = (0..kit.rig.parts.len() as u32)
            .map(|i| {
                let world = ee.u32_at(parts + 0xb0 * i + 0xa0) + 0x80;
                game_sim::ragdoll_pose::Bone { at: quad(world + 0x30), forward: quad(world + 0x20), up: quad(world + 0x10) }
            })
            .collect();
        riding.seed_drawn(game_sim::ragdoll_ride::drawn(&nodes));
        // B42: the main part's joint record (`+0x234`, 0x60 a joint, rest at `+0x50`), which the
        // first step's set reads before that frame's normalise call rewrites it.
        let main_joint = bodies[k].1.nodes[game_sim::ragdoll::MAIN] as u32;
        assert_eq!(ee.u32_at(parts + 0x40), main_joint, "the main part's joint");
        riding.seed_main_rest(quad(ee.u32_at(a + 0x234) + 0x60 * main_joint + 0x50));
        rider.body.veh.state = riding.actor.clip;
        rider.riding = Some(Box::new(riding));
        rider.holds = Some(Arc::new(holds));
        let (body, doll) = &mut bodies[k];
        rider.skeleton = Some(Arc::new(c33_posed(ee, a, r, body, doll, &m)));
        c33_anim_seed(ee, a, rider, body);
        c33_drawn(rider, body, doll);
    }
}

/// S488: body actor `a`'s animation state as the rider's body machine and its render body keep it,
/// so the first in-frame pose steps on from the image's clip, blend and phase and not from a machine
/// built fresh. The offsets are `watch/rider-joints.toml`'s (motion.rs's V12 seeds from the same):
/// the machine (`+0x680..+0x6a4`, `+0x8dc..+0x8fc`, the added record `+0x9f0`, the reverse clock
/// `+0xa3c`, the two axis lags behind `+0x630`/`+0x634`), the clock (`+0x3c` finished, `+0x48..+0x94`)
/// with its clips turned into the body's through the actor's clip table (`+0x900`), and the pose
/// snapshot's two buffers (`+0x248` rotations, `+0x24c` translations) that a running fade reads.
fn c33_anim_seed(ee: &Ee, a: u32, rider: &mut game_sim::rider::Rider, body: &mut game_render::rider::Body) {
    use game_sim::vehicle_anim::Lag;
    let f = |at: u32| F(ee.f32_at(a + at));
    let on = |at: u32| ee.u32_at(a + at) != 0;
    let lib = &body.pose_lib;
    let id = |at: u32| {
        let ptr = ee.u32_at(a + at);
        (ptr != 0).then(|| (0..60).find(|&s| ee.u32_at(a + 0x900 + 4 * s) == ptr).and_then(|s| lib.clip[s as usize])).flatten()
    };
    let lag = |at: u32| {
        let l = ee.u32_at(a + at);
        Lag { value: F(ee.f32_at(l)), tau: F(ee.f32_at(l + 4)), ratio: F(ee.f32_at(l + 8)) }
    };
    let veh = &mut rider.body.veh;
    veh.want = ee.i32_at(a + 0x8f4);
    veh.state = ee.i32_at(a + 0x8f0);
    veh.prev = ee.i32_at(a + 0x8ec);
    veh.seq = (ee.u32_at(a + 0x8fc) & 0xff) as u8;
    veh.rate = f(0x690);
    veh.ramp = f(0x68c);
    veh.started = on(0x688);
    veh.holding = on(0x684);
    veh.from_table = on(0x680);
    veh.cross = f(0x8dc);
    veh.cross_prev = f(0x8e0);
    veh.cross_start = f(0x8e4);
    veh.reverse = f(0xa3c);
    veh.extra = ee.i32_at(a + 0x9f0);
    veh.mix = lag(0x630);
    veh.phase = lag(0x634);
    rider.body.player = game_sim::motion::Player {
        clip: id(0x48),
        last: id(0x50),
        time: f(0x84),
        prev: f(0x54),
        finished: on(0x3c),
        fade: game_sim::motion::Fade {
            on: on(0x80),
            from: id(0x4c),
            clock: f(0x88),
            delay: f(0x90),
            length: f(0x94),
            end: f(0x8c),
        },
    };
    let (rot, pos) = (ee.u32_at(a + 0x248), ee.u32_at(a + 0x24c));
    let rig = &mut body.rig;
    for j in 0..rig.set.under.rot.len() as u32 {
        rig.set.under.rot[j as usize] = [0, 4, 8, 12].map(|o| ee.f32_at(rot + 0x10 * j + o));
        rig.set.under.pos[j as usize] = [0, 4, 8, 12].map(|o| ee.f32_at(pos + 0x10 * j + o));
    }
    rig.snapshot.take(&rig.set.under, None, 0);
    body.posed = true;
    // The body's two axis targets are the bike driver's pose and fore-aft as the next tick's request
    // latches them (`Rider::packed_pose`), so those come off the image's rider too (replay.rs's
    // `seed_2p_rider`: `+0x9e8`, `+0x9ec`, the fade flag `+0x9f0` and ramp `+0x9f4`). Unseeded, they
    // start near 0 against the image's 0.5 and pull the body's axes off for its first second.
    let r = ee.u32_at(a + 0x488) - 0xb80;
    let m = &mut rider.motion;
    m.pose = F(ee.f32_at(r + 0x9e8));
    m.fore_aft = F(ee.f32_at(r + 0x9ec));
    m.faded = ee.u32_at(r + 0x9f0) != 0;
    m.ramp = F(ee.f32_at(r + 0x9f4));
}

/// S347b1: body actor `a`'s animated skeleton (rider `r`'s) as the shell hands it to the riding body
/// ([`game_sim::rider::Rider::skeleton`]), in the frame of the ride transform `m`: replay.rs's
/// `r24_posed` on an [`Ee`]. The actor's joint records (`+0x234`, stride 0x60: `+0x40` the
/// translation, `+0x50` the quaternion) go into the render body's pose, and the bones it poses hang
/// under the bike node (`rider + 0x18`), taken into `m`'s frame.
fn c33_posed(ee: &Ee, a: u32, r: u32, body: &mut game_render::rider::Body, doll: &game_render::rider::Ragdolled, m: &game_sim::math::Mat4) -> game_sim::ragdoll_ride::Posed {
    use game_sim::math::{self, Mat4};
    let quad = |at: u32| Vf::from_bits([0, 4, 8, 12].map(|o| ee.u32_at(at + o)));
    let mat = |at: u32| Mat4([0, 0x10, 0x20, 0x30].map(|o| quad(at + o)));
    let (n, arr) = (ee.u32_at(a + 0x230) as usize, ee.u32_at(a + 0x234));
    assert_eq!(n, body.rig.set.skeleton.bones.len(), "the actor's joints are the render skeleton's");
    assert_eq!(ee.u32_at(a + 0x488) - 0xb80, r, "actor {a:#x}'s rider");
    let bnode = mat(ee.u32_at(r + 0x18) + 0x40);
    let pose = &mut body.rig.set.pose;
    for j in 0..n {
        let q = quad(arr + 0x60 * j as u32 + 0x50);
        pose.rot[j] = [q[0], q[1], q[2], q[3]];
        pose.turned[j] = true;
    }
    let t = quad(arr + 0x40);
    pose.pos[0] = [t[0], t[1], t[2], 1.0];
    pose.moved[0] = true;
    let posed = body.ragdoll_posed(doll);
    let to = math::mul(&bnode, &math::inverse(m).unwrap());
    let turn = |v: Vf| {
        let r = |k: usize| v[0] * to.0[0][k] + v[1] * to.0[1][k] + v[2] * to.0[2][k];
        Vf::new(r(0), r(1), r(2), 0.0)
    };
    let bones = posed.bones.iter().map(|b| game_sim::ragdoll_pose::Bone { at: math::transform_point(&to, b.at), forward: turn(b.forward), up: turn(b.up) }).collect();
    game_sim::ragdoll_ride::Posed { bones, rests: posed.rests }
}

/// T9: rider's chassis drawn off its ride as the shell's `ai_ride` draws an AI rider's (replay.rs's
/// S474 draw: `Chassis::ride`, then `chassis::grips`), and its "Bike" node and grips handed to the
/// rider's next riding step as its holds, in the chassis' model frame (the shell's `ride_holds_of`).
/// So the grips turn with the bars, where the image's holds, kept, never do.
fn c33_hold(rider: &mut game_sim::rider::Rider, chassis: &mut game_render::vehicle::Chassis, body: &mut game_render::rider::Body, dt: F) {
    let want = chassis.want_under(&rider.ride, &rider.tricks, rider.body.veh.want);
    let request = chassis.riding_under(rider, want);
    chassis.ride(dt, want, &request, rider, game_render::vehicle::Drive::of(&rider.ride));
    body.bare = chassis.bare_under(&rider.body.veh);
    let [Some(foot_l), Some(foot_r), Some(hand_l), Some(hand_r)] = game_render::chassis::grips(chassis) else { panic!("the chassis lacks a hold") };
    let point = |a: game_render::mat::V3| Vf::new(a.x, a.y, a.z, 1.0);
    let root = chassis.world[chassis.nodes.root.expect("a chassis root")];
    rider.holds = Some(Arc::new(game_sim::ragdoll_riding::Holds {
        root: game_sim::math::Mat4(root.0.map(Vf)),
        hands: [point(hand_l), point(hand_r)],
        feet: [point(foot_l), point(foot_r)],
    }));
}

/// R56: rider `r`'s chassis as its image last drew it: the pose of the nodes the vehicle holds
/// (`Bike` `+0x964`, `Handlebars` `+0x968`, `SwingArm` `+0x96c`, `Shocks` `+0x970`), each one's local
/// at `+0x40`, as the pose the chassis last showed, and posed by the motion driver (C33's riders are
/// in the air). So the first restart fades out of the image's pose, as the original's does, and not
/// out of the bind pose: fresh, every rider's drawn root was 0.07 ft off at 4, closing to 16.
fn c33_chassis_seed(ee: &Ee, r: u32, chassis: &mut game_render::vehicle::Chassis) {
    let sk = &chassis.stance.set.skeleton;
    let mut shown = game_render::skin::Pose::new(sk.bones.len());
    for (name, at) in [(game_render::vehicle::ROOT, 0x964), (game_render::vehicle::BARS, 0x968), (game_render::vehicle::SWINGARM, 0x96c), (game_render::vehicle::SHOCKS, 0x970)] {
        let j = sk.find(name).unwrap_or_else(|| panic!("the chassis lacks {name}"));
        let m: [[f32; 4]; 4] = [0, 0x10, 0x20, 0x30].map(|row| [0, 4, 8, 12].map(|c| ee.f32_at(ee.u32_at(r + at) + 0x40 + row + c)));
        // The rotation's quaternion, as `skin::rotation` builds the matrix from one.
        let w = (1.0 + m[0][0] + m[1][1] + m[2][2]).sqrt() * 0.5;
        assert!(w > 0.5, "rider {r:#x}: {name} turned past a quarter turn");
        let k = 0.25 / w;
        shown.rot[j] = [(m[1][2] - m[2][1]) * k, (m[2][0] - m[0][2]) * k, (m[0][1] - m[1][0]) * k, w];
        shown.pos[j] = [m[3][0], m[3][1], m[3][2], 1.0];
        shown.turned[j] = true;
        shown.moved[j] = true;
    }
    chassis.shown = shown;
    chassis.driven = true;
}

/// S488: the nodes the riding ragdoll drew, kept on the render body as the shell's `ai_ride` keeps
/// them, so a restart's pose snapshot is the drawn tree's as the original's is (F12) and the fade
/// after it starts where the original's does.
fn c33_drawn(rider: &game_sim::rider::Rider, body: &mut game_render::rider::Body, doll: &game_render::rider::Ragdolled) {
    body.drawn = match (&rider.riding, &rider.ragdoll_kit) {
        (Some(r), Some(kit)) if r.placed() && rider.drawn_ragdoll().is_none() => {
            Some(body.ragdoll_palette(doll, &r.nodes(kit), game_render::chassis::placed(&rider.ride)))
        }
        _ => None,
    };
}

/// The last tick [`C33_DUST`] is held to here: the table's window, whose end (132) has ai0's foot
/// dust (entry 4) drawing in both since S482b seeded the riding bodies.
const C33_DUST_THROUGH: u64 = 132;

/// S328c: the image's emitters as the Sim's, by Sim player: each rider's manager (`+0x1490`) entries
/// in its list's order. An entry: its tyre record `+4` (records of `0x2c` from the feed's `+4`, the
/// feed at the manager's `+0x30`), live bit `+8`, `Disable` `+0xc`, vtable `+0x10`, emitter `+0x14`;
/// the exhaust's MaxSpeed `+0x18`, the spray's clock `+0x28` and threshold `+0x30`. An emitter: its
/// switch byte `+0x240` (4 on, 2 just on), point `+0x40`, previous `+0x60`, node offset `+0x70`,
/// MinimumEmissionSpeed `+0x3c4`. Its place on the bike is its point in the seeded bike's frame.
fn c33_dust_seed(ee: &Ee, sim: &Sim) -> dust::Dust {
    use dust::{Emitter, Entry, Kind, Place, RiderDust};
    const EXHAUST: u32 = 0x0045_e780;
    const GENERIC: u32 = 0x0045_e6f0;
    const SPRAY: u32 = 0x0045_e720;
    const COLLISION: u32 = 0x0045_e690;
    const SPHERES: u32 = 0x0045_e660;
    let vf = |a: u32| Vf([0, 4, 8, 12].map(|o| ee.f32_at(a + o)));
    let mut riders: Vec<(usize, RiderDust)> = C33_RIDERS
        .iter()
        .enumerate()
        .map(|(k, &(r, _, _))| {
            let bike = sim.rider_of(c33_player(ee, k)).ride.transform();
            let (rear_node, rear) = c33_rear_node(ee, r);
            let (front_node, front) = c33_front_node(ee, r);
            let (exhaust_node, exhaust) = c33_exhaust_node(ee, r);
            let m = ee.u32_at(r + 0x1490);
            let records = ee.u32_at(ee.u32_at(m + 0x30) + 4);
            let entries: Vec<Entry> = (0..ee.u32_at(m + 0x34))
                .map(|i| {
                    let e = ee.u32_at(ee.u32_at(m + 0x3c) + 4 * i);
                    // S482b: the collision models and the bone collision entries (the foot dust),
                    // as S422 seeds them.
                    if matches!(ee.u32_at(e + 0x10), COLLISION | SPHERES) {
                        return super::wreck_dust::seed(ee, e).0;
                    }
                    let em = ee.u32_at(e + 0x14);
                    let kind = match ee.u32_at(e + 0x10) {
                        EXHAUST => Kind::Exhaust { max_speed: F(ee.f32_at(e + 0x18)) },
                        GENERIC => Kind::Generic,
                        SPRAY => Kind::Spray { threshold: F(ee.f32_at(e + 0x30)), clock: F(ee.f32_at(e + 0x28)) },
                        _ => Kind::Quiet,
                    };
                    let rec = ee.u32_at(e + 4);
                    let tyre = (rec != 0).then(|| ((rec - records) / 0x2c) as usize);
                    let flag = ee.read(e + 8, 1) as u8;
                    let switch = ee.read(em + 0x240, 1) as u8;
                    assert_eq!(vf(em + 0x70).0[..3], [0.0; 3], "{}'s entry {i}: an emitter offset", C33_NAMES[k]);
                    let pos = vf(em + 0x40);
                    let place = match ee.u32_at(em + 0xa4) {
                        n if n == rear_node => Place::Rear,
                        n if n == front_node => Place::Front,
                        n if n == exhaust_node => Place::Exhaust,
                        _ => Place::Chassis(dust::place_on(&bike, pos).expect("a bike transform that inverts")),
                    };
                    Entry {
                        kind,
                        tyre,
                        disable: ee.u32_at(e + 0xc) as u8,
                        live: flag & 1 != 0,
                        zone_off: flag & 2 != 0,
                        emitter: Emitter {
                            on: switch & 4 != 0,
                            just_on: switch & 2 != 0,
                            min_speed: F(ee.f32_at(em + 0x3c4)),
                            place,
                            pos,
                            prev: vf(em + 0x60),
                            // S422: the view cull, `UseViewDependentCull` +0x244, its distances +0x24c / +0x250 and the last test +0x248.
                            cull: (ee.u32_at(em + 0x244) != 0).then(|| dust::Cull { far: F(ee.f32_at(em + 0x24c)), directional: F(ee.f32_at(em + 0x250)) }),
                            culled: ee.u32_at(em + 0x248) != 0,
                        },
                        draws: 0,
                    }
                })
                .collect();
            // The snapshot the manager reads, through the feed's pointers: wreck `+0x24`, throttle
            // `+0x18`, clutch `+0x1c`, speed `+0x20`; a tyre record's ground `+8`, object `+0xc`,
            // spinning `+0x14`, side load `+0x20`.
            let feed = ee.u32_at(m + 0x30);
            let at = |p: u32, o: u32| ee.u32_at(p + o);
            let tyre = |t: u32| {
                let rec = records + 0x2c * t;
                dust::Tyre {
                    ground: ee.u32_at(at(rec, 8)) != 0,
                    object: ee.u32_at(at(rec, 0xc)) != 0,
                    spinning: ee.u32_at(at(rec, 0x14)) != 0,
                    side: F(ee.f32_at(at(rec, 0x20))),
                }
            };
            assert_eq!(ee.u32_at(feed), 2, "{}'s manager has two tyres", C33_NAMES[k]);
            let feed = dust::Feed {
                wrecking: ee.u32_at(at(feed, 0x24)) != 0,
                throttle: F(ee.f32_at(at(feed, 0x18))),
                clutch: F(ee.f32_at(at(feed, 0x1c))),
                airspeed: F(ee.f32_at(at(feed, 0x20))),
                tyres: [tyre(0), tyre(1)],
                rear_compression: F(ee.f32_at(r + 0x87c)),
                front_compression: F(ee.f32_at(r + 0x86c)),
                steer: F(ee.f32_at(r + 0x874)),
                grounded: ee.u32_at(r + 0x93c) != 0,
            };
            // S421: the rear tyre's emitters' point is the node's, from the seeded chassis.
            let point = rear.point(&bike, feed.rear_compression);
            for (i, e) in entries.iter().enumerate().filter(|(_, e)| e.emitter.place == Place::Rear) {
                assert_eq!(point.to_bits(), e.emitter.pos.to_bits(), "{}'s entry {i}: the rear node's point", C33_NAMES[k]);
            }
            assert!(!feed.grounded, "{}'s bars were not turned in the image: they are the clip's", C33_NAMES[k]);
            // S446: the front tyre's and the exhaust's nodes. Their points in the image are not the
            // nodes' now: the drive moved the nodes after the image's last emit (tests/ai/draws.rs).
            let places = |p: Place| entries.iter().filter(|e| e.emitter.place == p).count();
            assert_eq!((places(Place::Front), places(Place::Exhaust)), (2, 1), "{}'s front and exhaust emitters", C33_NAMES[k]);
            // S482b: the bone collisions the foot dust reads next, as the image's body left them.
            let spheres = (0..ee.u32_at(m + 0x34)).map(|i| ee.u32_at(ee.u32_at(m + 0x3c) + 4 * i)).find(|&e| ee.u32_at(e + 0x10) == SPHERES);
            let balls = spheres.map(|e| super::wreck_dust::seed(ee, e).2.balls).unwrap_or_default();
            (c33_player(ee, k), RiderDust { entries, feed, rear: Some(rear), front: Some(front), exhaust: Some(exhaust), balls, wrecked: false, hits: Vec::new() })
        })
        .collect();
    riders.sort_by_key(|r| r.0);
    dust::Dust { riders: riders.into_iter().map(|r| r.1).collect(), view: None }
}

/// S448: every rider's tyre tracks as the image holds them, by player, and each track's entry in its
/// rider's manager (`+0x1490`). A track entry's vtable is at `+0x10`: its tyre record `+4`, flag byte
/// `+8`, `Disable` `+0xc`, strip `+0x14`, zones `+0x18`, last tyre point `+0x30`, `FadeOut` `+0x40` and
/// what is left `+0x44`, `FadeIn` `+0x48` and `+0x4c`, `MinimumSpeedThreshold` `+0x50`, the
/// compressions `+0x54` / `+0x58`, `BasePercentAlpha` `+0x5c`, `Width` `+0x60`, `BreakAngle` in radians
/// `+0x64`. The snapshot is read through the manager's feed (`+0x30`): speed `+0x20`, wreck `+0x24`; a
/// tyre record's point `+0`, normal `+4`, ground `+8`, object `+0xc`, compression `+0x10`, spinning
/// `+0x14`, zone `+0x24` (pointers), and the zone the manager last saw `+0x29`.
fn c33_tracks_seed(ee: &Ee) -> (tire_track::Tracks, Vec<Vec<usize>>) {
    use tire_track::{Params, RiderTracks, Snapshot, Strip, Track, Tyre};
    const TRACK: u32 = 0x0045_e630;
    let vf = |a: u32| Vf([0, 4, 8, 12].map(|o| ee.f32_at(a + o)));
    let f = |a: u32| F(ee.f32_at(a));
    let byte = |a: u32| ee.read(a, 1) as u8;
    let mut riders: Vec<(usize, RiderTracks, Vec<usize>)> = C33_RIDERS
        .iter()
        .enumerate()
        .map(|(k, &(r, _, _))| {
            let m = ee.u32_at(r + 0x1490);
            let feed = ee.u32_at(m + 0x30);
            let records = ee.u32_at(feed + 4);
            let mut tracks = Vec::new();
            let mut entries = Vec::new();
            for i in 0..ee.u32_at(m + 0x34) {
                let e = ee.u32_at(ee.u32_at(m + 0x3c) + 4 * i);
                if ee.u32_at(e + 0x10) != TRACK {
                    continue;
                }
                let s = ee.u32_at(e + 0x14);
                let what = format!("{}'s track {i}", C33_NAMES[k]);
                assert_eq!((f(s + 0xd8), f(s + 0xe0), f(s + 0x1d8)), (F(8.0), tire_track::LAY_DISTANCE, F(1.0)), "{what}: its strip's segment, lay distance and u scale");
                let params = Params {
                    tyre: ((ee.u32_at(e + 4) - records) / 0x2c) as usize,
                    disable: ee.u32_at(e + 0xc) as u8,
                    zones: core::array::from_fn(|z| byte(e + 0x18 + z as u32)),
                    width: f(e + 0x60),
                    break_angle: f(e + 0x64),
                    min_speed: f(e + 0x50),
                    min_compression: f(e + 0x54),
                    max_compression: f(e + 0x58),
                    base_alpha: f(e + 0x5c),
                    fade_in: f(e + 0x48),
                    fade_out: f(e + 0x40),
                    height: f(s + 0xd4),
                    segment: f(s + 0xd8),
                    u_scale: f(s + 0x1d8),
                };
                let flag = byte(e + 8);
                let strip = Strip {
                    ended: ee.u32_at(s + 0x40) != 0,
                    count: ee.u32_at(s + 0x3c) as i32,
                    u: f(s + 0xd0),
                    anchor: vf(s + 0x50),
                    anchor_normal: vf(s + 0x60),
                    at: vf(s + 0x70),
                    normal: vf(s + 0x80),
                    edges: [vf(s + 0x90), vf(s + 0xa0)],
                    dir: vf(s + 0xb0),
                    start: vf(s + 0xc0),
                    last_lay: vf(s + 0x1c0),
                    alpha: byte(s + 0x1d4),
                };
                tracks.push(Track {
                    live: flag & 1 != 0,
                    zone_off: flag & 2 != 0,
                    out_left: f(e + 0x44),
                    in_left: f(e + 0x4c),
                    last: vf(e + 0x30),
                    strip,
                    ..Track::new(params)
                });
                entries.push(i as usize);
            }
            assert_eq!(entries, [12, 13], "{}'s tyre tracks", C33_NAMES[k]);
            let at = |p: u32, o: u32| ee.u32_at(p + o);
            let tyre = |t: u32| {
                let rec = records + 0x2c * t;
                Tyre {
                    at: vf(at(rec, 0)),
                    normal: vf(at(rec, 4)),
                    ground: ee.u32_at(at(rec, 8)) != 0,
                    object: ee.u32_at(at(rec, 0xc)) != 0,
                    compression: f(at(rec, 0x10)),
                    spinning: ee.u32_at(at(rec, 0x14)) != 0,
                    zone: byte(at(rec, 0x24)),
                }
            };
            let snap = Snapshot { tyres: [tyre(0), tyre(1)], speed: f(at(feed, 0x20)), wrecking: ee.u32_at(at(feed, 0x24)) != 0 };
            let zones = [byte(records + 0x29), byte(records + 0x2c + 0x29)];
            (c33_player(ee, k), RiderTracks { tracks, snap, zones }, entries)
        })
        .collect();
    riders.sort_by_key(|r| r.0);
    let entries = riders.iter().map(|r| r.2.clone()).collect();
    (tire_track::Tracks { riders: riders.into_iter().map(|r| r.1).collect() }, entries)
}

/// S421: rider `r`'s rear tyre emitter node in its image, and the Sim's chain for it. The vehicle holds
/// `SwingArm` at `+0x96c` and its pivot at `+0x9c0`, the rear wheel (`+0x7b4`) its mount at `+0xa0`;
/// a node's local is at `+0x40`, its parent at `+0x100`. The rear tyre's node is the one child of
/// `SwingArm` an emitter hangs from (`+0xa4`).
pub(crate) fn c33_rear_node(ee: &Ee, r: u32) -> (u32, dust::RearNode) {
    let vf = |a: u32| Vf([0, 4, 8, 12].map(|o| ee.f32_at(a + o)));
    let local = |n: u32| game_sim::math::Mat4([0, 0x10, 0x20, 0x30].map(|o| vf(n + 0x40 + o)));
    let arm = ee.u32_at(r + 0x96c);
    let m = ee.u32_at(r + 0x1490);
    let node = (0..ee.u32_at(m + 0x34))
        .map(|i| ee.u32_at(ee.u32_at(ee.u32_at(m + 0x3c) + 4 * i) + 0x14))
        .map(|em| ee.u32_at(em + 0xa4))
        .find(|&n| n != 0 && ee.u32_at(n + 0x100) == arm)
        .expect("an emitter under SwingArm");
    let pivot = vf(r + 0x9c0);
    assert_eq!(vf(arm + 0x70).to_bits(), pivot.to_bits(), "rider {r:#x}: SwingArm's place is its pivot");
    let rear = dust::RearNode { bike: local(ee.u32_at(arm + 0x100)), pivot, mount: vf(ee.u32_at(r + 0x7b4) + 0xa0), node: local(node) };
    (node, rear)
}

/// S446: rider `r`'s front tyre emitter node in its image, and the Sim's chain for it. The vehicle
/// holds `Handlebars` at `+0x968` and `Shocks` at `+0x970`, the steering axis at `+0x9d0`, the fork's
/// rest place at `+0x9b0` and its slide per compression at `+0x9e0`. The front tyre's node is the one
/// child of `Shocks` an emitter hangs from.
pub(crate) fn c33_front_node(ee: &Ee, r: u32) -> (u32, dust::FrontNode) {
    let vf = |a: u32| Vf([0, 4, 8, 12].map(|o| ee.f32_at(a + o)));
    let local = |n: u32| game_sim::math::Mat4([0, 0x10, 0x20, 0x30].map(|o| vf(n + 0x40 + o)));
    let (bars, shocks) = (ee.u32_at(r + 0x968), ee.u32_at(r + 0x970));
    let node = c33_emitter_nodes(ee, r).find(|&n| ee.u32_at(n + 0x100) == shocks).expect("an emitter under Shocks");
    assert_eq!(ee.u32_at(shocks + 0x100), bars, "rider {r:#x}: Shocks hangs under Handlebars");
    assert_eq!(ee.u32_at(bars + 0x100), ee.u32_at(r + 0x964), "rider {r:#x}: Handlebars hangs under Bike");
    let front = dust::FrontNode {
        bike: local(ee.u32_at(r + 0x964)),
        bars: local(bars),
        // The image's bars are the clip's while no wheel is down (C33's riders are in the air).
        pose: local(bars),
        shocks: local(shocks),
        node: local(node),
        axis: vf(r + 0x9d0),
        rest: vf(r + 0x9b0),
        slide: F(ee.f32_at(r + 0x9e0)),
    };
    (node, front)
}

/// S446: rider `r`'s exhaust emitter node in its image, and the Sim's chain for it: the exhaust
/// entry's emitter node, under the root of the pipe's own model, one of the models the vehicle holds
/// at `+0x954` (count `+0x950`) whose kind (`+0x958`) 0 has the drive copy `Bike`'s world into them.
pub(crate) fn c33_exhaust_node(ee: &Ee, r: u32) -> (u32, dust::ExhaustNode) {
    let vf = |a: u32| Vf([0, 4, 8, 12].map(|o| ee.f32_at(a + o)));
    let local = |n: u32| game_sim::math::Mat4([0, 0x10, 0x20, 0x30].map(|o| vf(n + 0x40 + o)));
    let m = ee.u32_at(r + 0x1490);
    let em = (0..ee.u32_at(m + 0x34))
        .map(|i| ee.u32_at(ee.u32_at(m + 0x3c) + 4 * i))
        .find(|&e| ee.u32_at(e + 0x10) == 0x0045_e780)
        .map(|e| ee.u32_at(e + 0x14))
        .expect("an exhaust entry");
    let node = ee.u32_at(em + 0xa4);
    let pipe = ee.u32_at(node + 0x100);
    assert_eq!(ee.u32_at(pipe + 0x100), 0, "rider {r:#x}: the pipe's root has no parent");
    let held = (0..ee.u32_at(r + 0x950)).find(|&i| ee.u32_at(ee.u32_at(r + 0x954) + 4 * i) == pipe);
    let kind = held.map(|i| ee.u32_at(ee.u32_at(r + 0x958) + 4 * i));
    assert_eq!(kind, Some(0), "rider {r:#x}: the pipe is a model that takes Bike's world");
    (node, dust::ExhaustNode { bike: local(ee.u32_at(r + 0x964)), pipe: local(pipe), node: local(node) })
}

/// Rider `r`'s emitters' nodes, in its manager's order (`+0xa4`; 0 for none skipped).
fn c33_emitter_nodes(ee: &Ee, r: u32) -> impl Iterator<Item = u32> + '_ {
    let m = ee.u32_at(r + 0x1490);
    (0..ee.u32_at(m + 0x34)).map(move |i| ee.u32_at(ee.u32_at(ee.u32_at(m + 0x3c) + 4 * i) + 0x14)).map(move |em| ee.u32_at(em + 0xa4)).filter(|&n| n != 0)
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

// ---------------------------------------------------------------------------------------------------
// S363 — the engine notes' start delays at the load, between the brains
// ---------------------------------------------------------------------------------------------------

/// The game's generator one draw back: its step is `state × a + 1` with an odd `a`, so it inverts.
fn s363_back(state: u64) -> u64 {
    const A: u64 = 0x5851_f42d_4c95_7f2d;
    // a⁻¹ mod 2⁶⁴ by Newton's iteration: each step doubles the bits that are right.
    let mut inv: u64 = A;
    for _ in 0..5 {
        inv = inv.wrapping_mul(2u64.wrapping_sub(A.wrapping_mul(inv)));
    }
    assert_eq!(A.wrapping_mul(inv), 1);
    state.wrapping_sub(1).wrapping_mul(inv)
}

/// How far back from C33's image the load's draws are looked for.
const S363_BACK: usize = 400_000;

/// C33's bikes' model files by player, as its image's vehicle loader holds them (the journal).
const S363_C33_MODELS: [&str; 4] = ["250SX_THQ_01.sub", "RM250.sub", "250SX.sub", "250SX.sub"];

/// **The load draws each bike's note delay before its brain** (S363), as `Sim::spawn_ai_noted` does:
/// C33's generator walked back from the image's state to the load, the three AI brains' ten dice
/// (brain `+0x22c`, as the spawn left them) found in it, and the draw before each brain's first die is
/// its bike's start delay (rider `+0x14a4`, counted down one tick by the image). The bikes are built
/// in model-name order (`engine_note::build_order`): ai0, ai1 (both 250SX), the human (250SX_THQ_01),
/// ai2 (RM250). Each brain's spawn is twelve draws (ten for the brain, two for the reader), so the
/// next bike's delay comes straight after. Then the Sim's spawn on the generator as it stood before
/// the first bike's delay builds the same notes and the same dice.
#[test]
fn s363_c33s_note_delays_draw_between_the_brains() {
    use game_sim::ai::{AiRider, Entry, Roster};
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let state = kit.ctx(C33_STATE);
    if !state.exists() {
        return skip(&format!("{} is not there: C33's capture", state.display()));
    }
    let mut ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let Some(at) = named(&["rand_state"]) else { return };
    // states[n]: the generator n draws before the image's; units[n] the draw that made states[n].
    let mut states = vec![s12e_u64(&ee, at[0])];
    for _ in 0..S363_BACK {
        states.push(s363_back(*states.last().unwrap()));
    }
    let unit = |n: usize| Rng::from_state(states[n + 1]).unit();
    let order = c33_by_player(&ee);
    let dice = |k: usize| -> [F; 10] { core::array::from_fn(|i| F(ee.f32_at(C33_RIDERS[k].2 + 0x22c + 4 * i as u32))) };
    // Where each AI rider's first die is, counted back from the image (the farther back, the earlier).
    let firsts: Vec<usize> = order[1..]
        .iter()
        .map(|&k| {
            let d = dice(k);
            (9..S363_BACK).find(|&n| (0..10).all(|i| unit(n - i) == d[i])).unwrap_or_else(|| panic!("{}'s dice are not in the last {S363_BACK} draws", C33_NAMES[k]))
        })
        .collect();
    println!("the brains' first dice, draws back from the image: {firsts:?}");
    // The loader builds by model name: the human's 250SX_THQ_01 after the two 250SX AI riders.
    let built = engine_note::build_order(&S363_C33_MODELS);
    assert_eq!(built, [2, 3, 0, 1], "C33's bikes, by model name");
    // Walk the build from the first bike's delay: each bike's delay, then an AI rider's 12 draws.
    let n0 = firsts[built[0] - 1] + 1;
    let mut n = n0;
    let mut drawn = vec![F::ZERO; 4];
    for &p in &built {
        let k = order[p];
        let (u, left) = (unit(n), F(ee.f32_at(C33_RIDERS[k].0 + 0x14a4)));
        println!("{} (player {p}): delay drawn {} {n} back, {} left in the image", C33_NAMES[k], u.0, left.0);
        assert!(left.0 < u.0 && u.0 - left.0 < 0.2, "{}: the draw {n} back ({}) is not its delay ({} left)", C33_NAMES[k], u.0, left.0);
        drawn[p] = u;
        n -= 1;
        if p > 0 {
            assert_eq!(firsts[p - 1], n, "{}'s brain right after its delay", C33_NAMES[k]);
            n -= 12;
        }
    }

    // The Sim's spawn from the generator before the first bike's delay: the same delays and dice, and
    // the generator left after the last reader's draws.
    let Some(retail) = super::character::ch_retail() else {
        return skip("no MX.PSR (or no Bike\\250sx.res in it): the retail character file is needed");
    };
    let character = Ini::parse_bytes(&retail);
    let tables = Arc::new(Tables::build());
    let Some(mut bikes) = Bikes::open() else { return };
    let names = C33_RIDERS.map(|(_, _, b)| track::physics_file(track::Engine::Cc250, false, if b == 0 { C33_HUMAN_TUNING } else { track::Tuning::A }));
    let phys: Vec<Phy> = names.iter().map(|n| bikes.phy(n, &tables)).collect();
    let seeded = |k: usize| c33_bike(&ee, C33_RIDERS[k].0, &phys[k], &names[k], &bikes.col);
    let mut sim = Sim::with_rider(0, Arc::clone(&tables), phys[order[0]].clone(), seeded(order[0]));
    for &k in &order[1..] {
        sim.add_player(phys[k].clone(), seeded(k));
    }
    let skills: Vec<i32> = order[1..].iter().map(|&k| ee.u32_at(C33_RIDERS[k].0 + 0x10d0 + 0x104) as i32).collect();
    let roster = Roster {
        humans: 1,
        ghost: false,
        ai: skills.iter().map(|&skill| AiRider { identity: game_sim::ai::FIRST_AI_IDENTITY, skill }).collect(),
        all_ai: None,
    };
    let app = ee.u32_at(named(&["app_pointer"]).unwrap()[0]);
    let event = ee.u32_at(ee.u32_at(app + 0x3d4) + 0xc);
    let (difficulty, event_kind) = (ee.read(event + 0x117, 1) as u8, ee.u32_at(event) as i32);
    let sounds = Arc::new(engine_note::NoteSounds::default());
    sim.spawn_ai_noted(&roster, Some(&character), difficulty, event_kind, &Entry::default(), Rng::from_state(states[n0 + 1]), &sounds, &S363_C33_MODELS);
    let notes = &sim.notes.as_ref().expect("notes").riders;
    assert_eq!(notes.iter().map(|n| n.delay).collect::<Vec<_>>(), drawn, "the notes' delays, by player");
    let lane = sim.ai.as_deref().expect("a lane");
    for ((_, pilot), &k) in lane.pilots.iter().zip(&order[1..]) {
        assert_eq!(pilot.wander.draws, dice(k), "{}'s dice", C33_NAMES[k]);
    }
    assert_eq!(lane.rng.state(), states[n + 1], "the generator after the last reader's two draws");
}

/// **The bikes' build order is the game's sort** (S363): the vehicle loader's `qsort` of its slots
/// by model name, run in the interpreter on C33's image over many name lists up to the loader's eight
/// slots (equal names, case differences, prefixes — seven and eight take the partitioning path), and
/// `engine_note::build_order` answers the same order every time.
#[test]
fn s363_the_build_order_is_the_loaders_sort() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let state = kit.ctx(C33_STATE);
    if !state.exists() {
        return skip(&format!("{} is not there: C33's capture", state.display()));
    }
    let mut ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let Some(at) = named(&["libc_qsort", "vehicle_model_order", "vehicle_model_names"]) else { return };
    let [qsort, order, names] = at[..] else { unreachable!() };
    // The slot indices go in the block after the loader's eight names.
    let base = names + 8 * 0x104;
    const POOL: [&str; 7] = ["250SX.sub", "250sx.SUB", "RM250.sub", "250SX_THQ_01.sub", "CR250.sub", "a", "A_"];
    let mut seed = 0x5363u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (seed >> 16) as usize
    };
    for case in 0..600 {
        let n = 1 + case % 8;
        // Some lists all one name: the shell's races, every bike the same model.
        let models: Vec<&str> = if case % 5 == 0 { vec![POOL[next() % POOL.len()]; n] } else { (0..n).map(|_| POOL[next() % POOL.len()]).collect() };
        for (i, m) in models.iter().enumerate() {
            for (j, b) in m.bytes().chain([0]).enumerate() {
                ee.write(names + i as u32 * 0x104 + j as u32, 1, u128::from(b));
            }
            ee.write(base + 4 * i as u32, 4, i as u128);
        }
        ee.call(qsort, &[u64::from(base), n as u64, 4, u64::from(order)], &[], 1_000_000);
        let theirs: Vec<usize> = (0..n).map(|i| ee.u32_at(base + 4 * i as u32) as usize).collect();
        assert_eq!(engine_note::build_order(&models), theirs, "case {case}: {models:?}");
    }
}
