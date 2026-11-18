//! MD168 — **MX vs. ATV Untamed's (SLUS-21701) race quad replayed from MD147's captures**: the port's
//! [`quad::Ride`] seeded from MD147's spawn image on Rocksbury, on Untamed's step ([`quad::Tail::untamed`]
//! and [`atv_moto::Scene::untamed`]), against the rows MD147 recorded every tick
//! (research/journal/2026-11-15-md147-untamed-atv-capture).
//!
//! The quad is made as the shell makes it, from the `untamed_atv` pack's files ([`quad::Ride::spawn`],
//! [`quad::Ride::rig`]), and its state is then read from the image at Untamed's quad offsets ([`u`]:
//! SLUS-21104's moved). The rigid body, the wheels, the engine and the world keep SLUS-21104's layout
//! inside; the scene nodes' words are 0x30 further in ([`NODE`]).
//!
//! The ground is Untamed's own: its ground query called in the interpreter on the image's terrain
//! (Rocksbury's track file is not on this disk), the record byte it writes handed to the wheel.
//!
//! Without the image, the captures or the pack the tests skip and say so.

use game_sim::handling::{self, seed};
use game_sim::quad::{self, Call, Params, Quad, Wheel};
use game_sim::{atv_moto, atv_wheel, r#move as mv};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::path::PathBuf;

/// MD147's spawn image's quad and its vtable word.
const QUAD: u32 = 0x01be_4760;
const QUAD_VTABLE: u32 = 0x003f_0470;
/// SLUS-21701's ground query (the same code as SLUS-21104's).
const GROUND_QUERY: u32 = 0x004b_0910;
/// Scratch words for the query's point, normal and record byte, past what the image uses.
const POINT: u32 = 0x01f0_0000;
const NORMAL: u32 = 0x01f0_0010;
const RECORD: u32 = 0x01f0_0020;
/// MD224: the collision record of the knockable bale steer.csv's take-off strikes (`TripleBale02`,
/// [`bale`]), in every MD147 state.
const BALE: u32 = 0x00aa_c3d0;
/// The frame's dt, as the image keeps it (0.033333).
const DT: u32 = 0x3d08_850a;
/// The scene nodes' words from `+0x30` are SLUS-21104's 0x30 further in.
const NODE: u32 = 0x30;

/// SLUS-21104's quad offset `o` at SLUS-21701's (pinned by MD168 against the two games' spawn images
/// and the paired functions' offsets): the base object's words from `+0x2c` 0x30 further in, the
/// run's block 0x24, the `+0xb00..+0x12c0` block 0x10 back (Untamed has 0x20 new bytes at
/// `+0x12b0`), then +0x10 to the engine, +0x20 round the rider's words and +0x40 from the stunt
/// clips to the wheels.
fn u(o: u32) -> u32 {
    match o {
        0..=0x2b => o,
        0x2c..=0x413 => o + 0x30,
        0x49c..=0x713 => o + 0x24,
        0xb00..=0x12bf => o - 0x10,
        0x12c0..=0x14c3 => o + 0x10,
        0x14c4..=0x1533 => o + 0x20,
        0x1654..=0x17bf => o + 0x40,
        _ => panic!("SLUS-21104's quad +{o:#x} has no known place in SLUS-21701's"),
    }
}

fn kit() -> Option<Kit> {
    Kit::load().map_err(|w| skip(&w)).ok()
}

/// One of MD147's states as its RAM image: `spawn`, or a hazard state (the spawn with the surface
/// table poked; their `.p2s`' `eeMemory.bin`).
fn image(kit: &Kit, state: &str) -> Option<Ee> {
    let path = kit.ctx("untamed/md147").join(format!("{state}{}", ".ee.bin"));
    let Ok(ee) = Ee::from_ram(&path) else {
        skip(&format!("{} is not there (MD147's; a hazard state's is its .p2s' eeMemory.bin)", path.display()));
        return None;
    };
    assert_eq!(ee.u32_at(QUAD + 4), QUAD_VTABLE, "{} is not MD147's spawn image", path.display());
    Some(ee)
}

/// The `untamed_atv` pack's race quad: its `.phy`, its chassis and its riding and wreck collision.
struct Files {
    ini: game_sim::phy::Ini,
    model: quad::Model,
    riding: game_sim::track::Rays,
    wreck: game_sim::track::Rays,
}

fn files() -> Option<Files> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../mods/machines/untamed_atv/Veh");
    let read = |p: &str| std::fs::read(dir.join(p)).ok();
    let (Some(phy), Some(psg), Some(riding), Some(wreck)) =
        (read("atv_Race250P.phy.bpb"), read("Anim/250r_atv_chasis.psg"), read("Col/ATV_R_ChasisM.col"), read(&format!("Col/{}", quad::WRECK_FILE)))
    else {
        skip(&format!("{} has not the race quad's files (the untamed_atv pack, off Untamed's disc)", dir.display()));
        return None;
    };
    Some(Files {
        ini: game_sim::bpb::ini(&phy).expect("atv_Race250P.phy.bpb"),
        model: quad::Model::parse(&psg).unwrap_or_else(|e| panic!("250r_atv_chasis.psg: {e}")),
        riding: game_sim::track::Rays::parse(&riding).unwrap_or_else(|e| panic!("ATV_R_ChasisM.col: {e}")),
        wreck: game_sim::track::Rays::parse(&wreck).unwrap_or_else(|e| panic!("WreckATVm.col: {e}")),
    })
}

fn f(ee: &Ee, a: u32) -> F {
    F(f32::from_bits(ee.u32_at(a)))
}
fn v(ee: &Ee, a: u32) -> Vf {
    let w = |k: u32| f32::from_bits(ee.u32_at(a + 4 * k));
    Vf::new(w(0), w(1), w(2), w(3))
}
fn b(ee: &Ee, a: u32) -> bool {
    ee.u32_at(a) != 0
}
fn i(ee: &Ee, a: u32) -> i32 {
    ee.i32_at(a)
}
fn put_v(ee: &mut Ee, a: u32, x: Vf) {
    for k in 0..4 {
        ee.set_u32(a + 4 * k as u32, x.0[k].to_bits());
    }
}

/// Where the quad's parts are in the image.
#[derive(Default, Debug)]
struct At {
    body: u32,
    world: u32,
    record: u32,
    engine: u32,
    wheels: [u32; 4],
    nodes: Vec<u32>,
    state: u32,
    area: u32,
}

/// The scene round the quad's node, breadth first over each node's parent, child and sibling.
fn scene_nodes(ee: &Ee, first: u32) -> Vec<u32> {
    let mut out = vec![first];
    let mut k = 0;
    while k < out.len() && out.len() < atv_moto::NODES {
        for o in [0xf0, 0xf4, 0xf8] {
            let n = ee.u32_at(out[k] + NODE + o);
            if n != 0 && !out.contains(&n) && out.len() < atv_moto::NODES {
                out.push(n);
            }
        }
        k += 1;
    }
    out
}

fn at(ee: &Ee) -> At {
    let w = |x: u32| ee.u32_at(QUAD + u(x));
    let mut at = At { body: w(0xb8), world: w(0xe8), record: w(0xc8), engine: w(0x142c), state: w(0xbc4), ..At::default() };
    for k in 0..4 {
        at.wheels[k] = w(0x17b0 + 4 * k as u32);
    }
    at.area = ee.u32_at(at.world + 0xa8);
    at.nodes = scene_nodes(ee, w(0xe4));
    at
}

/// The image's keys, as SLUS-21104's image reader reads them (tests/atv_quad/image.rs) at Untamed's
/// offsets.
fn params(ee: &Ee) -> Params {
    let q = |x: u32| f(ee, QUAD + u(x));
    Params {
        max_pitch_command: q(0x139c),
        pitch_gain: q(0x13a0),
        pitch_rate_gain: q(0x13a4),
        inv_wheelie_top_speed: q(0x13a8),
        max_stoppie_pitch_command: q(0x13cc),
        stoppie_pitch_gain: q(0x13d0),
        stoppie_yaw_rate_scale: q(0x13d4),
        stoppie_top_speed: q(0x13d8),
        steer_speed_1: q(0x1300),
        steer_scale_1: q(0x1304),
        steer_speed_2: q(0x1308),
        steer_scale_2: q(0x130c),
        full_lock: b(ee, QUAD + u(0x12f0)),
        full_lock_angle: q(0x12f4),
        full_lock_time: q(0x12f8),
        max_donut_speed: q(0x122c),
        donut_angle: q(0x1230),
        donut_angle_back: q(0x1234),
        front_spring: (q(0x137c), q(0x1380)),
        rear_spring: (q(0x1384), q(0x1388)),
    }
}

