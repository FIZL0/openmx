//! S174: the whole rider through the 2p recording's two trunk wrecks.
//!
//! S157 proved the pieces of P2's two scenery wrecks one at a time against the original's collision
//! node: the body rays' entries, the resolve, and the hit clause on the port's own contacts. This
//! puts them together the way the shell does: both riders built as the shell builds them (the 2p
//! state's Race500P, the bike's collision files, the 50cc reach), seeded off C10's RAM image of the
//! tick before the wreck — chassis, wheels, drivetrain, balance, air and levers — and one
//! [`Sim::step_all`] through the split free09 (terrain, props and trunks). The wreck test then reads
//! the list the frame composes itself: the ground's entries, the head ray's sweep against the other
//! rider, the cars' entries and the body rays' scenery, after the chassis' sweep back.
//!
//! - [`the_whole_rider_wrecks_p2_on_the_trunk_as_the_original`]: 130416 -> 130417 and 130895 ->
//!   130896 wreck P2 on the scenery (6) and leave P1 riding, as the next image does; S157's other
//!   two contacts (P2's nose ray at 130412, P1's in the ground at 131482) wreck nobody, as there;
//! - [`without_the_trees_nothing_wrecks_p2`]: the same ticks with the ecosystem taken out of the
//!   world wreck nobody, so it is the trunk and not the seeded state's attitude or loads that does.
//!
//! The images and the disc are game data, git-ignored: absent, the tests skip.

use game_render::disc::Disc;
use game_sim::Sim;
use game_sim::bike_col::{self, BikeCol};
use game_sim::camera::CurveTable;
use game_sim::eco::{Trees, Wooded};
use game_sim::phy::Phy;
use game_sim::ride::{Controls, Ride, Wheel};
use game_sim::rider::{Rider, Tables};
use game_sim::track::{self, Scene, Terrain};
use game_sim::trick::{Set, Wreck};
use game_sim::world::{Ground, Place, Props, World};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::path::PathBuf;
use std::sync::Arc;

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there", path.display()));
        return None;
    }
    Some(path)
}

/// C10's RAM image at `frame`.
fn image(frame: u32) -> Option<Ee> {
    let path = ctx(&format!("states/c10/c10-{frame}.ram"))?;
    Some(Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}")))
}

/// The game manager's slot and the frame counter (kit.toml's `@game_manager`, `@frame_counter`).
const GAME_MANAGER: u32 = 0x004e_1e98;
const FRAME_COUNTER: u32 = 0x0047_9d04;

/// P1's and P2's ridden vehicles, as S157 walks to them (watch/2p.toml).
fn players(ee: &Ee) -> [u32; 2] {
    let down = |hops: &[u32]| hops.iter().fold(ee.u32_at(GAME_MANAGER), |a, &h| ee.u32_at(a + h));
    let out = [down(&[0x34, 0x34, 0x8, 0x8, 0x8, 0x0]), down(&[0x34, 0x34, 0x8, 0x8, 0x0])];
    for (p, &v) in out.iter().enumerate() {
        assert_eq!(ee.u32_at(v), 2, "p{}'s list entry is not a ridden vehicle", p + 1);
        assert_eq!(ee.u32_at(ee.u32_at(v + 0x20) + 0x30), p as u32, "the list's p{} is another player", p + 1);
    }
    out
}

fn wrecks(ee: &Ee) -> [u32; 2] {
    players(ee).map(|v| ee.u32_at(v + 0x748))
}

/// One rider's words off an image, at the offsets `tests/replay.rs`'s save-state seeders read them.
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
        let h = &mut ride.held;
        h.stick = (self.float(r + 0x628), self.float(r + 0x62c));
        h.flick = self.float(r + 0x630);
        h.front_brake = self.float(r + 0x650);
        h.brake = h.front_brake;
        h.rear_brake = self.float(r + 0x654);
        h.lean = (self.float(r + 0x670), self.float(r + 0x674));
    }
}

