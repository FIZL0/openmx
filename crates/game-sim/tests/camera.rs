//! S7 — the camera for normal play, proved against the original.
//!
//! Twelve proofs. One for the sine table every rotation in the game reads, which is the one thing in
//! the ported module that is *not* bit-exact and so is the one proof with a tolerance in it. One for
//! the rotation itself. Then one for each of the camera's own routines — the field of view, the
//! forward/reverse test, the aim, the look-at point, the position, the eight lags, the preset switch
//! and the right stick — and finally one for a whole frame, which is also what proves the distance
//! limiting, since the original inlines that into its frame and has no routine for it.
//!
//! They all work the same way, and the way is worth explaining once. The original's camera is a C++
//! object that reads two other objects (a viewport and the rider's actor) and points at eight little
//! filter structs and an array of preset records. The harness lays all of that out in the
//! interpreter's RAM, writes a generated [`Follow`] into it, and runs the original. Then it writes the
//! *same* generated state again, applies the port to it, writes the port's answer over the top, and
//! compares the camera, the filters and the scratch out-parameters word for word. Comparing the
//! regions rather than a list of fields is deliberate: a field the original writes that the port does
//! not model shows up as a difference, which is exactly the bug a hand-written list of comparisons
//! hides. On top of that every call is checked for writes *outside* those regions — a write to the
//! presets, to the actor or to a global would be state the port cannot have.

use game_sim::air::AcosTable;
use game_sim::camera::{
    CURVE_ENTRIES, CurveTable, Follow, Nothing, Preset, Surface, Target, View, rotate,
};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip};
use std::cell::RefCell;
use std::rc::Rc;

/// The two tables the game builds at startup, where the harness has the game put them.
const ACOS: u32 = 0x0100_0000;
const CURVE: u32 = 0x0108_0000;

/// The camera object, its eight filters, and the scratch the out-parameters live in. These three are
/// the compared regions; everything after them is input the original must not write.
const CAM: u32 = 0x0120_0000;
const FILT: u32 = 0x0120_1000;
const SCR: u32 = 0x0120_2000;
/// Inside the scratch: the vectors the routines take and hand back.
const SCR_POSITION: u32 = SCR;
const SCR_LOOK_AT: u32 = SCR + 0x10;
const SCR_FOV: u32 = SCR + 0x20;
const SCR_SPARE: u32 = SCR + 0x24;
const SCR_HEADING: u32 = SCR + 0x30;
const SCR_VEL: u32 = SCR + 0x40;
const SCR_DIRECTION: u32 = SCR + 0x50;
const SCR_TARGET: u32 = SCR + 0x60;
const SCR_SIDEWAYS: u32 = SCR + 0x70;

const PRESETS: u32 = 0x0121_0000;
const VTABLE: u32 = 0x0122_0000;
const VIEWPORT: u32 = 0x0122_1000;
const ACTOR: u32 = 0x0122_2000;
const CONTROLS: u32 = 0x0122_3000;
const VP_VTABLE: u32 = 0x0122_4000;
/// Where the viewport's "here is what to look at" slot points. Nothing is there: a hook catches the
/// call and writes down the point, which is the only way that value leaves the original.
const SET_LOOK_AT: u32 = 0x0122_5000;

/// How many presets the generated cameras carry. Four is enough for a preset switch to have somewhere
/// to come from and somewhere to go.
const PRESET_COUNT: usize = 4;
/// One preset record, as the camera file's reader lays it out.
const PRESET: u32 = 0xa0;

const REGIONS: [(&str, u32, u32); 3] =
    [("camera", CAM, 0x180), ("filters", FILT, 0x100), ("scratch", SCR, 0x80)];

/// The class's own vtable, slot by slot, as the disc's copy holds it. The harness builds it out of
/// the names rather than copying the disc's table so that a slot that moves is a failed `addr`
/// lookup and not a wrong answer.
const SLOTS: [(u32, &str); 16] = [
    (0xf0, "camera_park_nudges"),
    (0xf4, "camera_unpark_nudges"),
    (0xf8, "camera_clear_nudges"),
    (0xfc, "camera_set_preset"),
    (0x100, "camera_set_look_filters"),
    (0x104, "camera_set_pos_filters"),
    (0x108, "camera_frame_noop"),
    (0x10c, "camera_ground_probe_zero"),
    (0x110, "camera_get_fov"),
    (0x114, "camera_forward_reverse"),
    (0x118, "camera_aim"),
    (0x11c, "camera_look_at"),
    (0x120, "camera_position"),
    (0x124, "camera_filter"),
    (0x128, "camera_load_presets"),
    (0x12c, "camera_nudge"),
];

/// pi as the game's own code writes it, for the generator's angle ranges.
const PI: f32 = 3.1415927;
/// How many generated cases each of the routine proofs runs.
const CASES: usize = 4000;

// --- the harness -----------------------------------------------------------------------------------

