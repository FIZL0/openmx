# 1 — the frames the port cannot step (FINAL)

Task V17. 2026-10-17.

V15 left two shapes of tick its stepping test would not compare, each with its shape asserted: the
frame a clip-to-clip transition **ends** on, and the frame a **wreck is declared** on. Both are now
compared. One of them turned out to be a port bug (the transition was simply not implemented in the
renderer); the other turned out to be a wrong reading of the recording, and the recording refutes it
itself.

## What I did

* **The clip-to-clip transition, ported.** `game_sim::motion::Played::fade` now carries a
  `motion::Cross` — the smooth-step weight, the clip the transition comes out of, and the clock that
  clip is sampled at — captured inside `Player::advance` *before* the advance's own end-of-frame
  reset. `game_render::skin::Clip::pose_crossed` is the sampler for it, with
  `game_render::skin::OutOf` for the two arms (the outgoing clip at the transition's clock, or the
  pose snapshot joint by joint). `game_render::anim::Set::apply_vehicle_crossed` dispatches it, and
  `Stance::frame` keeps the `Played` it used to throw away. `Set::apply_vehicle` and the five
  samplers it calls are untouched.
* **The stepping test** (`crates/game-sim/tests/motion.rs`,
  `the_ports_machine_steps_to_the_originals_clip_phase_and_pose`) poses the transition ticks instead
  of skipping them, and runs the wreck frame's missing step as the first of two on the pair that
  follows it. Both of V15's skips are gone from its `not posed` list; 1393 of the 1499 pairs are now
  posed and compared joint by joint, up from 1369, every one inside the 1e-4 the renderer is allowed.
* **Three helpers** in that test file — `captured_params`, `captured_request`, `captured_step` — so
  one pair can run two steps without a second copy of the step.
* `crates/game-sim/src/vehicle_anim.rs` needed **no change**: see below.

## What the original does

The transition is the motion object's second blend, and it is run by the advance, not by a selection
(`./kit fn 0x00144800`). With `+0x80` set the advance adds `dt` to the transition's clock `+0x88`,
computes `w` as the engine's smooth step over `((clock + dt) - delay - dt) / length` — the four
multiplies are inline at `./kit dis 0x144e1c 0x144e48` — and then branches on the **nullness of
`+0x4c`** (`./kit dis 0x144e48 0x144e90`):

* `+0x4c` non-null → `./kit fn 0x00145160`: the new clip at `+0x48` sampled at `+0x84`, the old clip
  at `+0x4c` sampled at the transition's clock `+0x88`, paired channel by channel **by position**,
  and the joint written is always the one the *new* clip's map names;
* `+0x4c` null → `./kit fn 0x00145810`: the same, with the snapshot buffers `+0x248` / `+0x24c`
  standing in for the old clip's samples, indexed by that same joint.

Both arms mix the same way: rotation `slerp(w, out_of, new)` through the shortest-path slerp
(`./kit fn 0x0013ee10`, whose signature is `(t, out, from, to, shortest)` — the output is the second
argument), translation `new + (out_of - new) * (1 - w)` in that grouping, which is the subtract,
broadcast-multiply, add the function builds in the vector unit.

Then the advance fires the action frames and **only then** drops the transition: `if (+0x8c <= +0x88)
{ +0x80 = 0; +0x88 = 0; }`. So the frame a transition ends on is posed with a weight of ~0.987 and
read back with no transition running at all. That is the whole of V15's first finding: the port drew
the new clip alone there, 1.3% of the outgoing pose was missing, and the shortest-path slerp negates
the new sample when the two quaternions face apart — "2.6e-3 away with every lane's sign flipped".

The mode dispatch (`./kit dis 0x249b20 0x249c40`) runs *after* the advance, so on modes 1, 2, 4 and 5
the blend entry overwrites what the transition posed and mode 0 — whose arm makes no call — is the
one the transition shows in. `apply_vehicle_crossed` is written that way.

## How it was proved

A PINE recording compared frame by frame: `context/capture/v12/rider-joints.csv` (V15's, 1500 ticks
at 30 Hz, 407 columns, game data and not in git), through the stepping test. Per tick the port's
machine is seeded from the previous row, stepped once, and the state, the ramp, the rate, the two
axes, the three cross phases, the reverse clock, the clip, the clip clock, the transition's clock and
all 22 joints are compared against the next row.

* **24 ticks are now posed with a transition running** — 8 out of the outgoing clip, 16 out of the
  pose snapshot, so both arms of `+0x4c` are exercised — worst lane `5.3e-5` (joint 6, ShoulderL, at
  tick 4175, which is one of the three end frames). The three ends are ticks 3267, 4175 (snapshot
  arm) and 3315 (clip arm).