/// Player `p`'s rider as the shell builds it for the 2p state's bike, seeded off the image.
fn rider(ee: &Ee, p: usize, phy: &Phy, col: &Arc<BikeCol>) -> Rider {
    let im = Image { ee, rider: players(ee)[p] };
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

/// Both riders' bike, `Race500P.phy`, and the bike's collision files out of the split track's archive
/// (S157's).
fn bike(tables: &Tables) -> Option<(Phy, BikeCol)> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
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
    Some((Phy::read_bytes(&phy, &tables.ride.curve), col))
}

/// The split free09 as the shell reads it — terrain, the scene's props out of both archives and the
/// ecosystem's trunks (S157's) — handed to `then`.
fn in_free09_ss<R>(then: impl FnOnce(&mut Wooded<Place<'_>>) -> R) -> Option<R> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
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
    Some(then(&mut Wooded { world: Place { ground, props }, trees }))
}

/// The two ticks C10 saw P2 wreck on a trunk: the image before each.
const WRECKS: [u32; 2] = [130416, 130895];

/// Every world contact C10 indexed (S157's four), the image before it and both riders' wreck reasons
/// after the tick: P2's nose ray on the first trunk (no wreck: 1.86 ft is below the 50cc reach), its
/// shoulder ray on the same trunk and on another (scenery, 6), and P1's nose ray in the ground with
/// the bike on its side (no wreck).
const CONTACTS: [(u32, [u32; 2]); 4] = [(130412, [0, 0]), (130416, [0, 6]), (130895, [0, 6]), (131482, [0, 0])];

/// Both riders off image `from`, one [`Sim::step_all`] on neutral pads through `world`: each
/// rider's wreck verdict.
fn step(from: u32, phy: &Phy, col: &Arc<BikeCol>, tables: &Arc<Tables>, world: &mut dyn World) -> Option<[Option<Wreck>; 2]> {
    let ee = image(from)?;
    assert_eq!(ee.u32_at(FRAME_COUNTER), from, "c10-{from}'s frame counter");
    assert_eq!(wrecks(&ee), [0, 0], "{from}: both riding before the tick");
    let mut sim = Sim::with_rider(0, Arc::clone(tables), phy.clone(), rider(&ee, 0, phy, col));
    sim.add_player(phy.clone(), rider(&ee, 1, phy, col));
    let dt = F(ee.f32_at(ps2_oracle::addr("frame_dt").unwrap()));
    let frames = sim.step_all(dt, &[Controls::default(), Controls::default()], world);
    Some([frames[0].wreck, frames[1].wreck])
}

#[test]
fn the_whole_rider_wrecks_p2_on_the_trunk_as_the_original() {
    let tables = Arc::new(Tables::build());
    let Some((phy, col)) = bike(&tables) else { return };
    let col = Arc::new(col);
    in_free09_ss(|world| {
        for (from, want) in CONTACTS {
            // C10 saved no image of 130413; S157's interpreter tick from 130412 says [0, 0].
            if let Some(next) = (from != 130412).then(|| image(from + 1)).flatten() {
                assert_eq!(wrecks(&next), want, "{from}: the original's reasons after the tick");
            }
            let Some(got) = step(from, &phy, &col, &tables, world) else { return };
            println!("{from} -> {}: port {got:?}, original {want:?}", from + 1);
            assert_eq!(got.map(|w| w.map_or(0, |w| u32::from(w.0))), want, "{from} -> {}: the wreck reasons", from + 1);
        }
    });
}

#[test]
fn without_the_trees_nothing_wrecks_p2() {
    let tables = Arc::new(Tables::build());
    let Some((phy, col)) = bike(&tables) else { return };
    let col = Arc::new(col);
    in_free09_ss(|world| {
        for from in WRECKS {
            let Some(got) = step(from, &phy, &col, &tables, &mut world.world) else { return };
            println!("{from} without the trees: {got:?}");
            assert_eq!(got, [None, None], "{from}: the props and the ground alone");
        }
    });
}