/// The image's state laid over `ride` (the pack's quad): its keys and words, the body, the wheels, the
/// engine, the air state, the scene's nodes, the tail's words. The collidables, the collision record
/// and the rays stay the pack's.
fn seeded(ee: &Ee, at: &At, mut ride: quad::Ride) -> quad::Ride {
    let o = QUAD;
    let qf = |x: u32| f(ee, o + u(x));
    let qb = |x: u32| b(ee, o + u(x));
    let qv = |x: u32| v(ee, o + u(x));
    let (bo, wo, ro, eo) = (at.body, at.world, at.record, at.engine);

    let mut wheels = [Wheel::default(); 4];
    for (w, &a) in wheels.iter_mut().zip(&at.wheels) {
        *w = Wheel {
            steered: b(ee, a + 0x2c),
            rear_pair: b(ee, a + 0xb4),
            touching: b(ee, a + 0xdc),
            normal: v(ee, a + 0x130),
            contact_vel: v(ee, a + 0x190),
            balancing: b(ee, a + 0x30),
            max_steer_angle: f(ee, a + 0x70),
            steer_command: f(ee, a + 0x8ac),
            donut: f(ee, a + 0x38),
            spring: f(ee, a + 0x48),
            inv_radius: f(ee, a + 0x838),
        };
    }
    let bal = (ee.u32_at(o + u(0x1524)), ee.u32_at(o + u(0x152c)));
    let vstate = (at.state != 0).then(|| ee.u32_at(at.state + 0x28));
    ride.p = params(ee);
    ride.q = Quad {
        on_ground: qb(0x324),
        all_down: qb(0x1270),
        front_down: qb(0x1274),
        rear_down: qb(0x1278),
        balancing: bal.0 == bal.1,
        contact_vel: qv(0xc30),
        brake: qf(0x310),
        gas: qf(0x314),
        gas_limited: qf(0x318),
        euler: qv(0x360),
        world_ang_vel: qv(0x340),
        front_brake: qf(0x12cc),
        rear_brake: qf(0x12d0),
        clutch_in: qf(0x12d4),
        lean: qf(0x12e8),
        lean_fore_aft: qf(0x12ec),
        pitch_integral: qf(0x13ac),
        stoppie: qb(0x13b0),
        push: qb(0xdd8),
        push_target: qf(0x123c),
        push_gain: qf(0x1240),
        push_gate: qf(0x12c4),
        gravity: v(ee, wo + 0x40),
        wheels,
        active: matches!(vstate, Some(s) if !(3..5).contains(&s)),
        control_off: qb(0xc80),
        speed_len: qf(0x32c),
        steer_command: qf(0x12c8),
        full_lock_timer: qf(0x12fc),
        stick_angle: qf(0x12b8),
        deflection: qf(0x12bc),
        attitude: qv(0x1250),
        local_vel: qv(0x350),
        heading: qv(0xbe0),
        up: qv(0xbf0),
        was_airborne: qb(0xde4),
        airborne: qb(0xde0),
        hang_time: qf(0x1190),
        crossup: qf(0x330),
        flipping: qb(0x1198),
        spinning: qb(0x11d4),
        authority: qf(0x1188),
        authority_time: qf(0x118c),
        tipped: b(ee, ro + 0x24),
        node_pos: qv(0xbd0),
        node_pos_kept: qv(0xc40),
        node_local: Vf::new(0.0, 0.0, 0.0, 1.0),
    };
    ride.body = seed::body(&|a| ee.u32_at(a), bo);

    let bike = &mut ride.bike;
    bike.held.lock.steer = !qb(0xdd0);
    bike.held.lock.throttle = !qb(0xdd4);
    bike.held.lock.steer_was = !qb(0x1298);
    bike.held.lock.latch = qb(0x129c);
    bike.joystick = game_sim::phy::Joystick {
        x_axis_rate_limit: qf(0x12d8),
        y_axis_rate_limit: qf(0x12dc),
        x_dpad_rate_limit: qf(0x12e0),
        y_dpad_rate_limit: qf(0x12e4),
        gas_rate_limit: qf(0x1310),
        brake_rate_limit: qf(0x1314),
        clutch_in_rate_limit: qf(0x1318),
        clutch_out_rate_limit: qf(0x131c),
    };
    // Untamed's run (`+0x6a4`) has its gate word 0 at the spawn: the gate does not cut.
    bike.gate_cut = false;
    let ck = &mut bike.clutch_keys;
    ck.clutch_torque_scale_time = qf(0x1350);
    ck.max_clutch_torque_scale_speed = qf(0x1358);
    ck.max_clutch_torque_scale = qf(0x135c);
    ck.up_shift_rpm = qf(0x13dc);
    ck.down_shift_rpm = qf(0x13e0);
    ck.cofg = qv(0x1130);
    ck.balance_cofg = qv(0x1150);
    ck.cofg_travel_forward = qf(0x1320);
    ck.cofg_travel_back = qf(0x1324);
    ck.clutch_cofg_travel_back = qf(0x1328);
    ck.brake_cofg_travel_forward = qf(0x132c);
    ck.drag = f(ee, wo + 4);
    bike.ride.clutch_timer = qf(0x1348);
    bike.ride.clutch_scale_start = qf(0x134c);
    bike.ride.clutch_cofg_weight = qf(0x1354);

    let e = &mut bike.engine;
    e.gear = i(ee, eo + 0x5c);
    e.gears = i(ee, eo + 0x6c);
    e.final_drive = f(ee, eo + 0x34);
    e.ratio = f(ee, eo + 0x74);
    e.inv_ratio = f(ee, eo + 0x78);
    e.clutch_torque_scale = f(ee, eo + 0x7c);
    e.clutch_locked = b(ee, eo + 0x58);
    e.rev_limit = f(ee, eo + 0xac);
    e.idle_throttle = f(ee, eo + 0x2c);
    e.engine_braking = f(ee, eo + 0x30);
    e.inv_inertia = f(ee, eo + 0x3c);
    e.clutch_static = f(ee, eo + 0x44);
    e.clutch_kinetic = f(ee, eo + 0x48);
    e.min_rpm = f(ee, eo + 0x54);
    e.curve = mv::Curve::at(ee.u32_at(eo + 0x80), ee.u32_at(eo + 0x84), f(ee, eo + 0x88));
    e.torque = f(ee, eo + 0x98);
    e.wheel_torque = f(ee, eo + 0x9c);
    e.rpm = f(ee, eo + 0xa0);
    e.road_rpm = f(ee, eo + 0xa4);
    e.torque_scale = f(ee, eo + 0xb4);
    let (ratios, torque, rpm) = (ee.u32_at(eo + 0x70), ee.u32_at(eo + 0x68), ee.u32_at(eo + 0x8c));
    for (k, r) in e.gear_ratios.iter_mut().enumerate() {
        *r = f(ee, ratios + 4 * k as u32);
    }
    for k in 0..e.curve_rpm.len() {
        e.curve_rpm[k] = f(ee, rpm + 4 * k as u32);
        e.curve_torque[k] = f(ee, torque + 4 * k as u32);
    }

    let k = &mut bike.air_keys;
    k.preload_time = qf(0xc5c);
    k.min_preload_time = qf(0xc64);
    k.max_preload_time = qf(0xc68);
    k.max_preload = qf(0xc6c);
    k.min_dpad_preload_time = qf(0x1160);
    k.max_dpad_preload_time = qf(0x1164);
    k.min_backflip_time = qf(0x11a4);
    k.max_backflip_time = qf(0x11a8);
    k.min_backflip_torque = qf(0x11b0);
    k.max_backflip_torque = qf(0x11b4);
    k.backflip_torque_rate_increase = qf(0x11b8);
    k.backflip_torque_rate_decrease = qf(0x11bc);
    k.airborne_pitch_down_wreck_angle = qf(0x1404);
    k.airborne_pitch_up_wreck_angle = qf(0x1408);
    k.front_wreck_speed_limit = qf(0x13e8);
    k.back_wreck_speed_limit = qf(0x13ec);
    k.side_wreck_speed_limit = qf(0x13f0);
    k.airborne_control_moment = qv(0x1170);
    k.airborne_damping = qf(0x1180);
    k.airborne_control_force = qf(0x1184);
    k.brake_gyro_torque = qf(0x1330);
    k.unroll_gain = qf(0x1338);
    k.unroll_torque = qf(0x133c);
    k.unyaw_gain = qf(0x1340);
    k.unyaw_torque = qf(0x1344);
    let air = &mut bike.air;
    air.preload_ready = qb(0xc50);
    air.preload_firing = qb(0xc54);
    air.preload_armed = qb(0xc58);
    air.preload_timer = qf(0xc60);
    air.preload_force = qv(0xc70);
    air.flip_torque_applied = qb(0x1194);
    air.flip_active = qb(0x119c);
    air.flip_armed = qb(0x11a0);
    air.flip_grace = qf(0x11ac);
    air.flip_torque = qv(0x11c0);

    for k in 0..4 {
        let (a, qw) = (at.wheels[k], &ride.q.wheels[k]);
        bike.wheels[k] = game_sim::wheel::Wheel {
            has_ground: ee.u32_at(a + 0x1e0) != 0,
            steered: qw.steered,
            balancing: qw.balancing,
            contact: qw.touching,
            contact_normal: qw.normal,
            max_steer_angle: qw.max_steer_angle,
            steer_angle: qw.steer_command,
            spring_rate: qw.spring,
            max_compression: f(ee, a + 0x3c),
            max_damper_speed: f(ee, a + 0x40),
            extension_rate: f(ee, a + 0x44),
            damping_out: f(ee, a + 0x4c),
            damping_in: f(ee, a + 0x50),
            probe: v(ee, a + 0x80),
            mount: v(ee, a + 0x90),
            ray_hit: i(ee, a + 0xd0),
            swept_contact: i(ee, a + 0xd4),
            terrain_wins: i(ee, a + 0xd8),
            ray_steps: i(ee, a + 0xec),
            compression: f(ee, a + 0xf0),
            last_compression: f(ee, a + 0xf4),
            spring_force: f(ee, a + 0xf8),
            probe_end: v(ee, a + 0x100),
            contact_point: v(ee, a + 0x120),
            load_point: v(ee, a + 0x140),
            mount_world: v(ee, a + 0x150),
            probe_end_world: v(ee, a + 0x160),
            contact_velocity: v(ee, a + 0x170),
            chassis_velocity: v(ee, a + 0x180),
            ground_normal: v(ee, a + 0x200),
            probe_length: f(ee, a + 0x214),
            steer_torque: f(ee, a + 0x60),
            steer_damping: f(ee, a + 0x64),
            steer_inv_inertia: f(ee, a + 0x6c),
            min_steer_angle: f(ee, a + 0x74),
            steer_rate: f(ee, a + 0x8b0),
            ..Default::default()
        };
        bike.more[k] = atv_wheel::More {
            record: ee.u8_at(a + 0x110),
            kept: v(ee, a + 0xc0),
            chassis_velocity_local: qw.contact_vel,
            ground_known: i(ee, a + 0x1a0),
            ground_velocity: v(ee, a + 0x1b0),
            ground_point_velocity: v(ee, a + 0x1c0),
            steer_axis: v(ee, a + 0x8c0),
        };
        let drive = atv_moto::Drive { spin: f(ee, a + 0x1d4), torque: f(ee, a + 0x210), from_torque: b(ee, a + 0xe0), forward: f(ee, a + 0x858) };
        bike.parts.drive[k] = drive;
        let mut surfaces = [quad::TireSurface::default(); 16];
        for (j, t) in surfaces.iter_mut().enumerate() {
            let at = |n: usize| f(ee, a + 0x238 + 0x40 * n as u32 + 4 * j as u32);
            (t.drive, t.side, t.drag) = (std::array::from_fn(at), std::array::from_fn(|n| at(8 + n)), std::array::from_fn(|n| at(21 + n)));
        }
        let brake = match ee.u32_at(a + 0x24) {
            0 => None,
            p => Some(f(ee, p)),
        };
        bike.spins[k] = atv_wheel::Spin {
            brake,
            parametric: i(ee, a + 0x34),
            donut: qw.donut,
            inv_inertia: f(ee, a + 0x58),
            tire_inertia: f(ee, a + 0x5c),
            arm: v(ee, a + 0xa0),
            no_moment: i(ee, a + 0xb0),
            rear_pair: i32::from(qw.rear_pair),
            other_load_scale: f(ee, a + 0xb8),
            slip: game_sim::tire::Slip { spinning: drive.from_torque, locked: b(ee, a + 0xe4), sliding: b(ee, a + 0xe8) },
            angle: f(ee, a + 0x1d0),
            spin: drive.spin,
            torque: drive.torque,
            cleared: [f(ee, a + 0x224), f(ee, a + 0x228)],
            side_per_load: f(ee, a + 0x234),
            surfaces,
            inv_radius: qw.inv_radius,
            local_velocity: v(ee, a + 0x840),
            local_force: v(ee, a + 0x850),
            frame: [v(ee, a + 0x8d0), v(ee, a + 0x8e0), v(ee, a + 0x8f0), v(ee, a + 0x900)],
        };
    }
    let wheel_index = |p: u32| at.wheels.iter().position(|&w| w == p).expect("the engine's driven wheel is one of the quad's");
    bike.parts.driven = (wheel_index(ee.u32_at(eo + 0x90)), (ee.u32_at(eo + 0x94) != 0).then(|| wheel_index(ee.u32_at(eo + 0x94))));

    let index = |p: u32| at.nodes.iter().position(|&n| n == p);
    for (k, &a) in at.nodes.iter().enumerate() {
        let n = &mut bike.parts.scene.nodes[k];
        let link = |off: u32| match ee.u32_at(a + NODE + off) {
            0 => None,
            p => index(p),
        };
        (n.parent, n.child, n.sibling) = (link(0xf0), link(0xf4), link(0xf8));
        let w = a + NODE;
        n.local = std::array::from_fn(|r| v(ee, w + 0x30 + 0x10 * r as u32));
        n.world = std::array::from_fn(|r| v(ee, w + 0x70 + 0x10 * r as u32));
        n.flags = ee.u8_at(w + 0x100);
        n.more = ee.u8_at(w + 0x101);
        n.spatial = b(ee, a + 8);
        n.centre = v(ee, w + 0xd0);
        n.half = v(ee, w + 0xe0);
        n.reach = f(ee, w + 0x108);
        n.own_centre = v(ee, w + 0xb0);
        n.own_half = v(ee, w + 0xc0);
        n.radius = f(ee, w + 0x104);
        let (count, list) = (ee.u32_at(w + 0x10c), ee.u32_at(w + 0x144));
        n.listed = 0;
        if count as usize <= atv_moto::NODES {
            let m: Option<Vec<usize>> = (0..count).map(|i| index(ee.u32_at(list + 4 * i))).collect();
            if let Some(m) = m {
                n.listed = m.len();
                n.list[..m.len()].copy_from_slice(&m);
            }
        }
    }
    bike.parts.scene.untamed = true;
    let n0 = at.nodes[0] + NODE;
    let sp = &mut bike.parts.spin;
    sp.armed = qb(0x11dc);
    sp.min_time = qf(0x11e0);
    sp.max_time = qf(0x11e4);
    sp.grace = qf(0x11e8);
    sp.min_pitch = qf(0x11ec);
    sp.max_pitch = qf(0x11f0);
    sp.min_yaw = qf(0x11f4);
    sp.max_yaw = qf(0x11f8);
    sp.rate = qf(0x1200);
    sp.moment = qv(0x1210);
    sp.heading = qf(0x1220);
    sp.dir = i(ee, o + u(0x1224));
    sp.flick = qf(0x12b4);
    sp.stunt_right = ee.u32_at(o + u(0x13bc));
    sp.stunt_left = ee.u32_at(o + u(0x13c0));
    sp.clip = i(ee, o + u(0x1758));
    bike.parts.cofg_off = qv(0x1140);
    bike.parts.last_position = v(ee, bo + 0x60);
    bike.parts.body_on = b(ee, bo);
    bike.parts.flags = ee.u8_at(o + u(0x45));
    bike.parts.spatial = b(ee, o + 8);
    bike.parts.bounds = atv_moto::Bounds { floor: f(ee, wo + 0x98), area: (at.area != 0).then(|| (v(ee, at.area + 0x30), v(ee, at.area + 0x40))) };
    bike.parts.collidables.sync = ee.u8_at(o + u(0x44)) & 1 != 0;
    bike.parts.rear_node = ee.u32_at(o + u(0x143c));

    bike.gas_cut = handling::step::GasCut { latched: qb(0x1764), timer: qf(0x1768) };
    bike.cut_time = qf(0x176c);
    bike.stick = game_sim::trick::Stick { x: qf(0x12a8), y: qf(0x12ac), digital: [qb(0x12a0), qb(0x12a4)] };
    bike.clutch = qf(0x12c0);
    bike.stick_hold = qf(0x12b0);
    bike.preload_cue = qb(0x700);
    bike.spin_applied = qb(0x11d0);
    bike.spin_active = qb(0x11d8);
    bike.touching = b(ee, ro + 0x34);
    assert!(!bike.touching, "the spawn image's collision record is touching something");
    bike.struck_class = false;
    bike.buttons = ee.u8_at(o + u(0x14ec)) & 0xf;
    bike.right_y = f(ee, n0 + 0x84);
    bike.up_y = f(ee, n0 + 0x94);
    bike.stunt_id = i(ee, o + u(0x1528));
    bike.rider_stunt = bal.0 as i32;
    bike.flip_clip = handling::FlipClip(i(ee, o + u(0x1754)));
    bike.flip_stunt = ee.u32_at(o + u(0x13b8));
    bike.gas_gyro_torque = qf(0x1334);
    bike.gyro = handling::Gyro { brake: qb(0x1290), gas: qb(0x1294) };
    bike.cofg = v(ee, bo + 0x1a0);

    let t = ride.tailed.as_mut().expect("a rigged quad has its tail");
    t.tail.untamed = true;
    t.tail.vehicle_state = vstate;
    // The hazard table, 12 bytes a surface: the hazard word, then its time.
    t.tail.surfaces = std::array::from_fn(|k| {
        let a = o + u(0xdf4) + 0xc * k as u32;
        quad::Surface { hazard: ee.u32_at(a) != 0, time: f(ee, a + 4) }
    });
    (t.vehicle.free, t.vehicle.stuck, t.vehicle.stuck_time) = (ee.u32_at(o + u(0xc14)), ee.u32_at(o + u(0xc10)), qf(0xdf0));
    // MD168a: where it was last free, and the place its put-back stands it at (the spot, the
    // heading flattened, the ground's up).
    t.vehicle.last_free = qv(0xc00);
    t.untamed = Some(handling::untamed::PutBack { saved_position: qv(0xd40), saved_orientation: qv(0xd50), saved_up: qv(0xd60), has_ground: true, ..Default::default() });
    // MD168b: the map edge's armed word (SLUS-21104's `+0x308`) is the image's, 0 in every MD147 state:
    // Free Ride does not arm the out-of-bounds throw, whose band on Untamed's step (300 ft, MD144a)
    // reaches Rocksbury's spawn.
    if let Some(w) = ride.wrecked.as_mut() {
        w.edge = qb(0x308);
    }
    ride
}

