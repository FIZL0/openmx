// --- S282b: the tick's tail ------------------------------------------------------------------------

use super::*;

const TL_AI: u32 = 0x0180_0000;
const TL_VT: u32 = 0x0180_1000;
const TL_IFACE: u32 = 0x0180_2000;
const TL_IFACE_VT: u32 = 0x0180_2400;
const TL_BIKE: u32 = 0x0180_3000;
/// Not code: the vtable slots the tail must not call land here, and panic.
const TL_HOOKS: u32 = 0x0186_0000;
const TL_AVOID: u32 = 0x0187_0000;
const TL_PLAN: u32 = 0x0187_0010;
const TL_ASSIST: u32 = 0x0187_0020;

/// What one of the tail's calls did: the brain as it saw it, and what it changed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TlCall {
    seen: fw::Brain,
    /// Applied by the call (the original's hook writes them, the port's closure too).
    set_heading: Option<Vf>,
    set_speed: Option<F>,
    flip: u64,
}

fn tl_brain(ee: &ps2_oracle::Ee) -> fw::Brain {
    fw::Brain {
        flags: ee.read(TL_AI + 0x30, 8) as u64,
        heading: sk_vf(ee, TL_AI + 0x1d0),
        speed: sk_f(ee, TL_AI + 0x1e0),
        assist_x: sk_f(ee, TL_AI + 0x1c0),
        assist_z: sk_f(ee, TL_AI + 0x1c8),
    }
}

fn tl_brain_bits(b: &fw::Brain) -> [u64; 8] {
    let h = b.heading.to_bits();
    [b.flags, h[0].into(), h[1].into(), h[2].into(), h[3].into(), b.speed.0.to_bits().into(), b.assist_x.0.to_bits().into(), b.assist_z.0.to_bits().into()]
}

fn tl_apply(b: &mut fw::Brain, c: &TlCall) {
    if let Some(h) = c.set_heading {
        b.heading = h;
    }
    if let Some(s) = c.set_speed {
        b.speed = s;
    }
    b.flags ^= c.flip;
}

fn tl_gen_call(p: &mut Pick) -> TlCall {
    let w = p.range(-1.0, 1.0);
    TlCall {
        seen: fw::Brain::default(),
        set_heading: p.chance(2).then(|| p.vec(1.0, w)),
        set_speed: p.chance(2).then(|| F(p.range(0.0, 150.0))),
        flip: if p.chance(3) { [ai::flag::ON_GROUND, fw::ASSIST, fw::AVOIDS, 0x8000][p.below(4) as usize] } else { 0 },
    }
}

/// A unit-ish vector on the ground, `y` small, `w` any.
fn tl_dir(p: &mut Pick) -> Vf {
    let a = p.range(-3.2, 3.2);
    let (s, c) = a.sin_cos();
    let w = p.range(-1.0, 1.0);
    Vf::new(c, p.range(-0.2, 0.2), s, w)
}

