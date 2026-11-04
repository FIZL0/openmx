//! R27 — free09's beach ball struck by a seeded whole ride, against PCSX2's f27a1 knock.
//!
//! F27a1's shell run (research/f27a1/ours.sh) heard a second bbhit about 20 ticks after the strike:
//! its bike caught the ball again, which the original's never does. This seeds the port's whole ride
//! from f27a1's RAM images instead of riding a tape up to the ball: the bike with S174's `Image`
//! seeder (tests/two_p_rider.rs's, copied), Race125P and the bike's collision files as the shell
//! loads them, and free09's woken balls with S195's `seed` (tests/prop_knock.rs's, copied). Then
//! [`Sim::step`] on an idle pad (aim.py's ride has no pad) through free09's ground and scene.
//!
//! Image `s195-f27a1-N` is the capture's row N+1 (`context/capture/s195/f27a1.csv`): the strike is
//! between rows 2903 and 2904, so every image is past it, and the bike pushes the ball on for two
//! more ticks (27.5, 33.4, then 35.2 ft/s).
//!
//! - [`the_launch_goes_on_as_the_original`]: images 2904 and 2905 one tick on, ball 2's body and the
//!   bike's chassis against the next image, bit for bit;
//! - [`the_balls_probe_pushes_it_off_the_bike`] (ignored, B29): 2903 one tick on, where the original's
//!   ball probe files a contact on the bike's frame that the port only hears;
//! - [`the_ball_flies_clear_of_the_bike`]: image 2903 ridden 60 ticks against the capture's rows,
//!   the bike never touching the ball again, as there;
//! - [`a_struck_ball_is_heard_in_its_strike_tick`] (B28): ball 2 put back unwoken under image
//!   2903's bike, its probe heard on the bike's frame in the tick the bike's rays wake it.
//!
//! The images, the capture and the disc are game data, git-ignored: absent, the tests skip.

use std::path::PathBuf;
use std::sync::Arc;

use game_render::disc::{Disc, Res};
use game_sim::Sim;
use game_sim::bike_col::{self, BikeCol};
use game_sim::biplane::{Area, Body};
use game_sim::phy::{Ini, Phy};
use game_sim::prop_set::PropSet;
use game_sim::prop_world::Listener;
use game_sim::ride::{Controls, Ride, Wheel};
use game_sim::rider::{Rider, Tables};
use game_sim::track::{self, Scene, Terrain};
use game_sim::trick::Set;
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

/// The `.toy` actor's collection list (tests/prop_collision.rs).
const LIST: u32 = 0x011b_43f4;

/// free09's terrain: three root cells by three, nine feet a sample.
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);

/// The ball the bike strikes.
const BALL: usize = 2;

fn context(kit: &Kit, rel: &str) -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context").join(rel);
    if here.exists() { here } else { kit.ctx(rel) }
}

/// f27a1's RAM image `n`, its gp set.
fn image(n: u32) -> Option<Ee> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let path = context(&kit, &format!("states/s195-f27a1-{n}.eeMemory.bin"));
    let mut ee = Ee::from_ram(&path).map_err(|why| skip(&format!("{why} — s3's context has f27a1's images"))).ok()?;
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// The human's ridden vehicle (watch/s195-knock.toml's).
fn the_bike(ee: &Ee) -> u32 {
    [0x34, 0x34, 0x8, 0x8, 0x0].iter().fold(ee.u32_at(0x004e_1e98), |a, &h| ee.u32_at(a + h))
}

mod coll {
    pub const FLAGS: u32 = 0x20;
    pub const ACTIVE: u32 = 2;
    pub const NAME: u32 = 0x144;
    pub const LISTENER: u32 = 0x34;
    pub const NODE: u32 = 0x248;
    pub const BODIES: u32 = 0x320;
}

mod obj {
    pub const LISTENER: u32 = 0x50;
    pub const BODY: u32 = 0x110;
    pub const COLLISION: u32 = 0x120;
    pub const NODE: u32 = 0x13c;
    pub const FRESH: u32 = 0x1b0;
    pub const COUNTDOWN: u32 = 0x1bc;
}

