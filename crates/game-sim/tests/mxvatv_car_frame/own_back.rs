// MD154 — the car's respawn, unhooked: the put-back with its ground reset and orient, then each
// tire's and the engine's reset. MD106b's test (`own_calls.rs`) runs with it unhooked, and what it
// hands out hooked here; a test of its own calls it alone over its branches.

use super::*;
use atv_car::Back;
use game_sim::quad::Part;

/// The respawn, and what it hands out: the ghost's start (the car's vt `+0x3a0`), the trail's
/// reset, the database's unhook, the node's rehang, a collidable's attach and turn-on (its object's
/// vt `+0x20`), and each tire's and the engine's reset (research/oracle.txt `# MD154`).
const RESPAWN: u32 = 0x004c_ab10;
const GHOST_START: u32 = 0x004c_d4d0;
const TRAIL_RESET: u32 = 0x004f_a760;
const UNHOOK: u32 = 0x0018_d540;
const REHANG: u32 = 0x0019_6c10;
const ATTACH: u32 = 0x0011_43e0;
const PART_ON: u32 = 0x0012_8080;
const TIRE_RESET: u32 = 0x004b_4e10;
const ENGINE_RESET: u32 = 0x004b_3f10;

/// The car's respawn words.
mod back {
    pub const AT: u32 = 0x50;
    pub const SPOT: u32 = 0xd40;
    pub const RUN_SPOT: u32 = 0xd70;
    pub const RUN: u32 = 0x680;
    /// The run's own three words.
    pub const RUN_WORDS: u32 = 0x94;
    pub const TERRAIN: u32 = 0x3b4;
    pub const GHOST: u32 = 0xde8;
    pub const GHOST_TIME: u32 = 0xdec;
    pub const TRAIL: u32 = 0x10dc;
    /// The engine's and the four tires' pointers.
    pub const PARTS: [u32; 5] = [0x1250, 0x1254, 0x1258, 0x125c, 0x1260];
    /// The collidables the ground reset turns on: the riding probe, the frame mesh, the sweep.
    pub const COLLIDABLES: [(u32, super::Part); 3] = [(0xcc, super::Part::Riding), (0xd0, super::Part::Frame), (0xd4, super::Part::Sweep)];
    /// The node's scene, and the world the collidables attach in, and its list.
    pub const SCENE: u32 = 0xb0;
    pub const WORLD: u32 = 0xb4;
    pub const LIST: u32 = 0xc0;
    /// The body's staged position, velocity, spin and orientation.
    pub const STAGED: [u32; 4] = [0x40, 0x50, 0x160, 0x180];
}

pub(crate) fn read_back(ee: &Ee, p: &Ptrs) -> Back {
    let c = |o: u32| car_at() + o;
    let quads = |o: u32| std::array::from_fn(|k| get_v(ee, c(o) + 0x10 * k as u32));
    let words = |a: u32| std::array::from_fn(|k| ee.u32_at(a + 4 * k as u32));
    let run = ee.u32_at(c(back::RUN));
    let staged = back::STAGED.map(|o| get_v(ee, p.body + o));
    Back {
        spot: quads(back::SPOT),
        at: quads(back::AT),
        run_spot: words(c(back::RUN_SPOT)),
        run: (run != 0).then(|| words(run + back::RUN_WORDS)),
        terrain: ee.u32_at(c(back::TERRAIN)) != 0,
        ghost: F(ee.f32_at(c(back::GHOST))),
        ghost_time: F(ee.f32_at(c(back::GHOST_TIME))),
        trail: ee.u32_at(c(back::TRAIL)) != 0,
        parts: back::PARTS.iter().all(|&o| ee.u32_at(c(o)) != 0),
        staged_position: staged[0],
        staged_velocity: staged[1],
        staged_spin: staged[2],
        staged_quat: staged[3],
    }
}

