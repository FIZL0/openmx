//! MD75d — a [`Ride`] read out of MD37a's `atv-r-spawn.ee.bin`: the quad at its spawn, at its own
//! addresses, with every word [`poke_ride`] lays read back from where the image keeps it, so that the
//! whole-frame proof (MD75e) can start the port from the game's own state.
//!
//! [`read_ride`] is [`poke_ride`]'s inverse: the image's quad (`IMAGE_QUAD`) and what its pointers
//! reach (the body, the world, the vehicle state, the collision record, the four wheels, the engine and
//! its tables, the gate, the scene node and its neighbours, the world's box) are read word for word,
//! and [`Image`] says where each was. What the [`Ride`] cannot hold is not dropped silently: it is
//! [`Image::gaps`], each word with the bits the ride does carry and why the rest are not.
//!
//! The check ([`the_spawned_quad_reads_back_word_for_word`]) pokes the read ride into a fresh image at
//! the harness's own layout and compares every word the poke wrote with the image's at the same place:
//! equal, or a pointer to the same place in the other layout, or inside a declared gap's mask.

use super::*;

/// The player's quad in MD37a's atv_ (Race) states (research/oracle.txt # MD37a).
pub const IMAGE_QUAD: u32 = 0x015e_0690;

/// Where the image keeps what [`read_ride`] read.
#[derive(Clone, Debug, Default)]
pub struct Image {
    pub quad: u32,
    pub body: u32,
    pub world: u32,
    pub state: u32,
    pub record: u32,
    pub record_vt: u32,
    pub wheels: [u32; 4],
    /// What each wheel's ground pointer (`+0x1e0`) points at; the harness lays [`WORLD`] there.
    pub ground: [u32; 4],
    pub engine: u32,
    pub ratios: u32,
    pub curve_torque: u32,
    pub curve_rpm: u32,
    pub gate: u32,
    pub thing: u32,
    pub actors: u32,
    pub search: u32,
    /// The scene nodes the ride holds, node 0 the quad's, and their vtables and database entries.
    pub nodes: Vec<u32>,
    pub node_vts: Vec<u32>,
    pub entries: Vec<u32>,
    pub area: u32,
    /// The five collidables, in [`COLLIDABLES`]' order (0 for one the quad has none of).
    pub parts: [u32; 5],
    /// MD75c2: each collidable's tree, bound or mesh, as `(harness, image, length)`, and its database
    /// (`+0x20`).
    pub part_blocks: Vec<(u32, u32, u32)>,
    pub part_dbs: Vec<u32>,
    /// The bound the harness lays for a collidable of neither kind (it reads as a default probe).
    pub part_own: Vec<u32>,
    /// MD75c3: each node's list of nodes its gather takes in, as `(harness, image, length)`, when every
    /// one is in the scene.
    pub node_lists: Vec<(u32, u32, u32)>,
    /// Words the ride does not carry whole: the image's address, the bits it carries, and why not the
    /// rest.
    pub gaps: Vec<(u32, u32, String)>,
}

/// A float word as it sits in RAM, bit for bit (not as the FPU would read it: a denormal stays one).
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

/// MD75c2: a collidable as [`lay_collidable`] lays it, read back: a ray probe ([`PART_VTABLE`]) or a mesh
/// ([`MESH_VTABLE`]); none for any other class (the sweep), whose words are declared gaps.
pub(super) fn collidable(ee: &Ee, a: u32, gaps: &mut Vec<(u32, u32, String)>) -> game_sim::atv_sync::Collidable {
    use game_sim::atv_sync::{Collidable, Kind};
    let xyz = |a: u32| Vf::new(f(ee, a).0, f(ee, a + 4).0, f(ee, a + 8).0, 0.0);
    let rows = |a: u32| std::array::from_fn(|k| v(ee, a + 0x10 * k as u32));
    let kind = match ee.u32_at(a + 0xc) {
        PART_VTABLE => {
            let (tree, bound) = (ee.u32_at(a + 0x17c), ee.u32_at(a + 0x180));
            Kind::Rays { tree: (tree != 0).then(|| (xyz(tree + 0xc), xyz(tree + 0x18))), bound: (v(ee, bound), v(ee, bound + 0x10)) }
        }
        MESH_VTABLE => {
            let mesh = ee.u32_at(a + 0x244);
            Kind::Mesh { local: rows(a + 0x170), centre: xyz(mesh + 0xc), half: xyz(mesh + 0x18), stood: rows(a + 0x1b0), before: rows(a + 0x1f0) }
        }
        vt => {
            for o in [0xc, 0x20, 0x50, 0x54, 0x58, 0x5c, 0x70, 0x174, 0x17c, 0x180].into_iter().chain((0x130..0x170).step_by(4)) {
                gaps.push((a + o, 0, format!("a collidable of vtable {vt:#010x}: the sync reads neither kind's words of it")));
            }
            return Collidable::default();
        }
    };
    Collidable { kind, frame: rows(a + 0x130), centre: v(ee, a + 0x50), reach: f(ee, a + 0x70), spatial: b(ee, a + 0x20) }
}