/// Rocksbury as Untamed's ground query answers it, in the interpreter on the image's terrain: the
/// point in, the ground point and its normal back, the record byte the query writes the wheel's
/// surface. Nothing on it is solid: the quad's sweeps hit nothing but the ground.
struct Rocksbury {
    ee: Ee,
    terrain: u32,
    record: u8,
    area: F,
    props: game_sim::world::Props,
}

impl game_sim::world::World for Rocksbury {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let ee = &mut self.ee;
        put_v(ee, POINT, at);
        ee.write(RECORD, 1, 0);
        ee.call(GROUND_QUERY, &[u64::from(self.terrain), u64::from(POINT), u64::from(NORMAL), 0, u64::from(RECORD)], &[], 2_000_000);
        self.record = ee.u8_at(RECORD);
        (v(ee, POINT), v(ee, NORMAL))
    }
    fn wheel_surface(&mut self, _at: Vf) -> Option<u8> {
        Some(self.record)
    }
    fn solid(&mut self, bundle: &game_sim::collide::Bundle, record: &mut game_sim::collide::Record) -> bool {
        self.props.sweep(bundle, record)
    }
    fn area(&self) -> Option<game_sim::biplane::Area> {
        Some(game_sim::biplane::Area { width: self.area, depth: self.area })
    }
}

