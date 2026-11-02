//! S228: every contact in the user's fly_and_collide recording, the port against the original.
//!
//! `context/recordings/fly_and_collide.p2m2` (PCSX2 2.9.52, 2026-10-29) is a two-player state of the
//! split free09 (the C10 session's: the same heap, the same records) in which P1 rides and P2 stands
//! at the start for the whole tape. Nobody flies: both biplanes stand parked on every tick (speed
//! under 0.003 ft/s), so every contact is P1's bike on the ground, the scenery or its own landings.
//! S228 captured the playback with watch/2p.toml (`context/capture/s228/fly.csv`, one row a tick at
//! the state's 60 Hz, 136505..140026) and saved the original's RAM in the ticks before each event
//! (`context/states/s228/s228-<frame>.ram`, research/c10/play_at.sh and research/s228/rams.py). The
//! index is research/s228/index.md.
//!
//! - [`p1_rides_each_event_as_the_original`]: P1's rider seeded off the image before each indexed
//!   event (each wreck, each nose-first landing), on the tape's pads and the capture's frame times
//!   through the split free09's ground, props and trunks, holds the capture's chassis velocity and
//!   centre bit for bit tick by tick, latches the original's lean and fore-aft request, and wrecks
//!   with the original's reason in the original's frame or rides out as it does.
//! - [`p1_rides_the_tape_as_the_original`]: the whole tape, 100 frames at a time from an image every
//!   100 frames; where a stretch parts from the capture it is pinned ([`HELD`]).
//!
//! The images, the capture and the disc are game data, git-ignored: absent, the tests skip.

use game_render::disc::Disc;
use game_sim::Sim;
use game_sim::bike_col::{self, BikeCol};
use game_sim::camera::CurveTable;
use game_sim::eco::{Trees, Wooded};
use game_sim::input::Report;
use game_sim::replay::P2m2;
use game_sim::phy::Phy;
use game_sim::ride::{Ride, Wheel};
use game_sim::rider::{Rider, Tables};
use game_sim::track::{self, Scene, Terrain};
use game_sim::trick::Set;
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::path::PathBuf;
use std::sync::Arc;

/// The capture of the tape's playback (watch/2p.toml).
const CAPTURE: &str = "capture/s228/fly.csv";

/// The game manager's slot (kit.toml's `@game_manager`).
const GAME_MANAGER: u32 = 0x004e_1e98;

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there (S228: research/s228/README.md)", path.display()));
        return None;
    }
    Some(path)
}

/// S228's RAM image at `frame`.
fn image(frame: u32) -> Option<Ee> {
    let path = ctx(&format!("states/s228/s228-{frame}.ram"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), frame, "s228-{frame}.ram is not frame {frame}");
    Some(ee)
}

/// P1's ridden vehicle: the actor list's fourth entry (watch/2p.toml), a ridden vehicle (type 2) of
/// player 0.
fn p1(ee: &Ee) -> u32 {
    let v = [0x34, 0x34, 0x8, 0x8, 0x8, 0x0].iter().fold(ee.u32_at(GAME_MANAGER), |a, &h| ee.u32_at(a + h));
    assert_eq!(ee.u32_at(v), 2, "p1's list entry is not a ridden vehicle");
    assert_eq!(ee.u32_at(ee.u32_at(v + 0x20) + 0x30), 0, "the list's p1 is another player");
    v
}

