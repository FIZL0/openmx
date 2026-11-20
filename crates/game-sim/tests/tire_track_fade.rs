//! R49 — does a tyre track switched off fade back in? The original's `vehicle_fx_track_update`, run
//! in the interpreter on a track as its off (`vehicle_fx_track_off`) leaves it, frame after frame,
//! beside the port's [`Track::update`].
//!
//! Off leaves no fade-out and all of the fade-in to go, and while the strip is ended the update takes
//! the tyre's point as its last, so `d` is 0: a marking frame there has `f = 0`, lays nothing and
//! keeps the strip ended. What starts it again is a frame that does **not** mark (the tyre neither
//! spinning nor pressed past its threshold, or the bike at or under its speed): that branch sets the
//! fade-in left to `f · FadeIn` with `f = out / FadeOut = 0`, so the next marking frame is at full
//! strength. Off the ground the track is switched off and not updated, so after a jump or a wreck
//! the first live frame that does not mark restarts it; a tyre pressed past its threshold on every
//! live frame never lays again, in both.
//!
//! The strip's two calls are stood in for by hooks: the add (vt +0x78) sets the strip going when its
//! alpha byte (`$f14 · 128`) is not 0 and the end (vt +0x7c) ends it, as [`Strip`]'s.

use game_sim::air::AcosTable;
use game_sim::rng::Rng;
use game_sim::tire_track::{Params, Snapshot, Track, Tyre};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip};
use std::cell::Cell;
use std::rc::Rc;

/// The track, the manager it hangs off (`+0x30` the bike's words), the bike's words (`+0x20` the
/// speed's pointer), the tyre's record (`+0x0` point, `+0x4` normal, `+0x10` compression, `+0x14`
/// spinning, each a pointer), the values they point at, the strip and its vtable.
const TRACK: u32 = 0x0120_0000;
const MGR: u32 = 0x0120_1000;
const BIKE: u32 = 0x0120_2000;
const REC: u32 = 0x0120_3000;
const AT: u32 = 0x0120_4000;
const NORMAL: u32 = 0x0120_4010;
const COMPRESSION: u32 = 0x0120_4020;
const SPINNING: u32 = 0x0120_4030;
const SPEED: u32 = 0x0120_4040;
const STRIP: u32 = 0x0120_5000;
const VTABLE: u32 = 0x0120_6000;
/// Where the strip's add and end are hooked.
const ADD: u32 = 0x0121_0000;
const END: u32 = 0x0121_0010;

/// The disc's rear track's numbers (free09's `mxparticles.dat`).
fn params() -> Params {
    Params {
        tyre: 1,
        disable: 0,
        zones: [0; 16],
        width: F(0.275),
        break_angle: Params::break_angle(F(10.0)),
        min_speed: F(3.0),
        min_compression: F(0.75),
        max_compression: F(1.0),
        base_alpha: F(0.4),
        fade_in: F(1.0),
        fade_out: F(10.0),
        height: F(0.3),
        segment: F(8.0),
        u_scale: F(1.0),
    }
}

/// One frame of the tyre: where it is, how pressed, spinning, and the bike's speed.
#[derive(Clone, Copy, Debug)]
struct Frame {
    z: f32,
    compression: f32,
    spinning: bool,
    speed: f32,
}

/// What a frame leaves: fade-out and fade-in left, and whether the strip was handed a row.
type After = (u32, u32, bool);

fn original(ee: &mut Ee, p: &Params, frames: &[Frame]) -> Vec<After> {
    let added = Rc::new(Cell::new(false));
    let a = Rc::clone(&added);
    ee.hooks.insert(
        ADD,
        Box::new(move |ee: &mut Ee| {
            a.set(true);
            let a8 = (128.0 * ee.f(14).clamp(0.0, 1.0)) as i32;
            if a8 > 0 {
                ee.set_u32(STRIP + 0x40, 0);
            }
        }),
    );
    ee.hooks.insert(END, Box::new(|ee: &mut Ee| ee.set_u32(STRIP + 0x40, 1)));
    ee.set_u32(TRACK, MGR);
    ee.set_u32(TRACK + 0x4, REC);
    ee.set_u32(TRACK + 0x14, STRIP);
    ee.set_u32(MGR + 0x30, BIKE);
    ee.set_u32(BIKE + 0x20, SPEED);
    for (o, at) in [(0x0, AT), (0x4, NORMAL), (0x10, COMPRESSION), (0x14, SPINNING)] {
        ee.set_u32(REC + o, at);
    }
    ee.set_u32(STRIP + 0x8, VTABLE);
    ee.set_u32(VTABLE + 0x78, ADD);
    ee.set_u32(VTABLE + 0x7c, END);
    for (o, v) in [(0x40, p.fade_out), (0x48, p.fade_in), (0x50, p.min_speed), (0x54, p.min_compression), (0x58, p.max_compression), (0x5c, p.base_alpha), (0x60, p.width), (0x64, p.height)] {
        ee.set_f32(TRACK + o, v.0);
    }
    // As off leaves it: the strip ended, no fade-out, all of the fade-in to go.
    ee.set_u32(STRIP + 0x40, 1);
    ee.set_f32(TRACK + 0x44, 0.0);
    ee.set_f32(TRACK + 0x4c, p.fade_in.0);
    for (k, v) in [0.0f32, 0.0, -50.0, 1.0].into_iter().enumerate() {
        ee.set_f32(TRACK + 0x30 + 4 * k as u32, v);
    }
    for (k, v) in [0.0f32, 1.0, 0.0, 0.0].into_iter().enumerate() {
        ee.set_f32(NORMAL + 4 * k as u32, v);
    }
    let mut out = Vec::new();
    for f in frames {
        for (k, v) in [0.0f32, 0.0, f.z, 1.0].into_iter().enumerate() {
            ee.set_f32(AT + 4 * k as u32, v);
        }
        ee.set_f32(COMPRESSION, f.compression);
        ee.set_u32(SPINNING, u32::from(f.spinning));
        ee.set_f32(SPEED, f.speed);
        added.set(false);
        ee.call(addr("vehicle_fx_track_update").unwrap(), &[TRACK as u64], &[], 100_000);
        out.push((ee.u32_at(TRACK + 0x44), ee.u32_at(TRACK + 0x4c), added.get()));
    }
    out
}