/// One capture row: its columns by name.
struct Row(std::collections::BTreeMap<String, String>);

impl Row {
    fn f(&self, k: &str) -> f32 {
        self.0.get(k).unwrap_or_else(|| panic!("no column {k}")).parse::<f32>().unwrap_or_else(|e| panic!("{k}: {e}"))
    }
    fn v(&self, k: &str) -> Vf {
        Vf::new(self.f(&format!("{k}.x")), self.f(&format!("{k}.y")), self.f(&format!("{k}.z")), self.f(&format!("{k}.w")))
    }
}

fn rows(kit: &Kit, name: &str) -> Option<Vec<Row>> {
    rows_in(kit, "md147", name)
}

/// A capture's rows: `task`'s (`context/capture/<task>`) `name`.csv.
fn rows_in(kit: &Kit, task: &str, name: &str) -> Option<Vec<Row>> {
    let path = kit.ctx(&format!("capture/{task}")).join(format!("{name}{}", ".csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("{} is not there ({}'s)", path.display(), task.to_uppercase()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
    Some(lines.map(|l| Row(head.iter().cloned().zip(l.split(',').map(str::to_string)).collect())).collect())
}

/// The pad as the row holds it where the reader stored it: the brake and gas (the controls' `z`/`w`)
/// and the stick.
fn controls(r: &Row) -> game_sim::ride::Controls {
    let (ctl, stick) = (r.v("ctl"), r.v("stick"));
    game_sim::ride::Controls { gas: F(ctl.0[3]), brake: F(ctl.0[2]), stick: (F(stick.0[0]), F(stick.0[1])), ..Default::default() }
}

struct Setup {
    ride: quad::Ride,
    world: Rocksbury,
    segments: [[Vf; 2]; 4],
}

/// MD250: Rocksbury's start spot and heading, `SX_A.scn`'s `[SceneInfo]` `DefaultStartPosition` and
/// `DefaultStartDirection` (`395.76,3.76,155.86` facing `0,0,1`), where the quad is made and its
/// wheel setup and part reader read the model. `w` 1 on both, as the creation copies them.
fn scene_start() -> (Vf, Vf) {
    let path = kit().expect("the kit").ctx("untamed/sx_a").join("env/sx_a/SX_A.scn.bpb");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e} (SX_A.res extracted, MD168b)", path.display()));
    let ini = game_sim::bpb::ini(&bytes).unwrap_or_else(|e| panic!("SX_A.scn: {e}"));
    let three = |key: &str| {
        let (_, _, value) = ini.entries().find(|&(s, k, _)| s == "SceneInfo" && k == key).unwrap_or_else(|| panic!("SX_A.scn has no {key}"));
        let w: Vec<f32> = value.split(',').map(|x| x.trim().parse().unwrap_or_else(|e| panic!("{key}: {e}"))).collect();
        Vf::new(w[0], w[1], w[2], 1.0)
    };
    (three("DefaultStartPosition"), three("DefaultStartDirection"))
}

/// The pack's quad made and rigged as the shell makes it, at the image's body and in its world's box;
/// the model read at the scene's start spot ([`scene_start`]).
fn made(fs: &Files, ee: &Ee, at: &At) -> quad::Ride {
    let spot = v(ee, at.body + 0x70);
    let bounds = (at.area != 0).then(|| (v(ee, at.area + 0x30), v(ee, at.area + 0x40)));
    let curve = game_sim::camera::CurveTable::build();
    let (spot, heading) = (Vf::new(spot.0[0], spot.0[1], spot.0[2], 1.0), Vf::new(0.0, 0.0, 1.0, 1.0));
    let (start, facing) = scene_start();
    let load = quad::Load::read_model(&fs.ini, &fs.model, start, facing, &curve).unwrap_or_else(|e| panic!("{e}"));
    let mut ride = quad::Ride::spawn(&load, &fs.model, spot, heading, bounds, Vec::<Call>::new());
    ride.rig(&load, &fs.riding, &fs.wreck);
    ride
}

fn setup(kit: &Kit, state: &str) -> Option<Setup> {
    let fs = files()?;
    let ee = image(kit, state)?;
    let at = at(&ee);
    let mut ride = seeded(&ee, &at, made(&fs, &ee, &at));
    // The map-edge gate as the image holds it (off in Free Ride): armed by default, Rocksbury's spawn
    // is inside the port's edge band and the quad is thrown every tick (MD144a's throw).
    if let Some(w) = ride.wrecked.as_mut() {
        w.edge = b(&ee, QUAD + u(0x308));
    }
    let segments = ride.trigger_segments().expect("the quad's trigger probe");
    let terrain = ee.u32_at(at.wheels[0] + 0x1e0);
    let area = f(&ee, terrain + 0x40);
    let props = solids(kit)?;
    Some(Setup { ride, world: Rocksbury { ee, terrain, record: 0, area, props }, segments })
}

/// MD198: Rocksbury's solids, the scene's collision list (`SX_A.scn`'s `[CollisionObjects]`, as the
/// Free Ride activity leaves it) placed from the disc's `SX_A.res` (`context/untamed/sx_a`): the
/// stadium's shell (`FP01A_A.col`), whose side wall hazard-time's quad strikes on row 255, the
/// towers, gates and lights. A file the archive does not have is skipped (`FP01A_E.col`).
fn solids(kit: &Kit) -> Option<game_sim::world::Props> {
    let dir = kit.ctx("untamed/sx_a");
    let Ok(bytes) = std::fs::read(dir.join("env/sx_a/SX_A.scn.bpb")) else {
        skip(&format!("{} has no SX_A.scn.bpb (SX_A.res extracted, MD168b)", dir.display()));
        return None;
    };
    let ini = game_sim::bpb::ini(&bytes).unwrap_or_else(|e| panic!("SX_A.scn: {e}"));
    let mut text = String::new();
    let mut last = None;
    for (section, key, value) in ini.entries() {
        if last != Some(section) {
            text += &format!("[{section}]\n");
            last = Some(section);
        }
        text += &format!("{key}={value}\n");
    }
    let scene = game_sim::track::Scene::parse(text.as_bytes()).unwrap_or_else(|e| panic!("SX_A.scn: {e}"));
    let files: Vec<PathBuf> = std::fs::read_dir(dir.join("col")).ok()?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    let (props, missed) = game_sim::world::Props::load(&scene, |name| {
        let p = files.iter().find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case(name))).ok_or("not in SX_A.res")?;
        std::fs::read(p).map_err(|e| e.to_string())
    });
    assert_eq!(missed, ["FP01A_E.col: not in SX_A.res"], "Rocksbury's solids not placed");
    Some(props)
}

/// The row's hex column as words.
fn hex_words(r: &Row, k: &str) -> Vec<u32> {
    let h = r.0.get(k).unwrap_or_else(|| panic!("no column {k}"));
    (0..h.len() / 8).map(|n| u32::from_le_bytes(std::array::from_fn(|b| u8::from_str_radix(&h[8 * n + 2 * b..8 * n + 2 * b + 2], 16).unwrap()))).collect()
}

/// The body's words the row holds that the port's body has otherwise: (offset, the row's, the port's).
fn body_off(r: &Row, body: &game_sim::atv_body::Body) -> Vec<(u32, u32, u32)> {
    let want = hex_words(r, "body");
    seed::body_words(body, 0).into_iter().filter(|&(o, w)| ((o / 4) as usize) < want.len() && want[(o / 4) as usize] != w).map(|(o, w)| (o, want[(o / 4) as usize], w)).collect()
}

/// Whether a row is a bad read: a copy of the row before or after it (a late or early read), or read
/// mid-tick (the body's force is cleared at the tick's end; MD147's torn rows).
fn bad_read(rows: &[Row], n: usize) -> bool {
    // The body's own words (to `+0x224`): the record's words after them tick on regardless (steer's
    // row 197 is row 196's body with `+0x260` one higher).
    let body = |k: usize| rows.get(k).map(|r| hex_words(r, "body")[..0x224 / 4].to_vec());
    let here = body(n).unwrap();
    let copy = (n > 0 && body(n - 1).as_ref() == Some(&here)) || body(n + 1).as_ref() == Some(&here);
    copy || here[0xa0 / 4..0xac / 4].iter().any(|&w| w != 0)
}