/// The scene round the quad's node, breadth first over each node's parent, child and sibling, as
/// many as [`SCENE_NODES`]: the addresses kept.
fn scene_nodes(ee: &Ee, first: u32) -> Vec<u32> {
    let mut out = vec![first];
    let mut k = 0;
    while k < out.len() && out.len() < SCENE_NODES {
        for o in [0xf0, 0xf4, 0xf8] {
            let n = ee.u32_at(out[k] + o);
            if n != 0 && !out.contains(&n) && out.len() < SCENE_NODES {
                out.push(n);
            }
        }
        k += 1;
    }
    out
}

/// **The quad at `quad` in `ee`, as a [`Ride`]**, and where each of its parts is. `dt` is not a word of
/// the quad's: the step's is the caller's.
pub fn read_ride(ee: &Ee, quad: u32, dt: F) -> Result<(Ride, Image), String> {
    use game_sim::{atv_moto, atv_wheel, handling, r#move as mv};
    let o = quad;
    let need = |a: u32, what: &str| match ee.u32_at(a) {
        0 => Err(format!("the quad at {o:#010x} has no {what} (+{:#x})", a - o)),
        p => Ok(p),
    };
    let mut at = Image {
        quad: o,
        body: need(o + 0xb8, "rigid body")?,
        world: need(o + 0xe8, "physics world")?,
        state: ee.u32_at(o + 0xbc4),
        record: need(o + 0xc8, "collision record")?,
        engine: need(o + 0x142c, "engine")?,
        actors: ee.u32_at(o + 0xc4),
        search: ee.u32_at(o + 0x1430),
        gate: ee.u32_at(o + 0x680),
        ..Image::default()
    };
    for k in 0..4 {
        at.wheels[k] = need(o + 0x17b0 + 4 * k as u32, "wheel")?;
        at.ground[k] = ee.u32_at(at.wheels[k] + 0x1e0);
    }
    let (bo, wo, ro, eo) = (at.body, at.world, at.record, at.engine);
    at.record_vt = ee.u32_at(ro + 4);
    at.thing = ee.u32_at(ro + 0x20c);
    (at.ratios, at.curve_torque, at.curve_rpm) = (ee.u32_at(eo + 0x70), ee.u32_at(eo + 0x68), ee.u32_at(eo + 0x8c));
    at.area = ee.u32_at(wo + 0xa8);
    at.nodes = scene_nodes(ee, need(o + 0xe4, "scene node")?);
    at.node_vts = at.nodes.iter().map(|&n| ee.u32_at(n + 4)).collect();
    at.entries = std::iter::once(o).chain(at.nodes.iter().copied()).map(|n| ee.u32_at(n + 8)).filter(|&e| e != 0).collect();
    // The collidables: the riding rays as the quad keeps them (+0x1438), the frame mesh, the sweep,
    // the trigger probe and the wreck rays.
    for (k, off) in [0x1438, 0xd0, 0xd4, 0x1430, 0x1434].into_iter().enumerate() {
        at.parts[k] = ee.u32_at(o + off);
    }
    let held_parts = at.parts;
    let states = std::array::from_fn(|k| if at.parts[k] == 0 { Default::default() } else { collidable(ee, at.parts[k], &mut at.gaps) });
    for (&c, &p) in COLLIDABLES.iter().zip(&held_parts).filter(|(_, p)| **p != 0) {
        let to = collidable_at(c);
        let laid: &[(u32, u32, u32)] = match ee.u32_at(p + 0xc) {
            PART_VTABLE => &[(0x300, 0x17c, 0x24), (0x340, 0x180, 0x20)],
            MESH_VTABLE => &[(0x380, 0x244, 0x24)],
            _ => {
                at.part_own.push(to + 0x340);
                &[]
            }
        };
        for &(o, ptr, len) in laid {
            at.part_blocks.push((to + o, ee.u32_at(p + ptr), len));
        }
        at.part_dbs.push(ee.u32_at(p + 0x20));
    }
    let part = |a: u32, what: &str| -> Result<Option<quad::Part>, String> {
        match a {
            0 => Ok(None),
            _ => held_parts.iter().position(|&p| p == a).map(|k| Some(COLLIDABLES[k])).ok_or(format!("the quad's {what} {a:#010x} is none of its five collidables")),
        }
    };

    let p = Params {
        max_pitch_command: f(ee, o + 0x139c),
        pitch_gain: f(ee, o + 0x13a0),
        pitch_rate_gain: f(ee, o + 0x13a4),
        inv_wheelie_top_speed: f(ee, o + 0x13a8),
        max_stoppie_pitch_command: f(ee, o + 0x13cc),
        stoppie_pitch_gain: f(ee, o + 0x13d0),
        stoppie_yaw_rate_scale: f(ee, o + 0x13d4),
        stoppie_top_speed: f(ee, o + 0x13d8),
        steer_speed_1: f(ee, o + 0x1300),
        steer_scale_1: f(ee, o + 0x1304),
        steer_speed_2: f(ee, o + 0x1308),
        steer_scale_2: f(ee, o + 0x130c),
        full_lock: b(ee, o + 0x12f0),
        full_lock_angle: f(ee, o + 0x12f4),
        full_lock_time: f(ee, o + 0x12f8),
        max_donut_speed: f(ee, o + 0x122c),
        donut_angle: f(ee, o + 0x1230),
        donut_angle_back: f(ee, o + 0x1234),
        front_spring: (f(ee, o + 0x137c), f(ee, o + 0x1380)),
        rear_spring: (f(ee, o + 0x1384), f(ee, o + 0x1388)),
    };
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
    let bal = (ee.u32_at(o + 0x1524), ee.u32_at(o + 0x152c));
    let vstate = (at.state != 0).then(|| ee.u32_at(at.state + 0x28));
    let q = Quad {
        on_ground: b(ee, o + 0x324),
        all_down: b(ee, o + 0x1270),
        front_down: b(ee, o + 0x1274),
        rear_down: b(ee, o + 0x1278),
        balancing: bal.0 == bal.1,
        contact_vel: v(ee, o + 0xc30),
        brake: f(ee, o + 0x310),
        gas: f(ee, o + 0x314),
        gas_limited: f(ee, o + 0x318),
        euler: v(ee, o + 0x360),
        world_ang_vel: v(ee, o + 0x340),
        front_brake: f(ee, o + 0x12cc),
        rear_brake: f(ee, o + 0x12d0),
        clutch_in: f(ee, o + 0x12d4),
        lean: f(ee, o + 0x12e8),
        lean_fore_aft: f(ee, o + 0x12ec),
        pitch_integral: f(ee, o + 0x13ac),
        stoppie: b(ee, o + 0x13b0),
        push: b(ee, o + 0xdd8),
        push_target: f(ee, o + 0x123c),
        push_gain: f(ee, o + 0x1240),
        push_gate: f(ee, o + 0x12c4),
        gravity: v(ee, wo + 0x40),
        wheels,
        active: matches!(vstate, Some(s) if !(3..5).contains(&s)),
        control_off: b(ee, o + 0xc80),
        speed_len: f(ee, o + 0x32c),
        steer_command: f(ee, o + 0x12c8),
        full_lock_timer: f(ee, o + 0x12fc),
        stick_angle: f(ee, o + 0x12b8),
        deflection: f(ee, o + 0x12bc),
        attitude: v(ee, o + 0x1250),
        local_vel: v(ee, o + 0x350),
        heading: v(ee, o + 0xbe0),
        up: v(ee, o + 0xbf0),
        was_airborne: b(ee, o + 0xde4),
        airborne: b(ee, o + 0xde0),
        hang_time: f(ee, o + 0x1190),
        crossup: f(ee, o + 0x330),
        flipping: b(ee, o + 0x1198),
        spinning: b(ee, o + 0x11d4),
        authority: f(ee, o + 0x1188),
        authority_time: f(ee, o + 0x118c),
        tipped: b(ee, ro + 0x24),
        node_pos: v(ee, o + 0xbd0),
        node_pos_kept: v(ee, o + 0xc40),
        // What the node hands back when asked: a call's answer, not a word of the quad's.
        node_local: Vf::new(0.0, 0.0, 0.0, 1.0),
    };
    let mut contacts = [WheelContact::default(); 4];
    for ((c, w), &a) in contacts.iter_mut().zip(&q.wheels).zip(&at.wheels) {
        *c = WheelContact { point: v(ee, a + 0x120), normal: w.normal, owner: ee.u32_at(a + 0x1f0), vehicle: ee.u32_at(a + 0x1f4), touching: i(ee, a + 0xd4) };
        if c.owner != 0 || c.vehicle != 0 {
            return Err(format!("wheel {a:#010x} stands on a collision object or a mover, which the reader does not read (only still ground)"));
        }
    }
    let body = Body {
        force: v(ee, bo + 0xa0),
        moment: v(ee, bo + 0x1b0),
        orientation: v(ee, bo + 0x190),
        ang_vel: v(ee, bo + 0x1d0),
        velocity: v(ee, bo + 0x80),
        reset_each_frame: v(ee, bo + 0xc0),
        position: v(ee, bo + 0x70),
        correction: v(ee, bo + 0xd0),
    };
    let s = State { p, q, body, bal, vstate, contacts, rear_node: ee.u32_at(o + 0x143c), dt };

    let mut held = game_sim::ride::Held::default();
    held.lock.steer = !b(ee, o + 0xdd0);
    held.lock.throttle = !b(ee, o + 0xdd4);
    held.lock.steer_was = !b(ee, o + 0x1298);
    held.lock.latch = b(ee, o + 0x129c);
    let joystick = game_sim::phy::Joystick {
        x_axis_rate_limit: f(ee, o + 0x12d8),
        y_axis_rate_limit: f(ee, o + 0x12dc),
        x_dpad_rate_limit: f(ee, o + 0x12e0),
        y_dpad_rate_limit: f(ee, o + 0x12e4),
        gas_rate_limit: f(ee, o + 0x1310),
        brake_rate_limit: f(ee, o + 0x1314),
        clutch_in_rate_limit: f(ee, o + 0x1318),
        clutch_out_rate_limit: f(ee, o + 0x131c),
    };
    let gate = (at.gate != 0).then(|| f(ee, at.gate + 0x1d0));

    let mut clutch_keys = mv::Params::default();
    clutch_keys.clutch_torque_scale_time = f(ee, o + 0x1350);
    clutch_keys.max_clutch_torque_scale_speed = f(ee, o + 0x1358);
    clutch_keys.max_clutch_torque_scale = f(ee, o + 0x135c);
    clutch_keys.up_shift_rpm = f(ee, o + 0x13dc);
    clutch_keys.down_shift_rpm = f(ee, o + 0x13e0);
    clutch_keys.cofg = v(ee, o + 0x1130);
    clutch_keys.balance_cofg = v(ee, o + 0x1150);
    clutch_keys.cofg_travel_forward = f(ee, o + 0x1320);
    clutch_keys.cofg_travel_back = f(ee, o + 0x1324);
    clutch_keys.clutch_cofg_travel_back = f(ee, o + 0x1328);
    clutch_keys.brake_cofg_travel_forward = f(ee, o + 0x132c);
    clutch_keys.drag = f(ee, wo + 4);
    let mut ride = mv::Ride::default();
    ride.clutch_timer = f(ee, o + 0x1348);
    ride.clutch_scale_start = f(ee, o + 0x134c);
    ride.clutch_cofg_weight = f(ee, o + 0x1354);

    let mut engine = mv::Engine {
        gear: i(ee, eo + 0x5c),
        gears: i(ee, eo + 0x6c),
        final_drive: f(ee, eo + 0x34),
        ratio: f(ee, eo + 0x74),
        inv_ratio: f(ee, eo + 0x78),
        clutch_torque_scale: f(ee, eo + 0x7c),
        clutch_locked: b(ee, eo + 0x58),
        rev_limit: f(ee, eo + 0xac),
        idle_throttle: f(ee, eo + 0x2c),
        engine_braking: f(ee, eo + 0x30),
        inv_inertia: f(ee, eo + 0x3c),
        clutch_static: f(ee, eo + 0x44),
        clutch_kinetic: f(ee, eo + 0x48),
        min_rpm: f(ee, eo + 0x54),
        curve: mv::Curve::at(ee.u32_at(eo + 0x80), ee.u32_at(eo + 0x84), f(ee, eo + 0x88)),
        torque: f(ee, eo + 0x98),
        wheel_torque: f(ee, eo + 0x9c),
        rpm: f(ee, eo + 0xa0),
        road_rpm: f(ee, eo + 0xa4),
        torque_scale: f(ee, eo + 0xb4),
        ..mv::Engine::default()
    };
    // The tables are read whole, past the gear and point counts: what follows them is carried as it is.
    for (k, r) in engine.gear_ratios.iter_mut().enumerate() {
        *r = f(ee, at.ratios + 4 * k as u32);
    }
    for k in 0..engine.curve_rpm.len() {
        engine.curve_rpm[k] = f(ee, at.curve_rpm + 4 * k as u32);
        engine.curve_torque[k] = f(ee, at.curve_torque + 4 * k as u32);
    }

    let mut k = air_keys_zero();
    k.preload_time = f(ee, o + 0xc5c);
    k.min_preload_time = f(ee, o + 0xc64);
    k.max_preload_time = f(ee, o + 0xc68);
    k.max_preload = f(ee, o + 0xc6c);
    k.min_dpad_preload_time = f(ee, o + 0x1160);
    k.max_dpad_preload_time = f(ee, o + 0x1164);
    k.min_backflip_time = f(ee, o + 0x11a4);
    k.max_backflip_time = f(ee, o + 0x11a8);
    k.min_backflip_torque = f(ee, o + 0x11b0);
    k.max_backflip_torque = f(ee, o + 0x11b4);
    k.backflip_torque_rate_increase = f(ee, o + 0x11b8);
    k.backflip_torque_rate_decrease = f(ee, o + 0x11bc);
    k.airborne_pitch_down_wreck_angle = f(ee, o + 0x1404);
    k.airborne_pitch_up_wreck_angle = f(ee, o + 0x1408);
    k.front_wreck_speed_limit = f(ee, o + 0x13e8);
    k.back_wreck_speed_limit = f(ee, o + 0x13ec);
    k.side_wreck_speed_limit = f(ee, o + 0x13f0);
    k.airborne_control_moment = v(ee, o + 0x1170);
    k.airborne_damping = f(ee, o + 0x1180);
    k.airborne_control_force = f(ee, o + 0x1184);
    k.brake_gyro_torque = f(ee, o + 0x1330);
    k.unroll_gain = f(ee, o + 0x1338);
    k.unroll_torque = f(ee, o + 0x133c);
    k.unyaw_gain = f(ee, o + 0x1340);
    k.unyaw_torque = f(ee, o + 0x1344);
    let mut air = game_sim::air::Air::default();
    air.preload_ready = b(ee, o + 0xc50);
    air.preload_firing = b(ee, o + 0xc54);
    air.preload_armed = b(ee, o + 0xc58);
    air.preload_timer = f(ee, o + 0xc60);
    air.preload_force = v(ee, o + 0xc70);
    air.flip_torque_applied = b(ee, o + 0x1194);
    air.flip_active = b(ee, o + 0x119c);
    air.flip_armed = b(ee, o + 0x11a0);
    air.flip_grace = f(ee, o + 0x11ac);
    air.flip_torque = v(ee, o + 0x11c0);

    let mut bike_wheels = [game_sim::wheel::Wheel::default(); 4];
    let mut more = [atv_wheel::More::default(); 4];
    let mut spins: [atv_wheel::Spin; 4] = Default::default();
    let mut drive = [atv_moto::Drive::default(); 4];
    for k in 0..4 {
        let (a, qw) = (at.wheels[k], &s.q.wheels[k]);
        bike_wheels[k] = game_sim::wheel::Wheel {
            has_ground: at.ground[k] != 0,
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
        more[k] = atv_wheel::More {
            record: ee.u8_at(a + 0x110),
            kept: v(ee, a + 0xc0),
            chassis_velocity_local: qw.contact_vel,
            ground_known: i(ee, a + 0x1a0),
            ground_velocity: v(ee, a + 0x1b0),
            ground_point_velocity: v(ee, a + 0x1c0),
            steer_axis: v(ee, a + 0x8c0),
        };
        at.gaps.push((a + 0x110, 0xff, "the contact record's first byte: the harness fills the rest".into()));
        drive[k] = atv_moto::Drive { spin: f(ee, a + 0x1d4), torque: f(ee, a + 0x210), from_torque: b(ee, a + 0xe0), forward: f(ee, a + 0x858) };
        let mut surfaces = [quad::TireSurface::default(); 16];
        for (j, t) in surfaces.iter_mut().enumerate() {
            let at = |n: usize| f(ee, a + 0x238 + 0x40 * n as u32 + 4 * j as u32);
            (t.drive, t.side, t.drag) = (std::array::from_fn(at), std::array::from_fn(|n| at(8 + n)), std::array::from_fn(|n| at(21 + n)));
        }
        let brake = match ee.u32_at(a + 0x24) {
            0 => None,
            p => Some(f(ee, p)),
        };
        spins[k] = atv_wheel::Spin {
            brake,
            parametric: i(ee, a + 0x34),
            donut: qw.donut,
            inv_inertia: f(ee, a + 0x58),
            tire_inertia: f(ee, a + 0x5c),
            arm: v(ee, a + 0xa0),
            no_moment: i(ee, a + 0xb0),
            rear_pair: i32::from(qw.rear_pair),
            other_load_scale: f(ee, a + 0xb8),
            slip: game_sim::tire::Slip { spinning: drive[k].from_torque, locked: b(ee, a + 0xe4), sliding: b(ee, a + 0xe8) },
            angle: f(ee, a + 0x1d0),
            spin: drive[k].spin,
            torque: drive[k].torque,
            cleared: [f(ee, a + 0x224), f(ee, a + 0x228)],
            side_per_load: f(ee, a + 0x234),
            surfaces,
            inv_radius: qw.inv_radius,
            local_velocity: v(ee, a + 0x840),
            local_force: v(ee, a + 0x850),
            frame: [v(ee, a + 0x8d0), v(ee, a + 0x8e0), v(ee, a + 0x8f0), v(ee, a + 0x900)],
        };
    }
    let wheel_index = |p: u32| at.wheels.iter().position(|&w| w == p);
    let driven = (
        wheel_index(ee.u32_at(eo + 0x90)).ok_or("the engine's first driven wheel is none of the quad's")?,
        match ee.u32_at(eo + 0x94) {
            0 => None,
            p => Some(wheel_index(p).ok_or("the engine's second driven wheel is none of the quad's")?),
        },
    );

    let mut nodes = [atv_moto::Node::default(); SCENE_NODES];
    let index = |p: u32| at.nodes.iter().position(|&n| n == p);
    for (k, (n, &a)) in nodes.iter_mut().zip(&at.nodes).enumerate() {
        let link = |off: u32, gaps: &mut Vec<(u32, u32, String)>| match ee.u32_at(a + off) {
            0 => None,
            p => index(p).or_else(|| {
                gaps.push((a + off, 0, format!("node {k}'s link to {p:#010x}, a node past the {SCENE_NODES} the scene holds")));
                None
            }),
        };
        let (parent, child, sibling) = (link(0xf0, &mut at.gaps), link(0xf4, &mut at.gaps), link(0xf8, &mut at.gaps));
        *n = atv_moto::Node {
            local: std::array::from_fn(|r| v(ee, a + 0x30 + 0x10 * r as u32)),
            world: std::array::from_fn(|r| v(ee, a + 0x70 + 0x10 * r as u32)),
            parent,
            child,
            sibling,
            flags: ee.u8_at(a + 0x100),
            spatial: b(ee, a + 8),
            centre: v(ee, a + 0xd0),
            half: v(ee, a + 0xe0),
            more: ee.u8_at(a + 0x101),
            reach: f(ee, a + 0x108),
            own_centre: v(ee, a + 0xb0),
            own_half: v(ee, a + 0xc0),
            radius: f(ee, a + 0x104),
            ..atv_moto::Node::default()
        };
        // MD75c3: the list, if the scene holds every node on it.
        let (count, list) = (ee.u32_at(a + 0x10c), ee.u32_at(a + 0x144));
        let members: Option<Vec<usize>> = if count as usize <= SCENE_NODES {
            (0..count).map(|i| index(ee.u32_at(list + 4 * i))).collect()
        } else {
            None
        };
        match members {
            Some(m) => {
                n.listed = m.len();
                n.list[..m.len()].copy_from_slice(&m);
                if !m.is_empty() {
                    at.node_lists.push((node_at(k) + super::NODE_LIST, list, 4 * count));
                }
            }
            None => {
                let why = format!("node {k}'s list of {count} at {list:#010x}: the gather's nodes past the {SCENE_NODES} the scene holds");
                at.gaps.push((a + 0x10c, 0, why.clone()));
                at.gaps.push((a + 0x144, 0, why));
            }
        }
        at.gaps.push((a + 0x100, 0xffff, "the node's two flag bytes; the word's other two are not the ride's".into()));
    }
    let n0 = at.nodes[0];
    let flags_word = o + 0x44;
    at.gaps.push((flags_word, 0xff01, "the collision sync's bit (+0x44 bit 0) and the flag byte (+0x45); the other bits are not the ride's".into()));
    let spin = atv_moto::Spin {
        armed: b(ee, o + 0x11dc),
        min_time: f(ee, o + 0x11e0),
        max_time: f(ee, o + 0x11e4),
        grace: f(ee, o + 0x11e8),
        min_pitch: f(ee, o + 0x11ec),
        max_pitch: f(ee, o + 0x11f0),
        min_yaw: f(ee, o + 0x11f4),
        max_yaw: f(ee, o + 0x11f8),
        rate: f(ee, o + 0x1200),
        moment: v(ee, o + 0x1210),
        heading: f(ee, o + 0x1220),
        dir: i(ee, o + 0x1224),
        flick: f(ee, o + 0x12b4),
        stunt_right: ee.u32_at(o + 0x13bc),
        stunt_left: ee.u32_at(o + 0x13c0),
        clip: i(ee, o + 0x1758),
    };
    let parts = atv_moto::Parts {
        drive,
        driven,
        cofg_off: v(ee, o + 0x1140),
        last_position: v(ee, bo + 0x60),
        body_on: b(ee, bo),
        flags: ee.u8_at(o + 0x45),
        record: true,
        spin,
        scene: atv_moto::Scene { nodes },
        bounds: atv_moto::Bounds { floor: f(ee, wo + 0x98), area: (at.area != 0).then(|| (v(ee, at.area + 0x30), v(ee, at.area + 0x40))) },
        spatial: b(ee, o + 8),
        collidables: atv_moto::Collidables {
            sync: ee.u8_at(o + 0x44) & 1 != 0,
            riding: part(ee.u32_at(o + 0xcc), "riding probe (+0xcc)")?,
            frame: part(ee.u32_at(o + 0xd0), "frame mesh (+0xd0)")?,
            active: part(ee.u32_at(o + 0xd8), "active probe (+0xd8)")?,
            states,
        },
        rear_node: ee.u32_at(o + 0x143c),
    };

    // The trick buttons' byte: on the quad, `+0x14ec` is not the bike's buttons word (oracle # MD37a),
    // and only its low byte is the ride's.
    let buttons = ee.u8_at(o + 0x14ec);
    at.gaps.push((o + 0x14ec, 0xff, "the trick buttons' byte: the quad's word above it is not the buttons".into()));
    let (touching, class) = (b(ee, ro + 0x34), if at.thing != 0 { ee.u32_at(at.thing + 0x94) } else { 0 });
    if at.thing == 0 {
        at.gaps.push((ro + 0x20c, 0, "no struck thing: the harness always lays one".into()));
    }
    let rider_stunt = bal.0 as i32;
    let bike = quad::Bike {
        held,
        gas_cut: handling::step::GasCut { latched: b(ee, o + 0x1764), timer: f(ee, o + 0x1768) },
        cut_time: f(ee, o + 0x176c),
        joystick,
        stick: game_sim::trick::Stick { x: f(ee, o + 0x12a8), y: f(ee, o + 0x12ac), digital: [b(ee, o + 0x12a0), b(ee, o + 0x12a4)] },
        clutch: f(ee, o + 0x12c0),
        gate_cut: gate.is_some_and(|g| F::ZERO < g),
        clutch_keys,
        ride,
        engine,
        air_keys: k,
        air,
        stick_hold: f(ee, o + 0x12b0),
        preload_cue: b(ee, o + 0x700),
        spin_applied: b(ee, o + 0x11d0),
        spin_active: b(ee, o + 0x11d8),
        touching,
        struck_class: touching && class & ee.u32_at(STRUCK_MASK) != 0,
        buttons: buttons & 0xf,
        right_y: f(ee, n0 + 0x84),
        up_y: f(ee, n0 + 0x94),
        stunt_id: i(ee, o + 0x1528),
        rider_stunt,
        flip_clip: handling::FlipClip(i(ee, o + 0x1754)),
        flip_stunt: ee.u32_at(o + 0x13b8),
        gas_gyro_torque: f(ee, o + 0x1334),
        gyro: handling::Gyro { brake: b(ee, o + 0x1290), gas: b(ee, o + 0x1294) },
        wheels: bike_wheels,
        more,
        ground: [atv_wheel::Ground::Still; 4],
        cofg: v(ee, bo + 0x1a0),
        spins,
        parts,
        contacts: None,
    };
    let r = Ride { s, bike, gate, class, high: buttons >> 4, tag: ee.u32_at(o + 0xd20), answers: Vec::new(), echo: false, scene: true };
    Ok((r, at))
}

/// The harness's layout against the image's: each test-side block, the image's block it stands for,
/// and its length. Words the poke writes are compared through these, and a pointer the poke lays is
/// equal to the image's if it is the same place in its block.
pub(super) fn blocks(at: &Image) -> Vec<(u32, u32, u32)> {
    let mut out = vec![
        (OBJ, at.quad, 0x1800),
        (BODY, at.body, 0x230),
        (WORLD, at.world, 0xb0),
        (STATE_OBJ, at.state, 0x40),
        (RECORD, at.record, 0x210),
        // The tables before the engine: they sit inside the harness's engine block.
        (RATIOS, at.ratios, 0x20),
        (CURVE_TORQUE, at.curve_torque, 0x40),
        (CURVE_RPM, at.curve_rpm, 0x40),
        (ENGINE, at.engine, 0x100),
        (GATE, at.gate, 0x1d4),
        (THING, at.thing, 0x98),
        (NODE, at.nodes[0], 0x98),
        (BOX, at.area, 0x50),
    ];
    for k in 0..4 {
        out.push((WHEEL[k], at.wheels[k], 0x920));
    }
    for (k, &n) in at.nodes.iter().enumerate() {
        out.push((node_at(k), n, 0x148));
    }
    out.extend(at.node_lists.iter().copied());
    // MD75c2: each collidable, and its tree, bound or mesh where the harness lays them.
    for (&c, &p) in COLLIDABLES.iter().zip(&at.parts).filter(|(_, p)| **p != 0) {
        out.push((collidable_at(c), p, 0x300));
    }
    out.extend(at.part_blocks.iter().copied());
    out.retain(|&(_, image, _)| image != 0);
    out
}

/// Pointers the harness lays to a stand-in of its own: the ground every wheel stands on is [`WORLD`],
/// the record's and the nodes' vtables are the harness's hook tables, the database entries are
/// [`ENTRY`], and each collidable is at [`collidable_at`].
pub(super) fn stand_ins(at: &Image) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = at.ground.iter().map(|&g| (WORLD, g)).collect();
    out.extend([(RECORD_VT, at.record_vt), (ACTORS, at.actors), (SEARCH, at.search)]);
    out.extend(at.node_vts.iter().map(|&vt| (NODE_VT, vt)));
    out.extend(at.entries.iter().map(|&e| (ENTRY, e)));
    out.extend(at.part_dbs.iter().map(|&d| (ENTRY, d)));
    out.extend(COLLIDABLES.iter().zip(&at.parts).filter(|(_, p)| **p != 0).map(|(&c, &p)| (collidable_at(c), p)));
    out
}

/// What the harness lays that is its own and stands for nothing in the image: the record's and the
/// nodes' hook tables, and the struck thing when the image's record has none.
pub(super) fn harness_only(at: &Image, a: u32) -> bool {
    (RECORD_VT..RECORD_VT + 0x100).contains(&a)
        || (NODE_VT..NODE_VT + 0x100).contains(&a)
        || (at.thing == 0 && (THING..THING + 0x100).contains(&a))
        || at.part_own.iter().any(|&b| (b..b + 0x20).contains(&a))
        // MD75c2: a collidable the quad has none of is laid as a default one.
        || COLLIDABLES.iter().zip(&at.parts).any(|(&c, &p)| p == 0 && (collidable_at(c)..collidable_at(c) + COLLIDABLE_SIZE).contains(&a))
}

/// The image's address of a word the poke wrote, through [`blocks`].
pub(super) fn image_at(blocks: &[(u32, u32, u32)], t: u32) -> Option<u32> {
    blocks.iter().find(|&&(tb, _, len)| (tb..tb + len).contains(&t)).map(|&(tb, ib, _)| ib + (t - tb))
}

/// Every word [`poke_ride`] writes of `r`, compared with `image`'s through `at`: the mismatches, as
/// `(test address, image address, poked, image's)`, and how many words were compared.
pub(super) fn compare(image: &Ee, fresh: &mut Ee, r: &Ride, at: &Image) -> (Vec<(u32, u32, u32, u32)>, usize) {
    reset_all(fresh);
    for (_, base, len) in [("vt", RECORD_VT, 0x100), ("thing", THING, 0x100), ("gate", GATE, 0x200), ("node vt", NODE_VT, 0x100), ("box", BOX, 0x100)] {
        for o in (0..len).step_by(4) {
            fresh.set_u32(base + o, 0);
        }
    }
    fresh.written.clear();
    poke_ride(fresh, r);
    let words: std::collections::BTreeSet<u32> = fresh.written.iter().map(|&a| a & !3).collect();
    let (blocks, stand_ins) = (blocks(at), stand_ins(at));
    let gap = |a: u32| at.gaps.iter().filter(|g| g.0 == a).fold(!0u32, |m, g| m & g.1);
    let mut bad = Vec::new();
    let mut n = 0;
    for &t in &words {
        if harness_only(at, t) {
            continue;
        }
        n += 1;
        let poked = fresh.u32_at(t);
        let Some(ia) = image_at(&blocks, t) else {
            bad.push((t, 0, poked, 0));
            continue;
        };
        let want = image.u32_at(ia);
        let same = poked == want
            || image_at(&blocks, poked) == Some(want)
            || stand_ins.contains(&(poked, want))
            || (poked ^ want) & gap(ia) == 0;
        if !same {
            bad.push((t, ia, poked, want));
        }
    }
    (bad, n)
}

/// **The spawned quad reads back word for word**: [`read_ride`] off `atv-r-spawn.ee.bin`, poked into a
/// fresh image at the harness's layout by [`poke_ride`], equals the image at every word the poke
/// writes, but for the words [`Image::gaps`] declares (and there, on the bits it says it carries).
/// Then one word of the read ride is changed, to see the comparison go red there.
#[test]
fn the_spawned_quad_reads_back_word_for_word() {
    let Some(image) = image() else { return };
    let mut fresh = super::image().unwrap();
    let (r, at) = read_ride(&image, IMAGE_QUAD, F::ZERO).unwrap_or_else(|e| panic!("{e}"));
    let (bad, n) = compare(&image, &mut fresh, &r, &at);
    let show = |bad: &[(u32, u32, u32, u32)]| {
        bad.iter().take(20).map(|&(t, ia, p, w)| format!("  test {t:#010x} = image {ia:#010x}: poked {p:#010x}, image {w:#010x}")).collect::<Vec<_>>().join("\n")
    };
    assert!(bad.is_empty(), "{} of {n} words differ from the image:\n{}", bad.len(), show(&bad));
    eprintln!("MD75d: {n} words equal; {} gaps declared:", at.gaps.len());
    for (a, mask, why) in &at.gaps {
        let held = (image.u32_at(*a) & !mask) != 0;
        eprintln!("  {a:#010x} carries {mask:#010x}{}: {why}", if held { " (the image has bits past it)" } else { "" });
    }
    assert!(n > 1500, "only {n} words compared: the poke did not lay the ride");

    // A word changed in the read ride goes red at its place, and only there.
    let mut wrong = r.clone();
    wrong.s.q.lean = F(f32::from_bits(r.s.q.lean.to_bits() ^ 1));
    wrong.bike.spins[3].frame[2] = Vf::new(1.0, 2.0, 3.0, 4.0);
    let (bad, _) = compare(&image, &mut fresh, &wrong, &at);
    let places: Vec<u32> = bad.iter().map(|b| b.1).collect();
    let want = [vec![IMAGE_QUAD + 0x12e8], (0..4).map(|k| at.wheels[3] + 0x8f0 + 4 * k).collect()].concat();
    assert_eq!(places, want, "the changed ride's mismatches:\n{}", show(&bad));
}
