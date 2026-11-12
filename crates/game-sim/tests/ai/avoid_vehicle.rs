// --- the generic vehicle avoidance (S317) ------------------------------------------------------------
//
// VehicleAI's whole avoidance pass runs in the interpreter, ten thousand times. What it calls is
// hooked: the scene walk (a fake vtable that hands out a generated list of solid objects, placed groups
// and others), the feeler sweep against each solid object (scripted answers and per-feeler hits, which
// the port is handed in the same order), the placed groups' part count (its answer is not read), the
// AI_IGNORE class bit, the ground under each feeler's point ahead (a scripted height), and the route's
// segment test (scripted per call). The bike's hit reaction runs as it is, on the real generator. The
// race's rider list is generated along the fan, its own bike included sometimes so the pass has to skip
// it. Compared each time: the steer direction (all four lanes), the aimed-for speed, the flag word, the
// whole fan and the bundle's centre, which objects were swept, every ground and segment point asked,
// the hit reaction's three recovery words, and the generator.

use super::*;
use game_sim::ai::{Thing, VehicleScene, VehicleTuning, blocked};

const V_AI: u32 = 0x0190_0000;
const V_OWN: u32 = 0x0190_1000; // its own bike: +0x40, +0x88, +0x8c
const V_RACE: u32 = 0x0190_2000; // +0x34 the rider list, +0x68 the track's path
/// The rider the interface belongs to: +0x08 the interface's vtable, +0x10d0 its brain, +0x14c0..c8
/// the recovery the hit reaction writes. The pass is handed it as its interface.
const V_SCENE: u32 = 0x0190_4000;
const V_SCENE_VT: u32 = 0x0190_3000;
const V_BUNDLE: u32 = 0x0190_6000; // +0x6c record, +0x134/+0x138/+0x13c, +0x150 the centre
const V_RECORD: u32 = 0x0190_6400;
const V_LENGTHS: u32 = 0x0190_6800;
const V_ORIGINS: u32 = 0x0190_7000;
const V_DIRS: u32 = 0x0190_8000;
const V_REC_NORMALS: u32 = 0x0190_9000;
const V_REC_POINTS: u32 = 0x0190_9800;
const V_REC_FLAGS: u32 = 0x0190_a000;
const V_REC_FRACTIONS: u32 = 0x0190_a400;
const V_ARGS: u32 = 0x0190_b000; // position, heading, steer at +0x00/+0x10/+0x20; speeds at +0x30/+0x34
const V_IGNORE: u32 = 0x0190_b100;
const V_WORLD: u32 = 0x0190_c000;
const V_SEGMENT: u32 = 0x0190_d000; // the route node, its next (+0x54) and that one's next
const V_HOOK_BEGIN: u32 = 0x0190_e000;
const V_HOOK_NEXT: u32 = 0x0190_e008;
const V_HOOK_END: u32 = 0x0190_e010;
const V_HOOK_GROUND: u32 = 0x0190_e018;
const V_RIDERS: u32 = 0x0191_0000;
const V_RIDER_SIZE: u32 = 0x200;
const V_OBJECTS: u32 = 0x0192_0000;
const V_MAX_FEELERS: u32 = 16;
const AI_IGNORE_BIT: u32 = 0x40;

#[derive(Default)]
struct VScript {
    scene: Vec<u32>,
    at: usize,
    opens: bool,
    begun: bool,
    feelers: u32,
    /// The sweep's answers in turn: whether it met the object, and the per-feeler hits.
    sweeps: Vec<(bool, Vec<bool>)>,
    swept: Vec<u32>,
    groups: usize,
    /// The ground's answers in turn (heights), and the points it was asked about.
    heights: Vec<f32>,
    grounds: Vec<[u32; 4]>,
    /// The segment test's answers in turn, and the points it was asked about.
    holds: Vec<bool>,
    held: Vec<[u32; 4]>,
}

/// The port's scene: the same walk and sweep answers, in the same order.
struct VPortScene {
    things: Option<Vec<Thing>>,
    sweeps: Vec<(bool, Vec<bool>)>,
    swept: Vec<u32>,
}

