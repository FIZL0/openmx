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
    fn solid(&mut self, _bundle: &game_sim::collide::Bundle, _record: &mut game_sim::collide::Record) -> bool {
        false
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
    let path = kit.ctx("capture/md147").join(format!("{name}{}", ".csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("{} is not there (MD147's)", path.display()));
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

/// The pack's quad made and rigged as the shell makes it, at the image's body and in its world's box.
fn made(fs: &Files, ee: &Ee, at: &At) -> quad::Ride {
    let spot = v(ee, at.body + 0x70);
    let bounds = (at.area != 0).then(|| (v(ee, at.area + 0x30), v(ee, at.area + 0x40)));
    let curve = game_sim::camera::CurveTable::build();
    let (spot, heading) = (Vf::new(spot.0[0], spot.0[1], spot.0[2], 1.0), Vf::new(0.0, 0.0, 1.0, 1.0));
    let load = quad::Load::read_model(&fs.ini, &fs.model, spot, heading, &curve).unwrap_or_else(|e| panic!("{e}"));
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
    Some(Setup { ride, world: Rocksbury { ee, terrain, record: 0, area }, segments })
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
    let body = |k: usize| rows.get(k).map(|r| hex_words(r, "body"));
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
    let rows = rows(kit, csv)?;
    let mut s = setup(kit, state)?;
    let dt = F(f32::from_bits(DT));
    let mut out = Replayed { off: Vec::new(), skipped: Vec::new(), clock: Vec::new(), before_put_back: Vec::new() };
    for (n, r) in rows.iter().enumerate().take(last + 1) {
        let before = s.ride.clone();
        s.ride.step(&controls(r), dt, &mut s.world, s.segments);
        out.clock.push((s.ride.tailed.as_ref().unwrap().vehicle.stuck_time, F(r.f("stuck_time"))));
        if bad_read(&rows, n) {
            out.skipped.push(n);
            continue;
        }
        let off = body_off(r, &s.ride.body);
        if !off.is_empty() {
            // Held off as well: the map edge, which the port's world box puts nearer than Rocksbury's
            // (hazard-time's row 180 and 238 throw it, reason 13; the original rides on).
            let mut held = before;
            held.tailed.as_mut().unwrap().vehicle.stuck_time = F::ZERO;
            if let Some(w) = held.wrecked.as_mut() {
                w.edge = false;
            }
            held.step(&controls(r), dt, &mut s.world, s.segments);
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
/// the bare world here has not: Rocksbury's solids (MD168's journal).
#[test]
fn untamed_quad_replays_md147_steer() {
    let Some(kit) = kit() else { return };
    let Some(r) = replay(&kit, "spawn", "steer", 196) else { return };
    assert!(r.off.is_empty(), "{} rows off:\n{}", r.off.len(), r.off.join("\n"));
    assert!(r.skipped.len() < 4, "rows {:?} were read badly", r.skipped);
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
/// the landing after it to row 190, its clock the row's from row 140. Row 180 itself was read before
/// the tick's tail: it is the port's tick with the put-back held off.
#[test]
fn untamed_quad_replays_md147_hazard_time_through_the_put_back() {
    let Some(kit) = kit() else { return };
    let Some(r) = replay(&kit, "hazard-time", "hazard-time", 190) else { return };
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

