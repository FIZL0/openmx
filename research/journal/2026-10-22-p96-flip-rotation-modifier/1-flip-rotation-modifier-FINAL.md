# 1 — flip rotation modifier (FINAL)

Task P96. 2026-10-22.

## What I did

- `crates/game-sim/src/air.rs`: a shared hook, `air::Spin(k)`. It is one rotation-speed factor that every
  rotating move reads. It does not push the bike harder. It speeds the air's rotation up in time: in the air
  the turning is `I w' = M - D w`, and that is the same motion `k` times as fast when
  - every moment is `k^2` (`Spin::moment`),
  - every term against the angular velocity is `k` (`Spin::damping`),
  - every ramping moment ramps `k^3` (`Spin::moment_rate`),
  - every angular window is `k` (`Spin::rate`),
  - every rotation clock runs `/k` (`Spin::time`).

  The hook is read in three places:
  - **Rotation rows**: a new `Rotation::tuned(&Params)` is now the only way `rotate` reads a row's tuning
    (both of its call sites). With `Params::spin` (new field, `None` from the loader) set, it scales the
    torques by k² and their rates by k³. That covers the disc's backflip, the port's flat spin, and any row
    added later with no change.
  - **The stick's rotation in the air**: front flips and back flips by pitching, 360s by yawing, and rolls.
    `Spin::params` runs once at load and scales `AirborneControlMoment` and `BrakeGyroTorque` by k²;
    `AirborneDamping`, `UnrollGain/Torque` and `UnyawGain/Torque` by k; and the control-authority time by
    /k. It leaves the six `[Backflip]` keys alone, because the row hook already scales them and doing both
    would count them twice.
  - **The chassis' angular windows**: `Mods::ride` scales the windows (the yaw one is the file's
    `YawRateLimit`) by k and the angular-speed limit by k². This matters: the disc's 125 holds yaw to 3
    rad/s, so without it a 360 cannot get faster.
- `crates/game-sim/src/modifiers.rs`: a new `Knob::Rotation` (`rotation_speed`, "Flip rotation speed",
  per player) takes over `Knob::SpinRate`'s menu row. `SpinRate` (`spin_rate`) stays as it was, off
  `Knob::MENU`, following P95's `PlaneSpeed` precedent: a preset or tape that carries `spin_rate` still parses
  and rides its torque-only numbers. `KNOBS` is now 16 and `MENU` is `KNOBS - 2`. The menu
  (`crates/openmx/src/menu.rs`) builds its rows from `Knob::MENU`, label and blurb, so it needed no edit.
- `crates/game-sim/src/phy.rs` and `tests/air.rs`: added `spin: None` to the two `air::Params` literals.
- opentennis has no counterpart (tennis has no air rotation), so nothing was taken from it.

## What the original does

The disc's `race250p.phy` (and the other bikes) has no `[Frontflip]` section and no air key beyond these:
- `[Airborne Control]`: AirborneDamping 450, AirborneControlMoment −400/500/−150, BrakeGyroTorque, Unroll*,
  Unyaw*
- `[Backflip]`: the six keys
- `[Mass Properties] YawRateLimit = 3`

The original's only rotating *move* is the backflip, a torque row (`air::BACKFLIP`). A front flip, a 360 or a
roll is the stick's `AirborneControlMoment` against `AirborneDamping`, so its turn rate is that moment over the
damping, held inside the chassis' angular windows. That is why scaling the backflip torques alone (P55) did not
change what a player sees in the same way, and never touched front flips or spins.

## How it was proved

`crates/game-sim/tests/rotation_modifier.rs` rides the stand-in bike whole and headless through `Sim`. The air
sections and their numbers are the test's own. The world is a ramp with nothing past it. Each run counts the
ticks for the body-frame angular velocity's lane to sum to a full turn:

| rotation | 100% | 150% | ratio | 70% |
|---|---|---|---|---|
| backflip (lip, stick fwd then back) | 118 | 78 | 0.661 | 169 |
| front flip (stick forward, in a drop) | 380 | 253 | 0.666 | 543 |
| 360 (stick sideways, held by the 2 rad/s yaw window) | 189 | 126 | 0.667 | 270 |

The test asserts 0.60–0.73 at 150%, more ticks at 70%, and the same direction of turn. Two more checks:
- `every_row_reads_the_factor` checks every row of `Rotations::new(true)`, so a new row is covered as it
  stands.
- `at_one_hundred_percent_nothing_moves` checks that at 100% the air `Params` (Debug text), every row's tuning
  and the chassis' angular limits are the file's, bit for bit.

`spin_rate_still_reads_as_the_torque_it_was` covers the old word. P95's `acceleration_modifiers.rs` menu assertion now also lets `SpinRate` off the menu. `./kit check` passes.

## Not verified / not 1:1

- **The modifier is ours.** The original has no such setting, so nothing at a value other than 100% can be
  compared against it. At 100% the knob is skipped (`None`), and the backflip oracle (`tests/air.rs`, with
  `spin: None`) still runs the same `rotate`.
- **"About" 2/3, not exactly.** Only the rotation is sped up, not the flight. Three things are left as they
  were:
  - the angular velocity the lip leaves the bike with;
  - the auto-levelling's 0.1 rad/s dead zone (`DEAD`, a constant in the step);
  - anything that couples rotation to translation (crossup against the velocity, the steep test).

  So a turn is near 1/k of the ticks, not exactly 1/k (0.661 for the backflip).
- **The stick's flick times are not scaled.** These are `MinBackflipTime`/`MaxBackflipTime`, which pick the
  torque from how fast the stick crossed. The backflip's 0.6 s grace to leave the ground is not scaled either.
  Both are how a flip is asked for, not how fast it turns.
- **Ground rotation is untouched.** That covers wheelies, stoppies, roll control and donuts, which are not
  flips.
- **Not ridden on the disc's own bike.** The proof uses the stand-in bike with hand-written air numbers of the
  disc's magnitude. The scaling argument does not depend on the numbers, but `race125p.phy` itself was not
  flown at 150%.
- **The menu row was not looked at on screen.** It comes from `Knob::MENU`/`label`/`blurb`, and
  `crates/openmx/tests/menu.rs` counts rows from `Knob::MENU.len()`.

## Next

Nothing.