impl ai::Scene for VPortScene {
    fn walk(&mut self) -> Option<Vec<Thing>> {
        self.things.clone()
    }
    fn sweep(&mut self, id: u32, fan: &[Feeler]) -> Option<Vec<bool>> {
        assert!(!fan.is_empty());
        self.swept.push(id);
        let (met, hits) = self.sweeps.remove(0);
        met.then_some(hits)
    }
}

/// The port's ground: the scripted heights in turn.
struct VPortGround {
    heights: Vec<f32>,
    asked: Vec<[u32; 4]>,
}

impl World for VPortGround {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.asked.push(at.to_bits());
        let h = self.heights.remove(0);
        (Vf::new(at.x(), h, at.z(), at.w()), Vf::new(0.0, 1.0, 0.0, 0.0))
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

#[test]
fn vehicle_avoidance_matches_over_ten_thousand_passes() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "ai_update_vehicle",
        "bundle_sweep_object",
        "group_part_count",
        "class_bit",
        "world_manager",
        "path_segment_holds",
        "ai_hit_reaction",
        "rand_state",
        "curve_table_build",
        "mem_alloc",
    ]) else {
        return;
    };
    let [update, sweep, part_count, class_bit, world_manager, holds, hit_reaction, rand_state, curve_build, mem_alloc] =
        at[..]
    else {
        unreachable!()
    };
    ee.hooks.insert(mem_alloc, Box::new(|ee| ee.set(2, 0x0100_0000)));
    ee.call(curve_build, &[], &[], 2_000_000_000);
    ee.hooks.remove(&mem_alloc);
    let curve = CurveTable::build();

    let script = Rc::new(RefCell::new(VScript::default()));
    let s = script.clone();
    ee.hooks.insert(V_HOOK_BEGIN, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.begun = true;
        ee.set(2, u64::from(s.opens));
    }));
    let s = script.clone();
    ee.hooks.insert(V_HOOK_NEXT, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        let o = s.scene.get(s.at).copied().unwrap_or(0);
        s.at += 1;
        ee.set(2, u64::from(o));
    }));
    ee.hooks.insert(V_HOOK_END, Box::new(|_| {}));
    let s = script.clone();
    ee.hooks.insert(V_HOOK_GROUND, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        let p = ee.r(5) as u32;
        s.grounds.push(got(ee, p));
        let h = s.heights.remove(0);
        ee.set_f32(p + 4, h);
        ee.set(2, 1);
    }));
    let s = script.clone();
    ee.hooks.insert(sweep, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.swept.push(ee.r(5) as u32);
        let (met, hits) = s.sweeps.remove(0);
        for k in 0..s.feelers {
            ee.set_u32(V_REC_FLAGS + 4 * k, u32::from(hits[k as usize]));
        }
        ee.set(2, u64::from(met));
    }));
    let s = script.clone();
    ee.hooks.insert(part_count, Box::new(move |ee| {
        s.borrow_mut().groups += 1;
        ee.set(2, 0);
    }));
    ee.hooks.insert(class_bit, Box::new(|ee| ee.set(2, u64::from(AI_IGNORE_BIT))));
    let s = script.clone();
    ee.hooks.insert(holds, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        let p = got(ee, ee.r(5) as u32);
        s.held.push(p);
        let r = s.holds.remove(0);
        ee.set(2, u64::from(r));
    }));

    ee.set_u32(world_manager, V_WORLD);
    ee.set_u32(V_SCENE + 8, V_SCENE_VT);
    ee.set_u32(V_SCENE + 0x10d0, V_AI);
    ee.set_u32(V_SCENE_VT + 0x2c, V_HOOK_BEGIN);
    ee.set_u32(V_SCENE_VT + 0x30, V_HOOK_NEXT);
    ee.set_u32(V_SCENE_VT + 0x34, V_HOOK_END);
    ee.set_u32(V_SCENE_VT + 0x38, V_HOOK_GROUND);
    ee.set_u32(V_SCENE_VT + 0x3c, hit_reaction);
    ee.set_u32(V_AI + 0x40, V_SCENE);
    ee.set_u32(V_AI + 0xd0, 5);
    ee.set_u32(V_AI + 0xd4, V_OWN);
    ee.set_u32(V_AI + 0x2a0, V_BUNDLE);
    ee.set_u32(V_OWN + 0x40, 1);
    ee.set_u32(V_OWN + 0x88, 1);
    ee.set_u32(V_OWN + 0x8c, 1);
    ee.set_u32(V_BUNDLE + 0x6c, V_RECORD);
    ee.set_u32(V_BUNDLE + 0x134, V_LENGTHS);
    ee.set_u32(V_BUNDLE + 0x138, V_ORIGINS);
    ee.set_u32(V_BUNDLE + 0x13c, V_DIRS);
    ee.set_u32(V_RECORD + 0x94, V_REC_NORMALS);
    ee.set_u32(V_RECORD + 0x98, V_REC_POINTS);
    ee.set_u32(V_RECORD + 0xa4, V_REC_FLAGS);
    ee.set_u32(V_RECORD + 0xa8, V_REC_FRACTIONS);

    let mut pick = Pick(0x5317_a01d_7e41_c1e5);
    let mut seen = std::collections::BTreeMap::<&str, usize>::new();
    for case in 0..10_000 {
        let p = &mut pick;
        let defaults = p.chance(4);
        let (mut t, mut v) = (AvoidTuning::default(), VehicleTuning::default());
        // free09's riders' numbers.
        (t.near_range, t.far_range, t.edge_look_ahead) = (F(5.0), F(100.0), F(10.0));
        if !defaults {
            t.near_range = F(p.range(0.5, 12.0));
            t.far_range = F(p.range(0.0, 120.0));
            t.edge_look_ahead = F(p.range(0.0, 20.0));
            t.feelers = 1 + p.below(V_MAX_FEELERS);
            t.feeler_height = F(p.range(0.0, 5.0));
            t.feeler_length = F(p.range(10.0, 150.0));
            v = VehicleTuning {
                probe_ahead: F(p.range(1.0, 30.0)),
                drop_run: F(p.range(1.0, 30.0)),
                climb_limit: F(p.range(0.0, 1.0)),
                climb_speed: F(p.range(0.0, 60.0)),
                drop_limit: F(p.range(-1.0, 0.0)),
                turn_limit: F(if p.chance(3) { 1e6 } else { p.range(0.0, 200.0) }),
            };
        }
        let n = t.feelers as usize;
        let yaw = p.range(-7.0, 7.0);
        let heading = Vf::new(yaw.sin(), p.range(-0.2, 0.2), yaw.cos(), p.range(-1.0, 1.0));
        let me = Me {
            position: Vf::new(p.range(200.0, 1800.0), p.range(-50.0, 50.0), p.range(200.0, 1800.0), 1.0),
            heading,
            velocity: p.vec(30.0, 0.0),
            speed: F(if p.chance(20) { p.range(-5.0, 0.0) } else { p.range(0.01, 60.0) }),
            held_speed: F(p.range(-5.0, 80.0)),
            progress: F(0.0),
        };
        let alias = p.chance(3);
        let steer_in = if alias {
            Steer { direction: heading, speed: me.speed }
        } else {
            let w = p.range(-1.0, 1.0);
            let y2 = yaw + p.range(-1.5, 1.5);
            Steer { direction: Vf::new(y2.sin(), p.range(-0.2, 0.2), y2.cos(), w), speed: F(p.range(0.0, 60.0)) }
        };
        let mut flags = p.next() & 0xffff_ffff_f9fd_fe00 | p.next() & 0x3f;
        for bit in [
            flag::NO_RIDER_SWEEP,
            flag::QUIET_AREA,
            flag::AVOID_DUE,
            flag::KEEP_ON_TRACK,
            flag::CLIMB_CHECK,
            flag::DROP_CHECK,
            0x200,
        ] {
            flags = if p.chance(2) { flags | bit } else { flags & !bit };
        }

        // The riders, most of them along the fan.
        let mut others = Vec::new();
        let mut list = Vec::new();
        let count = p.below(7) as u32;
        let own_at = if p.chance(3) { p.below(count + 1) as u32 } else { u32::MAX };
        for k in 0..count {
            if k == own_at {
                list.push(V_OWN);
            }
            let a = yaw + p.range(-0.9, 0.9);
            let d = p.range(-10.0, 110.0);
            let o = Other {
                active: !p.chance(8),
                marked: p.chance(3),
                position: Vf::new(
                    me.position.x() + a.sin() * d + p.range(-4.0, 4.0),
                    me.position.y() + p.range(-3.0, 3.0),
                    me.position.z() + a.cos() * d + p.range(-4.0, 4.0),
                    p.range(-1.0, 1.0),
                ),
                velocity: if p.chance(4) { Vf::new(0.0, 0.0, 0.0, 0.0) } else { p.vec(40.0, 0.0) },
                heading: p.vec(1.0, 0.0),
                speed: F(p.range(0.0, 60.0)),
                progress: F(0.0),
            };
            others.push(o);
            list.push(V_RIDERS + k * V_RIDER_SIZE);
        }
        if own_at == count {
            list.push(V_OWN);
        }

        // The scene the walk hands out.
        let cell = p.range(0.5, 4.0);
        let mut scene = Vec::new();
        let mut things = Vec::new();
        for k in 0..p.below(7) as u32 {
            let o = V_OBJECTS + k * 0x100;
            let kind = [1u8, 2, 2, 3, 3][p.below(5) as usize];
            let collidable = !p.chance(4);
            let ai_ignore = p.chance(5);
            let a = yaw + p.range(-0.9, 0.9);
            let d = p.range(-20.0, 160.0);
            let gx = ((me.position.x() + a.sin() * d) / cell).max(0.0) as u16;
            let gz = ((me.position.z() + a.cos() * d) / cell).max(0.0) as u16;
            ee.write(o + 2, 1, u128::from(kind));
            ee.write(o + 0x31, 1, u128::from(collidable));
            ee.set_u32(o + 0x84, if ai_ignore { AI_IGNORE_BIT | 0x1 } else { 0x2 });
            ee.write(o + 8, 2, u128::from(gx));
            ee.write(o + 0xc, 2, u128::from(gz));
            scene.push(o);
            things.push(match kind {
                2 => Thing::Solid { id: o, collidable, ai_ignore },
                3 => Thing::Group { x: gx, z: gz },
                _ => Thing::Other,
            });
        }
        let ignored: Vec<u32> = scene.iter().copied().filter(|_| p.chance(4)).collect();
        let opens = !p.chance(8);
        let sweeps: Vec<(bool, Vec<bool>)> =
            (0..scene.len()).map(|_| (!p.chance(3), (0..n).map(|_| p.chance(5)).collect())).collect();
        let base = me.position.y();
        let steep = p.chance(3);
        let heights: Vec<f32> =
            (0..n).map(|_| base + if steep { p.range(-40.0, 40.0) } else { p.range(-12.0, 12.0) }).collect();
        let hold_answers: Vec<bool> = (0..2 * n).map(|_| !p.chance(3)).collect();
        let track = !p.chance(5);
        let segments = [!p.chance(8), !p.chance(8), !p.chance(8)];

        // --- into RAM --------------------------------------------------------------------------
        ee.write(V_AI + 0x30, 8, u128::from(flags));
        ee.set_f32(V_AI + 0x80, me.held_speed.0);
        ee.set_u32(V_AI + 0xd8, V_RACE);
        ee.set_u32(V_RACE + 0x34, list.first().copied().unwrap_or(0));
        ee.set_u32(V_RACE + 0x68, u32::from(track));
        ee.set_u32(V_OWN + 4, 1);
        for (i, &r) in list.iter().enumerate() {
            ee.set_u32(r + 0x1c8, list.get(i + 1).copied().unwrap_or(0));
        }
        for (k, o) in others.iter().enumerate() {
            let r = V_RIDERS + k as u32 * V_RIDER_SIZE;
            ee.set_u32(r + 4, u32::from(o.active));
            ee.write(r + 8, 8, if o.marked { 0x8000_0004 } else { 0x8000_0003 });
            put(&mut ee, r + 0x60, o.position);
            put(&mut ee, r + 0x110, o.velocity);
        }
        for (f, x) in [
            (0x280, t.near_range),
            (0x284, t.far_range),
            (0x288, t.edge_look_ahead),
            (0x2a8, t.feeler_height),
            (0x2ac, t.feeler_length),
            (0x300, v.probe_ahead),
            (0x304, v.drop_run),
            (0x308, v.climb_limit),
            (0x30c, v.climb_speed),
            (0x310, v.drop_limit),
            (0x328, v.turn_limit),
        ] {
            ee.set_f32(V_AI + f, x.0);
        }
        ee.set_u32(V_AI + 0x2a4, n as u32);
        for k in 0..V_MAX_FEELERS {
            put(&mut ee, V_DIRS + 16 * k, Vf::new(-7.0, -7.0, -7.0, 1.0));
        }
        ee.set_f32(V_WORLD + 0xd4, cell);
        for (k, &o) in ignored.iter().enumerate() {
            ee.set_u32(V_IGNORE + 4 * k as u32, o);
        }
        ee.set_u32(V_AI + 0x224, if segments[0] { V_SEGMENT } else { 0 });
        ee.set_u32(V_SEGMENT + 0x54, if segments[1] { V_SEGMENT + 0x100 } else { 0 });
        ee.set_u32(V_SEGMENT + 0x154, if segments[2] { V_SEGMENT + 0x200 } else { 0 });
        let recovery_in = Recovery { back_up: F(p.range(-1.0, 1.0)), swerve: F(p.range(-1.0, 1.0)), swerve_bar: F(0.5) };
        ee.set_f32(V_SCENE + 0x14c0, recovery_in.back_up.0);
        ee.set_f32(V_SCENE + 0x14c4, recovery_in.swerve_bar.0);
        ee.set_f32(V_SCENE + 0x14c8, recovery_in.swerve.0);
        let rng_in = p.next();
        ee.write(rand_state, 8, u128::from(rng_in));
        let (speed_at, steer_at) = if alias { (V_ARGS + 0x30, V_ARGS + 0x10) } else { (V_ARGS + 0x34, V_ARGS + 0x20) };
        put(&mut ee, V_ARGS, me.position);
        put(&mut ee, V_ARGS + 0x10, me.heading);
        ee.set_f32(V_ARGS + 0x30, me.speed.0);
        if !alias {
            put(&mut ee, V_ARGS + 0x20, steer_in.direction);
            ee.set_f32(V_ARGS + 0x34, steer_in.speed.0);
        }
        *script.borrow_mut() = VScript {
            scene: scene.clone(),
            opens,
            feelers: n as u32,
            sweeps: sweeps.clone(),
            heights: heights.clone(),
            holds: hold_answers.clone(),
            ..VScript::default()
        };

        ee.call(
            update,
            &[
                u64::from(V_AI),
                ignored.len() as u64,
                u64::from(V_IGNORE),
                u64::from(V_ARGS),
                u64::from(V_ARGS + 0x10),
                u64::from(steer_at),
                u64::from(V_ARGS + 0x30),
                u64::from(speed_at),
            ],
            &[],
            BUDGET,
        );

        // --- the port ----------------------------------------------------------------------------
        let mut port_flags = flags;
        let mut steer = steer_in;
        let mut recovery = recovery_in;
        let mut rng = Rng::from_state(rng_in);
        let mut port_scene = VPortScene { things: opens.then(|| things.clone()), sweeps, swept: Vec::new() };
        let mut ground = VPortGround { heights, asked: Vec::new() };
        let mut answers = hold_answers.into_iter();
        let mut held = Vec::new();
        let mut off_path = |at: Vf| {
            if !segments.iter().all(|&s| s) {
                return false;
            }
            held.push(at.to_bits());
            if answers.next().unwrap() {
                return false;
            }
            held.push(at.to_bits());
            !answers.next().unwrap()
        };
        let mut around = VehicleScene {
            scene: &mut port_scene,
            cell: F(cell),
            ignore: &ignored,
            off_path: track.then_some(&mut off_path as &mut dyn FnMut(Vf) -> bool),
        };
        let pass = ai::avoid_vehicle(
            &t, &v, &me, &mut port_flags, &others, &mut around, &mut ground, &mut steer, &mut recovery, &mut rng,
            &curve,
        );
        drop(around);
        let what = format!("case {case}: flags {flags:#x}, {n} feelers, {} riders, scene {things:?}", others.len());

        assert_eq!(got(&ee, steer_at), steer.direction.to_bits(), "{what}: the steer direction");
        assert_eq!(ee.u32_at(speed_at), steer.speed.to_bits(), "{what}: the aimed-for speed");
        assert_eq!(ee.read(V_AI + 0x30, 8) as u64, port_flags, "{what}: the flags");
        assert_eq!(ee.read(rand_state, 8) as u64, rng.state(), "{what}: the generator");
        for (name, off, ours) in [
            ("back-up", 0x14c0, recovery.back_up),
            ("swerve bar", 0x14c4, recovery.swerve_bar),
            ("swerve", 0x14c8, recovery.swerve),
        ] {
            assert_eq!(ee.u32_at(V_SCENE + off), ours.to_bits(), "{what}: the {name}");
        }
        let s = script.borrow();
        match &pass {
            Some(pass) => {
                for (k, Feeler { origin, direction, length }) in pass.fan.iter().enumerate() {
                    let k = k as u32;
                    assert_eq!(got(&ee, V_DIRS + 16 * k), direction.to_bits(), "{what}: feeler {k}'s direction");
                    assert_eq!(got(&ee, V_ORIGINS + 16 * k), origin.to_bits(), "{what}: feeler {k}'s origin");
                    assert_eq!(ee.u32_at(V_LENGTHS + 4 * k), length.to_bits(), "{what}: feeler {k}'s length");
                }
                assert_eq!(got(&ee, V_BUNDLE + 0x150), me.position.to_bits(), "{what}: the bundle's centre");
                let look = ai::look(flags);
                assert_eq!(s.begun, look.fan, "{what}: the scene walk");
                let groups = if look.fan && opens && look.scenery {
                    things.iter().filter(|t| matches!(t, Thing::Group { .. })).count()
                } else {
                    0
                };
                assert_eq!(s.groups, groups, "{what}: the groups taken in");
                *seen.entry(if pass.took.is_some() { "took a feeler" } else { "hit reaction" }).or_default() += 1;
                for (bit, name) in [
                    (blocked::RIDER, "rider"),
                    (blocked::SOLID, "solid"),
                    (blocked::GROUP, "group"),
                    (blocked::OFF_PATH, "off path"),
                    (blocked::CLIMB, "climb"),
                    (blocked::DROP, "drop"),
                ] {
                    if pass.reasons & bit != 0 {
                        *seen.entry(name).or_default() += 1;
                    }
                }
                if pass.took.is_some_and(|k| k != n / 2) {
                    *seen.entry("turned").or_default() += 1;
                }
            }
            None => {
                assert!(!s.begun, "{what}: the scene walked");
                *seen.entry("idle").or_default() += 1;
            }
        }
        assert_eq!(s.swept, port_scene.swept, "{what}: the objects swept");
        assert_eq!(s.grounds, ground.asked, "{what}: the ground points");
        assert_eq!(s.held, held, "{what}: the segment points");
        if recovery != recovery_in {
            *seen.entry("recovery set").or_default() += 1;
        }
    }
    eprintln!("ai vehicle avoidance: {seen:?}");
    for (name, least) in [
        ("took a feeler", 4000),
        ("hit reaction", 300),
        ("idle", 200),
        ("rider", 300),
        ("solid", 300),
        ("group", 300),
        ("off path", 300),
        ("climb", 300),
        ("drop", 300),
        ("turned", 1000),
        ("recovery set", 200),
    ] {
        let got = seen.get(name).copied().unwrap_or(0);
        assert!(got >= least, "{name}: {got} of {least}: {seen:?}");
    }
}
