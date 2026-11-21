//! Every lifted function against the original, on the same bytes (S496): the oracle runs the original
//! over a RAM image, the lift runs over a [`Mem`] holding the same image, and what each answers and
//! leaves in memory must match.
//!
//! Inputs are written into both memories through one helper, so a case is the same bytes on both
//! sides by construction; nothing is translated into a port's own types and back.

use game_lift::{Mem, ai, gate};
use ps2_oracle::{Ee, Kit, addr, game_or_skip, skip};

const BUDGET: u64 = 200_000;

/// Scratch the cases live in, well above the ELF and its heap.
const GATE: u32 = 0x0180_0000;
const FROM: u32 = 0x0180_1000;
const TO: u32 = 0x0180_1010;

/// The xorshift every oracle test here draws its cases from.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        (self.next() >> 16) % n
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    /// Now and then a word no object holds: a denormal, an Inf/NaN pattern, a signed zero, any bits.
    fn stray(&mut self, x: f32) -> u32 {
        match self.below(40) {
            0 => self.next() as u32 & 0x807f_ffff,
            1 => 0x7f80_0000 | (self.next() as u32 & 0x807f_ffff),
            2 => (-0.0f32).to_bits(),
            3 => 0,
            4 => self.next() as u32,
            _ => x.to_bits(),
        }
    }
}

/// Both memories, written together.
struct Both {
    ee: Ee,
    mem: Mem,
}

impl Both {
    fn new(ee: Ee) -> Both {
        let mem = Mem::from_ram(ee.ram.clone());
        Both { ee, mem }
    }
    fn u32(&mut self, a: u32, v: u32) {
        self.ee.set_u32(a, v);
        self.mem.set_u32(a, v);
    }
    fn f(&mut self, p: &mut Pick, a: u32, x: f32) {
        let w = p.stray(x);
        self.u32(a, w);
    }
    fn quad(&mut self, p: &mut Pick, a: u32, v: [f32; 3]) {
        for (k, x) in v.iter().enumerate() {
            self.f(p, a + 4 * k as u32, *x);
        }
        let w = p.range(-2.0, 2.0);
        self.f(p, a + 12, w);
    }
    /// The words `[from, to)` of both memories, which must be the same.
    fn same(&self, from: u32, to: u32, what: &str) {
        for a in (from..to).step_by(4) {
            assert_eq!(self.mem.u32(a), self.ee.u32_at(a), "{what}: word {a:#x} differs");
        }
    }
}

fn unit_vector(p: &mut Pick) -> [f32; 3] {
    loop {
        let v = [p.range(-1.0, 1.0), p.range(-1.0, 1.0), p.range(-1.0, 1.0)];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if l > 0.1 && l <= 1.0 {
            return v.map(|c| c / l);
        }
    }
}

