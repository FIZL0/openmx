# 1 — kiowa rotor (FINAL)

Task S220. 2026-11-02.

## What I did

- `crates/game-sim/src/kiowa.rs`, appended after S148's controller:
  - `euler_quat(angles, table)`: the original's Euler-to-quaternion (`euler_to_quat`). Each half
    angle goes through `wrap` twice and the curve table, then is multiplied out on the FPU
    accumulator.
  - `Rotor` and `RotorState`, with `Rotor::step(commands, state, table) -> (force, moment)`
    (`kiowa_rotor_step`).
  - `Aero`, `AeroState` and `Airflow`, with `Aero::step(pad, airflow, state) -> (force, moment)`
    (`kiowa_aero_step`).
  - `Readings` and `Kiowa`, with:
    - `Kiowa::force(readings, body, table, h, wrapper)` (`kiowa_forces`, every substep).
    - `Kiowa::preforce(body)` (`kiowa_preforce`, once a frame).

    The wrapper's own force (gravity and contacts, `body_wrapper_force`) is passed in as a closure,
    as the biplane passes `collision_force`.
- Renamed S148's `Flight::low_speed_aero` to `high_speed_aero`. The word the controller reads is the
  aerodynamics' +0x100, and the key string table puts `HighSpeedAero` there (`LowSpeedAero` is +0xfc).
  The value is 36.67 ft/s (25 mph), so only the name and the docs changed.
- `research/oracle.txt`: an S220 block naming the five functions, the rotor and aero field maps, and
  the corrections to S148's lines.
- Follow-ups: S220a (the measure listener) and S220b (the pad input step, then the tick that joins
  everything into `biplane::World::step`).
- The state file was copied from s3's `context/states/` into s2's. It is git-ignored and not
  committed.

## What the original does

- **Rotor.**
  - Tilt and blade pitches:
    - tilt z = lateral cyclic × MaxLateralCyclic
    - tilt x = longitudinal × MaxLongitudinalCyclic
    - collective pitch = collective × MaxCollective
    - tail pitch = pedal × the tail's MaxPitchAngle

    Each is skipped if its bound pointer is null; on the live helicopter none is null.
  - Thrust: k = LiftCurveSlope·(rho/4·blades)·chord·rate·r², and T = k·((0 + pitch·0.667·rate·r) − 0).
    The original really does add and subtract zero, which matters for −0.
  - The thrust vector (0, T, 0, 1) is turned by the tilt's quaternion with the same VU0 sequence as
    `chassis::turn`, then handed to body vt+0x58.
  - The moment (long·Mx, pedal·My, lat·Mz) goes to vt+0x5c. Its w lane is stack garbage, and the body
    adds xyz only.
  - The step also computes the hub's velocity (rates × hub position + body-frame velocity), turns it
    by the tilt, and throws the result away.
  - The tail rotor's blade figures, the main rotor's Inertia/Position and MinAltitude/MaxAltitude are
    read by the file reader but no step reads them; the PC exe's step doesn't either.
- **Aero.**
  - Inputs and setup:
    - beta is clamped to ±15°.
    - ground = sqrt(vx² + vz²).
    - The level velocity (vx, 0, vz) is taken into the body frame.
    - fade = clamp((ground − LowSpeedAero)/(HighSpeedAero − LowSpeedAero)), or 0 when the two are equal.
    - q = area·|speed|²·density/2.
    - The roll and pitch rate damping is scaled by span/(2|speed|) and chord/(2|speed|), or 0 at or
      under 1 ft/s.
  - Coefficients (the yaw damping is not scaled):
    - lift = CLalpha·α
    - drag = CD0
    - side = CYbeta·β
    - roll = Clβ·β + Clp·p̂ + Clda·lat
    - pitch = CM0 + CMα·α + CMq·q̂ + CMde·long
    - yaw = Cnβ·β + Cnr·r + Cndr·pedal
  - The moments take the raw pad, not the controller's commands.
  - Force: q·fade·(side, lift, −drag·sign(speed)). Moment: (chord·pitch, span·yaw, span·roll), both
    scaled by q·fade.
  - Slow-speed drag on the level velocity, LowSpeedDrag·density/2·area:
    - It is subtracted below HighSpeedAero when the stick is < 0.3.
    - Otherwise, when the stick is ≤ −0.3, it is subtracted scaled by |α|.
