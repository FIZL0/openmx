# 1 — s12 ai tick 133 (FINAL)

Task S347. 2026-11-18.

## What I did

| Rider | Before (agrees through tick) | After |
|-------|------------------------------|-------|
| ai0 | 132 | 186 |
| ai1 | 147 | 200 (torn rows 133, 155) |
| ai2 | 206 | 273 |

`C33_AGREES_THROUGH` in crates/game-sim/tests/ai/race4.rs is now `[186, 200, 1802, 273, 84]`.

- The fix is one word in crates/game-sim/src/ride.rs: the ground step's `mv::Step::gas` is the throttle
  lever (`c.gas`, the original's `+0x63c`), not the rate-limited `held.gas` (`+0x658`).
  `gas_rate_limited` stays `held.gas` (the idle-clutch rule and the engine read that one).
- Three readers in move.rs see the change: the launch ramp's rearm (`clutch`), the donut test, and the
  low-speed upright controller. In the original all three load `+0x63c` (bike_clutch at 0x002574b8,
  bike_stand_upright at 0x00257504; tests/move.rs's oracles already poke `st.gas` at `+0x63c`). So the
  oracles never saw the bug: it was in what ride.rs handed the step.
- Nothing from ../opentennis applied (no bike clutch there).

## What the original does

- At tick 131 ai0's brain writes clutch lever `+0x644` = 1 and gas `+0x63c` = 1 (the hand-off). Tick 132's
  frame: the rate limiter has gas `+0x658` at 0.3333, but bike_clutch reads the lever, so it rearms
  (timer `+0x6c0` = 0). At 133 the lever is out (0) and the ramp starts: the torque multiplier goes up
  (engine torque 38.34 vs the port's 19.93) and the centre of gravity's local z moves back by the
  ramp's travel (`+0x190` z: 0 -> -0.6927, written at 0x002574ac). The chassis integrate then takes its
  centre off the transform with the new local, which is the 0.6 ft jump in the row.
- ai1 does the same at 146/147/148 (local z -0.6722 at 148).
- The port kept ai0's timer at 1.5149 (finished) because `held.gas` was 0.3333, not 1.

## How it was proved

- The bit-exact replay of C33 (`s12e_free09s_four_racers_ride_as_the_capture`) with the raised marks:
  `./kit check -- -p game-sim --test ai s12e_free09s_four_racers` passed.
- The diagnosis came from temporary probes, removed before landing:
  - S328's whole frame in Ee (draws.rs) from C33's image, run beside the Sim: the Ee bodies match the
    capture through 150 (ai1's row 133 is torn, a late read);
  - `collision_record_sweep_back` hooked: no strike for any AI bike near 133 (not a sweep back);
  - `chassis_integrate` hooked for ai0/ai1: the first substep of 133 moves the centre the wrong way;
    its local centre (`+0x190`) changed;
  - an `Ee::log` store watch on `+0x190..+0x1a0` named the writer (0x002574ac, in bike_clutch);
  - beside it the Sim's `clutch_timer`, levers and `held` against the Ee rider's `+0x6c0`, `+0x644`,
    `+0x63c`, `+0x658`, `+0x65c` at ticks 126-149;
  - `./kit dis 0x2574a0`: the rearm compares `+0x644` and `+0x63c` with 1.0.

## Not verified / not 1:1

- **The human's ride with the lever as the step's gas is not re-proved on a capture here** beyond the
  gate's existing replays (they pass: see `./kit done`). A pad's lever and its rate-limited copy only
  differ for the two frames after a throttle press or release, so a human ride shows it only when the
  clutch rearm, a donut or the upright controller falls in those frames.
- **MX vs. ATV's handling clutch** (`handling::step::clutch`, the `mxvatv` branch in ride.rs) reads the
  same `Step::gas`, so it now gets the lever too. Its own original was not checked for which word it
  reads.
- **ai0 parts at tick 188** (187 not agreed), ai1 at 201, ai2 at 274: all three with the upward velocity
  ~1.7 ft/s low and a spin difference, on a jump face. At ai0's 188 the Ee frame shows the first tick
  of the AI's stick back (y -0.3) and the local centre's z 0.015 on both sides; the springs differ in
  that tick (front spring force 707 vs 635, rear 578 vs 450) with the wheels' contact points and
  normals equal. Not traced further. Filed as S347a.
- **The generator still parts at 86** (untouched; not this task).

## Next

- S347a: ai0's tick 188 (ai1's 201, ai2's 274) with the same probes: the substep forces of 188.