mod rec {
    pub const TOUCHING: u32 = 0x40;
    pub const TOUCHING_OTHER: u32 = 0x44;
    pub const ON_GROUND: u32 = 0x48;
    pub const ON_OBJECT: u32 = 0x4c;
    pub const COFG_WORLD: u32 = 0xa0;
    pub const SWEPT_FROM: u32 = 0xd0;
    pub const SWEPT_TO: u32 = 0xe0;
    pub const NORMAL_SPEED: u32 = 0x1d8;
    pub const RELATIVE: u32 = 0x1e0;
}

mod body_at {
    pub const ENABLED: u32 = 0x00;
    pub const INV_MASS: u32 = 0x10;
    pub const DAMPING: u32 = 0x20;
    pub const LAST_POSITION: u32 = 0x50;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const LAST_FORCE: u32 = 0x80;
    pub const FORCE: u32 = 0x90;
    pub const ACCEL: u32 = 0xa0;
    pub const SHOCK_PEAK: u32 = 0xb0;
    pub const SHOCK: u32 = 0xc0;
    pub const STEP: u32 = 0xf0;
    pub const INV_INERTIA: u32 = 0x120;
    pub const ANG_DAMPING: u32 = 0x130;
    pub const LAST_QUAT: u32 = 0x150;
    pub const QUAT: u32 = 0x170;
    pub const COFG: u32 = 0x180;
    pub const MOMENT: u32 = 0x190;
    pub const LAST_MOMENT: u32 = 0x1a0;
    pub const RATES: u32 = 0x1b0;
    pub const ANG_ACCEL: u32 = 0x1c0;
    pub const TWIST_PEAK: u32 = 0x1d0;
    pub const TWIST: u32 = 0x1e0;
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(a + k)))
}

fn text(ee: &Ee, at: u32) -> String {
    (0..256).map(|k| ee.u8_at(at + k)).take_while(|&b| b != 0).map(|b| b as char).collect()
}

fn ee_body(ee: &Ee, b: u32) -> Body {
    use body_at::*;
    let q = |a: u32| quad(ee, b + a);
    Body {
        enabled: ee.u32_at(b + ENABLED) != 0,
        inv_mass: F(ee.f32_at(b + INV_MASS)),
        damping: q(DAMPING),
        last_position: q(LAST_POSITION),
        position: q(POSITION),
        velocity: q(VELOCITY),
        last_force: q(LAST_FORCE),
        force: q(FORCE),
        accel: q(ACCEL),
        shock_peak: q(SHOCK_PEAK),
        shock: q(SHOCK),
        step: q(STEP),
        inv_inertia: q(INV_INERTIA),
        ang_damping: q(ANG_DAMPING),
        last_quat: q(LAST_QUAT),
        quat: q(QUAT),
        cofg: q(COFG),
        moment: q(MOMENT),
        last_moment: q(LAST_MOMENT),
        rates: q(RATES),
        ang_accel: q(ANG_ACCEL),
        twist_peak: q(TWIST_PEAK),
        twist: q(TWIST),
    }
}

fn collections(ee: &Ee) -> Vec<u32> {
    let mut out = Vec::new();
    let mut cell = ee.u32_at(LIST + 4);
    while cell != 0 {
        out.push(ee.u32_at(cell));
        cell = ee.u32_at(cell + 8);
    }
    out
}