* **The wreck frame does not skip the rider's animation step.** This recording has two wrecks and
  they are read on opposite sides of the step. Tick 3259 is row 3258 again in *every* column but
  `+0x748` — not just the machine and the joints: the request block holds still too, because the
  vehicle update writes the request and the wreck reason together and only the reason had landed when
  the read came. Tick 4167, the other wreck, is read after the step and shows that frame stepping:
  35 → 30, the ramp to 0.8667, the rate to 4.0, a new clip sampled — and the frame after it latching
  once more, to 40. One latch each frame, nothing held still. V15's "the vehicle skips the rider's
  animation step" and "the next frame latches twice" are one read that landed early in a long frame.
  (A second, independent recording of the same replay window agrees: 1485 of its 1498 common ticks
  are bit-equal to this one, and tick 3259 is one of the 13 that differ — there it stepped. That
  capture is only in this worktree, so the proof that stands in the test is the two-wrecks one.)
* The pair out of that early row is therefore run as **two steps** and compared against the row after
  it, which lands every field on the original's except the two eased axes — see below.
* `./kit check`: TESTS PASSED, 510 tests. The stepping test's own line: 1499 ticks, worst number `0e0`,
  1393 posed, worst joint 8.47e-5 at tick 3289 (a mode-0 tick that was already compared before V17).

## Not verified / not 1:1

* **The two eased axes on the two-step pair (tick 3260) are bounded, not matched.** The request block
  the missing step was handed is in no row of the recording, so that step is run with the newest one
  the recording has — one frame stale. The mix axis lands 2.7e-3 off and the phase axis 2.9e-3; both
  are asserted under 3e-2 instead of the 1e-6 every other pair holds. Solving the lag backwards says
  the frame's real mix axis was 0.94546 against the 0.93155 it was given, so the error is explained
  to the digit, but it is not a match. Nothing else on that pair depends on the request block, and
  the pair after it seeds both axes from the capture again.
* **The want for that missing step is derived, not captured** (`+0x8f4` is written after the read):
  it is taken as the state the step latched, `now`'s `+0x8ec` = 30, corroborated by the other wreck
  being caught asking for 30 on its own wreck frame. The port's own chooser (`Update::want`) is not
  run here — its inputs are not in this capture — so "the chooser asks 30 on a wreck frame because
  its wreck arm is gated on the request's `+0x104` and not on `+0x748`" is read off the
  decompilation, not proved.
* **The pair *into* that row is still not posed** (it is the one entry left in the test's `not posed`
  list, besides the slipped stretch): the animation did not step between those two reads at all, so
  there is no step of the port to compare. The skip's *claim* has changed — the recorder's sample
  phase, not a frame the game skipped — and that is what the assertion now says.
* **"The blend overwrites the transition on modes 1, 2, 4 and 5" is read off the dispatch order, not
  measured.** All 24 of this recording's transition ticks are mode 0 (states 40 and 45), so a
  transition under a two- or four-clip blend is not exercised by any capture here.
* **The transition is not bit-exact and is not claimed to be.** It is the renderer's side: the
  sampler, the arc-cos table and the Taylor sines are the port's (V5), so the end frames come in at
  5.3e-5 of a quaternion lane rather than at zero. The weight itself is `ps2-float` arithmetic and is
  compared to the bit through the clock (`+0x88`, off by `0e0` on all 1499 pairs).
* **`+0x8f8` is still unread** — it is not a constant on this object, and nothing in this minute
  reaches the arm that reads it (all three cross phases are zero on all 1500 ticks). Carried over
  from V11/V15.
* **One recording, one rider, one track, one minute** of a human replay: three wrecks, two of them
  inside it. No gesture, no reverse, no start line, and the pair blend (mode 5) is still posed by
  nothing.
* The rider's *own* chassis (`Rig::frame`, `Set::apply`) was left alone: nothing in the rider's
  animation section calls the engine's play, so it starts no transition. Unverified only in the sense
  that I did not search for a fourth caller beyond the four the oracle lists.
* `crates/game-sim/src/vehicle_anim.rs` is **documentation only** in this task. The transition needs
  nothing from the sixty-state machine: it is started by `Player::play`, which the played-once groups
  already reach through `Frame::play`, and `Blend::run` already answers with the `Played` that now
  carries it. The file is named in the task line because the finding it was expected to fix — the
  vehicle skipping the rider's step — is not a real finding.
* The capture itself is game data (git-ignored `context/`), so the test answers "no capture, nothing
  proved" on a clean checkout. The second recording used for the cross-check is only in this
  worktree.

## Next

Nothing outstanding for V17. Two things a later task could take:

1. Pose a transition *under* a two- or four-clip blend, to show the blend overwrites it. It wants a
   capture with a gesture or a stunt in it, not a wreck.
2. The slipped stretch (105 pairs of 1499, ticks 3603..3706) is still the largest thing the
   comparison leaves out, and it is the recorder's, not the port's: re-record the same window with
   the emulator slowed down (`watch/rider-joints.toml`).