/// What a replay of `state` on `csv`'s pads through row `last` came to.
struct Replayed {
    /// The well-read rows whose body words the port's differs from: the row, and what differs.
    off: Vec<String>,
    /// The rows skipped as bad reads.
    skipped: Vec<usize>,
    /// The port's hazard clock after each tick, and the row's.
    clock: Vec<(F, F)>,
    /// MD168a: the rows read before the tick's put-back: off the port's after the tick, but the
    /// port's with the put-back held off (the tick stepped again from before it with the clock
    /// zeroed, so that it does not pass its limit).
    before_put_back: Vec<usize>,
}

fn replay(kit: &Kit, state: &str, csv: &str, last: usize) -> Option<Replayed> {
    replay_among(kit, state, csv, last, None)
}

/// [`replay`] with `among` (MD247): the quad stepped among Rocksbury's knockable props
/// ([`quad::Ride::step_among`]), its hull the one their passes strike.
fn replay_among(kit: &Kit, state: &str, csv: &str, last: usize, mut among: Option<&mut Among>) -> Option<Replayed> {
    let rows = rows(kit, csv)?;
    let mut s = setup(kit, state)?;
    let dt = F(f32::from_bits(DT));
    let mut out = Replayed { off: Vec::new(), skipped: Vec::new(), clock: Vec::new(), before_put_back: Vec::new() };
    let mut step = |ride: &mut quad::Ride, world: &mut Rocksbury, segments, r: &Row, among: Option<&mut Among>| match among {
        Some(a) => ride.step_among(&controls(r), dt, world, segments, &mut a.knocked, Some(&mut a.hull)),
        None => ride.step(&controls(r), dt, world, segments),
    };
    for (n, r) in rows.iter().enumerate().take(last + 1) {
        let before = (s.ride.clone(), among.as_deref().cloned());
        step(&mut s.ride, &mut s.world, s.segments, r, among.as_deref_mut());
        out.clock.push((s.ride.tailed.as_ref().unwrap().vehicle.stuck_time, F(r.f("stuck_time"))));
        if bad_read(&rows, n) {
            out.skipped.push(n);
            continue;
        }
        let off = body_off(r, &s.ride.body);
        if !off.is_empty() {
            let (mut held, mut again) = before;
            held.tailed.as_mut().unwrap().vehicle.stuck_time = F::ZERO;
            // The props as they stood before the tick, stepped again beside it.
            step(&mut held, &mut s.world, s.segments, r, again.as_mut());
            if body_off(r, &held.body).is_empty() {
                out.before_put_back.push(n);
                continue;
            }
        }
        if !off.is_empty() {
            let show: Vec<String> = off.iter().take(4).map(|(o, w, g)| format!("+{o:#x} {:e} (port {:e})", f32::from_bits(*w), f32::from_bits(*g))).collect();
            out.off.push(format!("row {n}: {} words off: {}", off.len(), show.join(", ")));
        }
    }
    Some(out)
}

/// The pack's race quad, made from its files at the spawn, against the image's quad: every key the
/// files give is the image's, bit for bit. The wheels' mounts are the image's to within a few bits
/// (the original stands them through its node transforms); the engine's tables are compared as far as
/// their counts.
#[test]
fn the_untamed_quad_from_its_files_is_md147s_spawned_quad() {
    let Some(kit) = kit() else { return };
    let Some(fs) = files() else { return };
    let Some(ee) = image(&kit, "spawn") else { return };
    let at = at(&ee);
    let made = made(&fs, &ee, &at);
    let img = seeded(&ee, &at, made.clone());
    let (mut rep, mut mounts) = (Vec::new(), Vec::new());
    let mut same = |what: &str, a: String, b: String| {
        if a != b {
            rep.push(format!("{what}:\n files {a}\n image {b}"));
        }
    };
    same("params", format!("{:?}", made.p), format!("{:?}", img.p));
    same("joystick", format!("{:?}", made.bike.joystick), format!("{:?}", img.bike.joystick));
    same("clutch keys", format!("{:?}", made.bike.clutch_keys), format!("{:?}", img.bike.clutch_keys));
    same("air keys", format!("{:?}", made.bike.air_keys), format!("{:?}", img.bike.air_keys));
    let eng = |e: &mv::Engine| {
        let (g, c) = (e.gears as usize, e.curve.points as usize);
        format!("{:?} {:?}", (e.gears, e.final_drive, e.rev_limit, e.idle_throttle, e.engine_braking, e.inv_inertia, e.clutch_static, e.clutch_kinetic, e.min_rpm), (&e.gear_ratios[..g], &e.curve_rpm[..c], &e.curve_torque[..c]))
    };
    same("engine", eng(&made.bike.engine), eng(&img.bike.engine));
    for k in 0..4 {
        let w = |r: &quad::Ride| {
            let w = &r.bike.wheels[k];
            format!("{:?} {:?}", (w.spring_rate, w.max_compression, w.max_damper_speed, w.extension_rate, w.damping_out, w.damping_in), (w.probe_length, w.steer_torque, w.steer_damping, w.steer_inv_inertia, w.min_steer_angle, w.max_steer_angle))
        };
        same(&format!("wheel {k}"), w(&made), w(&img));
        let sp = |r: &quad::Ride| {
            let s = &r.bike.spins[k];
            format!("{:?}", (s.brake, s.parametric, s.inv_inertia, s.tire_inertia, s.arm, s.no_moment, s.other_load_scale, s.inv_radius, s.surfaces))
        };
        same(&format!("spin {k}"), sp(&made), sp(&img));
        let (a, b) = (made.bike.wheels[k].mount, img.bike.wheels[k].mount);
        if (0..3).any(|j| (a.0[j] - b.0[j]).abs() > 2e-5) {
            mounts.push(format!("wheel {k}'s mount: files {a:?}, image {b:?}"));
        }
    }
    let body = |r: &quad::Ride| {
        let b = &r.body;
        format!("{:?}", (b.inv_mass, b.damping, b.inv_inertia, b.ang_damping, b.max_speed, b.max_accel, b.max_rates, b.max_ang_accel, b.linear_axes, b.angular_axes))
    };
    same("body keys", body(&made), body(&img));
    let t = |r: &quad::Ride| format!("{:?}", r.tailed.as_ref().map(|t| (t.tail.stunt_speed, t.tail.surfaces)));
    same("tail", t(&made), t(&img));
    let links = |r: &quad::Ride| format!("{:?}", r.bike.parts.scene.nodes[..at.nodes.len()].iter().map(|n| (n.parent, n.child, n.sibling)).collect::<Vec<_>>());
    same("scene", links(&made), links(&img));
    rep.extend(mounts);
    assert!(rep.is_empty(), "the pack's quad is not the image's:\n{}", rep.join("\n"));
}

/// steer.csv from the spawn: gas, full right, full left, half right, straight, then a jump. The
/// port's body is the row's, word for word, on every well-read row to the take-off (row 196). On the
/// take-off the original's quad is knocked sideways (its `x` velocity −4 → +2.4 in a tick) by what
/// the bare world here has not: Rocksbury's solids (MD168's journal). MD168b: it is a knockable bale
/// (`SX_A.scn`'s `TripleBale02` at (437.99, 9.28, 474.69), a 253 lb body with a collision record of its
/// own), its two-way contact with the quad not yet ported; the bale's `f.col` stood still as a solid
/// spins the port's quad instead.
///
/// MD247: on Rocksbury with its bales ([`bales`]), each with its own frame, still to row 196: none
/// wakes before the take-off. On tick 197 the quad's rays strike the top of a triple bale ([`BALE`]),
/// which wakes and files its own hits ([`untamed_bale_wakes_and_files_its_hits_on_md224_steer`]); but
/// the original's left front wheel's sweep also lands on the bale that tick (the wheel's swept-contact
/// word set on all four substeps), which the port's sweep does not see, and the quad's body is off
/// from row 197 (MD247's journal).
#[test]
fn untamed_quad_replays_md147_steer() {
    let Some(kit) = kit() else { return };
    let Some(ee) = image(&kit, "spawn") else { return };
    let Some(mut among) = bales(&kit, &ee) else { return };
    let Some(r) = replay_among(&kit, "spawn", "steer", 196, Some(&mut among)) else { return };
    assert!(r.off.is_empty(), "{} rows off:\n{}", r.off.len(), r.off.join("\n"));
    assert!(r.skipped.len() < 4, "rows {:?} were read badly", r.skipped);
    assert!(among.knocked.iter().all(|k| !k.body.is_some_and(|b| b.body.enabled)), "a bale woke before the take-off");
}