fn put_f(ee: &mut Ee, a: u32, v: F) {
    ee.set_f32(a, v.0);
}
fn put_b(ee: &mut Ee, a: u32, v: bool) {
    ee.set_u32(a, v as u32);
}
fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in [v.x(), v.y(), v.z(), v.w()].into_iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, x);
    }
}
fn get_v(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

/// One preset record. The 32-byte name at `+0x30` is left zeroed: the reader fills it and nothing in
/// the camera reads it, which is why [`Preset::name`] is not part of the comparison.
fn put_preset(ee: &mut Ee, at: u32, p: &Preset) {
    put_v(ee, at + 0x00, p.look_filters);
    put_v(ee, at + 0x10, p.pos_filters);
    put_v(ee, at + 0x20, p.offset_from_body_point);
    put_f(ee, at + 0x50, p.angle_y);
    put_f(ee, at + 0x54, p.angle_x);
    put_f(ee, at + 0x58, p.following_distance);
    put_f(ee, at + 0x5c, p.fov);
    put_f(ee, at + 0x60, p.reverse_angle_x_offset);
    put_f(ee, at + 0x64, p.reverse_following_offset);
    put_f(ee, at + 0x68, p.reverse_look_ahead_offset);
    put_f(ee, at + 0x6c, p.forward_reverse_switch_speed);
    put_b(ee, at + 0x70, p.cycle_thru_this_preset);
    put_b(ee, at + 0x74, p.filter_this_preset);
    put_f(ee, at + 0x78, p.look_ahead_distance);
    put_b(ee, at + 0x7c, p.is_look_back_cam);
    put_f(ee, at + 0x80, p.camera_swing);
    put_f(ee, at + 0x84, p.look_swing);
    put_f(ee, at + 0x88, p.min_following_distance);
    put_f(ee, at + 0x8c, p.max_following_distance);
    put_f(ee, at + 0x90, p.min_fov);
    put_f(ee, at + 0x94, p.max_fov);
    put_f(ee, at + 0x98, p.low_speed_angle_x_vel_cutoff);
    put_f(ee, at + 0x9c, p.low_speed_angle_x_scale_factor);
}

/// One filter: the value, the time constant, the blend it last used, and the pair of clamps the
/// constructor fills with plus and minus the largest float. Nothing in the camera reads the clamps,
/// so they are written here only so that both snapshots hold the same bytes.
fn put_lag(ee: &mut Ee, at: u32, l: &game_sim::camera::Lag) {
    put_f(ee, at + 0x0, l.value);
    put_f(ee, at + 0x4, l.tau);
    put_f(ee, at + 0x8, l.alpha);
    ee.set_f32(at + 0xc, f32::MAX);
    ee.set_f32(at + 0x10, -f32::MAX);
}

/// Where the eight filters live, in the original's order: the look-at's three, the position's three,
/// the field of view and the spare.
fn lag_at(k: u32) -> u32 {
    FILT + 0x20 * k
}

/// Write a camera into RAM: the presets it points at, the vtable, the two objects it reads, every
/// field of the object itself, and the eight filters with the pointers to them.
fn poke(ee: &mut Ee, c: &Follow) {
    for (i, p) in c.presets.iter().enumerate() {
        put_preset(ee, PRESETS + PRESET * i as u32, p);
    }
    for (slot, name) in SLOTS {
        ee.set_u32(VTABLE + slot, addr(name).unwrap());
    }
    ee.set_u32(VP_VTABLE + 0xc4, SET_LOOK_AT);
    ee.set_u32(VIEWPORT + 0x26c, VP_VTABLE);

    ee.set_u32(CAM + 0x08, VTABLE);
    ee.set_u32(CAM + 0x7c, VIEWPORT);
    ee.set_u32(CAM + 0x80, 0); // the base class's list of attached things: empty
    ee.set_u32(CAM + 0x8c, ACTOR);
    put_v(ee, CAM + 0xa0, c.offset);
    put_f(ee, CAM + 0xd0, c.distance);
    put_f(ee, CAM + 0xd4, c.yaw);
    put_f(ee, CAM + 0xd8, c.pitch);
    put_f(ee, CAM + 0xdc, c.base_fov);
    put_f(ee, CAM + 0xe0, c.camera_swing);
    put_f(ee, CAM + 0xe4, c.look_swing);
    ee.set_u32(CAM + 0xe8, 0); // no collision tester: the push-out is not ported
    for k in 0..3 {
        ee.set_u32(CAM + 0xf0 + 4 * k, lag_at(k));
        ee.set_u32(CAM + 0xfc + 4 * k, lag_at(3 + k));
        put_lag(ee, lag_at(k), &c.look_filter[k as usize]);
        put_lag(ee, lag_at(3 + k), &c.pos_filter[k as usize]);
    }
    ee.set_u32(CAM + 0x114, lag_at(6));
    ee.set_u32(CAM + 0x118, lag_at(7));
    put_lag(ee, lag_at(6), &c.fov_filter);
    put_lag(ee, lag_at(7), &c.spare_filter);
    put_f(ee, CAM + 0x11c, c.user_zoom);
    put_f(ee, CAM + 0x120, c.user_yaw);
    put_f(ee, CAM + 0x124, c.user_pitch);
    put_f(ee, CAM + 0x128, c.user_fov);
    for (k, v) in c.saved_user.iter().enumerate() {
        put_f(ee, CAM + 0x12c + 4 * k as u32, *v);
    }
    put_f(ee, CAM + 0x13c, c.reverse_angle_x);
    put_f(ee, CAM + 0x140, c.reverse_following);
    put_f(ee, CAM + 0x144, c.reverse_look_ahead);
    put_f(ee, CAM + 0x148, c.reverse_speed);
    put_f(ee, CAM + 0x14c, c.speed_reference);
    put_f(ee, CAM + 0x150, c.height_bias);
    ee.set_u32(CAM + 0x158, c.preset as u32);
    ee.set_u32(CAM + 0x15c, c.previous as u32);
    ee.set_u32(CAM + 0x160, c.presets.len() as u32);
    ee.set_u32(CAM + 0x164, PRESETS);
    put_b(ee, CAM + 0x168, c.snap);
    put_b(ee, CAM + 0x16c, c.pitch_down_locked);
    ee.set_u32(CAM + 0x170, 0);
    ee.set_u32(CAM + 0x174, 0);
}

/// The rider, as far as the camera can see it.
fn poke_target(ee: &mut Ee, t: &Target) {
    put_v(ee, ACTOR + 0x10, t.body_point);
    put_v(ee, ACTOR + 0x30, t.forward);
    put_v(ee, ACTOR + 0x40, t.up);
    put_v(ee, ACTOR + 0x50, t.vel);
    put_f(ee, ACTOR + 0x84, t.lean);
}

/// Zero every byte of every compared region, so that a byte the original writes and the port does not
/// cannot hide behind a value left over from the run before.
fn reset(ee: &mut Ee) {
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
}

/// Every word of every compared region, in order.
fn snap(ee: &Ee) -> Vec<u32> {
    let mut out = Vec::new();
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            out.push(ee.u32_at(base + o));
        }
    }
    out
}

/// Which field a differing word is, by name of its region and offset in it.
fn where_is(i: usize) -> String {
    let mut i = i as u32 * 4;
    for (name, _, len) in REGIONS {
        if i < len {
            return format!("{name} + {i:#x}");
        }
        i -= len;
    }
    "past the end".into()
}

