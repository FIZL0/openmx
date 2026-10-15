# 1 — s6 motion (FINAL)

Task S6. 2026-10-15.

## What I did

Created `crates/game-sim/src/motion.rs` and `crates/game-sim/tests/motion.rs`, and registered the
twelve functions the task needed in `research/oracle.txt` under a new "S6 — animation driving" section.

The module is three parts, because the original is:

- **the frame's header** (`Driver::frame`) — the cross-fade ramp falls by `blend_rate * dt` and clamps,
  the restart test runs, a restart snaps the ramp back to 1, and the fore-aft lean is mapped to 0..1
  before the switch sees it;
- **the selection** (`Driver::select`) — the action-id switch: a stunt plays its own clip held at the
  trick's progress, the airborne idle blends a pair of clips chosen by the lean, a grounded rider holds
  the middle clip, and an action whose set has no clip falls back on that same middle clip;
- **the playback** (`Player`, `Clip`, `Fade`) — the clip clock, the loop and the end, the transition's
  smooth-step weight, and the window one frame opens for the action frames inside it.

Also `Driver::freeze`, which is the two pose writes a held rider's idle makes rather than anything the
driving does.

## What the original does

**The header and the switch.** The rider's animation header is the top of its per-frame animation step;
read it with `./kit fn 0x00257e00`. It drops the no-hander flag pair, falls the ramp, computes the
restart test once (the same test `trick.rs` ports — a different action, a forced restart, or the same
gesture asked again with a new sequence number ≥ 2), and on a restart asks the motion object to
snapshot the rider's current joint poses into its two blend buffers, which is what the ramp fades
*from* (`./kit fn 0x00147270` — it is a pose snapshot, not a clip rewind, and `oracle.txt` now says so).
Then the switch on the action id. Only three arms matter to the simulation; the rest hand the motion
library nothing and let whatever is playing run on.

The airborne arm is the one with a simulation-visible output: it remaps the crossup angle from
`±MaxCrossupAngle` onto 0..1 with the engine's own clamped remap, writes it to **both** the shown-pose
field and `+0x788`, and `+0x788` is what `air::Loads::landing_crossup` reads and the wreck test judges.
So the animation decides part of whether a landing stands up. Then it picks the pair: at or above the
middle, back-and-middle with the mix running 0..1 over the top half; below it, middle-and-forward over
the bottom half. The middle clip is in both pairs, so the mix is continuous through neutral but runs
its full range twice across the rider's travel.

**The library.** Four entry points, one per blend mode, all recorded in `oracle.txt`: one clip
(`0x00146870`), two (`0x00146ed0`), three (`0x00147e40`), and the free-running advance (`0x00144800`).
Mode 3 is reachable in the image but not from the rider's switch, so it is unmodelled.

**The advance.** `0x00144800`: the clock steps by `dt`, the end test is `duration < t` (strictly — a
clip whose clock lands exactly on its duration has not ended), a looping clip wraps and a non-looping
one latches finished and holds its last pose. The transition weight is a smooth step over
`(clock - delay - dt) / length`, and its grouping is not negotiable: the disassembly at
`./kit dis 0x144dc0 0x144e60` divides, then takes `2u`, then `u*u` on its own, then `3 - 2u`, then one
multiply — `(3 - 2u) * (u * u)`, which is one ULP away from `((3 - 2u) * u) * u`. The `/ 1.0` the
original leaves in the normalise is kept too, because the PS2's divider is approximate and `x / 1.0`
is not always `x`.

**The window.** `0x00144f70` turns the frame's clip times into a window and hands it to `0x001363a0`,
which binary-searches the clip's key list and fires every key inside it, inclusive at both ends. A
frame that wraps a looping clip opens *two* windows — the tail of the clip and the head — and fires both,
in that order. `0x001363a0`'s bracket is `max(upper_bound(from) - 1, 0) .. upper_bound(to)`, which has
a visible quirk: with duplicate keys sitting exactly on the window's start, the earlier duplicate falls
outside the bracket and only the later one fires. The port reproduces it (`Clip::after`), because an
action frame firing once instead of twice is behaviour, not rounding.

