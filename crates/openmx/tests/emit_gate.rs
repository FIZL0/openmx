//! V71: particles.rs's [`Gate`] against the original's emit gate and its emitter's on, off and reset,
//! run on the boot ELF over a made-up emitter in RAM: every mode, every flag the gate reads, delays and
//! times either side of 0, and the game's `rand` hooked to a number the port draws as well.
//!
//! The emitter has no node and no parent system, and its view cull is off: the port's track emitters
//! have neither, and it does not cull. Without the disc the test says so and passes.

#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;

use particles::Gate;
use ps2_oracle::{Ee, addr, skip};
use std::cell::Cell;
use std::rc::Rc;

/// The made-up emitter, its vtable, and the global the gate reads its pause flag through.
const EMITTER: u32 = 0x0180_0000;
const VTABLE: u32 = 0x0180_1000;
const GLOBAL: u32 = 0x0180_2000;

/// The emitter's fields the gate reads and writes.
mod at {
    pub const FLAGS: u32 = 0x240;
    pub const IDLE: u32 = 0x1d8;
    pub const MODE: u32 = 0x270;
    pub const DELAY: u32 = 0x274;
    pub const DELAY_TIME: u32 = 0x278;
    pub const LEFT: u32 = 0x27c;
    pub const TIMES: u32 = 0x280;
}

struct Xs(u32);

impl Xs {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    /// A time: 0, either sign of a small one, or one in a few seconds.
    fn time(&mut self) -> f32 {
        match self.next() % 6 {
            0 => 0.0,
            1 => -self.unit() * 0.1,
            2 => self.unit() * 0.05,
            _ => self.unit() * 4.0,
        }
    }
}

fn gate_of(ee: &Ee) -> Gate {
    let e = |o: u32| ee.f32_at(EMITTER + o);
    Gate {
        mode: ee.u32_at(EMITTER + at::MODE) as i32,
        delay_time: e(at::DELAY_TIME),
        delay: e(at::DELAY),
        left: e(at::LEFT),
        times: [e(at::TIMES), e(at::TIMES + 4), e(at::TIMES + 8), e(at::TIMES + 12)],
        idle: e(at::IDLE),
        flags: ee.u8_at(EMITTER + at::FLAGS) & 0x0f,
    }
}

fn zero(ee: &mut Ee, a: u32, n: u32) {
    for k in (0..n).step_by(4) {
        ee.set_u32(a + k, 0);
    }
}

fn put(ee: &mut Ee, g: &Gate) {
    zero(ee, EMITTER, 0x300);
    ee.set_u32(EMITTER + 0x38, VTABLE);
    ee.set_u32(EMITTER + at::MODE, g.mode as u32);
    ee.set_f32(EMITTER + at::DELAY_TIME, g.delay_time);
    ee.set_f32(EMITTER + at::DELAY, g.delay);
    ee.set_f32(EMITTER + at::LEFT, g.left);
    for (k, t) in g.times.iter().enumerate() {
        ee.set_f32(EMITTER + at::TIMES + 4 * k as u32, *t);
    }
    ee.set_f32(EMITTER + at::IDLE, g.idle);
    ee.write(EMITTER + at::FLAGS, 1, u128::from(g.flags));
}

fn same(a: &Gate, b: &Gate) -> bool {
    a.mode == b.mode
        && a.flags == b.flags
        && [a.delay, a.left, a.idle].map(f32::to_bits) == [b.delay, b.left, b.idle].map(f32::to_bits)
}