/// Nothing the original writes may land outside the compared regions: a write to the presets, to the
/// rider or to a global would be state the port cannot have, and this is what would say so.
fn only_touched_the_regions(ee: &Ee) -> Result<(), String> {
    let frame = (ee.stack - 0x4000, ee.stack + 0x40);
    for (a, b) in ee.written_ranges() {
        let inside = REGIONS.iter().any(|&(_, base, len)| a >= base && b <= base + len)
            || (a >= frame.0 && b <= frame.1);
        if !inside {
            return Err(format!("the original wrote {a:#010x}..{b:#010x}, which is neither the camera nor the stack"));
        }
    }
    Ok(())
}

/// The two snapshots, word for word.
fn same(case: usize, what: &str, theirs: &[u32], ours: &[u32]) {
    let mut bad = 0;
    for (i, (&t, &o)) in theirs.iter().zip(ours).enumerate() {
        if t != o {
            if bad < 8 {
                println!("{what} case {case}: {} theirs {t:#010x} ours {o:#010x}", where_is(i));
            }
            bad += 1;
        }
    }
    assert_eq!(bad, 0, "{what} case {case}: {bad} words differ");
}

/// A machine with both startup tables in place, and the game's own `{cos, sin}` table handed back as
/// a [`CurveTable`] so that every rotation proof is bit-exact rather than approximate.
///
/// The arc cosine table written in is the port's own, which `tests/air.rs` has already proved is the
/// game's to the bit; doing it that way keeps every proof here off the 268 million instructions the
/// game's own builder takes.
fn harness() -> Option<(Ee, CurveTable, AcosTable)> {
    let mut ee = game_or_skip()?;
    ee.hooks.insert(addr("mem_alloc").unwrap(), Box::new(|ee| ee.set(2, CURVE as u64)));
    ee.call(addr("curve_table_build").unwrap(), &[], &[], 2_000_000_000);
    ee.hooks.remove(&addr("mem_alloc").unwrap());
    assert_eq!(ee.v0(), 1, "the game's curve table builder reported failure");
    assert_eq!(ee.u32_at(addr("curve_table_ptr").unwrap()), CURVE, "the table went somewhere else");
    let entries: Vec<[f32; 2]> = (0..CURVE_ENTRIES as u32)
        .map(|i| [ee.f32_at(CURVE + 8 * i), ee.f32_at(CURVE + 8 * i + 4)])
        .collect();

    let acos = AcosTable::build();
    for (i, e) in acos.entries().iter().enumerate() {
        ee.set_f32(ACOS + 4 * i as u32, *e);
    }
    ee.set_u32(addr("acos_table_ptr").unwrap(), ACOS);
    ee.set_f32(addr("acos_table_scale").unwrap(), 14999.5);
    Some((ee, CurveTable::from_entries(&entries), acos))
}

/// A generator. Plain host arithmetic: these are only inputs, and every one of them goes into the
/// original and into the port alike.
struct Gen(u64);

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn bits(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn unit(&mut self) -> f32 {
        self.bits() as f32 / 4294967296.0
    }
    fn f(&mut self, lo: f32, hi: f32) -> F {
        F(lo + (hi - lo) * self.unit())
    }
    fn b(&mut self) -> bool {
        self.bits() & 0x10000 != 0
    }
    fn one_in(&mut self, n: u32) -> bool {
        self.bits() % n == 0
    }
    /// Often exactly zero: a zero offset, a zero ramp and a zero speed are each a branch.
    fn maybe_zero(&mut self, lo: f32, hi: f32) -> F {
        if self.one_in(3) { F::ZERO } else { self.f(lo, hi) }
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, 1.0)
    }
    /// A direction. Not normalised with the port's own normalise, which is one of the things under
    /// test: a crude host normalise is enough to make a plausible axis.
    fn dir(&mut self) -> Vf {
        let (x, y, z) = (self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0, self.f(-1.0, 1.0).0);
        let n = (x * x + y * y + z * z).sqrt().max(1e-6);
        Vf::new(x / n, y / n, z / n, 1.0)
    }
    /// Three time constants. None of them may be zero: the filter divides by the time constant, as
    /// the original does, and a preset that wants no filtering says so with its own flag instead.
    fn taus(&mut self) -> Vf {
        Vf::new(self.f(0.05, 0.8).0, self.f(0.05, 0.8).0, self.f(0.05, 0.8).0, 0.0)
    }

    fn preset(&mut self) -> Preset {
        Preset {
            name: String::new(),
            look_filters: self.taus(),
            pos_filters: self.taus(),
            offset_from_body_point: if self.one_in(3) {
                Vf::new(0.0, self.f(-3.0, 3.0).0, self.f(-3.0, 3.0).0, 1.0)
            } else {
                self.vec(-3.0, 3.0)
            },
            angle_y: self.f(-PI, PI),
            angle_x: self.f(-1.7, 1.7),
            following_distance: self.f(6.0, 40.0),
            fov: self.f(50.0, 95.0),
            reverse_angle_x_offset: self.maybe_zero(0.0, 0.6),
            reverse_following_offset: self.maybe_zero(0.0, 8.0),
            reverse_look_ahead_offset: self.maybe_zero(0.0, 8.0),
            forward_reverse_switch_speed: self.maybe_zero(0.0, 1.0),
            cycle_thru_this_preset: self.b(),
            filter_this_preset: !self.one_in(4),
            look_ahead_distance: self.f(0.0, 25.0),
            is_look_back_cam: self.one_in(4),
            camera_swing: self.maybe_zero(0.0, 1.5),
            look_swing: self.maybe_zero(0.0, 1.5),
            min_following_distance: self.f(3.0, 10.0),
            max_following_distance: self.f(25.0, 120.0),
            min_fov: self.f(30.0, 55.0),
            max_fov: self.f(80.0, 120.0),
            low_speed_angle_x_vel_cutoff: self.f(0.0, 40.0),
            low_speed_angle_x_scale_factor: self.f(0.0, 1.0),
        }
    }

    /// A camera part way through a session: a preset applied, the player's four nudges somewhere, the
    /// reverse ramps somewhere, and eight filters holding something.
    fn cam(&mut self) -> Follow {
        let presets: Vec<Preset> = (0..PRESET_COUNT).map(|_| self.preset()).collect();
        let mut c = Follow::new(presets, self.f(0.5, 8.0));
        c.set_preset(self.bits() as usize % PRESET_COUNT);
        c.previous = self.bits() as usize % PRESET_COUNT;
        c.snap = self.one_in(4);
        c.offset = if self.one_in(3) {
            Vf::new(0.0, self.f(-3.0, 3.0).0, self.f(-3.0, 3.0).0, 1.0)
        } else {
            self.vec(-3.0, 3.0)
        };
        c.distance = self.f(3.0, 60.0);
        c.yaw = self.f(-PI, PI);
        c.pitch = self.f(-1.7, 1.7);
        c.base_fov = self.f(40.0, 110.0);
        c.camera_swing = self.maybe_zero(0.0, 1.5);
        c.look_swing = self.maybe_zero(0.0, 1.5);
        c.user_zoom = self.maybe_zero(-6.0, 12.0);
        c.user_yaw = self.maybe_zero(-PI, PI);
        c.user_pitch = self.maybe_zero(-1.7, 1.7);
        c.user_fov = self.maybe_zero(-30.0, 40.0);
        c.saved_user =
            [self.f(-5.0, 5.0), self.f(-1.0, 1.0), self.f(-1.0, 1.0), self.f(-10.0, 10.0)];
        c.reverse_angle_x = self.maybe_zero(0.0, 0.6);
        c.reverse_following = self.maybe_zero(0.0, 8.0);
        c.reverse_look_ahead = self.maybe_zero(0.0, 8.0);
        c.speed_reference = self.f(0.5, 3.0);
        c.height_bias = self.maybe_zero(0.0, 5.0);
        c.pitch_down_locked = self.one_in(3);
        for k in 0..3 {
            c.look_filter[k].value = self.f(-50.0, 50.0);
            c.pos_filter[k].value = self.f(-50.0, 50.0);
            c.look_filter[k].alpha = self.f(0.0, 1.0);
            c.pos_filter[k].alpha = self.f(0.0, 1.0);
        }
        c.fov_filter = game_sim::camera::Lag {
            value: self.f(40.0, 110.0),
            tau: self.f(0.05, 0.8),
            alpha: self.f(0.0, 1.0),
        };
        c.spare_filter = game_sim::camera::Lag {
            value: self.f(-1.0, 1.0),
            tau: self.f(0.05, 0.8),
            alpha: self.f(0.0, 1.0),
        };
        c
    }

    /// A rider: a point, two axes that are not quite each other's, a velocity and a lean.
    fn target(&mut self) -> Target {
        Target {
            body_point: self.vec(-200.0, 200.0),
            forward: self.dir(),
            up: self.dir(),
            vel: if self.one_in(5) { Vf::new(0.0, 0.0, 0.0, 1.0) } else { self.vec(-30.0, 30.0) },
            lean: self.maybe_zero(-1.0, 1.0),
        }
    }
}

