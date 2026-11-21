//! The bridge (S496): the sim's rider through the lifted controller must hand back what the port
//! hands back, to the bit, so `--ai` changes nothing a race can see. The port is the original's (S12b,
//! `game-sim/tests/ai/controls.rs`), and the lift is too (`tests/lift.rs`); this proves the layout in
//! between.

use game_lift::bridge;
use game_sim::ai::{self, Controls, Situation, Tuning, flag};
use game_sim::race;
use game_sim::ragdoll::Tables;
use ps2_float::{F, Vf};

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
    fn float(&mut self, lo: f32, hi: f32) -> F {
        F(lo + (hi - lo) * ((self.next() >> 40) as f32 / (1u64 << 24) as f32))
    }
    fn unit(&mut self) -> Vf {
        loop {
            let v = [self.float(-1.0, 1.0).0, self.float(-1.0, 1.0).0, self.float(-1.0, 1.0).0];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l > 0.1 && l <= 1.0 {
                return Vf([v[0] / l, v[1] / l, v[2] / l, self.float(-1.0, 1.0).0]);
            }
        }
    }
}

fn case(p: &mut Pick) -> (Situation, Tuning) {
    let speed = p.float(0.0, 80.0);
    let flags = [flag::ON_GROUND, flag::OVERSTEER_CORRECTION, flag::CRUISE, flag::GRADED_BRAKE, 0x0100_0000_0000]
        .iter()
        .filter(|_| p.below(2) == 0)
        .fold(0x03e0_b80b, |a, b| a | b);
    let at = Situation {
        up: p.unit(),
        forward: p.unit(),
        height: p.float(-20.0, 60.0),
        ground: p.float(-20.0, 40.0),
        ground_normal: p.unit(),
        spin: p.unit(),
        speed,
        flags,
        target_dir: p.unit(),
        target_speed: p.float(-10.0, 150.0),
        min_speed: p.float(0.0, 40.0),
        pitch: p.float(-1.5, 1.5),
    };
    let tune = Tuning {
        steering_gain: p.float(0.0, 6.0),
        oversteer_gain: p.float(-1.0, 1.0),
        aim_gain: p.float(-3.0, 3.0),
        pitch_distance: p.float(0.0, 30.0),
        pitch_gain: p.float(-4.0, 4.0),
        pitch_filter: p.float(0.0, 1.0),
        landing_pitch: if p.below(8) == 0 { p.float(-3.0e5, 3.0e5) } else { p.float(-20.0, 20.0) },
        slow_tolerance: p.float(0.0, 10.0),
        fast_tolerance: p.float(0.0, 10.0),
        cruise_base: p.float(0.0, 0.05),
        cruise_gain: p.float(0.0, 2.0),
        brake_gain: p.float(0.0, 0.25),
        steer_scale: p.float(-1.5, 1.5),
        pitch_scale: p.float(-1.5, 1.5),
    };
    (at, tune)
}

fn bits(c: &Controls) -> [u32; 12] {
    let v = c.flat_forward.0.map(f32::to_bits);
    [
        c.steer.to_bits(),
        c.pitch.to_bits(),
        c.throttle.to_bits(),
        c.brake.to_bits(),
        c.heading.to_bits(),
        c.aim.to_bits(),
        c.roll.to_bits(),
        c.target_speed.to_bits(),
        v[0],
        v[1],
        v[2],
        v[3],
    ]
}

#[test]
fn the_bridged_gate_answers_as_the_port_over_twenty_thousand_steps() {
    assert!(race::LIFTED.get().is_none(), "this test compares against the port");
    let mut p = Pick(0x6a7e_5496_0bad_cafe);
    let mut seen = [0usize; 5];
    for n in 0..20_000 {
        let normal = p.unit();
        let g = race::Gate {
            flags: p.next() as u32 & 0x1f | if p.below(8) != 0 { 1 } else { 0 },
            centre: Vf([p.float(-20.0, 20.0).0, p.float(-3.0, 3.0).0, p.float(-20.0, 20.0).0, 1.0]),
            normal,
            across: Vf([normal.z(), p.float(-0.2, 0.2).0, -normal.x(), 0.0]),
            width: p.float(-5.0, 40.0),
            height: p.float(-2.0, 10.0),
            reach_sq: p.float(-100.0, 2000.0),
        };
        let mid = Vf([g.centre.x() + p.float(-25.0, 25.0).0, p.float(-5.0, 12.0).0, g.centre.z() + p.float(-25.0, 25.0).0, 1.0]);
        let len = p.float(0.0, 6.0).0;
        let d = Vf([(normal.x() + p.float(-0.5, 0.5).0) * len, normal.y() * len, (normal.z() + p.float(-0.5, 0.5).0) * len, 0.0]);
        let back = if p.below(4) == 0 { -1.0 } else { 1.0 };
        let from = Vf([mid.x() - back * d.x(), mid.y() - back * d.y(), mid.z() - back * d.z(), 1.0]);
        let to = Vf([mid.x() + back * d.x(), mid.y() + back * d.y(), mid.z() + back * d.z(), 1.0]);
        let port = g.crossed(from, to);
        assert_eq!(bridge::crossed(&g, from, to), port, "case {n}: {g:?} {from:?} {to:?}");
        seen[port as usize] += 1;
    }
    for code in [0, 1, 2, 4] {
        assert!(seen[code] > 300, "answer {code} came up only {} times in 20,000: {seen:?}", seen[code]);
    }
}

#[test]
fn the_bridged_lift_hands_back_the_ports_controls_over_twenty_thousand_ticks() {
    assert!(ai::LIFTED.get().is_none(), "this test compares against the port");
    let tables = Tables::build();
    let mut p = Pick(0xb21d_9e00_5496_0001);
    for n in 0..20_000 {
        let (at, tune) = case(&mut p);
        let port = ai::controls(&tables, &at, &tune);
        let lift = bridge::controls(&tables, &at, &tune);
        assert_eq!(bits(&lift), bits(&port), "case {n}: {at:?} {tune:?}");
    }
}