- **Force listener.**
  - Unless the wreck word (+0x3d4) is set, the controller (vt+0x1a0) runs.
  - Then the wrapper's force runs.
  - Then, unless wrecked, the rotor runs and then the aero.
- **Preforce listener.** It runs the wrapper preforce (shock peak = (0,0,0,1)), then the MX slot +0x19c,
  which is an empty function.

## How it was proved

- `crates/game-sim/tests/kiowa.rs` has five new tests on S148's state, all bit-exact.

  | Test | Cases | What it varies and checks |
  | --- | --- | --- |
  | `the_tilts_quaternion_is_the_originals` | 10,000 | Angles, including several turns out. |
  | `the_rotor_is_the_originals` | 10,000 | Every parameter the step reads is varied about its shipped value (including 0 and the negation), along with the commands (sometimes ±3), the prior thrust lanes and the rates. It checks the state words and the vectors handed to the body's hooked force/moment slots. |
  | `the_aerodynamics_are_the_originals` | 10,000 | All 20 read parameters, lo == hi, speeds of exactly ±1, sticks of exactly ±0.3, β past the clamp, and random orientations. Branch counts: 2464 faded, 5342 slow-drag, 696 pulled-back drag, 3329 at or under 1 ft/s. |
  | `the_force_listener_is_the_originals` | 10,000 | 1247 wrecked cases. The controller runs for real; the wrapper force is hooked and counted. It checks the commands, both models' states, and the body's force/moment after the port's listener against the vectors the original handed the body. |
  | `the_preforce_listener_is_the_originals` | 1 | — |

- `./kit check -- -p game-sim --test kiowa`: 6 passed. Also run with `--nocapture` to see that none of
  them skipped.
- Mutations that make the tests fail:
  - stick `< 0.3` changed to `<=`
  - back drag `<=` changed to `<`
  - `≤ 1` changed to `< 1` (it survived until the exact ±1 speeds were added)
  - no β clamp
  - α not abs
  - the drag sign taken from |speed|
  - no lo == hi branch
  - the thrust's ±0 adds dropped
  - the thrust's product order changed
  - the controller run while wrecked
- Mutations that survive, and are bit-identical: the order of the two terms in an accumulator sum,
  `mula/madd` for the ground speed's squares, `-(fade·sign)` for `(-fade)·sign`, and a single `wrap`
  instead of two.
- `./kit audit`: only the existing `plan/DONE.md:519/:533` leaks.

## Not verified / not 1:1

- The rotor's null-pointer guards are not ported. The port always has its commands, and the live
  helicopter's pointers are all bound.
- The rotor's discarded hub-velocity work is not ported, since it has no effect.
- The rotor moment's w lane is garbage in the original; the port writes 0. No body reads it.
- No ride or capture was replayed: the helicopter is not yet stepped by `biplane::World::step`
  (S220b). The measure that fills `Readings` (speed, α, β, airspeed, pitch, roll) is S220a's.
- The force-listener test checks the body's accumulators by refeeding the original's vectors through
  the port's `add_force_local`/`add_moment_local`. Those two are the biplane's proven ports, not
  re-proven on this body. The Kiowa body's +0xd0 force listeners (if any) are not modelled.
- Who reads the rotor's MinAltitude/MaxAltitude, if anyone, is unknown. The controller's altitude
  limit is its own `MaxAltitude` (S148).
- The double `wrap` was not shown to matter at any angle.

## Next

S220a, then S220b.