/// The knocked collections as the state left them, read into the port's: every collection whose
/// active bit is set is woken the port's way (its settle listener and its body's object listener on,
/// the collision object filed), then the body, the node, the countdowns, the fresh flag and the
/// collision record's moving parts are the state's, and the world node's one listener list is the
/// state's, in its order (F20). Answers the woken collections' indices.
fn seed(ee: &Ee, set: &mut PropSet) -> Vec<usize> {
    let game = collections(ee);
    assert_eq!(game.len(), set.collections.len(), "the state's collections");
    let mut woken = Vec::new();
    // Each listener's address on the node, as the port names it.
    let mut named: Vec<(u32, (usize, Listener))> = Vec::new();
    let mut node_list = None;
    for (ci, &c) in game.iter().enumerate() {
        if ee.u32_at(c + coll::FLAGS) & coll::ACTIVE == 0 {
            continue;
        }
        let name = text(ee, c + coll::NAME);
        assert_eq!(ee.u32_at(c + coll::BODIES), 1, "{name}: one body");
        let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8));
        let (b, node, r) = (ee.u32_at(o + obj::BODY), ee.u32_at(o + obj::NODE), ee.u32_at(o + obj::COLLISION));
        set.wake(ci, 0);
        let port = &mut set.collections[ci];
        // The node's listeners as the state has them: the collection's settle and the object's own.
        let world_node = ee.u32_at(c + coll::NODE);
        let n = ee.u32_at(world_node + 0x48);
        let at = ee.u32_at(world_node + 0x48 + 8);
        let on: Vec<u32> = (0..n).map(|k| ee.u32_at(at + 4 * k)).collect();
        assert!(node_list.as_ref().is_none_or(|l| *l == on), "{name}: one listener list on the node");
        node_list = Some(on.clone());
        named.extend([(c + coll::LISTENER, (ci, Listener::Settle)), (o + obj::LISTENER, (ci, Listener::Object(0)))]);
        port.listeners = on
            .iter()
            .filter_map(|&l| {
                if l == c + coll::LISTENER {
                    Some(Listener::Settle)
                } else if l == o + obj::LISTENER {
                    Some(Listener::Object(0))
                } else {
                    None
                }
            })
            .collect();
        eprintln!("{name}: the node's listeners {:?}", port.listeners);
        assert!(port.listeners.contains(&Listener::Settle), "{name}: the settle listener is on");
        let body = ee_body(ee, b);
        if !body.enabled {
            port.disable(0);
        }
        port.bones[0].body = body;
        let ob = &mut port.objects[0];
        ob.node.rows = [0, 1, 2].map(|k| quad(ee, node + 0x80 + 16 * k));
        ob.node.origin = quad(ee, node + 0xb0);
        ob.countdown = F(ee.f32_at(o + obj::COUNTDOWN));
        ob.fresh = ee.u32_at(o + obj::FRESH) != 0;
        let col = ob.collision.as_mut().expect("a ball has a collision object");
        col.collider.cofg_world = quad(ee, r + rec::COFG_WORLD);
        col.collider.normal_speed = F(ee.f32_at(r + rec::NORMAL_SPEED));
        col.collider.relative = quad(ee, r + rec::RELATIVE);
        let rc = &mut col.record;
        rc.touching = ee.u32_at(r + rec::TOUCHING) != 0;
        rc.touching_other = ee.u32_at(r + rec::TOUCHING_OTHER) != 0;
        rc.on_ground = ee.u32_at(r + rec::ON_GROUND) != 0;
        rc.on_object = ee.u32_at(r + rec::ON_OBJECT) != 0;
        rc.swept_from = quad(ee, r + rec::SWEPT_FROM);
        rc.swept_to = quad(ee, r + rec::SWEPT_TO);
        woken.push(ci);
    }
    let on = node_list.unwrap_or_default();
    set.listeners = on.iter().filter_map(|l| named.iter().find(|(a, _)| a == l).map(|&(_, x)| x)).collect();
    // The node's list has one listener no prop collection put on (knock-2881's 0xe1e2b0), which the
    // port's set does not hold.
    let others: Vec<u32> = on.iter().copied().filter(|l| !named.iter().any(|(a, _)| a == l)).collect();
    eprintln!("the node's listeners {:?}, and {others:x?} no prop's", set.listeners);
    woken
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

/// free09 off the disc: the props as the ride loads them, the terrain's bytes and the scene, and
/// Race125P (the freeride's bike) with the bike's collision files.
struct Free09 {
    set: PropSet,
    terrain: Vec<u8>,
    scene: Scene,
    props: Option<Props>,
    phy: Phy,
    col: Arc<BikeCol>,
}

