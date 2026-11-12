// --- avoidance (S12c) -------------------------------------------------------------------------------
//
// The motocross AI's whole avoidance pass runs in the interpreter, ten thousand times. What it calls
// is hooked: the scene walk (a fake vtable that hands out a generated list of riders and placed
// scenery), the feeler sweep against each rider (it answers at random, and writes random hits into
// the feeler record — which the pass then reads and throws away), the scenery groups' part count, and
// the three track-path queries of the off-the-track test (scripted per case). The race's rider list
// is generated around the rider, its own bike included sometimes so the pass has to skip it.
// Compared each time: the steer direction (all four lanes), the aimed-for speed, the flag word, the
// feeler fan when the flags let it be built (and the fan buffer untouched when not), which objects
// were swept, how many scenery groups were taken in, and the point the off-the-track test asked about.

use super::*;

pub(crate) const AI: u32 = 0x0180_0000;
pub(crate) const OWN: u32 = 0x0180_1000; // its own bike: +0x40, +0x88, +0x8c, +0xbc progress
pub(crate) const RACE: u32 = 0x0180_2000; // +0x34 the rider list, +0x68 the track's path
pub(crate) const SCENE: u32 = 0x0180_3000; // +0x08 its vtable
const SCENE_VT: u32 = 0x0180_3100;
pub(crate) const BUNDLE: u32 = 0x0180_4000; // the feeler bundle: +0x6c record, +0x134/+0x138/+0x13c, +0x150
pub(crate) const FEELER_RECORD: u32 = 0x0180_4400;
pub(crate) const LENGTHS: u32 = 0x0180_4800;
pub(crate) const ORIGINS: u32 = 0x0180_5000;
pub(crate) const DIRS: u32 = 0x0180_6000;
const REC_NORMALS: u32 = 0x0180_7000;
const REC_POINTS: u32 = 0x0180_7800;
const REC_FLAGS: u32 = 0x0180_8000;
const REC_FRACTIONS: u32 = 0x0180_8400;
pub(crate) const ARGS: u32 = 0x0180_9000; // position, heading, steer at +0x00/+0x10/+0x20; speeds at +0x30/+0x34
pub(crate) const IGNORE: u32 = 0x0180_9100;
const WORLD: u32 = 0x0180_a000;
const PATH_AT: u32 = 0x0180_b000; // the located segment, its next (+0x54) and that one's next
const HOOK_BEGIN: u32 = 0x0180_c000;
const HOOK_NEXT: u32 = 0x0180_c008;
const HOOK_END: u32 = 0x0180_c010;
const RIDERS: u32 = 0x0181_0000;
const RIDER_SIZE: u32 = 0x200;
const OBJECTS: u32 = 0x0182_0000;
const MAX_FEELERS: u32 = 16;

impl Pick {
    pub(crate) fn fraction(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub(crate) fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.fraction()
    }
    pub(crate) fn chance(&mut self, n: u32) -> bool {
        self.below(n) == 0
    }
    pub(crate) fn vec(&mut self, r: f32, w: f32) -> Vf {
        Vf::new(self.range(-r, r), self.range(-r, r), self.range(-r, r), w)
    }
}

pub(crate) fn put(ee: &mut ps2_oracle::Ee, at: u32, v: Vf) {
    for (k, b) in v.to_bits().iter().enumerate() {
        ee.set_u32(at + 4 * k as u32, *b);
    }
}

pub(crate) fn got(ee: &ps2_oracle::Ee, at: u32) -> [u32; 4] {
    core::array::from_fn(|k| ee.u32_at(at + 4 * k as u32))
}

/// Which object the scene walk hands out: (address, type, collidable).
type SceneObject = (u32, u8, bool);

#[derive(Default)]
struct Script {
    scene: Vec<u32>,
    at: usize,
    opens: bool,
    begun: bool,
    swept: Vec<u32>,
    groups: usize,
    feelers: u32,
    rays: u64,
    holds: Vec<u64>,
    push: Option<Vf>,
    located: bool,
    asked: Option<[u32; 4]>,
}