fn port(p: &Params, frames: &[Frame]) -> Vec<After> {
    let (acos, mut rng) = (AcosTable::build(), Rng::default());
    let mut t = Track::new(*p);
    t.out_left = F::ZERO;
    t.in_left = p.fade_in;
    t.last = Vf([0.0, 0.0, -50.0, 1.0]);
    frames
        .iter()
        .map(|f| {
            let tyre = Tyre {
                at: Vf([0.0, 0.0, f.z, 1.0]),
                normal: Vf([0.0, 1.0, 0.0, 0.0]),
                ground: true,
                object: false,
                compression: F(f.compression),
                spinning: f.spinning,
                zone: 0,
            };
            let snap = Snapshot { tyres: [tyre; 2], speed: F(f.speed), wrecking: false };
            t.update(&snap, &acos, &mut rng);
            (t.out_left.0.to_bits(), t.in_left.0.to_bits(), t.draws == 1)
        })
        .collect()
}

/// Riding on at 30 ft/s, a foot a frame, pressed past the threshold (`marks`) or not.
fn ride(marks: &[bool]) -> Vec<Frame> {
    marks.iter().enumerate().map(|(n, &m)| Frame { z: n as f32, compression: if m { 0.9 } else { 0.5 }, spinning: false, speed: 30.0 }).collect()
}

#[test]
fn a_switched_off_track_lays_again_after_a_frame_that_does_not_mark() {
    let Some(mut ee) = game_or_skip() else { return };
    let p = params();
    // Pressed past the threshold from the landing on: off stays off.
    let pressed = ride(&[true; 12]);
    let theirs = original(&mut ee, &p, &pressed);
    assert_eq!(theirs, port(&p, &pressed));
    assert!(theirs.iter().all(|&(out, inn, added)| out == 0 && inn == p.fade_in.0.to_bits() && !added), "{theirs:?}");
    // One frame not pressed, then pressed: the fade-in is gone and the next frame lays at full strength.
    let mut marks = vec![true; 4];
    marks.extend([false, true, true, true]);
    let relaxed = ride(&marks);
    let theirs = original(&mut ee, &p, &relaxed);
    assert_eq!(theirs, port(&p, &relaxed));
    assert!(!theirs[..5].iter().any(|a| a.2), "{theirs:?}");
    assert_eq!(theirs[4].1, 0, "the frame not marking leaves no fade-in");
    assert!(theirs[5..].iter().all(|a| a.2), "{theirs:?}");
    // Below the speed it does not mark either: a track switched off on the grid starts once it moves.
    let mut grid: Vec<Frame> = (0..3).map(|_| Frame { z: 0.0, compression: 0.9, spinning: false, speed: 0.0 }).collect();
    grid.extend(ride(&[true; 4]).into_iter().map(|f| Frame { z: f.z + 1.0, ..f }));
    let theirs = original(&mut ee, &p, &grid);
    assert_eq!(theirs, port(&p, &grid));
    assert!(theirs[3..].iter().all(|a| a.2), "{theirs:?}");
}

#[test]
fn a_switched_off_track_fades_as_the_original_on_any_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let p = params();
    let mut s = 0x2545_f491u32;
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    };
    for case in 0..64 {
        let mut z = 0.0f32;
        let frames: Vec<Frame> = (0..24)
            .map(|_| {
                z += (next() % 300) as f32 / 100.0;
                Frame {
                    z,
                    compression: (next() % 1100) as f32 / 1000.0,
                    spinning: next() % 5 == 0,
                    speed: [0.0, 2.0, 3.0, 5.0, 30.0][(next() % 5) as usize],
                }
            })
            .collect();
        assert_eq!(original(&mut ee, &p, &frames), port(&p, &frames), "case {case}: {frames:?}");
    }
}