/// A gate near the origin and a step near it: through its plane, at its edges, or nowhere near.
fn gate_case(b: &mut Both, p: &mut Pick) {
    let a = p.range(0.0, 6.3);
    let normal = match p.below(3) {
        0 => [0.0, 0.0, 1.0],
        1 => [1.0, 0.0, 0.0],
        _ => [a.sin(), 0.0, a.cos()],
    };
    let across = match p.below(4) {
        0 => unit_vector(p),
        _ => [normal[2], if p.below(4) == 0 { p.range(-0.3, 0.3) } else { 0.0 }, -normal[0]],
    };
    let centre = [p.range(-50.0, 50.0), p.range(-5.0, 5.0), p.range(-50.0, 50.0)];
    let mut flags = p.next() as u32 & 0x1f;
    if p.below(8) != 0 {
        flags |= 1;
    }
    if p.below(3) != 0 {
        flags &= !0x10;
    }
    let width = match p.below(6) {
        0 => -p.range(0.0, 10.0),
        1 => 0.0,
        _ => p.range(1.0, 60.0),
    };
    let height = match p.below(5) {
        0 => -p.range(0.0, 5.0),
        _ => p.range(0.5, 20.0),
    };
    let reach = match p.below(4) {
        0 => 0.0,
        1 => -p.range(0.0, 100.0),
        _ => p.range(1.0, 10_000.0),
    };
    b.u32(GATE + 0x30, flags);
    b.quad(p, GATE + 0x40, centre);
    b.quad(p, GATE + 0x50, normal);
    b.quad(p, GATE + 0x60, across);
    b.f(p, GATE + 0x70, width);
    b.f(p, GATE + 0x74, height);
    b.f(p, GATE + 0x7c, reach);
    // A point on the plane, across the width (often at its edges) and up the height.
    let side = match p.below(6) {
        0 => [-1.0, 1.0, -1.0001, 1.0001, 0.9999, -0.9999, 0.0][p.below(7) as usize],
        1 => p.range(-3.0, 3.0),
        _ => p.range(-1.0, 1.0),
    } * width
        * 0.5;
    let up = match p.below(5) {
        0 => height + [0.0, 0.001, -0.001, 1.0][p.below(4) as usize],
        _ => p.range(-2.0, height.max(0.0) + 2.0),
    };
    let on = [0, 1, 2].map(|i| centre[i] + across[i] * side + if i == 1 { up } else { 0.0 });
    let t = match p.below(6) {
        0 => [0.0, 1.0, -0.0001, 1.0001, 0.5][p.below(5) as usize],
        1 => p.range(-2.0, 3.0),
        _ => p.range(0.0, 1.0),
    };
    let len = match p.below(10) {
        0 => 0.0,
        1 => p.range(0.0, 0.01),
        2 => p.range(10.0, 300.0),
        _ => p.range(0.1, 8.0),
    };
    let wobble = [p.range(-0.5, 0.5), p.range(-0.5, 0.5), p.range(-0.5, 0.5)];
    let sign = if p.below(4) == 0 { -1.0 } else { 1.0 };
    let dir = [0, 1, 2].map(|i| (normal[i] + wobble[i]) * sign);
    let to = [0, 1, 2].map(|i| on[i] - dir[i] * len * t);
    let from = if p.below(12) == 0 {
        [p.range(-500.0, 500.0), p.range(-50.0, 50.0), p.range(-500.0, 500.0)]
    } else {
        [0, 1, 2].map(|i| to[i] + dir[i] * len)
    };
    b.quad(p, FROM, from);
    b.quad(p, TO, to);
}

/// The free09 save-state's RAM, three AI riders on the ground controller, with gp set.
const STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";
const RIDERS: [u32; 3] = [0x0148_3eb0, 0x00c6_4000, 0x0104_8000];

fn state() -> Option<Ee> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

fn tables() -> ai::Tables {
    ai::Tables {
        curve: addr("curve_table_ptr").unwrap(),
        curve_scale: addr("curve_table_invstep").unwrap(),
        curve_step: addr("curve_table_step").unwrap(),
        acos: addr("fine_acos_table").unwrap(),
    }
}

/// The rider's floats the controller reads, and the range a case draws each from.
const READS: [(u32, f32, f32); 23] = [
    (0x70, -6.0, 6.0),
    (0x74, -6.0, 6.0),
    (0x80, 0.0, 120.0),
    (0xc4, -20.0, 40.0),
    (0x204, -20.0, 20.0),
    (0x1e0, 0.0, 120.0),
    (0x254, 0.0, 60.0),
    (0xf4, -1.5, 1.5),
    (0x100, -2.0, 2.0),
    (0x104, -2.0, 2.0),
    (0x208, -3.0, 3.0),
    (0x21c, -3.0, 3.0),
    (0x260, -5.0, 20.0),
    (0x264, -5.0, 20.0),
    (0x2b4, -2.0, 30.0),
    (0x2d0, -3.0, 3.0),
    (0x2d4, -3.0, 3.0),
    (0x2d8, -0.5, 1.5),
    (0x2ec, -8.0, 8.0),
    (0x32c, -0.1, 0.1),
    (0x330, -1.0, 1.0),
    (0x334, -1.0, 1.0),
    (0xa4, -1.0, 1.0),
];

/// The rider's quadwords the controller reads: up, forward, the direction asked for, the ground ahead.
const QUADS: [u32; 4] = [0xa0, 0xb0, 0x1d0, 0x2c0];

#[test]
fn the_lifted_ground_controller_matches_on_the_three_riders_as_they_are() {
    let Some(ee) = state() else { return };
    let controls = addr("ai_controls").unwrap();
    let t = tables();
    let mut b = Both::new(ee);
    for ai in RIDERS {
        b.ee.call(controls, &[u64::from(ai)], &[], BUDGET);
        assert_eq!(b.ee.v0(), 1);
        assert_eq!(ai::controls(&mut b.mem, &t, ai), 1);
        b.same(ai, ai + 0x400, &format!("rider {ai:#x}"));
    }
}