fn free09(tables: &Tables) -> Option<Free09> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let names: Vec<String> = ["Tracks\\free09.res", "Tracks\\free09_2.res"]
        .iter()
        .filter_map(|want| disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case(want)).map(|e| e.name.clone()))
        .collect();
    let archives: Vec<Res> = names.iter().map(|n| disc.res(n).expect("a free09 archive")).collect();
    let res = &archives[0];
    let toy = Ini::parse_bytes(&res.read_suffix("free09.toy").expect("free09.toy"));
    let scene = Scene::parse(&res.read_suffix("free09.scn").expect("free09.scn")).expect("free09.scn parses");
    let (x, z, sx, sz) = FREE09;
    let (set, said) = PropSet::load(
        &toy,
        "Tracks\\Free09\\",
        &scene.models,
        &mut |p| res.read(p).ok(),
        &mut |p| res.read_suffix(p).ok(),
        Some(Area::new(F(x), F(z), F(sx), F(sz))),
    );
    assert!(said.is_empty(), "free09 loads whole: {said:?}");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain = res.read_suffix(&leaf).expect("free09's terrain");
    let (props, _) =
        Props::load(&scene, |want| archives.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| "no archive of this track has it".to_string()));
    let res_names: Vec<String> = disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let frame = res_names.iter().find_map(|n| disc.res(n).ok()?.read(game_sim::flight_search::BIKE_COLLISION).ok()).expect("bikef.col");
    let mut set = set;
    set.bike_frame = Some(Arc::new(track::Mesh::parse(&frame).expect("bikef.col parses")));
    let split = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| split.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
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
    let phy = bikes.iter().find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok()).expect("race125p.phy in a bike archive");
    Some(Free09 { set, terrain, scene, props: Some(props), phy: Phy::read_bytes(&phy, &tables.ride.curve), col: Arc::new(col) })
}