// --- the two tables, and the rotation that reads them ----------------------------------------------

/// The engine's `{cos, sin}` table, against the one the game's own startup code builds.
///
/// This is the only proof in the file with a tolerance in it, and the reason is in the module's docs:
/// the game widens each angle to double, calls the SDK's double-precision `cos` and `sin`, and rounds
/// back, and this crate has no double transcendentals to do the same with. What the test says is how
/// far apart the two tables are — and the answer, printed, is what a reader has to decide is small
/// enough to aim a camera with.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_curve_table_is_the_games_own_to_a_rounding() {
    let Some(mut ee) = game_or_skip() else { return };
    ee.hooks.insert(addr("mem_alloc").unwrap(), Box::new(|ee| ee.set(2, CURVE as u64)));
    ee.call(addr("curve_table_build").unwrap(), &[], &[], 2_000_000_000);
    println!("the game's builder took {} instructions", ee.steps);
    assert_eq!(ee.v0(), 1, "the game's table builder reported failure");
    assert_eq!(ee.f32_at(addr("curve_table_invstep").unwrap()), 954.77045, "the index scale");

    let ours = CurveTable::build();
    assert_eq!(ours.entries().len(), CURVE_ENTRIES + 1, "6000 entries plus the duplicated last one");
    let (mut exact, mut worst, mut worst_at) = (0usize, 0.0f32, 0usize);
    for (i, e) in ours.entries().iter().take(CURVE_ENTRIES).enumerate() {
        let theirs = [ee.f32_at(CURVE + 8 * i as u32), ee.f32_at(CURVE + 8 * i as u32 + 4)];
        if theirs == *e {
            exact += 1;
        }
        for k in 0..2 {
            let d = (theirs[k] - e[k]).abs();
            if d > worst {
                worst = d;
                worst_at = i;
            }
        }
    }
    println!(
        "{exact} of {CURVE_ENTRIES} entries are bit-exact; the worst is entry {worst_at}, off by {worst:e}"
    );
    assert!(worst < 1e-6, "entry {worst_at} is off by {worst:e}, which is more than a rounding");
    // The duplicated last entry: the game builds 6000 and then copies the last one into a 6001st, so
    // that an index that rounds up off the end still reads something. Each table's duplicate is
    // checked against its own last entry, since the entries themselves are only equal to a rounding.
    let last = CURVE_ENTRIES as u32 - 1;
    assert_eq!(
        [ee.f32_at(CURVE + 8 * (last + 1)), ee.f32_at(CURVE + 8 * (last + 1) + 4)],
        [ee.f32_at(CURVE + 8 * last), ee.f32_at(CURVE + 8 * last + 4)],
        "the game's 6001st entry is not a copy of its 6000th"
    );
    assert_eq!(
        ours.entries()[CURVE_ENTRIES],
        ours.entries()[CURVE_ENTRIES - 1],
        "the port's 6001st entry is not a copy of its 6000th"
    );
}