/// The capture's few columns these tests read.
struct Rows {
    first: u64,
    cols: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Rows {
    fn row(&self, frame: u32) -> usize {
        let r = (u64::from(frame) - self.first) as usize;
        assert!(r < self.rows.len(), "no row {frame}");
        r
    }
    fn text(&self, frame: u32, col: &str) -> &str {
        let k = self.cols.iter().position(|c| c == col).unwrap_or_else(|| panic!("no column {col}"));
        &self.rows[self.row(frame)][k]
    }
    fn f(&self, frame: u32, col: &str) -> f32 {
        self.text(frame, col).parse::<f64>().unwrap_or_else(|_| panic!("{col} at {frame}")) as f32
    }
    fn u(&self, frame: u32, col: &str) -> u32 {
        self.text(frame, col).parse().unwrap_or_else(|_| panic!("{col} at {frame}"))
    }
    /// P1's `what` (`p1_ch_vel`, `p1_ch_centre`) on the row of `frame`, as bits.
    fn v3(&self, frame: u32, what: &str) -> [u32; 3] {
        ["x", "y", "z"].map(|a| self.f(frame, &format!("{what}.{a}")).to_bits())
    }
    /// P1's pad on the row of `frame` (`pad0_a`, the report's bytes in hex).
    fn pad(&self, frame: u32) -> Report {
        let hex = self.text(frame, "pad0_a").as_bytes();
        let mut b = [0u8; game_sim::input::REPORT_BYTES];
        for (i, pair) in hex.chunks(2).enumerate().take(b.len()) {
            b[i] = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap_or_else(|e| panic!("pad0_a at {frame}: {e}"));
        }
        Report::from_bytes(b)
    }
}

fn capture() -> Option<Rows> {
    let path = ctx(CAPTURE)?;
    let file = std::io::BufReader::new(std::fs::File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let keep = |c: &str| {
        c == "dt" || c == "pad0_a" || c == "p1_wreck_reason" || c.starts_with("p1_req_axes.") || c.starts_with("p1_ch_centre.") || c.starts_with("p1_ch_vel.")
    };
    let cap = game_sim::replay::two_p::read(file, keep).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
    Some(Rows { first: cap.frames[0], cols: cap.columns.clone(), rows: cap.rows })
}

/// One rider's words off an image, at the offsets `tests/replay.rs`'s save-state seeders read them
/// (S174's, tests/two_p_rider.rs).
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

    /// The wheel mounts the bike was built with (each wheel's `+0xa0`), front and rear.
    fn mounts(&self) -> (Vf, Vf) {
        (self.vf(self.at(0x7b0) + 0xa0), self.vf(self.at(0x7b4) + 0xa0))
    }

    /// The chassis' integrator state and node (replay.rs's `seed_chassis`).
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

    /// One wheel's whole state (replay.rs's `seed_wheel`).
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

    /// The engine, the clutch, the balance, the air state and the rate-limited levers (replay.rs's
    /// `seed_engine_outputs`, `drivetrain`, `seed_balance`, `seed_air`, `seed_held`).
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
        // What tests/air.rs pokes for the rest of the air step: the authority, the preload and the
        // flip, so a ride seeded on a lip or in a flip goes on with them.
        air.authority = self.float(r + 0x4e8);
        air.preload_ready = self.flag(r + 0x4a0);
        air.preload_firing = self.flag(r + 0x4a4);
        air.preload_armed = self.flag(r + 0x4a8);
        air.preload_timer = self.float(r + 0x4b0);
        air.preload_force = self.vf(r + 0x490);
        air.flip_torque_applied = self.flag(r + 0x4f4);
        air.flipping = self.flag(r + 0x4f8);
        air.flip_active = self.flag(r + 0x4fc);
        air.flip_armed = self.flag(r + 0x500);
        air.flip_grace = self.float(r + 0x50c);
        air.flip_torque = self.vf(r + 0x520);
        air.wreck_impact = self.vf(r + 0x560);
        let h = &mut ride.held;
        h.stick = (self.float(r + 0x628), self.float(r + 0x62c));
        h.flick = self.float(r + 0x630);
        h.front_brake = self.float(r + 0x650);
        h.brake = h.front_brake;
        h.rear_brake = self.float(r + 0x654);
        h.lean = (self.float(r + 0x670), self.float(r + 0x674));
    }
}