/// Every word [`read_back`] reads, where it lives; the pointers it holds as switches take the
/// image's (`ee`'s) back. Without the parts, one of the five (by the spot's `x`) is cleared.
pub(crate) fn back_words(ee: &Ee, b: &Back, p: &Ptrs) -> Vec<(u32, u32)> {
    let c = |o: u32| car_at() + o;
    let mut out = Vec::new();
    let mut vs: Vec<(u32, Vf)> = Vec::new();
    for k in 0..3u32 {
        vs.push((c(back::SPOT) + 0x10 * k, b.spot[k as usize]));
        vs.push((c(back::AT) + 0x10 * k, b.at[k as usize]));
    }
    for (o, v) in back::STAGED.into_iter().zip([b.staged_position, b.staged_velocity, b.staged_spin, b.staged_quat]) {
        vs.push((p.body + o, v));
    }
    let run = ee.u32_at(c(back::RUN));
    for k in 0..3u32 {
        out.push((c(back::RUN_SPOT) + 4 * k, b.run_spot[k as usize]));
        if let Some(r) = b.run {
            out.push((run + back::RUN_WORDS + 4 * k, r[k as usize]));
        }
    }
    let kept = |on: bool, o: u32| (c(o), if on { ee.u32_at(c(o)) } else { 0 });
    out.push(kept(b.run.is_some(), back::RUN));
    out.push(kept(b.terrain, back::TERRAIN));
    out.push(kept(b.trail, back::TRAIL));
    let gone = b.spot[0].x().to_bits() as usize % 5;
    for (k, &o) in back::PARTS.iter().enumerate() {
        out.push(kept(b.parts || k != gone, o));
    }
    out.push((c(back::GHOST), b.ghost.to_bits()));
    out.push((c(back::GHOST_TIME), b.ghost_time.to_bits()));
    for (a, v) in vs {
        for (k, x) in v.to_bits().into_iter().enumerate() {
            out.push((a + 4 * k as u32, x));
        }
    }
    out
}

/// The block the respawn writes in beyond MD106b's: the run's words.
pub(crate) fn back_blocks(ee: &Ee) -> Vec<(u32, u32)> {
    match ee.u32_at(car_at() + back::RUN) {
        0 => Vec::new(),
        run => vec![(run + back::RUN_WORDS, 0xc)],
    }
}

/// Hook what the respawn hands out (the respawn itself runs): each asserts what it was handed, on
/// the car's words as they are, and logs it.
pub(crate) fn hook_back(ee: &mut Ee, p: Ptrs, log: Shared<Vec<Call>>) {
    let c = |o: u32| car_at() + o;
    let part_of = move |ee: &Ee, a: u32| {
        back::COLLIDABLES.iter().find(|&&(o, _)| ee.u32_at(c(o)) == a && a != 0).unwrap_or_else(|| panic!("no collidable at {a:#x}")).1
    };
    let l = log.clone();
    ee.hooks.insert(GHOST_START, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, CAR, "the ghost started on something else, from {:#x}", ee.r(31));
        l.borrow_mut().push(Call::GhostStart);
    }));
    let l = log.clone();
    ee.hooks.insert(TRAIL_RESET, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, ee.u32_at(c(back::TRAIL)), "a trail reset from {:#x}", ee.r(31));
        l.borrow_mut().push(Call::TrailReset);
    }));
    let l = log.clone();
    ee.hooks.insert(UNHOOK, Box::new(move |ee| {
        let a = ee.r(4) as u32;
        let call = if a == p.node { Call::NodeUnhook } else { Call::PartUnhook(part_of(ee, a.wrapping_sub(0x18))) };
        l.borrow_mut().push(call);
    }));
    let l = log.clone();
    ee.hooks.insert(REHANG, Box::new(move |ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5) as u32, ee.r(6), ee.r(7)), (p.node, ee.u32_at(c(back::SCENE)), 1, 1), "the node's rehang");
        l.borrow_mut().push(Call::NodeRehang);
    }));
    let l = log.clone();
    ee.hooks.insert(ATTACH, Box::new(move |ee| {
        let (world, list) = (ee.u32_at(c(back::WORLD)), ee.u32_at(c(back::LIST)));
        assert_eq!((ee.r(5) as u32, ee.r(6) as u32, ee.r(7)), (world, list, 1), "a collidable's attach");
        let part = part_of(ee, ee.r(4) as u32);
        l.borrow_mut().push(Call::PartAttach(part));
    }));
    let l = log.clone();
    ee.hooks.insert(PART_ON, Box::new(move |ee| {
        let part = part_of(ee, (ee.r(4) as u32).wrapping_sub(0x24));
        l.borrow_mut().push(Call::PartOn(part));
    }));
    let l = log.clone();
    ee.hooks.insert(TIRE_RESET, Box::new(move |ee| {
        let k = p.tires.iter().position(|&t| t == ee.r(4) as u32).unwrap_or_else(|| panic!("a tire reset on {:#x}", ee.r(4)));
        l.borrow_mut().push(Call::TireReset(k));
    }));
    let l = log;
    ee.hooks.insert(ENGINE_RESET, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, p.engine, "an engine reset on something else");
        l.borrow_mut().push(Call::EngineReset);
    }));
}