#[test]
fn the_gate_is_the_originals() {
    let mut ee = match Ee::game() {
        Ok(ee) => ee,
        Err(why) => return skip(&why),
    };
    let names = ["emit_timing", "emitter_on", "emitter_off", "emitter_reset", "rand", "emitter_paused_by"];
    let Ok(a) = names.iter().map(|n| addr(n)).collect::<Result<Vec<u32>, String>>() else {
        return skip("the emitter's addresses are not in research/oracle.txt");
    };
    let (gate, on, off, reset, rand, paused_by) = (a[0], a[1], a[2], a[3], a[4], a[5]);
    zero(&mut ee, VTABLE, 0x100);
    ee.set_u32(VTABLE + 0x98, on);
    ee.set_u32(VTABLE + 0x9c, off);
    zero(&mut ee, GLOBAL, 0x400);
    ee.set_u32(paused_by, GLOBAL);
    let drawn = Rc::new(Cell::new(0i32));
    let d = Rc::clone(&drawn);
    ee.hooks.insert(rand, Box::new(move |ee: &mut Ee| ee.set(2, d.get() as u64)));
    let mut xs = Xs(0x0071_9a7e);
    let mut counts = [0usize; 4];
    for case in 0..20_000 {
        let mut g = Gate {
            mode: [0, 1, 2, 3, 5][(xs.next() % 5) as usize],
            delay_time: xs.time(),
            delay: xs.time(),
            left: xs.time(),
            times: [xs.time(), xs.time(), xs.time(), xs.time()],
            idle: xs.unit(),
            flags: (xs.next() & 0x0f) as u8,
        };
        let dt = [1.0 / 30.0, 1.0 / 60.0, xs.unit() * 0.1][(xs.next() % 3) as usize];
        drawn.set((xs.next() >> 1) as i32);
        let r = drawn.get();
        let before = g.clone();
        put(&mut ee, &g);
        // What is done to it: a frame, on, off or a reset.
        let what = (xs.next() % 4) as usize;
        counts[what] += 1;
        let (fun, ints, floats): (u32, Vec<u64>, Vec<f32>) = match what {
            0 => (gate, vec![u64::from(EMITTER)], vec![dt]),
            1 => (on, vec![u64::from(EMITTER)], vec![]),
            2 => (off, vec![u64::from(EMITTER)], vec![]),
            _ => (reset, vec![u64::from(EMITTER)], vec![]),
        };
        ee.call(fun, &ints, &floats, 10_000);
        let mut draw = || r;
        let port_emits = match what {
            0 => Some(g.pass(dt, &mut draw)),
            1 => {
                g.on();
                None
            }
            2 => {
                g.off();
                None
            }
            _ => {
                g.reset(&mut draw);
                None
            }
        };
        let theirs = gate_of(&ee);
        assert!(same(&g, &theirs), "case {case} ({what}, dt {dt}, rand {r}) from {before:?}:\n port     {g:?}\n original {theirs:?}");
        if let Some(emits) = port_emits {
            assert_eq!(emits, ee.v0() as u32 == 0, "case {case}: emits, dt {dt} from {before:?}");
        }
    }
    println!("frames, ons, offs, resets: {counts:?}");
}

/// A one-shot as the geysers' and the fireworks' run: switched on and reset, it rests a frame, waits
/// its delay, emits for its shot, and switches itself off once it has made something.
#[test]
fn a_one_shot_runs_its_delay_then_its_shot() {
    let emt = particles::Emt { mode: 1, delay: 0.75, times: [0.0, 0.5, 0.0, 0.0], ..particles::Emt::default() };
    let mut never = || 0;
    let mut g = Gate::new(&emt, &mut never);
    g.reset(&mut never);
    g.off();
    let dt = 1.0 / 30.0;
    assert!(!g.pass(dt, &mut never), "off");
    g.on();
    let frames: Vec<bool> = (0..60)
        .map(|_| {
            let e = g.pass(dt, &mut never);
            g.emitted(usize::from(e));
            e
        })
        .collect();
    let first = frames.iter().position(|e| *e).unwrap();
    let last = frames.iter().rposition(|e| *e).unwrap();
    // The rest frame, then 0.75 s of 1/30 (23 frames: 22 leave 0.0167 to run), then 0.5 s (15 frames,
    // and the one it crosses 0 on).
    assert_eq!(first, 1 + 23, "{frames:?}");
    assert!(frames[first..=last].iter().all(|e| *e));
    assert!((15..=16).contains(&(last + 1 - first)), "{} frames on", last + 1 - first);
    assert!(!g.is_on(), "switched itself off");
}