/// P1's rider as the shell builds it for the 2p state's bike (S174's: Race500P, the bike's collision
/// files, the 50cc reach), seeded off the image.
fn seeded_rider(ee: &Ee, phy: &Phy, col: &Arc<BikeCol>) -> Rider {
    let im = Image { ee, rider: p1(ee) };
    let c = im.at(0x7a4);
    let mut rider = Rider::new(phy, im.mounts(), im.vf(c + 0x60), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    rider.no_start_line();
    rider.carries_bike_col(Arc::clone(col));
    rider.fifty = track::fifty_reach(&track::physics_file(track::Engine::Cc500, false, track::Tuning::P));
    im.chassis(&mut rider.ride);
    im.wheel(0, &mut rider.ride.front);
    im.wheel(1, &mut rider.ride.rear);
    im.rest(&mut rider.ride);
    rider
}

/// The bike, `Race500P.phy`, and the bike's collision files out of the split track's archive (S157's).
fn bike(disc: &mut Disc, tables: &Tables) -> (Phy, BikeCol) {
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
        .find_map(|b| disc.res(b).ok()?.read_suffix("race500p.phy").ok())
        .expect("race500p.phy in a bike archive");
    (Phy::read_bytes(&phy, &tables.ride.curve), col)
}

/// The split free09 as the shell reads it — terrain, the scene's props out of both archives and the
/// ecosystem's trunks (S157's) — handed to `then`.
fn in_free09_ss<R>(disc: &mut Disc, then: impl FnOnce(&mut Wooded<Place<'_>>) -> R) -> R {
    let parts = ["Tracks\\free09_ss.res", "Tracks\\free09_ss_2.res"].map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e}")));
    let read = |want: &str| parts.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| format!("no archive of this track has {want}"));
    let scene = Scene::parse(&read(".scn").unwrap()).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = read(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = Terrain::parse(&bytes).expect("parse the terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let (props, _) = Props::load(&scene, read);
    let (trees, missed) = Trees::on_ground(&read(".esb").unwrap(), &ground, &CurveTable::build()).expect("the ecosystem");
    assert!(missed.is_empty(), "free09_ss's trunks the port lacks: {missed:?}");
    then(&mut Wooded { world: Place { ground, props }, trees })
}

/// One tick of the port's ride: the frame it stepped (`t`, from the image's frame on), and P1's
/// chassis velocity, centre and wreck after it.
#[derive(Debug, Clone, Copy)]
struct Tick {
    t: u32,
    vel: [u32; 3],
    centre: [u32; 3],
    wreck: Option<u8>,
    /// The pose and fore-aft the state pack latched this tick ([`Rider::packed_pose`],
    /// [`Rider::packed_fore_aft`]), which the body's request reads as its two axes.
    axes: [f32; 2],
}

/// **P1's ride by the port from image `from` through frame `to`**: the rider seeded off the image
/// ([`seeded_rider`]), on the split free09's ground, props and trunks. Each tick `t` polls P1's pad
/// off capture row `t` and steps at row `t + 1`'s `dt` (S182's rule on C12's capture of the same
/// session); P2's pad is missing.
fn ride(from: u32, to: u32) -> Option<([[u32; 3]; 2], Vec<Tick>)> {
    let ee = image(from)?;
    let cap = capture()?;
    let tape = tape()?;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let tables = Arc::new(Tables::build());
    let (phy, col) = bike(&mut disc, &tables);
    let col = Arc::new(col);
    let rider = seeded_rider(&ee, &phy, &col);
    let mut sim = Sim::with_rider(0x5228, tables, phy, rider);
    in_free09_ss(&mut disc, |world| {
        let dt_of = |t: u32| F(cap.f(t + 1, "dt"));
        // The pad the image's tick read, so the first tick's presses are not edges.
        sim.poll(dt_of(from - 1), &[tape_pad(&tape, from - 1), Report::missing()]);
        let three = |v: Vf| [0, 1, 2].map(|k| v.0[k].to_bits());
        let seed = [three(sim.rider.ride.chassis.vel), three(sim.rider.ride.chassis.cofg_world)];
        let mut out = Vec::new();
        for t in from..to {
            let dt = dt_of(t);
            let controls = sim.poll(dt, &[tape_pad(&tape, t), Report::missing()]);
            let frame = sim.step(dt, &controls, world);
            let ch = &sim.rider.ride.chassis;
            out.push(Tick {
                t,
                vel: three(ch.vel),
                centre: three(ch.cofg_world),
                wreck: frame.wreck.map(|w| w.0),
                axes: [sim.rider.packed_pose.0, sim.rider.packed_fore_aft.0],
            });
        }
        Some((seed, out))
    })
}

/// The tape's field the poll of tick `t` (frame `t + 1`) reads. The capture's `pad0_a` is read by
/// PINE on its own thread and drifts a frame either way along the tape, so the pads come off the tape
/// itself: field `t - 136507`, the one offset of -3..=1 under which every run holds longest.
fn tape_pad(tape: &P2m2, t: u32) -> Report {
    tape.field((t - FIELD_ZERO) as usize, 0)
}

/// The frame whose tick polls the tape's first field.
const FIELD_ZERO: u32 = 136507;

fn tape() -> Option<P2m2> {
    let path = ctx("recordings/fly_and_collide.p2m2")?;
    Some(P2m2::parse(&std::fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e:?}", path.display())))
}

/// How many of the port's ticks the capture confirms, bit for bit. The image ended frame `from`, and
/// the ride steps frame `t + 1` on row `t + 1`'s `dt` with row `t`'s pad, so each row from `from` on
/// reads one of the port's states in order: the image's own, then the state after each tick. PINE
/// reads on its own thread, so a row can repeat the state before it, miss one (a row read a frame
/// late), or be torn between two states axis by axis ([capture rows read at three phases]); a row
/// that is none of those parts the run.
fn held(cap: &Rows, seed: [[u32; 3]; 2], ticks: &[Tick]) -> usize {
    let states: Vec<[[u32; 3]; 2]> = std::iter::once(seed).chain(ticks.iter().map(|k| [k.vel, k.centre])).collect();
    let from = ticks[0].t;
    let mut at = 0;
    for r in from..=ticks.last().unwrap().t + 2 {
        let row = [cap.v3(r, "p1_ch_vel"), cap.v3(r, "p1_ch_centre")];
        let reads = |j: usize| {
            let pair = [states[j.saturating_sub(1)], states[j]];
            (0..2).all(|v| (0..3).all(|a| pair.iter().any(|s| s[v][a] == row[v][a])))
        };
        // A row read inside integrate carries the centre it rebuilds from the transform before the
        // final one is written ([torn rows]): its velocity reads a state, its centre none.
        let rebuilt = |j: usize| (0..3).all(|a| states[j][0][a] == row[0][a]);
        match (at..(at + 3).min(states.len())).find(|&j| reads(j)).or_else(|| (at..(at + 3).min(states.len())).find(|&j| rebuilt(j))) {
            Some(j) => at = j,
            None if r <= from + 1 && at == 0 => {} // a row before the image's state reached it
            None => break,
        }
    }
    // A state the rows confirm confirms the ticks that repeat it (a wrecked chassis stands still).
    while at + 1 < states.len() && states[at + 1] == states[at] {
        at += 1;
    }
    at
}

/// The first frame from `t` on whose row reads P1 wrecked (`p1_wreck_reason` non-zero), and the reason.
fn wrecked_from(cap: &Rows, t: u32, to: u32) -> Option<(u32, u32)> {
    (t..to).map(|f| (f, cap.u(f, "p1_wreck_reason"))).find(|&(_, r)| r != 0)
}

/// Each indexed event (research/s228/index.md): the image it rides from, the frame it rides to, and
/// the event's frame (the first row the capture reads it on).
const EVENTS: [(&str, u32, u32, u32); 9] = [
    ("landing, rode out", 136830, 136870, 136842),
    ("touch roll 75 deg, rode out", 136980, 137010, 136991),
    ("touch roll 72 deg, rode out", 137010, 137040, 137021),
    ("landing crossup: wreck 3 Right", 137240, 137270, 137251),
    ("airborne roll: wreck 8 NoseDive", 138425, 138450, 138436),
    ("landing, rode out", 138828, 138860, 138838),
    ("nose-first landing at -62.6 deg, rode out", 139050, 139090, 139063),
    ("wall at 59 ft/s: wreck 1 Front", 139435, 139460, 139446),
    ("trunk at 19 ft/s: wreck 6 Scenery", 139883, 139910, 139894),
];

/// Whether the port's two request axes on tick `k` are the original's: the pack's pose and fore-aft
/// against `p1_req_axes` x and y, bit for bit, on one of the rows that can read that tick ([`held`]).
fn axes_hold(cap: &Rows, k: &Tick) -> bool {
    (k.t + 1..=k.t + 3).any(|r| [cap.f(r, "p1_req_axes.x"), cap.f(r, "p1_req_axes.y")].map(f32::to_bits) == k.axes.map(f32::to_bits))
}

/// **P1 rides each indexed event as the original, bit for bit** (S228, S227). From the image before
/// each, on the tape's pads and the capture's frame times, every tick's chassis velocity and centre is
/// a row's ([`held`]) through the event and on to the run's end; the two axes the body's request reads
/// are the original's on every riding tick but the first (P227's turn lean, [`axes_hold`]); and the port wrecks with the
/// original's reason on the original's frame — two rows before the first row that reads the reason,
/// as the chassis' own jump shows — or, where the original rides out, does not wreck. The events: the
/// landing crossup (3), the airborne roll (8), the wall at 59 ft/s (1, a fore-aft g on a body ray),
/// the trunk at 19 ft/s (6, scenery), the one nose-first landing that rides out (139063, 62.6 deg nose
/// down), two other landings and the two touches at 72 and 75 deg of roll.
#[test]
fn p1_rides_each_event_as_the_original() {
    let Some(cap) = capture() else { return };
    for (what, from, to, at) in EVENTS {
        let Some((seed, ticks)) = ride(from, to) else { return };
        assert_eq!(held(&cap, seed, &ticks), ticks.len(), "{what}: the run parts from the original");
        // The first tick latches the motion driver's pose from before the image, which the seeder
        // leaves at its default; from the wreck on the body reads the wreck's request, not these. A
        // lone tick no row reads, between two that hold, is a state the capture skipped.
        let riding = ticks.iter().position(|k| k.wreck.is_some()).unwrap_or(ticks.len());
        let ok: Vec<bool> = ticks[..riding].iter().map(|k| axes_hold(&cap, k)).collect();
        let lean: Vec<u32> = (1..riding)
            .filter(|&i| !ok[i] && !(ok[i - 1] && ok.get(i + 1).copied().unwrap_or(true)))
            .map(|i| ticks[i].t)
            .collect();
        assert!(lean.is_empty(), "{what}: the request's axes part on {lean:?}");
        let port: Vec<(u32, u8)> = ticks.iter().filter_map(|k| k.wreck.map(|w| (k.t + 2, w))).collect();
        let original = wrecked_from(&cap, from + 2, to + 2).map(|(r, why)| (r, why as u8));
        assert_eq!(port, Vec::from_iter(original), "{what}: the wreck (row, reason)");
        if let Some((r, _)) = original {
            assert_eq!(r, at, "{what}: the index's frame");
        }
    }
}

/// The 100-frame stretches of the tape [`p1_rides_the_tape_as_the_original`] rides, from the image at
/// each one's start, and how many of each one's ticks hold ([`held`]). Where one parts, why
/// (research/s228/index.md, "Where the port parts"):
/// - **a wreck under way** (137310, 138510, 139510, 139910): the seeder makes a riding rider, and
///   the original's is a ragdoll; not a parting.
/// - **the rider's own put-back** (138610 at 138684, 139110 at 139141, 139710 at 139746): Select's put-back
///   off a respawn spot the port does not place the same.
/// - **a preload's force** (138810 at 138907): the stick thrown forward on a lip; the force is the
///   flick's speed, whose sampling is the port's own (`ride::Flick`).
/// - **a chassis contact the port does not make** (137410 at 137462, the stick full left; 137610 at
///   137637, the stick coming off centre under gas): the capture's chassis force and moment go
///   non-zero there on the ground, the port's stay zero.
///
/// The ground step's donut read its own angle off the stick until S228b, not the trick decoder's
/// look the original reads: 136610, 137910, 138010, 138110, 138210, 138310, 138710 and 139310 parted
/// there and hold now.
const HELD: [(u32, usize); 35] = [
    (136510, 100), (136610, 100), (136710, 100), (136810, 100), (136910, 100), (137010, 100), (137110, 100),
    (137210, 100), (137310, 0), (137410, 52), (137510, 100), (137610, 27), (137710, 100), (137810, 100),
    (137910, 100), (138010, 100), (138110, 100), (138210, 100), (138310, 100), (138410, 100), (138510, 0),
    (138610, 74), (138710, 100), (138810, 97), (138910, 100), (139010, 100), (139110, 31), (139210, 100),
    (139310, 100), (139410, 100), (139510, 0), (139610, 100), (139710, 36), (139810, 100), (139910, 0),
];

/// **P1 rides the whole tape as the original, 100 frames at a time**, as far as [`HELD`] says: each
/// stretch from its image on the tape's pads, the ticks that hold bit for bit pinned, so a fix shows
/// as a pin to raise and a regression as a pin missed. The stretches that hold hold to the end,
/// wrecks included (137249's crossup, 138434's airborne roll, 139444's wall, 139892's trunk).
#[test]
fn p1_rides_the_tape_as_the_original() {
    let Some(cap) = capture() else { return };
    let mut got = Vec::new();
    for (from, _) in HELD {
        let Some((seed, ticks)) = ride(from, from + 100) else { return };
        got.push((from, held(&cap, seed, &ticks)));
    }
    assert_eq!(got, HELD);
}


// ---------------------------------------------------------------------------------------------------
// S228a: the drawn rider's lean through the tape's turns
// ---------------------------------------------------------------------------------------------------

/// The drawn rider as the shell builds it beside a ride: the rider's body (its clip table and the
/// riding ragdoll's rig, off `thq1`, the one rider body on the disc), the bike's chassis out of the
/// archive `Race500P.phy` came from, and the riding ragdoll's kit.
struct Drawn {
    body: game_render::rider::Body,
    doll: game_render::rider::Ragdolled,
    chassis: game_render::vehicle::Chassis,
    kit: Arc<game_sim::ragdoll_ride::Kit>,
}

fn drawn(disc: &mut Disc, phy: &Phy) -> Drawn {
    use game_render::rider::{Body, Sex};
    let name = |disc: &Disc, want: &dyn Fn(&str) -> bool| {
        disc.entries().iter().map(|e| e.name.clone()).filter(|n| want(&n.to_ascii_lowercase())).min_by_key(String::len)
    };
    let stunts = {
        let app = name(disc, &|n| n.ends_with("appstart.res")).expect("the app archive");
        let bytes = disc.res(&app).unwrap().read_suffix("rider\\anim\\stunts.ini").expect("the stunt table");
        Set::new(game_sim::trick::Stunts::parse(&bytes).expect("the stunt table parses").selection(0))
    };
    let rider = name(disc, &|n| n.ends_with("thq1.res")).expect("the rider archive");
    let res = disc.res(&rider).unwrap();
    let rider_file = res.read_suffix("anim\\mrider.phy").expect("mrider.phy");
    let mut body = Body::load(vec![disc.res(&rider).unwrap()], "thq1", Sex::Male, "", &stunts.stunts, phy.motion()).expect("the body");
    let model = game_render::psg::Model::parse(&res.read_suffix(&format!("{}.psg", body.geometry())).expect("its model")).expect("the model");
    let doll = body.ragdoll_rig(&model, &rider_file).expect("every part names a node");
    let kit = Arc::new(game_sim::ragdoll_ride::Kit::new(&rider_file, doll.rig.clone()));
    let bikes: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| n.to_ascii_lowercase().starts_with("bike\\") && n.to_ascii_lowercase().ends_with(".res"))
        .collect();
    let bike = bikes
        .into_iter()
        .find(|b| disc.res(b).ok().is_some_and(|r| r.read_suffix("race500p.phy").is_ok()))
        .expect("race500p.phy's bike archive");
    let bike = disc.res(&bike).unwrap();
    let sub = bike.find_suffix(".sub").expect("the bike's .sub").name.clone();
    let sub = game_render::scene::Vehicle::parse(&game_render::ini::text(&bike.read(&sub).unwrap()));
    let chassis = game_render::vehicle::Chassis::load(vec![bike], &sub.mcf, "bike_", &stunts.stunts, game_render::vehicle::params())
        .expect("the chassis");
    Drawn { body, doll, chassis, kit }
}

/// One riding tick of [`ride_drawn`]: the frame stepped, and in the bike's frame (the shell's
/// [`placed`](game_render::chassis::placed)) the riding ragdoll's 18 part centres, as drawn.
struct Leaned {
    t: u32,
    parts: Vec<[f32; 3]>,
    bike: [[f32; 4]; 4],
    ride: [[f32; 4]; 4],
}

/// [`ride`] with the drawn rider beside it, every frame as the shell does it (crates/openmx
/// main.rs: the body posed off the ride, its skeleton handed to the riding ragdoll, the chassis
/// posed and its grips made the ragdoll's holds), stopping at the first wreck. Each tick the riding
/// ragdoll the frame's end stepped (S90, the drawn rider) is read off once it is placed.
fn ride_drawn(from: u32, to: u32) -> Option<(Vec<Tick>, Vec<Leaned>)> {
    use game_render::chassis::{grips, placed};
    let ee = image(from)?;
    let cap = capture()?;
    let tape = tape()?;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let tables = Arc::new(Tables::build());
    let (phy, col) = bike(&mut disc, &tables);
    let mut d = drawn(&mut disc, &phy);
    let col = Arc::new(col);
    let mut rider = seeded_rider(&ee, &phy, &col);
    rider.body_rates = game_render::vehicle::params();
    rider.ragdoll_kit = Some(Arc::clone(&d.kit));
    let mut sim = Sim::with_rider(0x5228, tables, phy, rider);
    let root = d.chassis.nodes.root.expect("a chassis root");
    in_free09_ss(&mut disc, |world| {
        let dt_of = |t: u32| F(cap.f(t + 1, "dt"));
        sim.poll(dt_of(from - 1), &[tape_pad(&tape, from - 1), Report::missing()]);
        let three = |v: Vf| [0, 1, 2].map(|k| v.0[k].to_bits());
        let (mut ticks, mut leaned) = (Vec::new(), Vec::new());
        for t in from..to {
            let dt = dt_of(t);
            let controls = sim.poll(dt, &[tape_pad(&tape, t), Report::missing()]);
            let frame = sim.step(dt, &controls, world);
            let r = &mut sim.rider;
            let ch = &r.ride.chassis;
            ticks.push(Tick {
                t,
                vel: three(ch.vel),
                centre: three(ch.cofg_world),
                wreck: frame.wreck.map(|w| w.0),
                axes: [r.packed_pose.0, r.packed_fore_aft.0],
            });
            if frame.wreck.is_some() || r.state.wreck.is_some() {
                break;
            }
            d.body.ride(dt, r, &controls, game_sim::rider::Gate::default());
            r.skeleton = Some(Arc::new(d.body.ragdoll_posed(&d.doll)));
            let want = d.chassis.want_under(&r.ride, &r.tricks, r.body.veh.want);
            let request = d.chassis.riding_under(r, want);
            d.chassis.frame(dt, want, &request, game_render::vehicle::Drive::of(&r.ride));
            d.body.bare = d.chassis.bare_under(&r.body.veh);
            let drawn_root = d.chassis.world[root].mul(placed(&r.ride));
            if let Some(rd) = r.riding.as_ref().filter(|rd| rd.placed()) {
                let bike = drawn_root;
                let into = |p: Vf| {
                    let o = bike.0[3];
                    let v = [p.x() - o[0], p.y() - o[1], p.z() - o[2]];
                    [0, 1, 2].map(|a| v[0] * bike.0[a][0] + v[1] * bike.0[a][1] + v[2] * bike.0[a][2])
                };
                leaned.push(Leaned { t, parts: rd.actor.ragdoll.bones.iter().map(|b| into(b.body.position)).collect(), bike: bike.0, ride: placed(&r.ride).0 });
            }
            let [Some(foot_l), Some(foot_r), Some(hand_l), Some(hand_r)] = grips(&d.chassis) else { panic!("the chassis lacks a hold") };
            let point = |a: game_render::mat::V3| Vf::new(a.x, a.y, a.z, 1.0);
            r.holds = Some(Arc::new(game_sim::ragdoll_riding::Holds {
                root: game_sim::math::Mat4(d.chassis.world[root].0.map(Vf)),
                hands: [point(hand_l), point(hand_r)],
                feet: [point(foot_l), point(foot_r)],
            }));
        }
        Some((ticks, leaned))
    })
}

/// The ragdoll parts the lean is read off: the pelvis and the head (the original's part order,
/// which the port's rig keeps; S109).
const PELVIS: usize = 0;
const HEAD: usize = 3;

/// The ticks after an image the port's riding ragdoll, built fresh where the original's has a
/// history, is not compared on.
const SETTLE: u32 = 30;

/// The riding ticks [`the_drawn_rider_leans_as_the_original_through_the_turns`] compares, and of
/// those the hard-turn ones (the original's head 0.5 ft or more to a side of its pelvis).
const DRAWN_TICKS: (usize, usize) = (1675, 274);

/// **S228a — the drawn rider's lean through the tape's turns, as drawn.** Every stretch of [`HELD`]
/// that holds past [`SETTLE`] is ridden by [`ride_drawn`], the drawn rider beside it as the shell
/// builds it (the riding ragdoll since S90), and on each tick both riders are riding the lean is read
/// off the ragdoll: the head's centre less the pelvis's, in the frame of the drawn bike's root (the
/// capture's `p1_bike_xform` is that node, not the chassis: S8), against the original's
/// `p1_bone<n>.pos` of rows t+1..t+3, the closest. Two things are held, each to a stated tolerance:
///
/// - **the lean**: to a side within 0.05 ft on the median tick, 0.25 ft on nine in ten and 0.75 ft on
///   every one; as a vector within 0.95 ft on every one.
/// - **the drawn bike's root** (which the lean is measured in, and which the ragdoll's holds hang
///   off): its up within 0.1 rad of the original's on nine ticks in ten and 0.3 rad on every one.
///
/// Where it parts, the port leans *less*: in the hard turns its lean to a side is 0.79 of the
/// original's on the mean (pinned; may only go up), and its drawn root rolls 0.90 of the original's
/// off the ride. S253 is to find which.
///
/// S228b's donut fix let eight more stretches hold, and their ticks moved the bounds: the worst lean
/// to a side 0.650 to 0.713, the worst root up 0.195 to 0.283, the hard turns' mean 0.802 to 0.795.
/// The stretches compared before ride and draw as they did.
#[test]
fn the_drawn_rider_leans_as_the_original_through_the_turns() {
    let Some(bones) = bones() else { return };
    let lean = |p: &[[f32; 3]]| [0, 1, 2].map(|k| p[HEAD][k] - p[PELVIS][k]);
    // Per tick: (side, whole, up angle, ours to a side, theirs to a side, our root's roll, theirs).
    let mut rows: Vec<(u32, u32, [f32; 7])> = Vec::new();
    for (from, held_ticks) in HELD.iter().copied().filter(|&(_, h)| h > SETTLE as usize) {
        let Some((_, leaned)) = ride_drawn(from, from + held_ticks as u32) else { return };
        for l in leaned.iter().filter(|l| l.t >= from + SETTLE) {
            let best = (l.t + 1..=l.t + 3)
                .filter(|&r| bones.riding(r))
                .map(|r| {
                    let (ours, theirs) = (lean(&l.parts), lean(&bones.parts(r)));
                    let m = &bones.bikes[(r - bones.first) as usize];
                    let up = (0..3).map(|k| l.bike[1][k] * m[1][k]).sum::<f32>().clamp(-1.0, 1.0).acos();
                    let whole = (0..3).map(|k| (ours[k] - theirs[k]).powi(2)).sum::<f32>().sqrt();
                    // The drawn root's roll off the ride's own frame: its up along the ride's right.
                    let roll = |d: &[[f32; 4]; 4]| (0..3).map(|k| d[1][k] * l.ride[0][k]).sum::<f32>();
                    [(ours[0] - theirs[0]).abs(), whole, up, ours[0], theirs[0], roll(&l.bike), roll(m)]
                })
                .min_by(|a, b| a[1].total_cmp(&b[1]));
            if let Some(b) = best {
                rows.push((from, l.t, b));
            }
        }
    }
    let n = rows.len();
    let sorted = |k: usize, of: &[&(u32, u32, [f32; 7])]| {
        let mut v: Vec<f32> = of.iter().map(|r| r.2[k]).collect();
        v.sort_by(f32::total_cmp);
        v
    };
    let all: Vec<_> = rows.iter().collect();
    let (side, whole, up) = (sorted(0, &all), sorted(1, &all), sorted(2, &all));
    for (name, v) in [("to a side", &side), ("as a vector", &whole), ("root up (rad)", &up)] {
        println!("{name}: median {:.3} p90 {:.3} p99 {:.3} worst {:.3}", v[n / 2], v[n * 9 / 10], v[n * 99 / 100], v[n - 1]);
    }
    let mut worst = rows.clone();
    worst.sort_by(|a, b| b.2[1].total_cmp(&a.2[1]));
    for w in worst.iter().take(5) {
        println!("worst: from {} tick {} {:?}", w.0, w.1, w.2);
    }
    let hard: Vec<_> = rows.iter().filter(|r| r.2[4].abs() >= 0.5).collect();
    let mean = |k: usize| hard.iter().map(|r| r.2[k].abs()).sum::<f32>() / hard.len() as f32;
    let (leans, rolls) = (mean(3) / mean(4), mean(5) / mean(6));
    println!("{} hard-turn ticks: lean to a side ours {:.3} theirs {:.3} ({leans:.3}); root roll ours {:.3} theirs {:.3} ({rolls:.3})", hard.len(), mean(3), mean(4), mean(5), mean(6));
    assert_eq!((n, hard.len()), DRAWN_TICKS, "the ticks compared, and the hard-turn ones");
    assert!(side[n / 2] <= 0.05 && side[n * 9 / 10] <= 0.25 && side[n - 1] <= 0.75, "the lean to a side");
    assert!(whole[n - 1] <= 0.95, "the lean as a vector: {}", whole[n - 1]);
    assert!(up[n * 9 / 10] <= 0.1 && up[n - 1] <= 0.3, "the drawn bike's root");
    assert!(leans >= 0.79, "the hard turns' lean is {leans} of the original's, down from 0.79");
}

/// The original's 18 riding ragdoll part centres (`p1_bone<n>.pos`) off the capture, in the bike's
/// frame of the same row (`p1_bike_xform`).
struct Bones {
    first: u32,
    rows: Vec<Vec<[f32; 3]>>,
    bikes: Vec<[[f32; 4]; 4]>,
    modes: Vec<u32>,
}

impl Bones {
    fn parts(&self, frame: u32) -> Vec<[f32; 3]> {
        self.rows[(frame - self.first) as usize].clone()
    }
    fn riding(&self, frame: u32) -> bool {
        self.modes[(frame - self.first) as usize] == 1
    }
}

fn bones() -> Option<Bones> {
    let path = ctx(CAPTURE)?;
    let file = std::io::BufReader::new(std::fs::File::open(&path).unwrap());
    let keep = |c: &str| c.starts_with("p1_bone") && c.contains(".pos.") || c.starts_with("p1_bike_xform.") || c == "p1_mode";
    let cap = game_sim::replay::two_p::read(file, keep).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
    let at = |name: &str| cap.columns.iter().position(|c| c == name).unwrap_or_else(|| panic!("no {name}"));
    let m: Vec<Vec<usize>> = (0..4).map(|i| (0..4).map(|j| at(&format!("p1_bike_xform.m{i}{j}"))).collect()).collect();
    let p: Vec<[usize; 3]> = (0..18).map(|b| ["x", "y", "z"].map(|a| at(&format!("p1_bone{b}.pos.{a}")))).collect();
    let mode = at("p1_mode");
    let modes = cap.rows.iter().map(|row| row[mode].parse::<f64>().unwrap() as u32).collect();
    let bikes = cap
        .rows
        .iter()
        .map(|row| [0, 1, 2, 3].map(|i| [0, 1, 2, 3].map(|j| row[m[i][j]].parse::<f64>().unwrap() as f32)))
        .collect();
    let rows = cap
        .rows
        .iter()
        .map(|row| {
            let g = |k: usize| row[k].parse::<f64>().unwrap() as f32;
            let o = [g(m[3][0]), g(m[3][1]), g(m[3][2])];
            p.iter()
                .map(|q| {
                    let v = [g(q[0]) - o[0], g(q[1]) - o[1], g(q[2]) - o[2]];
                    [0, 1, 2].map(|a| v[0] * g(m[a][0]) + v[1] * g(m[a][1]) + v[2] * g(m[a][2]))
                })
                .collect()
        })
        .collect();
    Some(Bones { first: cap.frames[0] as u32, rows, bikes, modes })
}
