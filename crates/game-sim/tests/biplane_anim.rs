//! The biplane's pilot and moving parts against the original: the human's biplane recording (C7's
//! playback of `biplane.p2m2`), re-recorded with `watch/biplane-pilot.toml` so the pilot's body and
//! the plane's request block are in the rows beside the controls they come from.
//!
//! The test is named in the plan as living in `tests/biplane.rs`; S34 is writing that file at the
//! same time, so this one is its own file and the name is the plan's.

use game_sim::biplane_anim::{self, CONE_BENT, CONE_BLUR, PLANE, Pilot, ROLL_BCK, ROLL_FWD, Reset};
use game_sim::motion::{Animations, ClipId};
use game_sim::replay::Capture;
use game_sim::rider_anim::Body;
use game_sim::vehicle_anim::{self, AIR, Blend, Lag, Library, TURN_RIDING};
use ps2_float::F;
use std::collections::HashMap;

/// A set of clips that has every name it is asked for, handing out handles in the order they are
/// loaded.
#[derive(Default)]
struct Shelf {
    names: Vec<String>,
}

impl Animations for Shelf {
    fn load(&mut self, name: &str) {
        if !self.names.iter().any(|n| n == name) {
            self.names.push(name.to_owned());
        }
    }
    fn bound(&self, name: &str) -> Option<ClipId> {
        self.names.iter().position(|n| n == name).map(|i| ClipId(i as u32 + 1))
    }
}

/// The capture's cells by column name.
struct Rows {
    cap: Capture,
    at: HashMap<String, usize>,
}

impl Rows {
    fn cell(&self, row: usize, col: &str) -> &str {
        let i = *self.at.get(col).unwrap_or_else(|| panic!("no column {col}"));
        &self.cap.rows[row][i]
    }
    fn f(&self, row: usize, col: &str) -> F {
        F(self.cell(row, col).parse::<f64>().unwrap_or_else(|e| panic!("{col}: {e}")) as f32)
    }
    fn u(&self, row: usize, col: &str) -> u64 {
        self.cell(row, col).parse().unwrap_or_else(|e| panic!("{col}: {e}"))
    }
    fn i(&self, row: usize, col: &str) -> i32 {
        self.u(row, col) as u32 as i32
    }
    fn b(&self, row: usize, col: &str) -> bool {
        self.u(row, col) != 0
    }
    fn fc(&self, row: usize) -> u64 {
        self.u(row, "frame_counter")
    }
    fn lag(&self, row: usize, col: &str) -> Lag {
        Lag {
            value: self.f(row, &format!("{col}.x")),
            tau: self.f(row, &format!("{col}.y")),
            ratio: self.f(row, &format!("{col}.z")),
        }
    }
    /// A captured 4x4 as `[row][column]`.
    fn mat(&self, row: usize, col: &str) -> [[f64; 4]; 4] {
        let mut m = [[0.0; 4]; 4];
        for (i, r) in m.iter_mut().enumerate() {
            for (j, v) in r.iter_mut().enumerate() {
                *v = self.f(row, &format!("{col}.m{i}{j}")).0 as f64;
            }
        }
        m
    }
}

fn capture() -> Option<Rows> {
    let path = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("capture"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture"),
    }
    .join("biplane/pilot.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!(
            "skipped: no {} — play context/recordings/biplane.p2m2 back with C7's playback script and \
             watch/biplane-pilot.toml, frame counters 10971..12688 (journal \
             2026-10-20-s35-s35-the-biplane-moves)",
            path.display()
        );
        return None;
    };
    let cap = Capture::parse(&text).expect("the recorder's format");
    let at = cap.columns.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
    let c = Rows { cap, at };
    let n = c.cap.rows.len();
    assert_eq!((c.fc(0), c.fc(n - 1), n), (10971, 12688, 1718), "the capture is the one measured");
    Some(c)
}

/// The three rows whose request block matches neither its own tick's sticks nor the one before: torn
/// reads, by every sign — the row after each one is written from the torn row's own sticks again. The
/// test takes the block as captured on these and checks everything downstream of it as usual.
const TORN: [u64; 3] = [12020, 12049, 12465];

/// The rows read before the pilot's body had stepped on its tick: the lags are the row before's to the
/// bit, and the next row is exactly two of the port's steps on. The port steps on as usual and these
/// rows are checked against the row before instead.
const EARLY: [u64; 1] = [12167];