/// The rotation every part of the game uses, and the one the camera turns its heading by twice.
///
/// Bit-exact, because the table it reads is the game's own: `CurveTable::from_entries` is what lets a
/// proof hand the port the disc's table instead of the one the port would build.
#[test]
fn the_rotation_is_the_games_own() {
    let Some((mut ee, curve, _)) = harness() else { return };
    let mut g = Gen(0x5eed_0001);
    for case in 0..CASES {
        let angle = if g.one_in(4) { g.f(-20.0, 20.0) } else { g.f(-PI, PI) };
        let (axis, v) = (g.dir(), g.vec(-100.0, 100.0));
        reset(&mut ee);
        put_v(&mut ee, SCR_DIRECTION, axis);
        put_v(&mut ee, SCR_TARGET, v);
        ee.call(addr("rotate_about_axis").unwrap(), &[SCR_DIRECTION as u64, SCR_TARGET as u64, SCR_POSITION as u64], &[angle.0], 100_000);
        only_touched_the_regions(&ee).unwrap();
        let theirs = get_v(&ee, SCR_POSITION);
        let ours = rotate(angle, axis, v, &curve);
        assert_eq!(
            [theirs.x().to_bits(), theirs.y().to_bits(), theirs.z().to_bits(), theirs.w().to_bits()],
            [ours.x().to_bits(), ours.y().to_bits(), ours.z().to_bits(), ours.w().to_bits()],
            "case {case}: angle {}, axis {axis:?}, v {v:?}", angle.0
        );
    }
}

// --- the camera's own routines ----------------------------------------------------------------------

/// The field of view: the preset's plus the player's offset, held between the preset's two limits,
/// with the offset rewritten when it hits one.
#[test]
fn the_field_of_view_is_the_games_own() {
    let Some((mut ee, _, _)) = harness() else { return };
    let mut g = Gen(0x5eed_0002);
    for case in 0..CASES {
        let mut c = g.cam();
        reset(&mut ee);
        poke(&mut ee, &c);
        ee.call(addr("camera_get_fov").unwrap(), &[CAM as u64], &[], 100_000);
        only_touched_the_regions(&ee).unwrap();
        let (theirs, theirs_fov) = (snap(&ee), ee.f0());

        let ours_fov = c.field_of_view();
        reset(&mut ee);
        poke(&mut ee, &c);
        same(case, "field of view", &theirs, &snap(&ee));
        assert_eq!(theirs_fov.to_bits(), ours_fov.0.to_bits(), "case {case}: the answer");
    }
}

/// Yaw and pitch, their wraps and their clamps, and the anti-windup rewrite of the player's pitch.
#[test]
fn the_aim_is_the_games_own() {
    let Some((mut ee, _, _)) = harness() else { return };
    let mut g = Gen(0x5eed_0003);
    for case in 0..CASES {
        let mut c = g.cam();
        // Swings far beyond anything a lean produces, so that the yaw wrap is exercised on both sides.
        let swing = if g.one_in(3) { g.f(-20.0, 20.0) } else { g.f(-1.0, 1.0) };
        reset(&mut ee);
        poke(&mut ee, &c);
        ee.call(addr("camera_aim").unwrap(), &[CAM as u64], &[swing.0], 100_000);
        only_touched_the_regions(&ee).unwrap();
        let theirs = snap(&ee);

        c.aim(swing);
        reset(&mut ee);
        poke(&mut ee, &c);
        same(case, "aim", &theirs, &snap(&ee));
    }
}

/// The forward/reverse test and the three ramps it drives.
#[test]
fn the_forward_reverse_test_is_the_games_own() {
    let Some((mut ee, _, acos)) = harness() else { return };
    let mut g = Gen(0x5eed_0004);
    let (mut reversing, mut ramping_down) = (0usize, 0usize);
    for case in 0..CASES {
        let mut c = g.cam();
        let heading = g.dir();
        // Half the time the rider is actually going roughly where it is pointed, which is the branch
        // that is not reverse; the other half is anywhere at all.
        let vel = if g.b() {
            Vf::new(heading.x(), heading.y(), heading.z(), 1.0)
        } else {
            g.vec(-30.0, 30.0)
        };
        let speed = g.f(0.0, 30.0);
        reset(&mut ee);
        poke(&mut ee, &c);
        put_v(&mut ee, SCR_HEADING, heading);
        put_v(&mut ee, SCR_VEL, vel);
        ee.call(
            addr("camera_forward_reverse").unwrap(),
            &[CAM as u64, SCR_HEADING as u64, SCR_VEL as u64],
            &[speed.0],
            100_000,
        );
        only_touched_the_regions(&ee).unwrap();
        let (theirs, theirs_said) = (snap(&ee), ee.v0() != 0);

        let ours_said = c.switch_forward_reverse(speed, heading, vel, &acos);
        reset(&mut ee);
        poke(&mut ee, &c);
        put_v(&mut ee, SCR_HEADING, heading);
        put_v(&mut ee, SCR_VEL, vel);
        same(case, "forward/reverse", &theirs, &snap(&ee));
        assert_eq!(theirs_said, ours_said, "case {case}: whether it is reverse");
        if theirs_said {
            reversing += 1;
        } else if c.reverse_following != 0.0 {
            ramping_down += 1;
        }
    }
    println!("{reversing} of {CASES} cases were reverse, {ramping_down} were ramping back down");
    assert!(reversing > 100 && ramping_down > 100, "the generator did not reach both branches");
}

