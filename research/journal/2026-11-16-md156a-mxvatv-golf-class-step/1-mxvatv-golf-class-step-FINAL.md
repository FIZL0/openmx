# 1 — mxvatv golf class step (FINAL)

Task MD156a. 2026-11-16.

## What I did

- Ported the golf cart's per-frame class step (vt +0x488, `./kit fn 0x004cf9b0`, 2196 bytes) as
  `atv_car::golf_class_step`, beside MD151's buggy `class_step`, with its own `GolfParts` / `GolfPosing`.
- Two node calls the buggy step did not need: `node::aim_as_is` (the aim's inline path: rows up×dir, up,
  dir as they are) and `node::aim_squared` (aim along with the basis squared first: `./kit fn 0x00194260`
  with t0 = 1).
- New test `tests/mxvatv_car_frame/golf_step.rs`; `update_beside.rs` now runs the golf cart's step for
  real in its copy and compares its part words and car words each update; `drives.rs` asserts every
  non-truck drive compared the class step on every update.
- research/oracle.txt `# MD156a`: every word, call and flag.

## What the original does

- Sets the up's w (+0x14ac) to 1, builds a steering frame at +0x14b0 from the origin, (0,0,1) and the up.
- Turns T0's steer into a coarse-table (sin, 0, cos) through that frame and lays the steering wheel's rows
  straight down (up×dir, up, dir).
- Spins the two front wheels by T0/T1's angles.
- Each front corner (T1 with its compression, T0 with it negated): each of three arms is aimed along its
  ahead with its up turned about z by the compression; the knuckle's up is the line between its two ends,
  its ahead turned about y by that tire's steer.
- Two link pairs aimed at each other.
- Rear: lift = the larger rear compression; the axle aimed at its target from a point raised by the lift;
  the rear wheel spun by T3's angle; the hub moved up by the lift.
- Two shocks aimed from their feet at their tops.
- No null checks; the only branches are the steer wraps and the max.

## How it was proved

- Function oracle, 10,000 generated cases on md39a/golf.ee.bin, nothing hooked
  (`golf_step.rs` `the_golf_carts_class_step_is_the_originals`): steers (10 % past ±π, some 0), angles
  ±40, compressions −0.1..0.6, mount/up/arm/knuckle/axle/hub/shock points jiggled, part positions moved and
  current bits cleared. Every port word equals the original's, and the original wrote nothing in the
  parts or the car block the port did not. Seven branch counters each ≥ 30; a one-bit flip goes red.
- The golf drive (`drives.rs` `the_golf_carts_drive_is_the_ports`): every update's class step's part
  words and car words compared against the original run in the copy.
- Mutants: turn_xy's msub → madd reds the oracle at case 0; turn_xz's madd → msub reds the golf drive at
  frame 1 (16 words). Both restored with cp + touch.
- `./kit check -- -p game-sim --test mxvatv_car_frame`: 36 pass.

## Not verified / not 1:1

- A NaN steer: the port's coarse lookup (camera's `CurveTable::coarse`) wraps on `a > π`, the original on
  `!(w <= π)`. A NaN index would crash the original anyway; not generated.
- Missing parts: the original has no null checks; the port reads node 0 the same way, but no case has a
  missing part.
- The stack temporaries' w lanes the original leaves as garbage are not modelled; the callees zero or
  overwrite them, and the oracle compares every word written.
- The truck's class step is still not ported (MD156b); its drive only logs the call.
- The golf drive is a short straight-ish drive: steers stay small there; big steers and stretched rears are
  covered by the oracle only.

## Next

- MD156b: the truck's class step, the same way.