/// A generated state of the respawn's words, as the frame test runs it: the spot round the body
/// (its up near the world's), the ground's normal the query answers, and the ghost's length; the
/// working copies, the ghost's clock and the staged words, which the respawn overwrites, anything.
pub(crate) fn generate_back(r: &mut Rng, v: &mut atv_car::Vehicle, body: &Body, s: &mut Script) {
    let Some(b) = &mut v.back else { return };
    let w = |r: &mut Rng| if r.chance(0.5) { 1.0 } else { r.range(-2.0, 2.0) };
    let p = body.position;
    b.spot[0] = Vf::new(p.x() + r.range(-30.0, 30.0), p.y() + r.range(-5.0, 20.0), p.z() + r.range(-30.0, 30.0), w(r));
    b.spot[1] = Vf::new(r.range(-1.0, 1.0), r.range(-0.3, 0.3), r.range(-1.0, 1.0), w(r));
    b.spot[2] = Vf::new(r.range(-0.4, 0.4), r.range(0.5, 1.2), r.range(-0.4, 0.4), w(r));
    for a in &mut b.at {
        let w = r.range(-2.0, 2.0);
        *a = r.vec(50.0, w);
    }
    (b.staged_position, b.staged_velocity, b.staged_spin, b.staged_quat) = (r.vec(50.0, 1.0), r.vec(30.0, 0.0), r.vec(3.0, 0.0), r.quat());
    b.ghost = F(r.range(-1.0, 5.0));
    b.ghost_time = F(r.range(0.0, 5.0));
    s.back_normal = Vf::new(r.range(-0.4, 0.4), r.range(0.6, 1.0), r.range(-0.4, 0.4), w(r));
}

/// More of the respawn's branches, for the test that calls it alone: the run's spot set or not (one
/// word or both), no run, no terrain, no trail, a part missing, no riding probe, no frame mesh.
fn widen_back(r: &mut Rng, v: &mut atv_car::Vehicle) {
    let b = v.back.as_mut().expect("a respawn's words");
    for x in &mut b.run_spot[..2] {
        *x = if r.chance(0.3) { 0 } else { r.word() | 1 };
    }
    b.run_spot[2] = r.word();
    b.run = r.chance(0.8).then(|| [r.word(), r.word(), r.word()]);
    b.terrain = r.chance(0.7);
    b.trail = r.chance(0.7);
    b.parts = r.chance(0.8);
    if r.chance(0.2) {
        v.collidables.riding = None;
    }
    if r.chance(0.2) {
        v.collidables.frame = None;
    }
}