## How it was proved

Four tests in `crates/game-sim/tests/motion.rs`, three of them against the oracle, all bit-exact:

1. `the_driving_matches_over_ten_thousand_frames` — the rider's animation header and switch over
   **10,000 generated frames** (5,222 of them restarting, 1,450 airborne, 857 asking for an action the
   set has no clip for). The four library entry points are hooked to record what they were asked for
   and return at the sentinel; `trick.rs`'s own update runs first for the progress, as the original
   orders it. Compares the six fields, the library call and the restart test.
2. `the_frame_advance_matches_over_ten_thousand_frames` — the free-running advance over **10,000
   frames** (2,048 looped, 2,028 ended, 3,579 inside a transition, 2,723 inert): the clock, the previous
   clock, the last clip, the finished flag, the transition flag and its weight.
3. `the_action_frame_window_matches` — the window in two parts: `0x001363a0` called directly over
   **10,000 generated key lists and windows** (5,129 firing, 1,811 firing twice), and the wrapped
   two-window case over **2,000 frames**, with the heap hooked onto a bump allocator and the action
   objects called through fabricated vtables so the original really does walk its own container.
4. `the_frame_counts_are_the_games` — port only, because no oracle call can say what the numbers mean
   in frames: the cross-fade is **ten frames** at thirty a second and not nine, a restart puts the ramp
   back to a whole 1, a frozen rider has no cross-fade at all, the crossup sweep's ends and middle, the
   airborne pair at five leans, an action frame a quarter into a stunt's clip fires on **two consecutive
   frames**, a one-second looping clip comes round on frame **31**, and a one-second clip ends on frame
   31 with its last action frame firing on that frame.

## Not verified / not 1:1

- **The channels themselves are not ported.** What a clip *looks* like — sampling its rotation and
  position channels, the quaternion slerp at `0x0013ee10`, the VU0 skinning that applies the result — is
  the renderer's, and this task is the timing. Nothing in the simulation reads a joint pose.
- **Blend mode 3** (`0x00147e40`, three clips) is in the image but no arm of the rider's switch sets it
  — Ghidra drops the two blocks that would — so it is not modelled. If a mode-3 caller turns up, this
  module has no answer for it.
- **`MaxCrossupAngle` was not read off the disc.** `Params::max_crossup` has no proven retail value;
  only `blend_rate`'s 3.0 is proved, from the constructor. The remap is proved at whatever value the
  test hands it, so the shape is right and the number is not.
- **The transition flag's setter is not ported**, only its clock and its weight. Whoever starts a
  transition is outside this task; `Fade::on` has to be set by the caller.
- **The restart test is duplicated** between `trick.rs` and `motion.rs` (`restarted`), and so is the
  engine's clamped remap. Deliberate: S6 was told not to edit files it did not come for, and both are
  private one-liners. They should be shared when something else needs them.
- **The pose snapshot is hooked, not run.** Proof 1 stubs `0x00147270` out, so what the cross-fade
  fades *from* is unproven — only that a restart happened on the right frame and the ramp was 1.
- **The no-hander flag pair at `+0xa44`/`+0xa48`** is cleared at the top of the original's animation
  header, inside this task's territory, but the flags belong to `trick.rs` and are left to it.
- **Nothing was proved on the live emulator.** No PINE recording, no save-state capture; every proof is
  the function oracle. A whole-frame comparison would catch an ordering mistake between the animation
  header and the rest of the rider update that these per-function proofs cannot.

## Next

Nothing for S6. For whoever takes the renderer's side of the motion library: the clip and motion-object
offsets are in `oracle.txt` under "S6 — animation driving", and `Blend` is already the shape the
sampler needs — a clip, a phase and a weight.