#[test]
fn the_lifted_ground_controller_matches_over_ten_thousand_cases() {
    let Some(ee) = state() else { return };
    let controls = addr("ai_controls").unwrap();
    let t = tables();
    let mut b = Both::new(ee);
    let mut p = Pick(0x9e37_79b9_7f4a_7c15);
    let saved: Vec<Vec<u32>> =
        RIDERS.iter().map(|&ai| (0..0x100).map(|k| b.mem.u32(ai + 4 * k)).collect()).collect();
    let mut paths = [0usize; 4];
    for case in 0..10_000 {
        let r = p.below(3) as usize;
        let ai = RIDERS[r];
        for (k, w) in saved[r].iter().enumerate() {
            b.u32(ai + 4 * k as u32, *w);
        }
        // One case in five carries words no rider holds.
        let wild = p.below(5) == 0;
        let mut put = |b: &mut Both, p: &mut Pick, a: u32, x: f32| {
            if wild { b.f(p, a, x) } else { b.u32(a, x.to_bits()) }
        };
        let flags = saved[r][0x30 / 4] & !0x3001_0020 | (p.next() as u32 & 0x3001_0020);
        b.u32(ai + 0x30, flags);
        for (o, lo, hi) in READS {
            if p.below(4) != 0 {
                let x = p.range(lo, hi);
                put(&mut b, &mut p, ai + o, x);
            }
        }
        for o in QUADS {
            if p.below(3) != 0 {
                let v = unit_vector(&mut p);
                let s = if o == 0x1d0 { p.range(0.1, 50.0) } else { 1.0 };
                for (k, x) in v.iter().enumerate() {
                    put(&mut b, &mut p, ai + o + 4 * k as u32, x * s);
                }
            }
        }
        // Sometimes right at the landing height, where the original compares twice.
        if p.below(10) == 0 {
            let h = b.mem.f(ai + 0x2b4).0 + b.mem.f(ai + 0x204).0;
            b.u32(ai + 0xc4, h.to_bits());
        }
        b.ee.call(controls, &[u64::from(ai)], &[], BUDGET);
        assert_eq!(b.ee.v0(), 1, "case {case}");
        assert_eq!(ai::controls(&mut b.mem, &t, ai), 1);
        b.same(ai, ai + 0x400, &format!("case {case}, rider {ai:#x}"));
        let above = b.mem.f(ai + 0xc4) - b.mem.f(ai + 0x204);
        paths[if flags & 0x20 != 0 {
            0
        } else if !(above <= b.mem.f(ai + 0x2b4)) {
            1
        } else {
            2 + usize::from(wild)
        }] += 1;
    }
    for (path, n) in paths.iter().enumerate() {
        assert!(*n > 200, "path {path} came up only {n} times in 10,000: {paths:?}");
    }
}

#[test]
fn the_lifted_gate_crossing_matches_over_twenty_thousand_steps() {
    let Some(ee) = game_or_skip() else { return };
    let crossed = match addr("start_gate_crossed") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut b = Both::new(ee);
    let mut p = Pick(0x51f0_96a3_9e37_79b9);
    let mut seen = [0usize; 5];
    for case in 0..20_000 {
        gate_case(&mut b, &mut p);
        // Now and then a null end, which answers before anything is read.
        let (from, to) = match p.below(200) {
            0 => (0, TO),
            1 => (FROM, 0),
            _ => (FROM, TO),
        };
        b.ee.call(crossed, &[u64::from(GATE), u64::from(from), u64::from(to)], &[], BUDGET);
        let want = b.ee.v0() as u32;
        let got = gate::crossed(&b.mem, GATE, from, to);
        assert_eq!(got, want, "case {case}");
        b.same(GATE, GATE + 0x80, "the gate");
        seen[want.min(4) as usize] += 1;
    }
    for (code, n) in seen.iter().enumerate().filter(|&(c, _)| c != 3) {
        assert!(*n > 300, "answer {code} came up only {n} times in 20,000: {seen:?}");
    }
}