/// The pilot, end to end: the rider build's record, the mount, the plane's request block from the
/// controls and the reset flag, and the pilot's body playing the record off its two lags — every
/// number bit for bit, on every tick of the human's flight, against the original's own pilot.
///
/// The order inside a tick is the capture's to tell, and it does: the measure step writes the block
/// from the sticks as the *previous* tick left them (1714 of 1717 ticks; the three that are not are
/// [`TORN`]), the reset comes before the measure and the reset flag's latch after it, and the body
/// eases its lags towards *this* tick's block.
#[test]
fn the_biplane_asks_for_the_originals_clips() {
    let Some(c) = capture() else { return };
    let n = c.cap.rows.len();

    // 1 — the rider build's half, and the mount.
    let mut shelf = Shelf::default();
    let mut lib = Library::default();
    biplane_anim::register(&mut lib, true, PLANE, &mut shelf);
    // A second flying machine of the same type adds nothing: the first record of a key stays.
    biplane_anim::register(&mut lib, true, PLANE, &mut shelf);
    let (bck, fwd) = (shelf.bound(ROLL_BCK), shelf.bound(ROLL_FWD));
    assert_eq!(lib.extra, vec![vehicle_anim::Extra { key: PLANE, a: bck, b: fwd }]);
    assert!(biplane_anim::flies(1) && biplane_anim::flies(2) && !biplane_anim::flies(0));
    let mut body = Body::default();
    assert_eq!(body.veh.extra, -1, "a body nobody has mounted plays its own machine");
    body.veh.mount(&lib, PLANE);
    assert_eq!(c.u(0, "body_records"), lib.extra.len() as u64);

    // 2 — the body as the first row has it, and the two fade rates read off the restarts.
    let d = &mut body.veh;
    (d.want, d.state, d.prev) = (c.i(0, "body_want"), c.i(0, "body_state"), c.i(0, "body_prev"));
    (d.ramp, d.rate) = (c.f(0, "body_ramp"), c.f(0, "body_rate"));
    (d.mix, d.phase) = (c.lag(0, "mix_lag"), c.lag(0, "phase_lag"));
    let restarted = |state: i32| {
        (1..n)
            .find(|&i| c.i(i, "body_state") == state && c.i(i - 1, "body_state") != state)
            .map(|i| c.f(i, "body_rate"))
            .expect("the flight restarts both states")
    };
    let vp = vehicle_anim::Params {
        rate_pose: restarted(AIR),
        rate_default: restarted(TURN_RIDING),
        ..vehicle_anim::Params::default()
    };

    let mut latch = Reset { just: c.b(0, "just_reset"), seen: c.b(0, "reset_seen") };
    let mut block = Pilot::block();
    let (mut resets, mut air, mut torn, mut early) = (0, 0, 0, 0);
    for i in 1..n {
        let fc = c.fc(i);
        assert_eq!(c.i(i, "plane_type"), PLANE, "{fc}: the list moved");
        assert_eq!(c.u(i, "body_req"), c.u(i, "plane_addr") + 0x690, "{fc}: not the plane's pilot");
        assert_eq!(c.i(i, "body_extra"), body.veh.extra, "{fc}: the mounted record");

        // The reset, the measure, the latch.
        if c.b(i, "just_reset") && !latch.just {
            latch.reset();
            resets += 1;
        }
        let pilot = Pilot {
            elevator: c.f(i - 1, "elev_cmd"),
            aileron: c.f(i - 1, "ail_cmd"),
            held: latch.just,
            on_ground: c.b(i, "on_ground"),
            speed: c.f(i, "speed"),
            ..Pilot::default()
        };
        pilot.ask(&mut block);
        latch.frame();
        assert_eq!((latch.just, latch.seen), (c.b(i, "just_reset"), c.b(i, "reset_seen")), "{fc}: the latch");
        let r = &block.request;
        assert_eq!(r.held, c.b(i, "req_held"), "{fc}: holding");
        assert_eq!(r.at_base, c.b(i, "req_at_base"), "{fc}: at base");
        assert_eq!(block.airborne, c.b(i, "req_airborne"), "{fc}: airborne");
        assert_eq!(r.band.0.to_bits(), c.f(i, "req_band").0.to_bits(), "{fc}: band");
        let theirs = (c.f(i, "req_phase"), c.f(i, "req_mix"));
        if TORN.contains(&fc) {
            assert_ne!((r.phase_axis, r.mix_axis), theirs, "{fc}: no longer torn");
            (block.request.phase_axis, block.request.mix_axis) = theirs;
            torn += 1;
        } else {
            let bits = |x: F| x.0.to_bits();
            assert_eq!(
                (bits(r.phase_axis), bits(r.mix_axis)),
                (bits(theirs.0), bits(theirs.1)),
                "{fc}: the axes' targets — ours {:?}, theirs {theirs:?}",
                (r.phase_axis, r.mix_axis)
            );
        }

        // The pilot's body: the chooser's pick, then the step.
        body.veh.want = if block.airborne { AIR } else { TURN_RIDING };
        assert_eq!(body.veh.want, c.i(i, "body_want"), "{fc}: the state asked for");
        air += usize::from(block.airborne);
        let frame = body.veh.frame(c.f(i, "dt"), &vp, &block.request, &lib, false, |_| F::ONE);
        let d = &body.veh;
        assert_eq!((d.state, d.prev), (c.i(i, "body_state"), c.i(i, "body_prev")), "{fc}: the states");
        if EARLY.contains(&fc) {
            assert_eq!((c.lag(i, "mix_lag"), c.lag(i, "phase_lag")), (c.lag(i - 1, "mix_lag"), c.lag(i - 1, "phase_lag")));
            early += 1;
            continue;
        }
        assert_eq!((d.mix, d.phase), (c.lag(i, "mix_lag"), c.lag(i, "phase_lag")), "{fc}: the lags");
        assert_eq!((d.ramp, d.rate), (c.f(i, "body_ramp"), c.f(i, "body_rate")), "{fc}: the ramp");
        let unit = |x: F| x.max(F::ZERO).min(F::ONE);
        assert_eq!(
            frame.blend,
            Blend::Two { phase: unit(d.phase.value), mix: unit(d.mix.value), ramp: d.ramp, a: bck, b: fwd },
            "{fc}: the clips"
        );
    }
    assert_eq!((resets, torn, early), (2, TORN.len(), EARLY.len()), "the flight is the one measured");
    assert!(air > 1000 && n - 1 - air > 200, "both states are played: {air} ticks in the air");
}