/// MD247: tick 197 of MD224's steer with the bales, the two lists as the original files them (read
/// off the original stepping the image; research/oracle.txt `# MD247`). The quad's: its two hits on
/// the bale, then the bale's nine mirrors. The bale's (it alone woke): the quad's two mirrors, its
/// nine heightfield rays' hits on the terrain (rays 4, 5, 6, 7, 15, 16, 20, 21, 23 of its 26), then its
/// nine hits on the quad's hull face by face, each face's in ray order (rays 0, 2, 8, 11, 24, 25 on
/// one, 4, 6, 23 on the other). The entries' rays, normals and points were the original's bit for bit.
#[test]
fn untamed_bale_wakes_and_files_its_hits_on_md224_steer() {
    let Some(kit) = kit() else { return };
    let Some(rows) = rows_in(&kit, "md224", "steer") else { return };
    let Some(mut s) = setup(&kit, "spawn") else { return };
    let Some(mut among) = bales(&kit, &s.world.ee) else { return };
    let dt = F(f32::from_bits(DT));
    for r in rows.iter().take(198) {
        s.ride.step_among(&controls(r), dt, &mut s.world, s.segments, &mut among.knocked, Some(&mut among.hull));
    }
    assert!(among.knocked.iter().filter(|k| k.body.is_some_and(|b| b.body.enabled)).map(|k| k.id).eq([BALE]), "only the struck bale woke");
    let quad: Vec<(u32, u32)> = s.ride.recorded.as_ref().expect("the quad's record").record.contacts.iter().map(|c| (c.kind, c.other)).collect();
    let mut want = vec![(0, BALE); 2];
    want.extend([(1, BALE); 9]);
    assert_eq!(quad, want, "the quad's list (kind, other)");
    let bale = among.knocked.iter().find(|k| k.id == BALE).expect("the bale");
    let rays = bale.own.as_ref().expect("the bale's frame").rays.pairs();
    let ray = |c: &handling::record::Contact| rays.iter().position(|&[a, b]| a.to_bits() == c.from.to_bits() && b.to_bits() == c.to.to_bits());
    let got: Vec<(u32, u32, Option<usize>)> = bale.record.contacts.iter().map(|c| (c.kind, c.other, ray(c))).collect();
    let mut want = vec![(1, 1, None); 2];
    want.extend([4, 5, 6, 7, 15, 16, 20, 21, 23].map(|k| (0, 0, Some(k))));
    want.extend([0, 2, 8, 11, 24, 25, 4, 6, 23].map(|k| (0, 1, Some(k))));
    assert_eq!(got, want, "the bale's list (kind, other, ray)");
}

/// The settled rows' hazard clock is the port's: the row's stuck time is the port's after the tick,
/// or (a late read of it; MD147) the port's a tick before.
fn clock_matches(clock: &[(F, F)], from: usize) {
    for n in from..clock.len() {
        let (port, row) = clock[n];
        // A put-back tick's row may hold the count that passed 1, read before the put-back zeroes it
        // (hazard-on's row 92).
        let passed = port == F::ZERO && row.to_bits() == (clock[n - 1].0 + F(f32::from_bits(DT))).to_bits();
        assert!(row.to_bits() == port.to_bits() || row.to_bits() == clock[n - 1].0.to_bits() || passed, "row {n}: the stuck time is {row:?}, the port's {port:?} (and {:?} a tick before)", clock[n - 1].0);
    }
}

/// hazard-time.csv: the spawn with every surface (hazard 0, time 1), jump.pad. In the air Untamed's
/// clock counts ([`quad::Tail::untamed`]; SLUS-21104's would take the free reset) and passes 1 on
/// the 31st tick in the air, row 180, where it puts the quad back (MD168a: Untamed's put-back,
/// [`quad::Ride::tail_on_untamed`]) to where it was last free, the tick it left the ground (row 150),
/// stood on the ground. The port's body is the row's on every well-read row through the put-back and
/// the landing after it, and on along the ground to row 254, its clock the row's from row 140. Row 180
/// itself was read before the tick's tail: it is the port's tick with the put-back held off. MD198: no
/// map-edge throw on the way (the image's edge word is off in Free Ride); on row 255 the quad strikes
/// the stadium's side wall (`FP01A_A.col`, [`solids`]) and wrecks, where the port's bounce is not yet
/// the original's (MD198a).
#[test]
fn untamed_quad_replays_md147_hazard_time_through_the_put_back() {
    let Some(kit) = kit() else { return };
    let Some(r) = replay(&kit, "hazard-time", "hazard-time", 254) else { return };
    assert!(r.off.is_empty(), "{} rows off:\n{}", r.off.len(), r.off.join("\n"));
    assert!(r.skipped.len() < 4, "rows {:?} were read badly", r.skipped);
    assert_eq!(r.before_put_back, [180], "the rows read before a put-back");
    clock_matches(&r.clock, 140);
}

/// hazard-on.csv: the spawn with every surface (hazard 1, time 1), jump.pad. The clock counts on the
/// ground from the first tick and puts the quad back on the 31st (row 30), and every 31 ticks it
/// stays on the hazard after. With a wheel on a hazard the spot is never kept (MD168a), so each
/// put-back stands it at the spawn's. The port's body is the row's on every well-read row of the
/// capture, its clock the row's; the six put-back rows were read before their tick's tail.
#[test]
fn untamed_quad_replays_md147_hazard_on_through_the_put_backs() {
    let Some(kit) = kit() else { return };
    let Some(r) = replay(&kit, "hazard-on", "hazard-on", 369) else { return };
    assert!(r.off.is_empty(), "{} rows off:\n{}", r.off.len(), r.off.join("\n"));
    assert!(r.skipped.len() < 4, "rows {:?} were read badly", r.skipped);
    assert_eq!(r.before_put_back, [30, 61, 216, 278, 309, 340], "the rows read before a put-back");
    clock_matches(&r.clock, 1);
}


/// MD224: Rocksbury's `TripleBale02` at (437.99, 9.28, 474.69), the knockable bale steer.csv's
/// take-off strikes, as the spawn image holds it: its record ([`BALE`]) and its collidable mesh
/// (`TripleBalef.col`) at the rows its record's node (`+0xe8`) holds (`+0x60..+0xa0`, Untamed's node
/// layout). The scene's own frame for the model ([`game_sim::props::frame`]) is not those rows to the
/// bit: they come off the body's quaternion.
fn bale(kit: &Kit, ee: &Ee) -> Option<game_sim::quad::Knockable> {
    let bytes = std::fs::read(kit.ctx("untamed/sx_a/col/TripleBalef.col")).ok()?;
    let mesh = game_sim::track::Mesh::parse(&bytes).unwrap_or_else(|e| panic!("TripleBalef.col: {e}"));
    let node = ee.u32_at(BALE + 0xe8);
    let rows: [Vf; 4] = std::array::from_fn(|k| v(ee, node + 0x60 + 0x10 * k as u32));
    let record = game_sim::handling::record::read(&|a| ee.u32_at(a), BALE);
    Some(game_sim::quad::Knockable::new(BALE, mesh, rows, record).with_body(knocked(ee, BALE)))
}

/// MD247: Untamed's knockable prop's vtable (a bale's, single or triple).
const KNOCKABLE_VTABLE: u32 = 0x003e_f2b0;

/// MD247: Rocksbury's knockable props and the quad's hull their passes strike.
#[derive(Clone)]
struct Among {
    knocked: Vec<game_sim::quad::Knockable>,
    hull: game_sim::quad::Knockable,
}

/// MD247: every knockable prop the image holds (Rocksbury's bales: each object of
/// [`KNOCKABLE_VTABLE`]), each with its body ([`knocked`]) and its own frame
/// ([`game_sim::quad::KnockFrame`]): its rays its `m.col`'s (the struck bale's checked against the
/// part's rays in the image), its gravity (the object's `+0x118`, its `+0x40`) and its world's rate (`+0x110`, its `+4`),
/// its record's impulse mask. Its mesh is its collidable's `f.col` (the record's `+0x208`, the name in
/// `+0xc4`..), its frame its node's rows. The quad's hull is its chassis' frame mesh
/// ([`quad::hull_file`]).
fn bales(kit: &Kit, ee: &Ee) -> Option<Among> {
    let dir = kit.ctx("untamed/sx_a/col");
    let files: Vec<PathBuf> = std::fs::read_dir(&dir).ok()?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    let read = |name: &str| {
        let p = files.iter().find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case(name)));
        std::fs::read(p.unwrap_or_else(|| panic!("{name} is not in {}", dir.display()))).unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let mut meshes = std::collections::BTreeMap::new();
    let mut knocked = Vec::new();
    for obj in (0x0010_0000..ee.ram.len() as u32 - 0x200).step_by(16) {
        if ee.u32_at(obj + 4) != KNOCKABLE_VTABLE {
            continue;
        }
        let (body, rec, world) = (ee.u32_at(obj + 0xe8), ee.u32_at(obj + 0xf8), ee.u32_at(obj + 0x110));
        if rec == 0 || ee.u32_at(rec + 0xe4) != body {
            continue;
        }
        let c = ee.u32_at(rec + 0x208);
        // The name is the printable run after the collidable's `+0xc4` word's leading bytes.
        let name: String = (0..0x3c).map(|k| ee.u8_at(c + 0xc4 + k)).skip_while(|b| !b.is_ascii_graphic()).take_while(|&b| b != 0).map(char::from).collect();
        let shape = name.rsplit('\\').next().unwrap_or(&name).to_ascii_lowercase();
        // A stale copy of an object (its collidable gone) is no prop.
        let Some(stem) = shape.strip_suffix("f.col") else { continue };
        let rays_name = format!("{stem}m.col");
        let (mesh, rays) = meshes
            .entry(shape.clone())
            .or_insert_with(|| {
                let mesh = game_sim::track::Mesh::parse(&read(&shape)).unwrap_or_else(|e| panic!("{shape}: {e}"));
                let rays = game_sim::track::Rays::parse(&read(&rays_name)).unwrap_or_else(|e| panic!("{rays_name}: {e}"));
                (mesh, game_sim::flight_search::Rays::of(&rays))
            })
            .clone();
        if rec == BALE {
            // The part's rays (`+0x198` their count, `+0x1a0` them) are the file's.
            let part = ee.u32_at(rec + 0x200);
            let list = ee.u32_at(part + 0x1a0);
            let image: Vec<[Vf; 2]> = (0..ee.u32_at(part + 0x198)).map(|k| [v(ee, list + 0x20 * k), v(ee, list + 0x20 * k + 0x10)]).collect();
            let bits = |r: &[[Vf; 2]]| r.iter().map(|[a, b]| (a.to_bits(), b.to_bits())).collect::<Vec<_>>();
            assert_eq!(bits(&rays.pairs()), bits(&image), "the bale's rays from {rays_name}");
        }
        let node = ee.u32_at(rec + 0xe8);
        let frame: [Vf; 4] = std::array::from_fn(|k| v(ee, node + 0x60 + 0x10 * k as u32));
        let record = handling::record::read(&|a| ee.u32_at(a), rec);
        let k = self::knocked(ee, rec);
        let held = game_sim::atv_contact::Held {
            record: game_sim::atv_contact::Record { collider: k.collider, impulse_mask: v(ee, rec + 0x150) },
            mass: k.mass,
            ..Default::default()
        };
        let world = game_sim::biplane::World { iterations: 1, rate: f(ee, world + 4) };
        let own = game_sim::quad::KnockFrame { rays, gravity: v(ee, ee.u32_at(obj + 0x118) + 0x40), world, held };
        knocked.push(game_sim::quad::Knockable::new(rec, mesh, frame, record).with_body(k).with_frame(own));
    }
    assert!(knocked.iter().any(|k| k.id == BALE), "the struck bale is not among the image's knockables");
    let hull = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../mods/machines/untamed_atv/Veh/Col").join(quad::hull_file("ATV_R_ChasisM.col")?)).ok()?;
    let hull = game_sim::track::Mesh::parse(&hull).unwrap_or_else(|e| panic!("ATV_R_ChasisF.col: {e}"));
    let hull = game_sim::quad::Knockable::new(1, hull, [Vf::ZERO; 4], Default::default());
    Some(Among { knocked, hull })
}