#[test]
fn the_ticks_tail_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_seek_tail", "ai_tick_end"]) else { return };
    let [tail, end] = at[..] else { unreachable!() };
    for slot in (0..0x100).step_by(4) {
        for (vt, name) in [(TL_VT, "brain"), (TL_IFACE_VT, "interface")] {
            let target = match (vt, slot) {
                (TL_VT, 0x74) => TL_AVOID,
                (TL_VT, 0x84) => TL_PLAN,
                (TL_IFACE_VT, 0x18) => TL_ASSIST,
                _ => {
                    let h = TL_HOOKS + slot + if vt == TL_VT { 0 } else { 0x100 };
                    ee.hooks.insert(h, Box::new(move |_| panic!("the tail called the {name}'s vtable slot {slot:#x}")));
                    h
                }
            };
            ee.set_u32(vt + slot, target);
        }
    }
    // The three calls the tail makes: each logs the brain as it saw it and its arguments, then
    // applies the change the case asks of it.
    let todo: Rc<RefCell<[TlCall; 3]>> = Rc::default();
    let log: Rc<RefCell<Vec<(usize, TlCall, Vec<u64>, u32)>>> = Rc::default();
    for (k, addr) in [TL_AVOID, TL_PLAN, TL_ASSIST].into_iter().enumerate() {
        let (todo, log) = (todo.clone(), log.clone());
        ee.hooks.insert(addr, Box::new(move |ee| {
            let mut c = todo.borrow()[k];
            c.seen = tl_brain(ee);
            let args = (4..12).map(|r| ee.r(r) as u32 as u64).collect();
            log.borrow_mut().push((k, c, args, ee.f(12).to_bits()));
            let mut b = c.seen;
            tl_apply(&mut b, &c);
            put(ee, TL_AI + 0x1d0, b.heading);
            ee.set_f32(TL_AI + 0x1e0, b.speed.0);
            ee.write(TL_AI + 0x30, 8, u128::from(b.flags));
            ee.set(2, 1);
        }));
    }
    ee.hooks.insert(end, Box::new(|ee| ee.set(31, ps2_oracle::DONE as i32 as i64 as u64)));

    let mut p = Pick(0x5282_b7a1_1ee7_c0de);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    for case in 0..10_000 {
        ee.load(TL_AI, &SK_ZERO[..0x1000]);
        ee.load(TL_BIKE, &SK_ZERO[..0x100]);
        ee.load(TL_IFACE, &SK_ZERO[..0x200]);
        ee.set_u32(TL_AI + 8, TL_VT);
        ee.set_u32(TL_IFACE + 8, TL_IFACE_VT);
        ee.set_u32(TL_AI + 0x40, TL_IFACE);

        let mut flags = p.next() & 0xffff_ffff_0000_f000;
        for bit in [fw::AVOIDS, ai::flag::ON_GROUND, ai::flag::NO_RIDER_SWEEP, fw::ASSIST, fw::SWEEP_BY_BIKE] {
            if p.chance(2) {
                flags |= bit;
            }
        }
        let forward = tl_dir(&mut p);
        let heading = match p.below(4) {
            0 => forward,
            1 => tl_dir(&mut p),
            2 => {
                let w = p.range(-1.0, 1.0);
                p.vec(1.5, w)
            }
            _ => {
                // near the 45° edge
                let f = forward;
                let a = p.range(0.70, 0.72);
                let b = (1.0 - a * a).sqrt();
                Vf::new(f.x() * a - f.z() * b, f.y(), f.z() * a + f.x() * b, f.w())
            }
        };
        let velocity = match p.below(4) {
            0 => {
                let w = p.range(-1.0, 1.0);
                p.vec(60.0, w)
            }
            1 => sk_along(heading, p.range(0.0, 60.0)),
            _ => sk_along(tl_dir(&mut p), p.range(0.0, 60.0)),
        };
        let len = (velocity.x() * velocity.x() + velocity.y() * velocity.y() + velocity.z() * velocity.z()).sqrt();
        let own_speed = match p.below(8) {
            0 => 0.0,
            1 => -p.range(0.0, 5.0),
            2 => p.range(0.0, 80.0),
            _ => len,
        };
        let air_cone = match p.below(5) {
            0 => p.range(-1.0, 1.0),
            1 => 1.0,
            _ => p.range(0.3, 1.0),
        };
        let bike_word = match p.below(3) {
            0 => None,
            1 => Some(0),
            _ => Some(p.next() as u32 | 1),
        };
        let ride = fw::Tail { forward, velocity, own_speed: F(own_speed), air_cone: F(air_cone), bike_word };
        let mut brain = fw::Brain {
            flags,
            heading,
            speed: F(p.range(0.0, 150.0)),
            assist_x: F(p.range(-9.0, 9.0)),
            assist_z: F(p.range(-9.0, 9.0)),
        };
        let calls = [tl_gen_call(&mut p), tl_gen_call(&mut p), tl_gen_call(&mut p)];
        *todo.borrow_mut() = calls;
        log.borrow_mut().clear();
        let dt = p.range(0.01, 0.05);

        ee.write(TL_AI + 0x30, 8, u128::from(brain.flags));
        put(&mut ee, TL_AI + 0x1d0, brain.heading);
        ee.set_f32(TL_AI + 0x1e0, brain.speed.0);
        ee.set_f32(TL_AI + 0x1c0, brain.assist_x.0);
        ee.set_f32(TL_AI + 0x1c8, brain.assist_z.0);
        put(&mut ee, TL_AI + 0xb0, ride.forward);
        put(&mut ee, TL_AI + 0x50, ride.velocity);
        ee.set_f32(TL_AI + 0x80, own_speed);
        ee.set_f32(TL_IFACE + 0x150, air_cone);
        ee.set_u32(TL_AI + 0x150, p.next() as u32);
        ee.set_u32(TL_AI + 0x154, p.next() as u32);
        if let Some(w) = bike_word {
            ee.set_u32(TL_AI + 0xd4, TL_BIKE);
            ee.set_u32(TL_BIKE + 0x40, w);
        }
        ee.set(19, u64::from(TL_AI));
        ee.set(3, brain.flags);
        ee.set_f(22, dt);
        ee.call(tail, &[], &[], BUDGET);
        assert_eq!(ee.v0() as u32, 1, "case {case}: the answer");

        let before = brain;
        let mut mine: Vec<(usize, TlCall)> = Vec::new();
        let mut plan_log: Vec<fw::Brain> = Vec::new();
        let mut assist_log: Vec<fw::Brain> = Vec::new();
        fw::tail(
            &mut brain,
            &ride,
            &mut |b| {
                let mut c = calls[0];
                c.seen = *b;
                mine.push((0, c));
                tl_apply(b, &c);
            },
            &mut |b| {
                plan_log.push(*b);
                tl_apply(b, &calls[1]);
            },
            &mut |b| {
                assist_log.push(*b);
                tl_apply(b, &calls[2]);
            },
        );
        let what = format!("case {case}: {before:?} {ride:?} calls {calls:?}");
        let theirs = tl_brain(&ee);
        assert_eq!(tl_brain_bits(&theirs), tl_brain_bits(&brain), "{what}: the brain\n original {theirs:?}\n port     {brain:?}");

        let log = log.borrow();
        let order: Vec<usize> = log.iter().map(|e| e.0).collect();
        let mut want_order: Vec<usize> = mine.iter().map(|e| e.0).collect();
        want_order.extend(std::iter::repeat_n(1, plan_log.len()));
        want_order.extend(std::iter::repeat_n(2, assist_log.len()));
        assert_eq!(order, want_order, "{what}: the calls");
        let mut port_seen = mine.iter().map(|e| e.1.seen).chain(plan_log.iter().copied()).chain(assist_log.iter().copied());
        for (k, c, args, f12) in log.iter() {
            let ours = port_seen.next().unwrap();
            assert_eq!(tl_brain_bits(&c.seen), tl_brain_bits(&ours), "{what}: call {k} saw\n original {:?}\n port     {ours:?}", c.seen);
            match k {
                0 => assert_eq!(
                    args[..],
                    [TL_AI, ee.u32_at(TL_AI + 0x150), ee.u32_at(TL_AI + 0x154), TL_AI + 0xc0, TL_AI + 0x1d0, TL_AI + 0x1d0, TL_AI + 0x1e0, TL_AI + 0x1e0].map(u64::from),
                    "{what}: the avoidance's arguments"
                ),
                1 => {
                    assert_eq!(args[0], u64::from(TL_AI), "{what}: the plan's brain");
                    assert_eq!(*f12, dt.to_bits(), "{what}: the plan's dt");
                }
                _ => {
                    assert_eq!(args[0], u64::from(TL_IFACE), "{what}: the assist's interface");
                    assert_eq!(*f12, dt.to_bits(), "{what}: the assist's dt");
                }
            }
        }

        let mut count = |k: &'static str| *seen.entry(k).or_default() += 1;
        if before.flags & fw::AVOIDS != 0 {
            count("avoids");
            if mine[0].1.seen.heading != before.heading {
                count("turned to 45");
            }
        }
        if (before.flags ^ brain.flags) & ai::flag::NO_RIDER_SWEEP != 0 {
            count("sweep flipped");
        }
        let after_avoid = mine.first().map_or(before, |(_, c)| {
            let mut b = c.seen;
            tl_apply(&mut b, c);
            b
        });
        if plan_log[0].heading != after_avoid.heading {
            count("air cone");
        }
        if !assist_log.is_empty() {
            count("assist");
        }
    }
    eprintln!("tail: {seen:?}");
    for (k, n) in [("avoids", 3000), ("turned to 45", 800), ("sweep flipped", 800), ("air cone", 800), ("assist", 3000)] {
        assert!(seen.get(k).copied().unwrap_or(0) >= n, "only {:?} ticks went through {k}", seen.get(k));
    }
}