/// A 3x3 rotation about a unit axis by `t`, in the PS2's row-vector order: `v' = v · R`.
fn rot(axis: [f64; 3], t: f64) -> [[f64; 3]; 3] {
    let [x, y, z] = axis;
    let (s, c) = t.sin_cos();
    let k = 1.0 - c;
    [
        [c + x * x * k, x * y * k + z * s, x * z * k - y * s],
        [y * x * k - z * s, c + y * y * k, y * z * k + x * s],
        [z * x * k + y * s, z * y * k - x * s, c + z * z * k],
    ]
}

fn off(a: [[f64; 3]; 3], b: &[[f64; 4]; 4]) -> f64 {
    (0..3).flat_map(|i| (0..3).map(move |j| (i, j))).map(|(i, j)| (a[i][j] - b[i][j]).abs()).fold(0.0, f64::max)
}

/// The moving parts against the bones the original posed, on every tick: the surface step's angles
/// turned into matrices the way the drawing side has to, against the bone matrices the plane's model
/// was left with.
///
/// This one only reaches the picture, so it is a tolerance and not bits: the rear flaps, rudder and
/// prop agree to a few millionths, the wing flaps to about a thousandth (they turn about the hinge the
/// plane kept beside the rest matrix, which this test stands in for with the rest matrix's own first
/// row). The surface step sees the crash as the tick before left it, unless this tick's reset has
/// cleared it: it runs after the reset and before the wreck test.
#[test]
fn the_biplanes_parts_move_with_its_controls() {
    let Some(c) = capture() else { return };
    let (mut wrecked, mut swung) = (0, 0.0f64);
    for i in 1..c.cap.rows.len() {
        let fc = c.fc(i);
        // The surface step runs after the reset and before the wreck test: the first wrecked row's prop
        // has still turned, and so has the prop on the row the reset puts the plane back.
        let crashed = c.u(i - 1, "crash") != 0 && c.u(i, "crash") != 0;
        let s = biplane_anim::surfaces(
            c.f(i, "pitch_cmd"),
            c.f(i, "max_elevator"),
            c.f(i, "aileron"),
            c.f(i, "rudder"),
            c.f(i, "prop_angle"),
            crashed,
        );
        // The top left flap: its rest matrix, turned about its hinge first.
        let rest = c.mat(i, "rest_top_left");
        let hinge = {
            let [x, y, z] = [rest[0][0], rest[0][1], rest[0][2]];
            let n = (x * x + y * y + z * z).sqrt();
            [x / n, y / n, z / n]
        };
        let turn = rot(hinge, s.flaps[1].0 as f64);
        let mut want = [[0.0; 3]; 3];
        for (r, row) in want.iter_mut().enumerate() {
            for (k, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|j| turn[r][j] * rest[j][k]).sum();
            }
        }
        let bone = c.mat(i, "bone_top_left");
        assert!(off(want, &bone) < 2e-3, "{fc}: top left flap");
        assert_eq!(bone[3], rest[3], "{fc}: the flap stays on its hinge");
        // Its mirror turns the other way.
        assert_eq!(s.flaps[3], -s.flaps[1]);
        swung = swung.max((s.flaps[1].0 as f64).abs());
        // The rear flap and the rudder: a rotation about a fixed axis, the position kept.
        assert!(off(rot([1.0, 0.0, 0.0], s.rear.0 as f64), &c.mat(i, "bone_rear_left")) < 1e-5, "{fc}: rear flap");
        assert!(off(rot([0.0, 1.0, 0.0], s.rudder.0 as f64), &c.mat(i, "bone_rudder")) < 1e-5, "{fc}: rudder");
        // The prop: turned by the engine while it flies, left where it was once wrecked.
        let prop = c.mat(i, "bone_prop");
        match s.prop {
            Some(angle) => {
                assert!(off(rot([0.0, 0.0, 1.0], angle.0 as f64), &prop) < 1e-5, "{fc}: prop");
                assert_eq!(s.cone, CONE_BLUR);
            }
            None => {
                assert_eq!(prop, c.mat(i - 1, "bone_prop"), "{fc}: a wrecked prop does not turn");
                assert_eq!(s.cone, CONE_BENT);
                wrecked += 1;
            }
        }
    }
    assert!(wrecked > 100, "the flight ends in a wreck: {wrecked} ticks");
    assert!(swung > 0.1, "the ailerons move: {swung} rad at most");
}