/// The whole ride off image `ee`: the human's rider as the shell builds it, seeded, and the woken
/// balls in its traffic.
fn seeded(ee: &Ee, free: &Free09, tables: &Arc<Tables>) -> Sim {
    let im = Image { ee, rider: the_bike(ee) };
    let c = im.at(0x7a4);
    let mut rider = Rider::new(&free.phy, im.mounts(), im.vf(c + 0x60), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    rider.no_start_line();
    rider.carries_bike_col(Arc::clone(&free.col));
    rider.fifty = track::fifty_reach("race125p.phy");
    im.chassis(&mut rider.ride);
    im.wheel(0, &mut rider.ride.front);
    im.wheel(1, &mut rider.ride.rear);
    im.rest(&mut rider.ride);
    let mut sim = Sim::with_rider(0, Arc::clone(tables), free.phy.clone(), rider);
    sim.traffic.props = free.set.clone();
    let woken = seed(ee, &mut sim.traffic.props);
    assert!(woken.contains(&BALL), "ball {BALL} woken: {woken:?}");
    sim
}

fn ball_body(ee: &Ee) -> Body {
    ee_body(ee, ee.u32_at(ball_object(ee) + obj::BODY))
}

/// Ball 2's physics object in the image.
fn ball_object(ee: &Ee) -> u32 {
    ee.u32_at(ee.u32_at(collections(ee)[BALL] + coll::BODIES + 8))
}

/// A collision record's list, entry by entry: kind, point, normal, arm and other arm.
fn list_at(ee: &Ee, r: u32) -> Vec<String> {
    let (n, at) = (ee.u32_at(r + 0x100), ee.u32_at(r + 0x108));
    (0..n)
        .map(|k| {
            let e = at + 0x80 * k;
            format!(
                "kind {} from {:?} on_body {:?} normal {:?} point {:?} arm {:?} other_arm {:?} surface {}",
                ee.u32_at(e),
                quad(ee, e + 0x10).0,
                quad(ee, e + 0x20).0,
                quad(ee, e + 0x30).0,
                quad(ee, e + 0x40).0,
                quad(ee, e + 0x50).0,
                quad(ee, e + 0x60).0,
                ee.u32_at(e + 0x70)
            )
        })
        .collect()
}

fn len3(v: Vf) -> f32 {
    (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt()
}

/// Whether the ball's record holds a mirror of the bike's strike (an entry of kind 1).
fn struck(sim: &Sim) -> bool {
    let rc = &sim.traffic.props.collections[BALL].objects[0].collision.as_ref().unwrap().record;
    rc.contacts.iter().any(|e| e.kind == 1)
}

fn step(sim: &mut Sim, world: &mut Place<'_>, ee: &Ee) {
    let dt = F(ee.f32_at(addr("frame_dt").unwrap()));
    sim.step(dt, &Controls::default(), world);
}

/// Image `from` one tick on, against image `from + 1`: ball 2's body and the bike's chassis velocity,
/// the port's and the original's contact lists printed. Answers what differs.
fn one_tick(from: u32) -> Option<Vec<String>> {
    let tables = Arc::new(Tables::build());
    let mut free = free09(&tables)?;
    let props = free.props.take().unwrap();
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let mut world = Place { ground: Ground::new(&terrain, &free.scene.terrain), props };
    let mut bad = Vec::new();
    {
        let (ee, next) = (image(from)?, image(from + 1)?);
        let mut sim = seeded(&ee, &free, &tables);
        step(&mut sim, &mut world, &ee);
        let got = sim.traffic.props.collections[BALL].bones[0].body;
        let want = ball_body(&next);
        let ch = sim.rider.ride.chassis.vel;
        let want_ch = Vf::from_bits([0, 4, 8, 12].map(|k| next.u32_at(next.u32_at(the_bike(&next) + 0x7a4) + 0x260 + k)));
        let rc = &sim.traffic.props.collections[BALL].objects[0].collision.as_ref().unwrap().record;
        for e in &rc.contacts {
            println!("  port ball entry kind {} from {:?} on_body {:?} normal {:?} point {:?} arm {:?} other_arm {:?} surface {}", e.kind, e.from.0, e.on_body.0, e.normal.0, e.point.0, e.arm.0, e.other_arm.0, e.surface);
        }
        for e in list_at(&next, next.u32_at(ball_object(&next) + obj::COLLISION)) {
            println!("  orig ball entry {e}");
        }
        for e in list_at(&next, next.u32_at(the_bike(&next) + 0x7d0)) {
            println!("  orig bike entry {e}");
        }
        println!(
            "{from} -> {}: ball vel port {:?} ({:.3}) original {:?} ({:.3}); bike vel port {:?} original {:?}; struck {}",
            from + 1,
            got.velocity.0,
            len3(got.velocity),
            want.velocity.0,
            len3(want.velocity),
            ch.0,
            want_ch.0,
            struck(&sim)
        );
        for (what, p, o) in [("ball velocity", got.velocity, want.velocity), ("ball position", got.position, want.position), ("bike velocity", ch, want_ch)] {
            if p.to_bits() != o.to_bits() {
                bad.push(format!("{from}: {what} port {:?} original {:?}", p.0, o.0));
            }
        }
    }
    Some(bad)
}


/// The bike's rays keep pushing the ball on the two ticks after the strike, 2904 -> 2905 (the
/// bike's entry and the ball's mirror alone) and 2905 -> 2906 (clear): bit for bit.
#[test]
fn the_launch_goes_on_as_the_original() {
    for from in [2904, 2905] {
        let Some(bad) = one_tick(from) else { return };
        assert!(bad.is_empty(), "{bad:#?}");
    }
}

/// 2903 -> 2904, the tick after the strike: the original's ball 2 also has its own probe's entry on
/// the bike's frame collidable (kind 0, normal up out of the bike) and the bike its mirror, which
/// throws the ball up (vy 19.31). The port only hears that hit (F27a2) and files no contact, so its
/// ball leaves at vy 13.88. B29.
#[test]
#[ignore = "B29: the ball's probe on the bike's frame files no contact"]
fn the_balls_probe_pushes_it_off_the_bike() {
    let Some(bad) = one_tick(2903) else { return };
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn the_ball_flies_clear_of_the_bike() {
    let tables = Arc::new(Tables::build());
    let Some(mut free) = free09(&tables) else { return };
    let props = free.props.take().unwrap();
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let mut world = Place { ground: Ground::new(&terrain, &free.scene.terrain), props };
    let Some(ee) = image(2903) else { return };
    let Ok(kit) = Kit::load() else { return };
    let Ok(csv) = std::fs::read_to_string(context(&kit, "capture/s195/f27a1.csv")).map_err(|why| skip(&format!("f27a1.csv: {why}"))) else { return };
    let mut lines = csv.lines();
    let names: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = |n: &str| names.iter().position(|&c| c == n).unwrap_or_else(|| panic!("no column {n}"));
    let rows: Vec<Vec<&str>> = lines.map(|l| l.split(',').collect()).collect();
    let row = |frame: u32| rows.iter().find(|r| r[0] == frame.to_string());
    let v3 = |r: &Vec<&str>, n: &str| Vf::new(r[col(&format!("{n}.x"))].parse().unwrap(), r[col(&format!("{n}.y"))].parse().unwrap(), r[col(&format!("{n}.z"))].parse().unwrap(), 0.0);
    let mut sim = seeded(&ee, &free, &tables);
    let mut again = Vec::new();
    for t in 1..=60u32 {
        step(&mut sim, &mut world, &ee);
        let b = sim.traffic.props.collections[BALL].bones[0].body;
        let bike = sim.rider.ride.chassis.cofg_world;
        let gap = len3(Vf::new(b.position.x() - bike.x(), b.position.y() - bike.y(), b.position.z() - bike.z(), 0.0));
        let hit = struck(&sim);
        let heard = &sim.traffic.props.collections[BALL].heard.0;
        if !heard.is_empty() {
            println!("+{t:2} ball {BALL} heard {heard:?}, shock peak {:?}", b.shock_peak.0);
        }
        if hit && t > 2 {
            again.push(t);
        }
        // Image 2903 + t is the capture's row 2904 + t.
        let Some(r) = row(2904 + t) else { break };
        let (bv, bp, cv, cp) = (v3(r, "b2_vel"), v3(r, "b2_pos"), v3(r, "ch_vel"), v3(r, "ch_cofg"));
        let ogap = len3(Vf::new(bp.x() - cp.x(), bp.y() - cp.y(), bp.z() - cp.z(), 0.0));
        let touch = r[col("b2_touch")];
        println!(
            "+{t:2} ball {:6.2} vy {:6.2} y {:7.2} | orig {:6.2} vy {:6.2} y {:7.2}  bike {:6.2} | orig {:6.2}  gap {:5.2} | orig {:5.2}  struck {} | orig touch_other {}",
            len3(b.velocity),
            b.velocity.y(),
            b.position.y(),
            len3(bv),
            bv.y(),
            bp.y(),
            len3(sim.rider.ride.chassis.vel),
            len3(cv),
            gap,
            ogap,
            hit as u8,
            &touch[8..10]
        );
    }
    assert!(again.is_empty(), "the bike strikes the ball again on ticks {again:?} after image 2903");
}

/// **A struck ball is heard in its strike tick** (B28). The original's node re-reads its filed count
/// at every step of its collide walk, and the bike's strike files the woken ball behind the bike, so
/// the ball's probe searches in the strike's own tick: f27a1's queue holds the ball's entry on the
/// bike's frame on the strike tick, at the ball's shock peak. No image before the strike exists, so
/// this stands one in: image 2903's ride with ball 2 as the track loads it (asleep, unfiled), where
/// the bike's rays strike it. One step hears the ball's probe on the bike, and the hearing moves
/// nothing: ball and chassis are bit for bit those of the same step with no bike frame heard.
#[test]
fn a_struck_ball_is_heard_in_its_strike_tick() {
    let tables = Arc::new(Tables::build());
    let Some(mut free) = free09(&tables) else { return };
    let props = free.props.take().unwrap();
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let mut world = Place { ground: Ground::new(&terrain, &free.scene.terrain), props };
    let Some(ee) = image(2903) else { return };
    let unwoken = |heard: bool| {
        let mut sim = seeded(&ee, &free, &tables);
        let set = &mut sim.traffic.props;
        set.collections[BALL] = free.set.collections[BALL].clone();
        set.listeners.retain(|&(c, _)| c != BALL);
        if !heard {
            set.bike_frame = None;
        }
        let o = &set.collections[BALL].objects[0];
        assert!(!o.filed && !set.collections[BALL].bones[0].body.enabled, "the ball as loaded: asleep, unfiled");
        sim
    };
    let mut sim = unwoken(true);
    step(&mut sim, &mut world, &ee);
    assert!(struck(&sim), "the bike's rays strike the ball");
    let ball = &sim.traffic.props.collections[BALL];
    assert!(ball.objects[0].filed, "the strike filed it");
    assert_eq!(ball.heard.0, vec![(0, game_sim::material_hit::Struck::Bike(0))], "its probe heard on the bike in the strike tick");
    let mut deaf = unwoken(false);
    step(&mut deaf, &mut world, &ee);
    assert!(deaf.traffic.props.collections[BALL].heard.0.is_empty());
    let (a, b) = (ball.bones[0].body, deaf.traffic.props.collections[BALL].bones[0].body);
    for (what, p, o) in [
        ("ball position", a.position, b.position),
        ("ball velocity", a.velocity, b.velocity),
        ("ball shock peak", a.shock_peak, b.shock_peak),
        ("bike velocity", sim.rider.ride.chassis.vel, deaf.rider.ride.chassis.vel),
    ] {
        assert_eq!(p.to_bits(), o.to_bits(), "{what}: the hearing moved it");
    }
}