/// The look-at point: the look-ahead, its two speed corrections, the swung heading it runs along, the
/// height bias, and the look-back camera's shortcut.
#[test]
fn the_look_at_point_is_the_games_own() {
    let Some((mut ee, curve, _)) = harness() else { return };
    let mut g = Gen(0x5eed_0005);
    for case in 0..CASES {
        // `mut`, and the scene below, only because the look-at asks the ground probe for a lift: over
        // `Nothing` that probe is the `return 0.0f` this class's vtable really holds, so what is
        // compared here is the original's own function. S23's journal entry has the rest.
        let mut c = g.cam();
        let t = g.target();
        let (speed, fov_ratio, swing) = (g.maybe_zero(0.0, 30.0), g.f(0.3, 1.0), g.maybe_zero(-1.0, 1.0));
        let (heading, reverse) = (g.dir(), g.b());
        reset(&mut ee);
        poke(&mut ee, &c);
        put_v(&mut ee, SCR_TARGET, t.body_point);
        put_v(&mut ee, SCR_HEADING, heading);
        put_v(&mut ee, SCR_VEL, t.vel);
        ee.call(
            addr("camera_look_at").unwrap(),
            &[
                SCR_LOOK_AT as u64,
                CAM as u64,
                SCR_TARGET as u64,
                SCR_HEADING as u64,
                SCR_VEL as u64,
                reverse as u64,
            ],
            &[speed.0, fov_ratio.0, swing.0],
            200_000,
        );
        only_touched_the_regions(&ee).unwrap();
        let theirs = snap(&ee);

        let ours = c.look_at(
            speed,
            fov_ratio,
            swing,
            t.body_point,
            heading,
            t.vel,
            reverse,
            &curve,
            &mut Nothing,
        );
        reset(&mut ee);
        poke(&mut ee, &c);
        put_v(&mut ee, SCR_TARGET, t.body_point);
        put_v(&mut ee, SCR_HEADING, heading);
        put_v(&mut ee, SCR_VEL, t.vel);
        put_v(&mut ee, SCR_LOOK_AT, ours);
        same(case, "look at", &theirs, &snap(&ee));
    }
}

/// A ground for the probe to find: a surface a fixed distance above — or below — whatever point it is
/// asked about, and a note of the point it was asked about.
///
/// A fixed rise is what lets a case *choose* the lift the probe will answer, which is the only way the
/// original can be made to answer the same one: its own probe needs a world object and a terrain tree,
/// and this harness has neither. A rise below the point takes the probe's other arm, the decay, where
/// the lift is zero because nothing has lifted it yet.
struct Dirt {
    rise: F,
    asked: Vf,
}

impl Surface for Dirt {
    fn on_surface(&mut self, point: Vf) -> Vf {
        self.asked = point;
        Vf::new(point.x(), (F(point.y()) + self.rise).0, point.z(), point.w())
    }
}

/// The ground probe: the point it is asked about, and where its answer lands in the look-at.
///
/// The class this harness builds answers zero for every point — slot `+0x10c` is a `return 0.0f`,
/// which is why `the_look_at_point_is_the_games_own` cannot see any of this: with a zero lift the probe
/// could be asking about anywhere. So the slot is hooked here. The port runs first, over a [`Dirt`]
/// that answers a chosen lift; the hook then hands the original *that* lift and writes down the point
/// it was asked about. What this proves is the probe's point and the order its answer is added in — the
/// lift's own arithmetic belongs to the derived class and is read off the disassembly, not run, which
/// S23's journal entry says in as many words.
#[test]
fn the_ground_probes_point_is_the_games_own() {
    let Some((mut ee, curve, _)) = harness() else { return };
    let asked = Rc::new(RefCell::new(Vf::ZERO));
    let answer = Rc::new(RefCell::new(0.0f32));
    {
        let (asked, answer) = (asked.clone(), answer.clone());
        ee.hooks.insert(
            addr("camera_ground_probe_zero").unwrap(),
            Box::new(move |ee| {
                let p = ee.r(5) as u32;
                *asked.borrow_mut() = get_v(ee, p);
                ee.set_f(0, *answer.borrow());
            }),
        );
    }
    let mut g = Gen(0x5eed_000c);
    let mut lifted = 0;
    for case in 0..CASES {
        let mut c = g.cam();
        // The look-back camera returns before the probe is ever asked, and has its own proof above.
        c.presets[c.preset].is_look_back_cam = false;
        let t = g.target();
        let (speed, fov_ratio, swing) = (g.maybe_zero(0.0, 30.0), g.f(0.3, 1.0), g.maybe_zero(-1.0, 1.0));
        let (heading, reverse) = (g.dir(), g.b());
        let mut dirt = Dirt { rise: g.f(-20.0, 20.0), asked: Vf::ZERO };

        // Ours first, because the hook has to hand the original the lift ours worked out.
        let ours =
            c.look_at(speed, fov_ratio, swing, t.body_point, heading, t.vel, reverse, &curve, &mut dirt);
        *answer.borrow_mut() = c.ground_lift.0;
        if c.ground_lift != F::ZERO {
            lifted += 1;
        }

        reset(&mut ee);
        poke(&mut ee, &c);
        put_v(&mut ee, SCR_TARGET, t.body_point);
        put_v(&mut ee, SCR_HEADING, heading);
        put_v(&mut ee, SCR_VEL, t.vel);
        ee.call(
            addr("camera_look_at").unwrap(),
            &[
                SCR_LOOK_AT as u64,
                CAM as u64,
                SCR_TARGET as u64,
                SCR_HEADING as u64,
                SCR_VEL as u64,
                reverse as u64,
            ],
            &[speed.0, fov_ratio.0, swing.0],
            200_000,
        );
        only_touched_the_regions(&ee).unwrap();
        let (theirs, theirs_asked) = (snap(&ee), *asked.borrow());

        reset(&mut ee);
        poke(&mut ee, &c);
        put_v(&mut ee, SCR_TARGET, t.body_point);
        put_v(&mut ee, SCR_HEADING, heading);
        put_v(&mut ee, SCR_VEL, t.vel);
        put_v(&mut ee, SCR_LOOK_AT, ours);
        same(case, "look at over ground", &theirs, &snap(&ee));
        assert_eq!(
            [
                theirs_asked.x().to_bits(),
                theirs_asked.y().to_bits(),
                theirs_asked.z().to_bits(),
                theirs_asked.w().to_bits()
            ],
            [
                dirt.asked.x().to_bits(),
                dirt.asked.y().to_bits(),
                dirt.asked.z().to_bits(),
                dirt.asked.w().to_bits()
            ],
            "case {case}: the point the ground probe was asked about"
        );
    }
    println!("{lifted} of {CASES} cases found ground above the probe's point and lifted the look-at");
    assert!(lifted > 100, "the generator did not reach the lift");
}

