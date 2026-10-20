# 2 — the biplane flies under its own power (FINAL)

Task S41. 2026-10-20.

## What I did

- **The blocker in part 1 is gone.** S38 and S39 landed, and P31's commit 0586297 (already on this
  branch) wired `Flight::frame` in the original's order. Since a2b9027, `Sim::step` flies the plane on
  I6's mapping (`flight::pad`). So this session's work was proving that wiring and seeding the
  acceptance run correctly. `crates/game-sim/src` is unchanged; everything is in
  `crates/game-sim/tests/replay.rs`.
- **The capture's first row is one plane frame stale.** In `context/capture/acceptance/biplane.csv`,
  row 0 to row 1 is two plane frames, and every later pair is one:
  - the prop angle steps by 2 × 0.7583 rad, where every later pair steps by 0.7583;
  - `body_pos.z` creeps two frames' worth;
  - the port, stepped twice from row 0, lands on row 1's prop angle and position bit for bit.

  `past_the_first_row` drops that row, so the fixture's tick 0 is the second row.
- **The run's hidden state comes from C7's save state.** `context/states/biplane.p2m2_SaveState.eeMemory.bin`
  is the moment row 0 was read: its body position, velocity, quat, rates and prop angle all equal
  row 0's bits.
  - `off_the_save_state` checks those five against row 0. If they differ, it refuses the state.
  - Then it copies what the row does not carry: the whole rigid body (accumulators, last pose, shock,
    twist), the three tires' state, the engine's noise clock, and the session's noise tables
    (`SmoothNoise::from_tables`).
  - The run then steps that flight twice, on row 0's idle pad. `onto_the_row` puts the second row's
    recorded lanes back over the result, so tick 0 agrees by construction.
- **`TheOriginalsGround`**: a `World` whose `ground` runs the game's own ground query in the
  interpreter, against the world the plane's tires ask. I needed it because the port's `Ground::at` is
  this repo's own bilinear blend, and under the plane it answers 134.99857 where the game answers
  134.99858. That 1 ulp changes the strut force, and with it the body, on frame 1.
- **New test: `the_plane_flies_frame_for_frame_like_the_original`.** Starting from the save state, it
  runs one second (30 frames) of the game's `dynamics_world_step` in the interpreter beside `Sim::tick_in`.
  After every frame it compares, bit for bit:
  - the body: position, velocity, quat and rates;
  - each of the three tires: its ground point, compression, last compression, spring force and spin;
  - the engine: thrust, rpm, prop angle and noise clock.

  It passes.
- **`BIPLANE_AGREES_THROUGH` stays at 0.** Over the original's ground the run parts at tick 1:
  `plane.body.vel[0]` is −3.450012e-5 in the port and −3.451043e-5 in the capture. Over the port's own
  ground it also parts at tick 1, with −3.451057e-5 in the capture. The test prints both verdicts.
- `./kit check`: TESTS PASSED.

## What the original does

- One frame of the plane is `dynamics_world_step` on the dynamics world (oracle.txt names both). It is
  about 72k EE instructions. Given the same ground answers, the port's `Flight::frame` produces
  identical body, tire and engine words for at least 30 frames.
- **The game's real frame does more than that step.** Run the interpreter's world step alone from the
  save state, and it already differs from the capture on the first compared frame. For row 2's
  `body_vel.y`, the step alone gives 1.5421e-3 and the capture has 1.4945e-3.
- The plane's collision record (the pointer at the plane's `+0x170`) holds one contact in the state. The
  record's search is `./kit fn 0x002708a0`. I called it once before each interpreter world step, and
  the step's results did not change. So either the search needs setup I did not do, or the missing work
  is somewhere else in the game frame. S48's brief puts the contact search and resolve into
  `Flight::frame`.
- The original's terrain query, the one under the ground query, is `./kit fn 0x001dce80`. Its cell
  reader is `./kit fn 0x001ddaf0`. Neither is ported, and no plan task covers them.

## How it was proved

- **Function oracle, whole-frame:** `the_plane_flies_frame_for_frame_like_the_original` (tests/replay.rs).
  It runs 30 frames on the idle pad from C7's save state, and every frame checks 4 body vectors, 3
  ground vectors and 16 words bit for bit.
- **Save-state capture:** `the_biplane_acceptance_run` against `context/capture/acceptance/biplane.csv`.
  It agrees at tick 0 and parts at tick 1.

## Not verified / not 1:1

- **The run parts from the capture at tick 1, by a few ulp of `body.vel`.** The port matches the
  original's world step, but the game's frame does something beyond it. The suspect is the plane's
  collision search and resolve (S48). Calling the search directly did not reproduce the capture, so
  that cause is not confirmed.
- **The port's ground is not the original's.** `world::Ground::at` is a stand-in blend, 1 ulp low under
  the plane. The original's terrain query is unported, and no task covers it. The gated verdict and the
  frame-for-frame test both run over `TheOriginalsGround`, which needs the save state's RAM.
- **`TheOriginalsGround` answers no surface (it returns 0) and no sweep (`solid` is false).** Nothing in
  the plane's frame read either in these runs. But a flight that hits something solid would need the
  sweep.
- **The frame-for-frame proof covers only 30 frames on the idle pad, rolling on the ground.** It does
  not cover the air, the controls under input, or a crash.
- **The two pre-steps are flown on row 0's pad, which I assumed was the pad the game read on both
  frames.** Row 0 and row 1 both record it idle.
- **Why row 0 is stale is not proved.** The recorder reading before the game's step on its first frame
  is the explanation the frame counter fits. It was not traced.
- **I did not check the pre-stepped flight's `measure`, `crash` timers, or tire `frame` matrices against
  the game after the two steps.** `onto_the_row` keeps them from the port's steps, and they match only
  insofar as the 30-frame test's lanes imply.
- **Carried over from part 1:** the elevator lane parts at 111 on the fixture stick's 0xff→0xfe clamp,
  and the aileron lane parts at 150 on a torn capture row.

## Next

- S48: the contact search and resolve in `Flight::frame`'s substeps. Then raise `BIPLANE_AGREES_THROUGH`.
  If the run still parts at tick 1, trace the rest of the game's frame around the world step (whatever
  writes the body between two `dynamics_world_step` calls).
- A task for the terrain query, so the run can drop `TheOriginalsGround`.