/// MD246: Untamed's quad record's resolution, its vt `+0x88` (SLUS-21104's, the entry's fields moved:
/// [`ENTRY_OTHER`]; research/oracle.txt `# MD246`).
const RESOLVE: u32 = 0x0020_6170;
/// Untamed's record list entry (`0x90` bytes): the kind a half-word at `+0`, the surface a signed
/// half-word at `+2`, the other record at `+0x80`, the mover at `+0x84`. The ray, normal, point, arms
/// and relative velocity sit where SLUS-21104's do.
const ENTRY_OTHER: u32 = 0x80;
const ENTRY_MOVER: u32 = 0x84;
/// The collidable's flags (the record's `+0x208`), SLUS-21104's `+0x94`.
const COLLIDABLE_FLAGS: u32 = 0xc0;

/// The record at `rec` as [`game_sim::handling::record::held`] reads SLUS-21104's, at Untamed's
/// collidable flags, its list left out.
fn untamed_held(ee: &Ee, rec: u32) -> game_sim::atv_contact::Held {
    let c = ee.u32_at(rec + 0x208);
    let w = |a: u32| match a {
        _ if a == rec + 0xf0 => 0,
        _ if c != 0 && a == c + 0x94 => ee.u32_at(c + COLLIDABLE_FLAGS),
        _ => ee.u32_at(a),
    };
    game_sim::handling::record::held(&w, rec).expect("a list left out")
}

/// A knockable body as the quad's resolution pushes it: the record at `rec` and its body (`+0xe4`),
/// the body's mass and its centre of mass (`+0x1a0`).
fn knocked(ee: &Ee, rec: u32) -> game_sim::atv_contact::Knocked {
    let h = untamed_held(ee, rec);
    let body = ee.u32_at(rec + 0xe4);
    game_sim::atv_contact::Knocked { collider: h.record.collider, body: h.body.expect("a body"), mass: h.mass, cofg: v(ee, body + 0x1a0) }
}

/// The quad's record at `rec` as the gravity's call holds it: [`untamed_held`] with its list read at
/// Untamed's layout, each entry's other one of `others` (as [`game_sim::atv_contact::Held::bodies`]),
/// and those records' bodies.
fn untamed_quad_held(ee: &Ee, rec: u32, others: &[u32]) -> game_sim::atv_contact::Held {
    let mut h = untamed_held(ee, rec);
    let list = ee.u32_at(rec + 0xf8);
    h.entries = (0..ee.u32_at(rec + 0xf0))
        .map(|k| {
            let e = list + 0x90 * k;
            assert_eq!(ee.u32_at(e + ENTRY_MOVER), 0, "a contact with a mover");
            let other = ee.u32_at(e + ENTRY_OTHER);
            let contact = game_sim::contact::Contact {
                kind: u32::from(ee.u16_at(e)),
                from: v(ee, e + 0x10),
                on_body: v(ee, e + 0x20),
                normal: v(ee, e + 0x30),
                point: v(ee, e + 0x40),
                arm: v(ee, e + 0x50),
                other_arm: v(ee, e + 0x60),
                surface: i32::from(ee.u16_at(e + 2) as i16),
                other: (other != 0).then(|| others.iter().position(|&o| o == other).expect("an other record held")),
                mover: None,
            };
            game_sim::atv_contact::Entry { contact, relative: v(ee, e + 0x70) }
        })
        .collect();
    h.bodies = others.iter().map(|&o| knocked(ee, o)).collect();
    h
}

/// `h`'s list written at Untamed's layout over the record at `rec`'s, naming `others`' records.
fn put_entries(ee: &mut Ee, rec: u32, h: &game_sim::atv_contact::Held, others: &[u32]) {
    let list = ee.u32_at(rec + 0xf8);
    ee.set_u32(rec + 0xf0, h.entries.len() as u32);
    for (k, en) in h.entries.iter().enumerate() {
        let (e, c) = (list + 0x90 * k as u32, &en.contact);
        ee.set_u32(e, (c.kind & 0xffff) | ((c.surface as u32) << 16));
        for (at, q) in [(0x10, c.from), (0x20, c.on_body), (0x30, c.normal), (0x40, c.point), (0x50, c.arm), (0x60, c.other_arm), (0x70, en.relative)] {
            put_v(ee, e + at, q);
        }
        ee.set_u32(e + ENTRY_OTHER, c.other.map_or(0, |k| others[k]));
        ee.set_u32(e + ENTRY_MOVER, 0);
    }
}

/// A deterministic generator for the cases (tests/atv_contact.rs's).
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * ((self.next() >> 40) as f32 / (1u64 << 24) as f32)
    }
    fn one_in(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
    /// `x` with each lane moved by up to `by`, `w` kept.
    fn near(&mut self, x: Vf, by: f32) -> Vf {
        Vf::new(x.0[0] + self.range(-by, by), x.0[1] + self.range(-by, by), x.0[2] + self.range(-by, by), x.0[3])
    }
    /// A unit direction near `n`, `w` kept.
    fn aim(&mut self, n: Vf, by: f32) -> Vf {
        let r = self.near(n, by);
        let l = (0..3).map(|k| r.0[k] * r.0[k]).sum::<f32>().sqrt();
        Vf::new(r.0[0] / l, r.0[1] / l, r.0[2] / l, n.0[3])
    }
    /// A unit quaternion near `q`.
    fn turn(&mut self, q: Vf, by: f32) -> Vf {
        let r = Vf::new(q.0[0] + self.range(-by, by), q.0[1] + self.range(-by, by), q.0[2] + self.range(-by, by), q.0[3] + self.range(-by, by));
        let n = (0..4).map(|k| r.0[k] * r.0[k]).sum::<f32>().sqrt();
        Vf::new(r.0[0] / n, r.0[1] / n, r.0[2] / n, r.0[3] / n)
    }
}