/// Where the camera is. The original's routine also computes a yaw-driven sideways swing into stack
/// slots nothing reads; this says the answer is the target point plus the direction times the
/// distance, and `the_positions_swing_is_dead_code` says the swing really is dead.
#[test]
fn the_position_is_the_games_own() {
    let Some((mut ee, _, _)) = harness() else { return };
    let mut g = Gen(0x5eed_0006);
    for case in 0..CASES {
        let c = g.cam();
        let (direction, target_point, look_at, sideways) =
            (g.dir(), g.vec(-200.0, 200.0), g.vec(-200.0, 200.0), g.dir());
        reset(&mut ee);
        poke(&mut ee, &c);
        put_v(&mut ee, SCR_DIRECTION, direction);
        put_v(&mut ee, SCR_TARGET, target_point);
        put_v(&mut ee, SCR_LOOK_AT, look_at);
        put_v(&mut ee, SCR_SIDEWAYS, sideways);
        ee.call(
            addr("camera_position").unwrap(),
            &[
                SCR_POSITION as u64,
                CAM as u64,
                SCR_DIRECTION as u64,
                SCR_TARGET as u64,
                SCR_HEADING as u64,
                SCR_LOOK_AT as u64,
                SCR_SIDEWAYS as u64,
            ],
            &[],
            100_000,
        );
        only_touched_the_regions(&ee).unwrap();
        let theirs = snap(&ee);

        let ours = c.position(direction, target_point);
        reset(&mut ee);
        poke(&mut ee, &c);
        put_v(&mut ee, SCR_DIRECTION, direction);
        put_v(&mut ee, SCR_TARGET, target_point);
        put_v(&mut ee, SCR_LOOK_AT, look_at);
        put_v(&mut ee, SCR_SIDEWAYS, sideways);
        put_v(&mut ee, SCR_POSITION, ours);
        same(case, "position", &theirs, &snap(&ee));
    }
}

/// The swing the position routine computes is dead code: the same camera at every yaw around the
/// circle gives the same position. If it were ever read, this is the test that would fail.
#[test]
fn the_positions_swing_is_dead_code() {
    let Some((mut ee, _, _)) = harness() else { return };
    let mut g = Gen(0x5eed_0007);
    for case in 0..200 {
        let mut c = g.cam();
        // The arm the original's swing is computed in needs a preset that is not the look-back cam.
        c.presets[c.preset].is_look_back_cam = false;
        let (direction, target_point) = (g.dir(), g.vec(-200.0, 200.0));
        let mut first = None;
        for step in 0..32 {
            c.yaw = F(-PI + 6.2831855 * step as f32 / 32.0);
            reset(&mut ee);
            poke(&mut ee, &c);
            put_v(&mut ee, SCR_DIRECTION, direction);
            put_v(&mut ee, SCR_TARGET, target_point);
            ee.call(
                addr("camera_position").unwrap(),
                &[
                    SCR_POSITION as u64,
                    CAM as u64,
                    SCR_DIRECTION as u64,
                    SCR_TARGET as u64,
                    SCR_HEADING as u64,
                    SCR_LOOK_AT as u64,
                    SCR_SIDEWAYS as u64,
                ],
                &[],
                100_000,
            );
            let got = get_v(&ee, SCR_POSITION);
            let bits = [got.x().to_bits(), got.y().to_bits(), got.z().to_bits(), got.w().to_bits()];
            match first {
                None => first = Some(bits),
                Some(was) => assert_eq!(was, bits, "case {case}: yaw {} moved the camera", c.yaw.0),
            }
        }
    }
}

/// The eight lags, the snap that loads them instead, and the preset that does not filter at all.
#[test]
fn the_filters_are_the_games_own() {
    let Some((mut ee, _, _)) = harness() else { return };
    let mut g = Gen(0x5eed_0008);
    let (mut filtered, mut snapped, mut skipped) = (0usize, 0usize, 0usize);
    for case in 0..CASES {
        let mut c = g.cam();
        let dt = if g.one_in(4) { g.f(0.8, 2.0) } else { g.f(0.0, 0.1) };
        let (mut position, mut look_at) = (g.vec(-200.0, 200.0), g.vec(-200.0, 200.0));
        let (mut fov, mut spare) = (g.f(40.0, 110.0), g.maybe_zero(-1.0, 1.0));
        let target_point = g.vec(-200.0, 200.0);
        if c.snap {
            snapped += 1;
        } else if c.preset().filter_this_preset {
            filtered += 1;
        } else {
            skipped += 1;
        }
        reset(&mut ee);
        poke(&mut ee, &c);
        put_v(&mut ee, SCR_POSITION, position);
        put_v(&mut ee, SCR_LOOK_AT, look_at);
        put_f(&mut ee, SCR_FOV, fov);
        put_f(&mut ee, SCR_SPARE, spare);
        put_v(&mut ee, SCR_TARGET, target_point);
        ee.call(
            addr("camera_filter").unwrap(),
            &[
                CAM as u64,
                SCR_POSITION as u64,
                SCR_LOOK_AT as u64,
                SCR_FOV as u64,
                SCR_SPARE as u64,
                SCR_TARGET as u64,
            ],
            &[dt.0],
            100_000,
        );
        only_touched_the_regions(&ee).unwrap();
        let theirs = snap(&ee);

        c.filter(dt, &mut position, &mut look_at, &mut fov, &mut spare);
        reset(&mut ee);
        poke(&mut ee, &c);
        put_v(&mut ee, SCR_POSITION, position);
        put_v(&mut ee, SCR_LOOK_AT, look_at);
        put_f(&mut ee, SCR_FOV, fov);
        put_f(&mut ee, SCR_SPARE, spare);
        put_v(&mut ee, SCR_TARGET, target_point);
        same(case, "filter", &theirs, &snap(&ee));
    }
    println!("{filtered} filtered, {snapped} snapped, {skipped} did not filter");
    assert!(filtered > 100 && snapped > 100 && skipped > 100, "the generator missed a branch");
}