#[test]
fn avoidance_matches_over_ten_thousand_passes() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "ai_update_mx",
        "bundle_sweep_object",
        "group_part_count",
        "world_manager",
        "path_locate",
        "path_segment_holds",
        "path_node_direction",
        "curve_table_build",
        "mem_alloc",
    ]) else {
        return;
    };
    let [update, sweep, part_count, world_manager, locate, holds, node_direction, curve_build, mem_alloc] = at[..]
    else {
        unreachable!()
    };
    ee.hooks.insert(mem_alloc, Box::new(|ee| ee.set(2, 0x0100_0000)));
    ee.call(curve_build, &[], &[], 2_000_000_000);
    ee.hooks.remove(&mem_alloc);
    let curve = CurveTable::build();

    let script = Rc::new(RefCell::new(Script::default()));
    let s = script.clone();
    ee.hooks.insert(HOOK_BEGIN, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.begun = true;
        ee.set(2, u64::from(s.opens));
    }));
    let s = script.clone();
    ee.hooks.insert(HOOK_NEXT, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        let o = s.scene.get(s.at).copied().unwrap_or(0);
        s.at += 1;
        ee.set(2, u64::from(o));
    }));
    ee.hooks.insert(HOOK_END, Box::new(|_| {}));
    let s = script.clone();
    ee.hooks.insert(sweep, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.swept.push(ee.r(5) as u32);
        // A random answer, and random hits for the pass to read and drop.
        let mut x = s.rays.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        for k in 0..s.feelers {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            ee.set_u32(REC_FLAGS + 4 * k, (x & 1) as u32);
            for l in 0..3 {
                ee.set_f32(REC_POINTS + 16 * k + 4 * l, ((x >> (8 + 8 * l)) & 0xff) as f32 - 128.0);
            }
        }
        s.rays = x;
        ee.set(2, x & 2);
    }));
    let s = script.clone();
    ee.hooks.insert(part_count, Box::new(move |ee| {
        s.borrow_mut().groups += 1;
        ee.set(2, 0);
    }));
    let s = script.clone();
    ee.hooks.insert(locate, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.asked = Some(got(ee, ee.r(6) as u32));
        let out = ee.r(8) as u32;
        ee.set_u32(out + 4, PATH_AT);
        ee.set(2, u64::from(s.located));
    }));
    let s = script.clone();
    ee.hooks.insert(holds, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        let r = if s.holds.is_empty() { 0 } else { s.holds.remove(0) };
        ee.set(2, r);
    }));
    let s = script.clone();
    ee.hooks.insert(node_direction, Box::new(move |ee| {
        let s = s.borrow();
        if let Some(v) = s.push {
            put(ee, ee.r(6) as u32, v);
        }
        ee.set(2, u64::from(s.push.is_some()));
    }));

    ee.set_u32(world_manager, WORLD);
    ee.set_f32(WORLD + 0xd4, 0.5);
    ee.set_u32(SCENE + 8, SCENE_VT);
    ee.set_u32(SCENE_VT + 0x2c, HOOK_BEGIN);
    ee.set_u32(SCENE_VT + 0x30, HOOK_NEXT);
    ee.set_u32(SCENE_VT + 0x34, HOOK_END);
    ee.set_u32(AI + 0x40, SCENE);
    ee.set_u32(AI + 0xd0, 2);
    ee.set_u32(AI + 0xd4, OWN);
    ee.set_u32(AI + 0x224, 0);
    ee.set_u32(AI + 0x2a0, BUNDLE);
    ee.set_u32(OWN + 0x40, 1);
    ee.set_u32(OWN + 0x88, 1);
    ee.set_u32(OWN + 0x8c, 1);
    ee.set_u32(BUNDLE + 0x6c, FEELER_RECORD);
    ee.set_u32(BUNDLE + 0x134, LENGTHS);
    ee.set_u32(BUNDLE + 0x138, ORIGINS);
    ee.set_u32(BUNDLE + 0x13c, DIRS);
    ee.set_u32(FEELER_RECORD + 0x94, REC_NORMALS);
    ee.set_u32(FEELER_RECORD + 0x98, REC_POINTS);
    ee.set_u32(FEELER_RECORD + 0xa4, REC_FLAGS);
    ee.set_u32(FEELER_RECORD + 0xa8, REC_FRACTIONS);
    ee.set_u32(PATH_AT + 0x54, PATH_AT + 0x100);

    let mut pick = Pick(0x7a11_c0de_5eed_0c12);
    let (mut swerved, mut braked, mut fans, mut edged, mut idle) = (0, 0, 0, 0, 0);
    for case in 0..10_000 {
        let p = &mut pick;
        let defaults = p.chance(4);
        let mut t = AvoidTuning::default();
        if !defaults {
            t = AvoidTuning {
                near_range: F(p.range(2.0, 15.0)),
                far_range: F(p.range(0.0, 45.0)),
                far_radius: F(p.range(1.0, 12.0)),
                far_response: F(p.range(0.0, 1.0)),
                far_approach_limit: F(p.range(0.0, 4.0)),
                far_min_braking_speed: F(p.range(0.0, 40.0)),
                far_braking_response: F(p.range(0.0, 2.0)),
                near_separation: F(p.range(0.0, 1.0)),
                near_response: F(p.range(0.0, 1.0)),
                near_braking_response: F(p.range(0.0, 1.5)),
                passing_speed: F(p.range(-5.0, 15.0)),
                near_min_braking_speed: F(p.range(0.0, 40.0)),
                front_window: F(p.range(-5.0, 20.0)),
                rear_window: F(p.range(-20.0, 5.0)),
                drift_match: F(p.range(-1.0, 1.0)),
                far_heading_cos: F(p.range(-1.0, 1.0)),
                edge_look_ahead: F(p.range(0.0, 20.0)),
                edge_push: F(p.range(0.0, 2.0)),
                feelers: 1 + p.below(MAX_FEELERS),
                feeler_height: F(p.range(0.0, 5.0)),
                feeler_length: F(p.range(10.0, 150.0)),
            };
        }
        let yaw = p.range(-7.0, 7.0);
        let heading = Vf::new(yaw.sin(), p.range(-0.2, 0.2), yaw.cos(), p.range(-1.0, 1.0));
        let me = Me {
            position: p.vec(500.0, 1.0),
            heading,
            velocity: p.vec(30.0, 0.0),
            speed: F(if p.chance(20) { p.range(-5.0, 0.0) } else { p.range(0.01, 60.0) }),
            held_speed: F(p.range(-5.0, 60.0)),
            progress: F(p.range(0.0, 100.0)),
        };
        let mut flags = p.next() & 0xffff_ffff_0000_0000 | p.next() & 0x0000_0000_fffe_00ff;
        for bit in [flag::NO_RIDER_SWEEP, flag::QUIET_AREA, flag::AVOID_DUE, flag::KEEP_ON_TRACK, flag::SKIP_MARKED] {
            flags = if p.chance(2) { flags | bit } else { flags & !bit };
        }
        let reach = if p.chance(3) { 8.0 } else { 40.0 };
        let mut others = Vec::new();
        let mut list = Vec::new();
        let count = p.below(9) as u32;
        let own_at = if p.chance(3) { p.below(count + 1) as u32 } else { u32::MAX };
        for k in 0..count {
            if k == own_at {
                list.push(OWN);
            }
            let o = Other {
                active: !p.chance(8),
                marked: p.chance(3),
                position: Vf::new(
                    me.position.x() + p.range(-reach, reach),
                    me.position.y() + p.range(-3.0, 3.0),
                    me.position.z() + p.range(-reach, reach),
                    p.range(-1.0, 1.0),
                ),
                velocity: if p.chance(4) { Vf::new(0.0, 0.0, 0.0, 0.0) } else { p.vec(30.0, 0.0) },
                heading: p.vec(1.0, 0.0),
                speed: F(if p.chance(4) { 0.0 } else { p.range(0.0, 60.0) }),
                progress: F(p.range(0.0, 100.0)),
            };
            others.push(o);
            list.push(RIDERS + k * RIDER_SIZE);
        }
        if own_at == count {
            list.push(OWN);
        }
        let w = p.range(-1.0, 1.0);
        let steer_in = Steer { direction: p.vec(1.0, w), speed: F(p.range(0.0, 60.0)) };

        // The scene the walk hands out.
        let mut scene: Vec<SceneObject> = Vec::new();
        for k in 0..p.below(6) as u32 {
            let kind = [1u8, 2, 2, 3][p.below(4) as usize];
            scene.push((OBJECTS + k * 0x100, kind, p.chance(2) || kind != 2));
        }
        let ignored: Vec<u32> = scene.iter().filter(|_| p.chance(4)).map(|o| o.0).collect();
        let opens = !p.chance(8);

        // The off-the-track test's answers.
        let located = !p.chance(5);
        let next_next = !p.chance(6);
        let hold = [u64::from(p.chance(3)), u64::from(p.chance(3))];
        let push = (!p.chance(4)).then(|| p.vec(1.0, 0.0));

        // --- into RAM --------------------------------------------------------------------------
        ee.write(AI + 0x30, 8, u128::from(flags));
        put(&mut ee, AI + 0x50, me.velocity);
        ee.set_f32(AI + 0x80, me.held_speed.0);
        ee.set_f32(OWN + 0xbc, me.progress.0);
        ee.set_u32(AI + 0xd8, RACE);
        ee.set_u32(RACE + 0x34, list.first().copied().unwrap_or(0));
        ee.set_u32(RACE + 0x68, 1);
        ee.set_u32(OWN + 4, 1);
        let next_of = |i: usize| list.get(i + 1).copied().unwrap_or(0);
        for (i, &r) in list.iter().enumerate() {
            ee.set_u32(r + 0x1c8, next_of(i));
        }
        for (k, o) in others.iter().enumerate() {
            let r = RIDERS + k as u32 * RIDER_SIZE;
            ee.set_u32(r + 4, u32::from(o.active));
            ee.write(r + 8, 8, if o.marked { 0x8000_0004 } else { 0x8000_0003 });
            put(&mut ee, r + 0x60, o.position);
            put(&mut ee, r + 0x110, o.velocity);
            put(&mut ee, r + 0x120, o.heading);
            ee.set_f32(r + 0x108, o.speed.0);
            ee.set_f32(r + 0xbc, o.progress.0);
        }
        for (f, v) in [
            (0x280, t.near_range),
            (0x284, t.far_range),
            (0x3d8, t.far_radius),
            (0x3dc, t.far_response),
            (0x3e0, t.far_approach_limit),
            (0x3e4, t.far_min_braking_speed),
            (0x3e8, t.far_braking_response),
            (0x3a4, t.near_separation),
            (0x3ac, t.near_response),
            (0x3b0, t.near_braking_response),
            (0x2f8, t.passing_speed),
            (0x3b4, t.near_min_braking_speed),
            (0x3d0, t.front_window),
            (0x3d4, t.rear_window),
            (0x3a8, t.drift_match),
            (0x3ec, t.far_heading_cos),
            (0x288, t.edge_look_ahead),
            (0x3c8, t.edge_push),
            (0x2a8, t.feeler_height),
            (0x2ac, t.feeler_length),
        ] {
            ee.set_f32(AI + f, v.0);
        }
        // The scenery branch's own numbers: read only into what the pass throws away.
        for f in [0x3b8, 0x3bc, 0x3c0, 0x3c4, 0x3cc] {
            ee.set_f32(AI + f, pick.range(0.0, 200.0));
        }
        let p = &mut pick;
        ee.set_u32(AI + 0x2a4, t.feelers as u32);
        for k in 0..MAX_FEELERS {
            put(&mut ee, DIRS + 16 * k, Vf::new(-7.0, -7.0, -7.0, 0.25));
        }
        for &(o, kind, collidable) in &scene {
            ee.write(o + 2, 1, u128::from(kind));
            ee.write(o + 0x31, 1, u128::from(collidable));
            ee.write(o + 8, 2, p.below(4000) as u128);
            ee.write(o + 0xc, 2, p.below(4000) as u128);
        }
        for (k, &o) in ignored.iter().enumerate() {
            ee.set_u32(IGNORE + 4 * k as u32, o);
        }
        ee.set_u32(PATH_AT + 0x154, u32::from(next_next) * (PATH_AT + 0x200));
        put(&mut ee, ARGS, me.position);
        put(&mut ee, ARGS + 0x10, me.heading);
        put(&mut ee, ARGS + 0x20, steer_in.direction);
        ee.set_f32(ARGS + 0x30, me.speed.0);
        ee.set_f32(ARGS + 0x34, steer_in.speed.0);
        *script.borrow_mut() = Script {
            scene: scene.iter().map(|o| o.0).collect(),
            opens,
            feelers: t.feelers as u32,
            holds: hold.to_vec(),
            push,
            located,
            ..Script::default()
        };

        ee.call(
            update,
            &[
                u64::from(AI),
                ignored.len() as u64,
                u64::from(IGNORE),
                u64::from(ARGS),
                u64::from(ARGS + 0x10),
                u64::from(ARGS + 0x20),
                u64::from(ARGS + 0x30),
                u64::from(ARGS + 0x34),
            ],
            &[],
            BUDGET,
        );

        // --- the port ----------------------------------------------------------------------------
        let mut port_flags = flags;
        let mut steer = steer_in;
        let mut asked = None;
        let mut edge = |ahead: Vf| {
            asked = Some(ahead.to_bits());
            let off = located && next_next && hold == [0, 0];
            off.then(|| push.unwrap_or(ahead))
        };
        let fan = ai::avoid(&t, &me, &mut port_flags, &others, Some(&mut edge), &mut steer, &curve);
        let look = ai::look(flags);
        let what = format!("case {case}: flags {flags:#x}, {} riders, scene {scene:?}", others.len());

        assert_eq!(got(&ee, ARGS + 0x20), steer.direction.to_bits(), "{what}: the steer direction");
        assert_eq!(ee.u32_at(ARGS + 0x34), steer.speed.to_bits(), "{what}: the aimed-for speed");
        assert_eq!(ee.read(AI + 0x30, 8) as u64, port_flags, "{what}: the flags");
        let s = script.borrow();
        let ran = me.speed > F(0.0);
        let walked = ran && look.fan;
        assert_eq!(s.begun, walked, "{what}: the scene walk");
        match &fan {
            Some(fan) => {
                fans += 1;
                for (k, Feeler { origin, direction, length }) in fan.iter().enumerate() {
                    let k = k as u32;
                    assert_eq!(got(&ee, DIRS + 16 * k)[..3], direction.to_bits()[..3], "{what}: feeler {k}'s direction");
                    assert_eq!(got(&ee, ORIGINS + 16 * k), origin.to_bits(), "{what}: feeler {k}'s origin");
                    assert_eq!(ee.u32_at(LENGTHS + 4 * k), length.to_bits(), "{what}: feeler {k}'s length");
                }
                assert_eq!(got(&ee, BUNDLE + 0x150), me.position.to_bits(), "{what}: the bundle's centre");
            }
            None => {
                for k in 0..MAX_FEELERS {
                    assert_eq!(got(&ee, DIRS + 16 * k), Vf::new(-7.0, -7.0, -7.0, 0.25).to_bits(), "{what}: a fan built");
                }
            }
        }
        let walked = walked && opens;
        let want_swept: Vec<u32> = if walked && look.riders {
            scene.iter().filter(|o| o.1 == 2 && o.2 && !ignored.contains(&o.0)).map(|o| o.0).collect()
        } else {
            Vec::new()
        };
        assert_eq!(s.swept, want_swept, "{what}: the riders swept");
        let want_groups = if walked && look.scenery { scene.iter().filter(|o| o.1 == 3).count() } else { 0 };
        assert_eq!(s.groups, want_groups, "{what}: the scenery taken in");
        let edge_on = ran && flags & flag::KEEP_ON_TRACK != 0;
        assert_eq!(s.asked, if edge_on { asked } else { None }, "{what}: the point ahead");
        drop(s);

        idle += usize::from(!ran);
        edged += usize::from(edge_on && asked.is_some() && located && next_next && hold == [0, 0]);
        braked += usize::from(steer.speed != steer_in.speed);
        swerved += usize::from(ran && others.iter().any(|o| o.active));
    }
    eprintln!("ai avoidance: {fans} fans, {braked} brakes, {edged} pushed back, {swerved} with riders, {idle} idle");
    assert!(fans > 3000 && braked > 500 && edged > 300 && idle > 200, "fans {fans}, brakes {braked}, edges {edged}, idle {idle}");
}
