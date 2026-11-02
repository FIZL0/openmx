//! V104: the toy trigger against the original, on the boot ELF.
//!
//! The strike: a ray probe's tree, made up, against a box shape, through the ray class's box strike
//! (`ray_tree_box_strike`) and through `creature::probe_strikes_box`.
//!
//! The contact: one made-up trigger, its shape, its actions and up to three actors in scratch RAM,
//! run a tick at a time through the original's trigger update and contact callback, and the same
//! trigger through `Creatures::step_among`. The actors are segments placed so that each tick each one
//! is struck or not and kept or not in any of the four ways; the original's exit test is hooked to
//! answer what the port's `Trigger::keeps` says of that actor, `rand` to draw from a copy of the
//! port's generator, and the actions' run to count its calls. After every tick the two must agree on
//! the touch bits, the fired bit, the count, the contact list slot by slot, every action's timer and
//! pending actor, and how many times each action ran coming in and going out.
//!
//! Without the disc the tests say so and pass.

use game_render::creature::{self, Action, Actor, Creatures, Rays, Trigger};
use game_render::mat::v3;
use game_sim::ray_probe::{RayProbe, Segment};
use game_sim::rng::Rng;
use game_sim::track::Node;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, skip};
use std::cell::RefCell;
use std::rc::Rc;

struct Xs(u32);

impl Xs {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    /// In -1..1.
    fn signed(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

fn put(ee: &mut Ee, at: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(at + 4 * k as u32, v.0[k]);
    }
}

/// A rotation off a random unit quaternion, its rows right, up, look, then `origin`.
fn frame(xs: &mut Xs, origin: Vf) -> [Vf; 4] {
    let q = [xs.signed(), xs.signed(), xs.signed(), xs.signed()];
    let n = q.iter().map(|c| c * c).sum::<f32>().sqrt().max(1e-6);
    let [w, x, y, z] = q.map(|c| c / n);
    [
        Vf::new(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y), 0.0),
        Vf::new(2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x), 0.0),
        Vf::new(2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y), 0.0),
        origin,
    ]
}

fn names(list: &[&str]) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    for n in list {
        match addr(n) {
            Ok(a) => out.push(a),
            Err(_) => {
                skip(&format!("{n} is not in research/oracle.txt"));
                return None;
            }
        }
    }
    Some(out)
}

/// A made-up tree over `segments` probe segments: every node's box the bounds of what is below it,
/// grown or shrunk a little at random so the descent sometimes passes a node whose segments miss
/// and sometimes stops at one whose segments would strike.
fn tree(xs: &mut Xs, segments: &[Segment]) -> (Vec<Node>, u16) {
    fn build(xs: &mut Xs, segs: &[Segment], ids: &[u16], nodes: &mut Vec<Node>) -> u16 {
        if ids.len() == 1 {
            return 0x8000 | ids[0];
        }
        let at = nodes.len();
        nodes.push(Node { volume: F::ZERO, centre: [F::ZERO; 3], half_extent: [F::ZERO; 3], children: [0; 2] });
        let mid = ids.len() / 2;
        let a = build(xs, segs, &ids[..mid], nodes);
        let b = build(xs, segs, &ids[mid..], nodes);
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for &i in ids {
            let s = segs[i as usize];
            for k in 0..3 {
                lo[k] = lo[k].min(s.from.0[k]).min(s.to.0[k]);
                hi[k] = hi[k].max(s.from.0[k]).max(s.to.0[k]);
            }
        }
        let grow = 0.8 + xs.unit() * 0.4;
        nodes[at].centre = [0, 1, 2].map(|k| F((lo[k] + hi[k]) * 0.5));
        nodes[at].half_extent = [0, 1, 2].map(|k| F((hi[k] - lo[k]) * 0.5 * grow + 0.01));
        nodes[at].children = [a, b];
        at as u16
    }
    let ids: Vec<u16> = (0..segments.len() as u16).collect();
    let mut nodes = Vec::new();
    let root = build(xs, segments, &ids, &mut nodes);
    (nodes, root)
}