/// Switching preset: the copies, the six new time constants, the snap it arms, and what happens to
/// the player's four nudges on the way into and out of a look-back camera.
#[test]
fn switching_preset_is_the_games_own() {
    let Some((mut ee, _, _)) = harness() else { return };
    let mut g = Gen(0x5eed_0009);
    let (mut parked, mut unparked) = (0usize, 0usize);
    for case in 0..CASES {
        let mut c = g.cam();
        let to = g.bits() as usize % PRESET_COUNT;
        if c.presets[to].is_look_back_cam {
            parked += 1;
        }
        if c.presets[c.preset].is_look_back_cam {
            unparked += 1;
        }
        reset(&mut ee);
        poke(&mut ee, &c);
        ee.call(addr("camera_set_preset").unwrap(), &[CAM as u64, to as u64], &[], 100_000);
        only_touched_the_regions(&ee).unwrap();
        let theirs = snap(&ee);

        c.set_preset(to);
        reset(&mut ee);
        poke(&mut ee, &c);
        same(case, "set preset", &theirs, &snap(&ee));
    }
    println!("{parked} switches parked the nudges, {unparked} took them back out");
    assert!(parked > 100 && unparked > 100, "the generator missed a look-back switch");
}

/// The right stick: the two modifiers that turn it into a zoom or a widen, the swing it is otherwise,
/// the refusal to pitch down while that is locked, and both wraps.
///
/// The original asks the pad layer for the two modifier controls by name and then reads their value
/// out of a table of 0x18-byte entries. The hook hands back a different entry for each of the two, so
/// that the proof can hold one down without the other.
#[test]
fn the_nudge_is_the_games_own() {
    let Some((mut ee, _, _)) = harness() else { return };
    let which = Rc::new(RefCell::new(0u32));
    {
        let which = which.clone();
        ee.hooks.insert(
            addr("pad_control_index").unwrap(),
            Box::new(move |ee| {
                let mut n = which.borrow_mut();
                ee.set(2, *n as u64);
                *n = 1 - *n;
            }),
        );
    }
    let mut g = Gen(0x5eed_000a);
    let mut held = [0usize; 4];
    for case in 0..CASES {
        let mut c = g.cam();
        let (x, y) = (g.f(-1.0, 1.0), g.f(-1.0, 1.0));
        let (zoom, fov) = (g.one_in(3), g.one_in(3));
        held[(zoom as usize) * 2 + fov as usize] += 1;
        *which.borrow_mut() = 0;
        reset(&mut ee);
        poke(&mut ee, &c);
        ee.set_f32(CONTROLS + 0x04, if zoom { 1.0 } else { 0.0 });
        ee.set_f32(CONTROLS + 0x1c, if fov { 1.0 } else { 0.0 });
        ee.call(
            addr("camera_nudge").unwrap(),
            &[CAM as u64, CONTROLS as u64, 0],
            &[0.0, 0.0, x.0, y.0],
            100_000,
        );
        only_touched_the_regions(&ee).unwrap();
        let theirs = snap(&ee);

        c.nudge(x, y, zoom, fov);
        reset(&mut ee);
        poke(&mut ee, &c);
        same(case, "nudge", &theirs, &snap(&ee));
    }
    println!("the four modifier combinations came up {held:?} times");
    assert!(held.iter().all(|&n| n > 100), "the generator missed a modifier combination");
}

// --- the whole frame -------------------------------------------------------------------------------

/// One frame of the camera, end to end: everything above in the original's order, plus the distance
/// limiting, which the original inlines into its frame and has no routine of its own for.
///
/// The three calls the original's last three lines make are hooked: two of them are how the view
/// leaves the camera (the viewport's own setters), and the third is the base class's update, which is
/// not the camera. What the hooks write down is exactly the [`View`] the port returns.
#[test]
fn one_frame_is_the_games_own() {
    let Some((mut ee, curve, acos)) = harness() else { return };
    let view = Rc::new(RefCell::new(View::default()));
    {
        let view = view.clone();
        ee.hooks.insert(
            addr("viewport_set_eye").unwrap(),
            Box::new(move |ee| {
                let (p, f) = (ee.r(5) as u32, ee.r(9) as u32);
                let mut v = view.borrow_mut();
                v.position = get_v(ee, p);
                v.fov = F(ee.f32_at(f));
            }),
        );
    }
    {
        let view = view.clone();
        ee.hooks.insert(
            SET_LOOK_AT,
            Box::new(move |ee| {
                let l = ee.r(5) as u32;
                view.borrow_mut().look_at = get_v(ee, l);
            }),
        );
    }
    ee.hooks.insert(addr("camera_base_update").unwrap(), Box::new(|_| {}));

    let mut g = Gen(0x5eed_000b);
    for case in 0..CASES {
        let mut c = g.cam();
        let t = g.target();
        let dt = if g.one_in(4) { g.f(0.8, 2.0) } else { g.f(0.0, 0.1) };
        let nearest = if g.one_in(3) { g.f(0.0, 40.0) } else { g.f(0.0, 6.0) };
        reset(&mut ee);
        poke(&mut ee, &c);
        poke_target(&mut ee, &t);
        ee.set_f32(VIEWPORT + 0x19c, nearest.0);
        *view.borrow_mut() = View::default();
        ee.call(addr("camera_update").unwrap(), &[CAM as u64], &[dt.0], 1_000_000);
        only_touched_the_regions(&ee).unwrap();
        let (theirs, theirs_view) = (snap(&ee), *view.borrow());

        let ours_view = c.update(dt, &t, nearest, &curve, &acos);
        reset(&mut ee);
        poke(&mut ee, &c);
        same(case, "one frame", &theirs, &snap(&ee));
        for (k, a, b) in [
            ("position", theirs_view.position, ours_view.position),
            ("look at", theirs_view.look_at, ours_view.look_at),
        ] {
            assert_eq!(
                [a.x().to_bits(), a.y().to_bits(), a.z().to_bits(), a.w().to_bits()],
                [b.x().to_bits(), b.y().to_bits(), b.z().to_bits(), b.w().to_bits()],
                "case {case}: the view's {k}"
            );
        }
        assert_eq!(
            theirs_view.fov.0.to_bits(),
            ours_view.fov.0.to_bits(),
            "case {case}: the view's field of view"
        );
    }
}
