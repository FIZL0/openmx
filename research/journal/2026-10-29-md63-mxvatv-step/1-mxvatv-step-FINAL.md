# 1 — mxvatv step (FINAL)

Task MD63. 2026-10-29.

## What I did

- Scoped first. The brief lists nine pieces of SLUS-21104's step to diff and port, each with an oracle.
  One session will not cover that, so I split it (AGENT.md: split first, then do the first part) and did
  the two smallest pieces that differ, the clutch and the preload, end to end:
  - `crates/game-sim/src/handling/step.rs` (new, `pub mod step` in handling.rs): `clutch` and `preload`,
    SLUS-21104's as they read there.
  - ride.rs: under MX vs. ATV (`Ride::mxvatv` is `Some`) the frame's clutch call and the substep's
    preload call go to these. Original runs exactly the code it ran before.
  - `handling::Flip` gains `preload_cue` (hashed, and in the snapshot's wire list). It exists only on an
    MX vs. ATV ride, so Original's hashes and sizes do not move.
  - `crates/game-sim/tests/mxvatv_step.rs` (new): the oracle tests.
  - research/oracle.txt: an `# MD63` block with the clutch, preload and input-ramp field maps and
    `atv_post_event`.
- Filed MD63a (input limiter + `[Joystick]` steer ramp), MD63b (roll/pitch, balance, stand-upright),
  MD63c (steer integrate, wreck test) and MD63d (GravityScale, the 16 FrictionZones).
- opentennis: nothing to take. It has no MX vs. ATV and no bike step. The harness is MD40's
  (`tests/mxvatv_air.rs`), copied and cut down.

## What the original does

- **Clutch** (`./kit fn atv_bike_clutch`; MX Unleashed `./kit fn bike_clutch`). It has the same three
  cases as MX Unleashed's. With the clutch at 1 and the gas at 1 it rearms, and MX vs. ATV also zeroes
  the start multiplier (+0x134c) and the cofg weight (+0x1354) there. With the clutch at 0 and the timer
  at or under ClutchTorqueScaleTime it runs. Anything else does nothing. The run is MX Unleashed's,
  except the **last term is gone**: MX Unleashed's chassis cofg.z −= weight × lerp(travel back) is not
  there, so a launch moves no centre of gravity. The weight is still computed and never read. An empty
  range takes the branch's delay slot `x − 0` (the start itself).
- **Preload** (`./kit fn atv_bike_preload`; MX Unleashed `./kit fn bike_preload`). The lip, the arm
  (stick y > 0.8), the fire (y < −0.8 and armed) and the firing countdown with its add-force (body
  vt +0x80, the add-force stub) are MX Unleashed's. Two things differ:
  1. Equal Min/Max preload times give the force `0 − min` (`./kit dis 0x004e6204 --program atv`: the
     equal branch loads 0 and its delay slot subtracts the minimum). MX Unleashed's gives 0.
  2. A cue runs after every path. With the latch (+0x700) clear and the timer above 0, the latch is set
     and the event (8, 14) is posted: 0x00272300(*0x00442eb0, bike+0x674, 8, 14, bike+0xd20, 0). With
     the latch set and the timer at or under 0, the latch clears. So it posts on the first substep the timer
     stands above zero after being at or under it. That is the hold's first count, and a fire from an
     arm whose timer is still 0. A fire after a counted hold finds the latch set and posts nothing.
- **Input limiter, diffed for MD63a** (`./kit fn atv_bike_input_limiter` vs `./kit fn bike_input_limiter`):
  there is no stick squaring and no player-controller gate. A first call (0x004a7990) zeroes the gas
  when bike+0x680's object has +0x1d0 > 0. A new gas cut: while +0xdd4 (MX Unleashed's +0x614, throttle
  on) is clear the latch +0x1764 is 1 and the timer +0x1768 is 0. Once it is set and latched, the gas
  is forced to 0 until the gas is let go or the timer passes +0x176c. The gas filter's second
  zero-condition is +0x1524 == +0x152c in place of MX Unleashed's hands-off +0xa44. The lean pair, the
  hold latch, the brake and the clutch walks read the same.
- **Input ramp** (`[Joystick]` InputRampAmount → +0x12f4 × π/180, InputRampTime → +0x12f8, read in
  0x004df740). It is used only in atv_bike_ground_head's steer command. With |lean x| ≥ 0.987 a timer
  +0x12fc runs, and steer = lean·k·(max − amount) + lean·amount·min(1, t / time). Full lock comes in
  over InputRampTime.

## How it was proved

Function oracle, `crates/game-sim/tests/mxvatv_step.rs`, SLUS-21104 run in ps2-oracle off
`context/mxvatv-ps2/state04.ee.bin` (copied from s1's context/, git-ignored). The bike, body and engine
regions are compared word for word, and the original may write nothing else:

- `mxvatv_clutch_is_the_originals`: 10,000 states. Every case (rearmed, left alone, started, started with
  no speed range, running, running with no time range) has ≥ 100.
- `mxvatv_preload_is_the_originals`: 10,000 states. Every path (firing, fired out, off the lip, armed,
  fired, fired with equal times, holding, waiting) has ≥ 100. The event post is hooked, and its
  arguments must equal what the port's answer implies (≥ 500 posts).
- `mxvatv_clutch_moves_no_cofg`: the port's clutch gives MX Unleashed's multiplier on a launch while
  MX Unleashed's moves the cofg (no image needed).
- Mutation check: putting back MX Unleashed's equal-times 0, dropping the rearm's zeroing, or dropping
  the cue each reds the oracle.
- Original's hashes: the new code runs only when `Ride::mxvatv` is `Some` (MX vs. ATV). `./kit check
  --all` is the gate.

## Not verified / not 1:1

- **No MX vs. ATV ride replays bit-exact**, as in MD40. There is no SLUS-21104 bike ride capture (C15),
  and MD62 and MD63a–d are still MX Unleashed's under MX vs. ATV's numbers.
- The preload reads the port's `Step::stick`, which is **squared** (MX Unleashed's limiter). MX vs.
  ATV's limiter does not square it, so under MX vs. ATV the preload's ±0.8 lines sit on a different
  stick until MD63a lands. The oracle proves the function on the stick it is given.
- The cue's event (8, 14) is posted to nothing. The port returns it from `step::preload` and the ride
  drops it. Who receives it (the rider's preload pose, a sound?) was not read.
- The clutch's speed, the preload's lip fields and the lever fields are taken to be the same values the
  ride already feeds MX Unleashed's versions (the same field roles at the PC-layout offsets). That was
  not checked against an MX vs. ATV capture.
- The input limiter and input ramp are diffed above but **not ported** (MD63a). Roll/pitch, balance,
  stand-upright (MD63b), steer integrate and the wreck test (MD63c), GravityScale and the FrictionZones
  (MD63d) were not diffed in this task.

## Next

MD63a: port the limiter and the steer ramp. The diff above is the starting point. The ramp lives in
ground_head, which the bike step does not yet route by handling.