#[test]
fn the_box_strike_is_the_originals() {
    let mut ee = match Ee::game() {
        Ok(ee) => ee,
        Err(why) => return skip(&why),
    };
    let Some(a) = names(&["ray_tree_box_strike"]) else { return };
    let strike = a[0];
    const CENTRE: u32 = 0x0180_0000;
    const HALF: u32 = 0x0180_0010;
    const BOX: u32 = 0x0180_0020;
    const PROBE: u32 = 0x0180_0060;
    const LISTS: u32 = 0x0180_00a0;
    const NODES: u32 = 0x0180_1000;
    const SEGS: u32 = 0x0180_4000;
    let mut xs = Xs(0x5eed_0104);
    let (mut hits, cases) = (0, 6000);
    for case in 0..cases {
        let n = 1 + xs.below(11) as usize;
        let reach = 4.0 + xs.unit() * 20.0;
        let segments: Vec<Segment> = (0..n)
            .map(|_| {
                let from = Vf::new(xs.signed() * reach, xs.signed() * reach, xs.signed() * reach, 1.0);
                let len = xs.unit() * 6.0;
                let to = Vf::new(from.x() + xs.signed() * len, from.y() + xs.signed() * len, from.z() + xs.signed() * len, 1.0);
                Segment { from, to }
            })
            .collect();
        let (nodes, root) = tree(&mut xs, &segments);
        let origin = Vf::new(xs.signed() * 30.0, xs.signed() * 30.0, xs.signed() * 30.0, 1.0);
        let probe_at = frame(&mut xs, origin);
        // The box near where the probe's segments land, so about half the cases strike.
        let near = |xs: &mut Xs| xs.signed() * reach * 1.2;
        let world = |p: [f32; 3]| {
            let c = |k: usize| probe_at[3].0[k] + probe_at[0].0[k] * p[0] + probe_at[1].0[k] * p[1] + probe_at[2].0[k] * p[2];
            Vf::new(c(0), c(1), c(2), 1.0)
        };
        let at = world([near(&mut xs), near(&mut xs), near(&mut xs)]);
        let box_at = frame(&mut xs, at);
        let half = Vf::new(0.5 + xs.unit() * 8.0, 0.5 + xs.unit() * 8.0, 0.5 + xs.unit() * 8.0, 0.0);
        let centre = Vf::ZERO;

        put(&mut ee, CENTRE, centre);
        put(&mut ee, HALF, half);
        for k in 0..4 {
            put(&mut ee, BOX + 16 * k as u32, box_at[k]);
            put(&mut ee, PROBE + 16 * k as u32, probe_at[k]);
        }
        ee.set_u32(LISTS, NODES);
        ee.set_u32(LISTS + 4, SEGS);
        for (i, nd) in nodes.iter().enumerate() {
            let at = NODES + 0x24 * i as u32;
            ee.write(at + 8, 2, nd.children[0] as u128);
            ee.write(at + 10, 2, nd.children[1] as u128);
            for k in 0..3 {
                ee.set_f32(at + 0xc + 4 * k as u32, nd.centre[k].0);
                ee.set_f32(at + 0x18 + 4 * k as u32, nd.half_extent[k].0);
            }
        }
        for (i, s) in segments.iter().enumerate() {
            put(&mut ee, SEGS + 0x20 * i as u32, s.from);
            put(&mut ee, SEGS + 0x20 * i as u32 + 0x10, s.to);
        }
        ee.call(strike, &[CENTRE, HALF, BOX, root as u32, LISTS, PROBE].map(u64::from), &[], 200_000);
        let want = ee.v0() as u32 != 0;
        let probe = RayProbe { transform: probe_at, root, nodes: &nodes, segments: &segments };
        let got = creature::probe_strikes_box(centre, half, &box_at, &probe);
        assert_eq!(got, want, "case {case}: {n} segments, half {half:?}");
        hits += want as usize;
    }
    println!("{hits} of {cases} struck");
    assert!(hits > cases / 5 && hits < cases * 4 / 5, "{hits} of {cases}: the cases should go both ways");
}

/// The four ways an actor can be this tick: (struck, kept). Segments about a 10-foot cube at the
/// origin: a point inside is both; a long diagonal past an edge, whose box overlaps the cube but which
/// misses it, is kept and not struck; a far first segment and a second inside is struck and not kept.
fn actor(struck: bool, kept: bool) -> Rays {
    let far = [v3(500.0, 0.0, 0.0), v3(500.0, 0.0, 0.0)];
    let inside = [v3(1.0, 2.0, 3.0), v3(1.0, 2.0, 3.0)];
    // Rising on both axes: the original takes a segment's box as its midpoint and half its span,
    // signed, so a falling one would be a box of negative size that reaches nothing.
    let grazing = [v3(-15.0, 6.0, 0.0), v3(-5.0, 16.0, 0.0)];
    Rays(match (struck, kept) {
        (true, true) => vec![inside],
        (false, true) => vec![grazing, far],
        (true, false) => vec![far, inside],
        (false, false) => vec![far],
    })
}