/// **The car's respawn is the original's** (MD154): MXVehicle's respawn called alone, unhooked,
/// against [`atv_car::Own`] answering [`Call::Respawn`], 2,000 generated states: the car, its body,
/// tires, nodes, collidables and respawn words word for word, the same calls handed out in the same
/// order, nothing else written in those blocks, and every branch seen. (MD106b's frame test reaches
/// it too, from the unwreck and from outside the world.)
#[test]
fn the_cars_respawn_is_the_originals() {
    let Some(pristine) = image() else { return };
    let Some(mut ee) = image() else { return };
    let p = ptrs(&ee);
    let o = own_ptrs(&ee, &p);
    let image_car = read_car(&ee, &p);
    let image_body = seed::body(&|a| ee.u32_at(a), p.body);
    let mut image_v = read_vehicle(&ee, &p, &o);
    image_v.back = Some(read_back(&ee, &p));
    let b = image_v.back.expect("read");
    assert!(b.terrain && b.trail && b.parts && b.run.is_some(), "the image's terrain, trail, parts and run");
    assert!(image_v.scene.nodes[0].parent.is_none(), "the image's car's node has a parent");
    let log: Shared<Vec<Call>> = Rc::default();
    let script: Shared<Script> = Rc::default();
    let rng: Shared<game_sim::rng::Rng> = Rc::default();
    hook(&mut ee, p, log.clone(), script.clone());
    hook_own(&mut ee, p, &o, log.clone());
    hook_hand_outs(&mut ee, p, &o, log.clone(), rng.clone());
    hook_back(&mut ee, p, log.clone());
    let mut r = Rng(0x0154_b0b0_0000_0001);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    let mut blocks: Vec<(u32, u32)> = [(CAR, 0x1500), (p.body, 0x230)].into_iter().chain(p.tires.map(|t| (t, 0x200))).collect();
    blocks.extend(o.nodes.iter().map(|&n| (n, 0x110)));
    blocks.extend(hand_blocks(&ee, &p));
    blocks.extend(back_blocks(&ee));
    const CASES: usize = 2000;
    for case in 0..CASES {
        let (mut car, body, _) = generate(&mut r, &image_car, &image_body, &p);
        let mut s = Script::new(&mut r);
        let mut v = generate_own(&mut r, &image_v, &mut car, &body, &mut s);
        widen_back(&mut r, &mut v);
        let before = [car_words(&car, &p), seed::body_words(&body, p.body), vehicle_words(&pristine, &v, &p, &o)].concat();

        // The original.
        poke(&mut ee, &before);
        *script.borrow_mut() = s.clone();
        *rng.borrow_mut() = v.rng;
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(RESPAWN, &[u64::from(CAR)], &[], 5_000_000);
        let written = ee.written_ranges();

        // The port.
        let (mut pc, mut pb, mut pv) = (car, body, v.clone());
        let mut port = Port { log: Vec::new(), script: s };
        atv_car::Own { v: &mut pv, rest: &mut port }.call(Call::Respawn, &mut pc, &mut pb);
        let after = [car_words(&pc, &p), seed::body_words(&pb, p.body), vehicle_words(&pristine, &pv, &p, &o)].concat();

        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .map(|&(a, x)| format!("  {a:#x}: port {x:#010x} ({}), original {:#010x} ({})", f32::from_bits(x), ee.u32_at(a), ee.f32_at(a)))
            .collect();
        let theirs = log.borrow().iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        let ours = port.log.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        assert!(bad.is_empty(), "case {case}: {} words differ\n{}\ncalls {ours:?}", bad.len(), bad.join("\n"));
        assert_eq!(ours, theirs, "case {case}: the calls");
        let compared: std::collections::BTreeSet<u32> = after.iter().map(|&(a, _)| a).collect();
        for &(from, to) in &written {
            for a in (from & !3..to).step_by(4) {
                let ours = blocks.iter().any(|&(b, n)| (b..b + n).contains(&a));
                assert!(!ours || compared.contains(&a), "case {case}: the original wrote {a:#x}, which is not compared");
            }
        }

        let b = v.back.expect("generated");
        let c = v.collidables;
        see("on the ground", b.terrain);
        see("no terrain", !b.terrain);
        see("run moved", b.run.is_some() && b.run_spot[0] != 0 && b.run_spot[1] != 0);
        see("run kept", b.run.is_some() && (b.run_spot[0] == 0 || b.run_spot[1] == 0));
        see("no run", b.run.is_none());
        see("trail", b.trail);
        see("no trail", !b.trail);
        see("parts reset", b.parts);
        see("a part missing", !b.parts);
        see("node unhooked", v.flags & 0x02 != 0);
        see("node kept", v.flags & 0x02 == 0);
        see("sync on", c.sync);
        see("sync off", !c.sync);
        see("no riding probe", c.riding.is_none());
        see("no frame", c.frame.is_none());
        see("frame synced", has_moved(&ours, "Frame"));
        see("riding probe synced", has_moved(&ours, "Riding"));

        for (from, to) in written {
            for a in from..to {
                ee.write(a, 1, u128::from(pristine.u8_at(a)));
            }
        }
    }
    eprintln!("MD154: {CASES} respawns, seen {seen:?}");
    for (what, &k) in &seen {
        assert!(k >= 30, "{what} seen only {k} times");
    }
    assert_eq!(seen.len(), 17, "a branch was never seen: {seen:?}");
}

