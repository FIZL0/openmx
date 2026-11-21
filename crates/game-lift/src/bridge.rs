//! The bridge between the sim and the lift (S496): the sim's rider laid out as the original's brain
//! object in a small [`Mem`], the lifted controller run over it, and its outputs read back into the
//! sim's own [`Controls`]; and the same for the start gate and a step past it. With the shell's `--ai`
//! the sim calls [`controls`] and [`crossed`] in place of its ports (`game_sim::ai::LIFTED`,
//! `game_sim::race::LIFTED`).
//!
//! The memory holds only what the controller reads: the brain at 0, and the sim's own curve table and
//! fine arc cosine (the game's to the bit) at places of the bridge's choosing, handed to the lift as
//! [`Tables`]. No address of the original's is needed.
#![deny(clippy::float_arithmetic)]

use crate::ai::{self, Tables};
use crate::{Mem, gate};
use game_sim::ai::{CURVE_INV_STEP, CURVE_STEP, Controls, Situation, Tuning};
use game_sim::{race, ragdoll};
use ps2_float::{F, Vf};
use std::cell::RefCell;

/// Where the bridge puts things.
const BRAIN: u32 = 0;
const BRAIN_LEN: u32 = 0x400;
const TABLES: Tables = Tables { curve: 0x400, curve_scale: 0x404, curve_step: 0x408, acos: 0x500 };
const CURVE: u32 = 0x1000;
const FINE: u32 = 0x10000;
const LEN: usize = 0x20000;

/// A memory with the tables laid out and an empty brain.
fn laid_out(t: &ragdoll::Tables) -> Mem {
    let mut m = Mem::sized(LEN);
    m.set_u32(TABLES.curve, CURVE);
    m.set_f(TABLES.curve_scale, F(CURVE_INV_STEP));
    m.set_f(TABLES.curve_step, F(CURVE_STEP));
    for (k, [c, s]) in t.curve.entries().iter().enumerate() {
        m.set_f(CURVE + 8 * k as u32, F(*c));
        m.set_f(CURVE + 8 * k as u32 + 4, F(*s));
    }
    let mut at = FINE;
    for (k, p) in t.fine.pieces().iter().enumerate() {
        let piece = TABLES.acos + 12 * k as u32;
        m.set_f(piece, p.base);
        m.set_f(piece + 4, p.scale);
        m.set_u32(piece + 8, at);
        for e in p.entries.iter() {
            m.set_f(at, F(*e));
            at += 4;
        }
    }
    assert!(at as usize <= LEN, "the fine arc cosine overruns the bridge's memory");
    m
}

thread_local! {
    static MEM: RefCell<Option<Mem>> = const { RefCell::new(None) };
}

/// [`game_sim::ai::controls`], run by the lift.
pub fn controls(t: &ragdoll::Tables, at: &Situation, tune: &Tuning) -> Controls {
    MEM.with(|cell| {
        let mut cell = cell.borrow_mut();
        let m = cell.get_or_insert_with(|| laid_out(t));
        run(m, at, tune)
    })
}

/// Where the gate and the step's two ends go, in a memory of their own.
const GATE: u32 = 0;
const FROM: u32 = 0x80;
const TO: u32 = 0x90;

thread_local! {
    static GATE_MEM: RefCell<Mem> = RefCell::new(Mem::sized(0x100));
}

/// [`race::Gate::crossed`], run by the lift.
pub fn crossed(gate: &race::Gate, from: Vf, to: Vf) -> race::Crossing {
    GATE_MEM.with(|cell| {
        let m = &mut *cell.borrow_mut();
        m.set_u32(GATE + 0x30, gate.flags);
        m.set_vf(GATE + 0x40, gate.centre);
        m.set_vf(GATE + 0x50, gate.normal);
        m.set_vf(GATE + 0x60, gate.across);
        m.set_f(GATE + 0x70, gate.width);
        m.set_f(GATE + 0x74, gate.height);
        m.set_f(GATE + 0x7c, gate.reach_sq);
        m.set_vf(FROM, from);
        m.set_vf(TO, to);
        match gate::crossed(m, GATE, FROM, TO) {
            gate::THROUGH => race::Crossing::Through,
            gate::BESIDE => race::Crossing::Beside,
            gate::OPEN => race::Crossing::Open,
            _ => race::Crossing::Not,
        }
    })
}

fn run(m: &mut Mem, at: &Situation, tune: &Tuning) -> Controls {
    let b = BRAIN;
    for o in (0..BRAIN_LEN).step_by(4) {
        m.set_u32(b + o, 0);
    }
    m.set_vf(b + 0xa0, at.up);
    m.set_vf(b + 0xb0, at.forward);
    m.set_f(b + 0xc4, at.height);
    m.set_f(b + 0x204, at.ground);
    m.set_vf(b + 0x2c0, at.ground_normal);
    m.set_vf(b + 0x70, at.spin);
    m.set_f(b + 0x80, at.speed);
    m.set_u32(b + 0x30, at.flags as u32);
    m.set_u32(b + 0x34, (at.flags >> 32) as u32);
    m.set_vf(b + 0x1d0, at.target_dir);
    m.set_f(b + 0x1e0, at.target_speed);
    m.set_f(b + 0x254, at.min_speed);
    m.set_f(b + 0xf4, at.pitch);
    for (o, v) in [
        (0x208, tune.steering_gain),
        (0x21c, tune.oversteer_gain),
        (0x2d0, tune.aim_gain),
        (0x2b4, tune.pitch_distance),
        (0x2d4, tune.pitch_gain),
        (0x2d8, tune.pitch_filter),
        (0x2ec, tune.landing_pitch),
        (0x260, tune.slow_tolerance),
        (0x264, tune.fast_tolerance),
        (0x32c, tune.cruise_base),
        (0x330, tune.cruise_gain),
        (0x334, tune.brake_gain),
        (0x100, tune.steer_scale),
        (0x104, tune.pitch_scale),
    ] {
        m.set_f(b + o, v);
    }
    ai::controls(m, &TABLES, b);
    Controls {
        steer: m.f(b + 0xf0),
        pitch: m.f(b + 0xf4),
        throttle: m.f(b + 0xe0),
        brake: m.f(b + 0xe4),
        heading: m.f(b + 0x1f4),
        aim: m.f(b + 0x1f0),
        roll: m.f(b + 0x1f8),
        flat_forward: m.vf(b + 0x270),
        target_speed: m.f(b + 0x1e0),
    }
}