#[test]
fn the_contact_callback_is_the_originals() {
    let Some(mut ee) = ps2_oracle::game_or_skip() else { return };
    let Some(a) = names(&[
        "trigger_contact",
        "trigger_update",
        "toy_triggers_live",
        "rand_raw",
        "rand_seeded",
        "realloc",
        "shape_exit_test",
        "node_update_children",
    ]) else {
        return;
    };
    let [contact, update, live, rand, seeded, realloc, exit_test, children] = a[..] else { unreachable!() };

    // The check the cases lean on: each of the four kinds of actor is what it says.
    let cube = Trigger { half: v3(10.0, 10.0, 10.0), right: v3(1.0, 0.0, 0.0), up: v3(0.0, 1.0, 0.0), look: v3(0.0, 0.0, 1.0), ..Default::default() };
    for (s, k) in [(true, true), (false, true), (true, false), (false, false)] {
        let r = Actor::Rays(actor(s, k));
        assert_eq!((cube.struck_by(&r), cube.keeps(&r)), (s, k));
    }

    const TRIG: u32 = 0x0180_0000;
    const SHAPE: u32 = 0x0180_1000;
    const ACTORS: u32 = 0x0180_2000;
    const ACTIONS: u32 = 0x0180_3000;
    const LIST: u32 = 0x0180_3800;
    const VTABLE: u32 = 0x0180_3c00;
    const RUN: u32 = 0x0180_3e00;
    const HEAP: u32 = 0x0181_0000;
    const DT: f32 = 1.0 / 30.0;

    // The hooks' view of the world: the port's keeps answers by actor, the runs counted.
    let keeps: Rc<RefCell<Vec<bool>>> = Rc::default();
    let runs: Rc<RefCell<Vec<[u32; 2]>>> = Rc::default();
    let rng: Rc<RefCell<Rng>> = Rc::default();
    let heap = Rc::new(RefCell::new(HEAP));
    {
        let keeps = keeps.clone();
        ee.hooks.insert(exit_test, Box::new(move |ee: &mut Ee| {
            let i = ((ee.r(5) as u32 - ACTORS) / 0x100) as usize;
            let k = keeps.borrow()[i];
            ee.set(2, k as u64);
        }));
        let runs = runs.clone();
        ee.hooks.insert(RUN, Box::new(move |ee: &mut Ee| {
            let a = ((ee.r(4) as u32 - ACTIONS) / 0x40) as usize;
            let enter = ee.r(7) as u32 & 1;
            runs.borrow_mut()[a][enter as usize] += 1;
        }));
        let rng = rng.clone();
        ee.hooks.insert(rand, Box::new(move |ee: &mut Ee| {
            let r = rng.borrow_mut().rand();
            ee.set(2, r as u64);
        }));
        let heap = heap.clone();
        ee.hooks.insert(realloc, Box::new(move |ee: &mut Ee| {
            let (old, size) = (ee.r(4) as u32, ee.r(5) as u32);
            let at = *heap.borrow();
            *heap.borrow_mut() += (size + 15) & !15;
            if old != 0 {
                for k in 0..size.saturating_sub(4) {
                    let b = ee.u8_at(old + k);
                    ee.write(at + k, 1, b as u128);
                }
            }
            ee.set(2, at as u64);
        }));
        ee.hooks.insert(children, Box::new(|_: &mut Ee| {}));
    }
    ee.set_u32(live, 1);
    ee.set_u32(seeded, 1);

    let mut xs = Xs(0x0c0_ffee);
    let (mut fired, mut ran, mut left) = (0u32, [0u32; 2], 0u32);
    for case in 0..400 {
        let actors = 1 + xs.below(3) as usize;
        let n = 3 + xs.below(2) as usize;
        let percent = [100, 100, 50, 75, 0][xs.below(5) as usize];
        let one_time = xs.below(4) == 0;
        let mut t = Trigger {
            half: v3(10.0, 10.0, 10.0),
            right: v3(1.0, 0.0, 0.0),
            up: v3(0.0, 1.0, 0.0),
            look: v3(0.0, 0.0, 1.0),
            enabled: true,
            one_time,
            percent,
            ..Default::default()
        };
        for _ in 0..n {
            t.actions.push(Action {
                equals: [-1, -1, 1, 2, 3][xs.below(5) as usize],
                greater: [-1, -1, 0, 1, 2][xs.below(5) as usize],
                delay: [0.0, 0.0, 0.05, 0.1, 0.2][xs.below(5) as usize],
                upon_exit: xs.below(3) == 0,
                ..Default::default()
            });
        }
        let mut c = Creatures::default();
        c.rng.seed(xs.next());
        *rng.borrow_mut() = c.rng;
        *runs.borrow_mut() = vec![[0; 2]; n];
        *heap.borrow_mut() = HEAP;

        for k in 0..0x400 {
            ee.set_u32(TRIG + k * 4, 0);
            ee.set_u32(SHAPE + k * 4, 0);
            ee.set_u32(ACTORS + k * 4, 0);
            ee.set_u32(ACTIONS + k * 4, 0);
        }
        for k in 0..8 {
            ee.set_u32(VTABLE + 4 * k, RUN);
        }
        ee.set_u32(SHAPE + 0x78, TRIG);
        ee.set_u32(SHAPE + 0x68, 6);
        ee.write(TRIG + 0x29, 1, 1);
        ee.write(TRIG + 0x31c, 1, 0x7 | if one_time { 0x8 } else { 0 });
        ee.set_u32(TRIG + 0xdc, percent as u32);
        ee.set_u32(TRIG + 0xe0, n as u32);
        ee.set_u32(TRIG + 0xfc, LIST);
        ee.set_u32(TRIG + 0x314, SHAPE);
        for (i, act) in t.actions.iter().enumerate() {
            let at = ACTIONS + 0x40 * i as u32;
            ee.set_u32(LIST + 4 * i as u32, at);
            ee.set_u32(at + 4, act.equals as u32);
            ee.set_u32(at + 8, act.greater as u32);
            ee.set_f32(at + 0xc, act.delay);
            ee.set_u32(at + 0x14, act.upon_exit as u32);
            ee.set_u32(at + 0x1c, VTABLE);
        }
        c.triggers.push(t);

        for tick in 0..60 {
            let ways: Vec<(bool, bool)> = (0..actors).map(|_| (xs.below(2) == 0, xs.below(3) != 0)).collect();
            let rays: Vec<Actor> = ways.iter().map(|&(s, k)| Actor::Rays(actor(s, k))).collect();
            *keeps.borrow_mut() = ways.iter().map(|w| w.1).collect();

            ee.call(update, &[TRIG as u64], &[DT], 100_000);
            for (i, &(s, _)) in ways.iter().enumerate() {
                if s {
                    ee.call(contact, &[SHAPE as u64, (ACTORS + 0x100 * i as u32) as u64], &[], 100_000);
                }
            }
            c.step_among(DT, &rays);

            let tr = &c.triggers[0];
            let at = format!("case {case} tick {tick} ({actors} actors, {percent}%, one-time {one_time}, ways {ways:?})");
            let bits = ee.u8_at(TRIG + 0x31c);
            assert_eq!((bits >> 5 & 1 != 0, bits >> 6 & 1 != 0, bits >> 4 & 1 != 0), (tr.touched, tr.touched_last, tr.fired), "{at}: touch, last, fired");
            assert_eq!(ee.i32_at(TRIG + 0x100), tr.count, "{at}: count");
            let (list, len) = (ee.u32_at(SHAPE + 0xe0), ee.i32_at(SHAPE + 0xe4) as u32);
            let want: Vec<Option<usize>> = (0..len)
                .map(|k| match ee.u32_at(list + 4 * k) {
                    0 => None,
                    a => Some(((a - ACTORS) / 0x100) as usize),
                })
                .collect();
            assert_eq!(tr.inside, want, "{at}: the contact list");
            for (i, act) in tr.actions.iter().enumerate() {
                let a = ACTIONS + 0x40 * i as u32;
                assert_eq!(act.timer.to_bits(), ee.u32_at(a + 0x10), "{at}: action {i}'s timer {} vs {}", act.timer, ee.f32_at(a + 0x10));
                let pending = match ee.u32_at(a) {
                    0 => None,
                    p => Some(((p - ACTORS) / 0x100) as usize),
                };
                assert_eq!(act.pending, pending, "{at}: action {i}'s pending actor");
                assert_eq!(act.ran, runs.borrow()[i], "{at}: action {i}'s runs (out, in)");
            }
            assert_eq!(c.rng, *rng.borrow(), "{at}: the draws");
            left += want.iter().filter(|s| s.is_none()).count() as u32;
        }
        let tr = &c.triggers[0];
        fired += tr.count as u32;
        for a in &tr.actions {
            ran[0] += a.ran[0];
            ran[1] += a.ran[1];
        }
    }
    println!("fired {fired}, ran out {} in {}, empty slots seen {left}", ran[0], ran[1]);
    assert!(fired > 1000 && ran[0] > 100 && ran[1] > 1000 && left > 100, "the cases reach every path");
}