fn has_moved(calls: &[String], part: &str) -> bool {
    calls.iter().any(|c| c.starts_with(&format!("PartMove {{ part: {part}")))
}

/// **A tire's and the engine's reset are the original's** (MD154): each called alone on the image's
/// four tires and engine, their reset words drawn anything first, 200 times, against [`atv_car::tire_reset`]
/// and [`atv_car::engine_reset`]: every tire and engine word the port holds, and nothing else
/// written.
#[test]
fn the_respawns_tire_and_engine_resets_are_the_originals() {
    let Some(mut ee) = image() else { return };
    let p = ptrs(&ee);
    let mut r = Rng(0x0154_7e7e_0000_0001);
    const ZEROED: [u32; 11] = [0xa4, 0xa8, 0x194, 0x1fc, 0x200, 0x1e0, 0xcc, 0xd4, 0x204, 0x19c, 0x1a0];
    const QUADS: [u32; 8] = [0x130, 0x160, 0x180, 0x210, 0x230, 0x110, 0x120, 0x1c0];
    for case in 0..200 {
        for k in 0..4 {
            let t = p.tires[k];
            for o in ZEROED {
                ee.set_u32(t + o, if o == 0xa4 || o == 0xa8 { r.word() & 1 } else { r.word() });
            }
            for o in QUADS {
                let w = r.range(-2.0, 2.0);
                put_v(&mut ee, t + o, r.vec(100.0, w));
            }
            let (mut car, keys, mut state) = (read_car(&ee, &p), read_tire_keys(&ee, t), read_tire_state(&ee, t));
            ee.set(28, GP);
            ee.call(TIRE_RESET, &[u64::from(t)], &[], 10_000);
            let written = ee.written_ranges();
            atv_car::tire_reset(&mut car.tires[k], &mut state);
            let after = [car_words(&car, &p), tire_words(&keys, &state, t)].concat();
            let bad: Vec<String> = after.iter().filter(|&&(a, x)| ee.u32_at(a) != x).map(|&(a, x)| format!("  tire +{:#x}: port {x:#010x}, original {:#010x}", a.wrapping_sub(t), ee.u32_at(a))).collect();
            assert!(bad.is_empty(), "case {case}, tire {k}:\n{}", bad.join("\n"));
            let compared: std::collections::BTreeSet<u32> = after.iter().map(|&(a, _)| a).collect();
            for &(from, to) in &written {
                for a in (from & !3..to).step_by(4) {
                    assert!(compared.contains(&a), "case {case}, tire {k}: the original wrote tire +{:#x}, which is not compared", a.wrapping_sub(t));
                }
            }
        }
        let e = p.engine;
        for o in [engine::RPM, engine::WHEEL_RPM, engine::GEAR] {
            ee.set_u32(e + o, r.word());
        }
        let mut s = read_engine_state(&ee, &p);
        ee.call(ENGINE_RESET, &[u64::from(e)], &[], 1_000);
        let written = ee.written_ranges();
        atv_car::engine_reset(&mut s);
        let after = [(e + engine::RPM, s.rpm.to_bits()), (e + engine::WHEEL_RPM, s.wheel_rpm.to_bits()), (e + engine::GEAR, s.gear as u32)];
        for (a, x) in after {
            assert_eq!(ee.u32_at(a), x, "case {case}: engine +{:#x}", a - e);
        }
        for &(from, to) in &written {
            for a in (from & !3..to).step_by(4) {
                assert!(after.iter().any(|&(b, _)| b == a), "case {case}: the original wrote engine +{:#x}", a - e);
            }
        }
    }
}