/// MD246: the quad's resolution of its hits on the bale ([`game_sim::atv_contact::Held::resolve`]
/// among [`game_sim::atv_contact::Held::bodies`]) against Untamed's own ([`RESOLVE`]) in the
/// interpreter. Case 0 is steer's tick 197 as its gravity's first call meets it: the port's pass's two
/// hits on the still bale (MD224's, the original's row 197 bit for bit), the quad's body after tick 196
/// (row 196's) and the bale's record and body from the spawn image. The other cases move both bodies
/// (place, speed, spin, turn), the contacts' normals and points, their kinds and how many: the
/// impulse and the spring both run. Every word the port writes (both bodies, each entry's arms and
/// relative velocity, the records' centres in the world, the quad record's normal speed and relative
/// velocity) is the original's, and the original writes nothing else.
#[test]
fn untamed_quad_resolves_its_hits_on_the_bale_as_the_original() {
    use game_sim::atv_contact::Held;
    let Some(kit) = kit() else { return };
    let Some(rows) = rows_in(&kit, "md224", "steer") else { return };
    let Some(mut s) = setup(&kit, "spawn") else { return };
    let Some(bale) = bale(&kit, &s.world.ee) else { return };
    let Some(mut ee) = image(&kit, "spawn") else { return };
    let mut knocked = [bale];
    let dt = F(f32::from_bits(DT));
    for r in rows.iter().take(197) {
        s.ride.step_among(&controls(r), dt, &mut s.world, s.segments, &mut knocked, None);
    }
    assert_eq!(knocked[0].body, Some(self::knocked(&ee, BALE)), "the bale's body moved before the quad struck it");
    // Tick 197's pass: two hits on the bale, held with the bale's body.
    let before = (s.ride.clone(), knocked.clone());
    let Some(ee0) = image(&kit, "spawn") else { return };
    s.ride.record_among(dt, &mut s.world, &mut knocked, None);
    let held = s.ride.recorded.as_ref().expect("the quad's record").held.clone();
    assert_eq!(held.entries.len(), 2, "the hits held on tick 197");
    assert!(held.entries.iter().all(|e| e.contact.other == Some(0)) && held.bodies.len() == 1, "each hit's other side is the bale's body");

    let rec = ee.u32_at(QUAD + 0xf8);
    let (quad_body, bale_body) = (ee.u32_at(rec + 0xe4), ee.u32_at(BALE + 0xe4));
    assert_eq!(ee.u32_at(ee.u32_at(rec + 4) + 0x88), RESOLVE, "the quad record's vt +0x88");
    // The quad's record from its files is the image's, but for what the resolution writes first and
    // the inverse inertia, some 36 ulps off: read at the scene's start spot, the part reader's box is
    // still the image's but for 8 ulps in y and 34 in z, and no turn of the frame gives it (MD250). The
    // cases below read the image's.
    let fresh = |c: &game_sim::contact::Collider| game_sim::contact::Collider { cofg_world: Vf::ZERO, normal_speed: F::ZERO, relative: Vf::ZERO, inv_inertia: Vf::ZERO, ..*c };
    let image_record = untamed_held(&ee, rec).record;
    assert_eq!(fresh(&held.record.collider), fresh(&image_record.collider), "the quad's record is not the image's");
    assert_eq!(held.record.impulse_mask.to_bits(), image_record.impulse_mask.to_bits(), "the quad's impulse mask");

    let mut p = Pick(0x4d32_4624_6b61_6c65);
    let (mut impulses, mut springs) = (0, 0);
    for case in 0..10_000 {
        // The case written into the image: the quad's list, its body (and centre of mass), the bale's body.
        let mut quad = s.ride.body;
        let mut bale = held.bodies[0].body;
        let mut h = held.clone();
        let cofg = s.ride.bike.cofg;
        if case > 0 {
            quad.position = p.near(quad.position, 1.0);
            quad.velocity = p.near(quad.velocity, 30.0);
            quad.rates = p.near(quad.rates, 4.0);
            quad.quat = p.turn(quad.quat, 0.3);
            bale.position = p.near(bale.position, 1.0);
            bale.velocity = p.near(bale.velocity, 20.0);
            bale.rates = p.near(bale.rates, 3.0);
            bale.quat = p.turn(bale.quat, 0.3);
            for e in &mut h.entries {
                e.contact.normal = p.aim(e.contact.normal, 0.2);
                e.contact.point = p.near(e.contact.point, 0.5);
                e.contact.kind = u32::from(p.one_in(4));
            }
            if p.one_in(3) {
                h.entries.truncate(1);
            }
        }
        for (a, w) in seed::body_words(&quad, quad_body).into_iter().chain(seed::body_words(&bale, bale_body)) {
            ee.set_u32(a, w);
        }
        put_v(&mut ee, quad_body + 0x1a0, cofg);
        put_entries(&mut ee, rec, &h, &[BALE]);

        // The port on what the image now holds, then the original.
        let mut port = untamed_quad_held(&ee, rec, &[BALE]);
        assert!(port.resolve(v(&ee, quad_body + 0x1a0)), "case {case}: the port did not resolve");
        ee.written.clear();
        ee.call(RESOLVE, &[u64::from(rec), 1], &[], 400_000);

        let mut compared = std::collections::BTreeSet::new();
        let bale_held = Held { record: game_sim::atv_contact::Record { collider: port.bodies[0].collider, ..Default::default() }, body: Some(port.bodies[0].body), ..Default::default() };
        let words = |a: u32| ee.u32_at(a);
        let mine = game_sim::handling::record::held_words(&port, rec, &words).into_iter().chain(game_sim::handling::record::held_words(&bale_held, BALE, &words));
        for (a, w) in mine {
            assert_eq!(w, ee.u32_at(a), "case {case}: the port wrote {w:#010x} at {a:#x}, the original {:#010x}", ee.u32_at(a));
            compared.extend(a..a + 4);
        }
        let stray: Vec<u32> = ee.written.iter().copied().filter(|&a| !(ee.stack - 0x4000..ee.stack).contains(&a) && !compared.contains(&a)).collect();
        assert!(stray.is_empty(), "case {case}: the original wrote what the port does not: {:x?}", &stray[..stray.len().min(16)]);
        // The last contact's normal speed says which way it was resolved.
        if port.record.collider.normal_speed < F(-10.0) {
            impulses += 1;
        } else {
            springs += 1;
        }
        if case == 0 {
            // The quad leaves the bale's top at some 22 ft/s along the normal: past the spring's reach,
            // so neither body is pushed (the bale's shove is its own resolution's, MD247).
            assert!(port.record.collider.normal_speed > F::ZERO, "tick 197: the quad is not leaving the bale");
            assert_eq!(port.body.map(|b| (b.velocity, b.force)), Some((quad.velocity, quad.force)), "tick 197: the quad was pushed");
            assert_eq!(port.bodies[0].body, bale, "tick 197: the bale was pushed");
        }
    }
    assert!(impulses > 500 && springs > 500, "the cases ran {impulses} impulses and {springs} springs");

    // The whole tick: the gravity's call resolves the two hits every substep with the bale held, and
    // the bale's body comes back to it as it went (still), out of the quad's held list.
    let (mut ride, mut knocked) = before;
    let calls = ride.rest.len();
    ride.step_among(&controls(&rows[197]), dt, &mut s.world, s.segments, &mut knocked, None);
    assert!(!ride.rest[calls..].contains(&Call::GravityRecord), "the gravity's call was not answered on tick 197");
    assert_eq!(knocked[0].body, Some(self::knocked(&ee0, BALE)), "the bale's body after tick 197");
    assert!(ride.recorded.as_ref().is_some_and(|r| r.held.bodies.is_empty() && r.held.entries.len() == 2), "the held list after tick 197");
}

/// One entry of a captured record list (Untamed's layout, research/journal/*md224*): the kind (a
/// half-word, 0 the record's own, 1 handed to it), the ray (`+0x10`, `+0x20`), the normal and point,
/// and the other record (`+0x80`).
fn listed(r: &Row, col: &str, k: usize) -> (u32, [u32; 16], u32) {
    let w = hex_words(r, col);
    let e = &w[k * 0x24..];
    (e[0] & 0xffff, std::array::from_fn(|j| e[4 + j]), e[0x20])
}

/// MD224's steer.csv (research/md224: MD147's steer.pad from the spawn again, with the quad's
/// collision record and list and the bale's record, list and body): rows to 196 are MD147's word
/// for word. Row 197 is read between the tick's record pass and its step: the quad's record holds
/// two contacts of its own, its rays' hits on the top of the still bale (`TripleBalef.col`), each
/// naming the bale's record, and the bale's list opens with their mirrors. The port's pass on that
/// tick among the bale ([`game_sim::quad::Ride::step_among`]) files those two and the two mirrors:
/// the same rays, normals and points, bit for bit.
#[test]
fn untamed_quad_record_strikes_the_bale_on_md224_steer() {
    let Some(kit) = kit() else { return };
    let Some(rows) = rows_in(&kit, "md224", "steer") else { return };
    let Some(mut s) = setup(&kit, "spawn") else { return };
    let Some(bale) = bale(&kit, &s.world.ee) else { return };
    let mut knocked = [bale];
    let dt = F(f32::from_bits(DT));
    for r in rows.iter().take(198) {
        s.ride.step_among(&controls(r), dt, &mut s.world, s.segments, &mut knocked, None);
    }
    let r = &rows[197];
    let count = hex_words(r, "rec")[0xf0 / 4] as usize;
    assert_eq!(count, 2, "the capture's row 197 lists {count} contacts");
    let record = &s.ride.recorded.as_ref().expect("the quad's record").record;
    let words = |c: &game_sim::handling::record::Contact| {
        let q = [c.from, c.to, c.normal, c.point];
        (c.kind, std::array::from_fn::<u32, 16, _>(|j| q[j / 4].0[j % 4].to_bits()), c.other)
    };
    let got: Vec<(u32, [u32; 16], u32)> = record.contacts.iter().map(words).collect();
    let want: Vec<(u32, [u32; 16], u32)> = (0..count).map(|k| listed(r, "rec_list", k)).collect();
    assert_eq!(got, want, "the port's pass on tick 197 against the original's list");
    // The bale's list opens with the mirrors: the hits turned round, naming the quad's record.
    let quad_record = s.world.ee.u32_at(QUAD + 0xf8);
    let mirrors: Vec<(u32, [u32; 16], u32)> = knocked[0].record.contacts.iter().map(words).map(|(k, w, _)| (k, w, quad_record)).collect();
    let want: Vec<(u32, [u32; 16], u32)> = (0..count).map(|k| listed(r, "bale_list", k)).collect();
    assert_eq!(mirrors, want, "the bale's mirrors on tick 197");
    assert!(knocked[0].record.contacts.iter().all(|c| c.other == 1), "a mirror names the quad's record");
}

