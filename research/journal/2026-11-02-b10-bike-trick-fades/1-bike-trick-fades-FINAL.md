# 1 — bike trick fades (FINAL)

Task B10. 2026-11-02.

## What I did

- `anim::Stance` has a new flag, `restart_tricks`, and only `vehicle::Chassis::load` sets it. On a
  frame whose selection comes from the table (a gesture slot, a pack extra, the no-hander),
  `Stance::frame` changes what the 60-state step gives back:
  - a stunt cross (`Blend::Pair`) becomes the new clip alone (`Blend::One` at the trick's
    progress). The ramp runs from 1 at the restart, at the fade rate, as every other arm runs it,
    where the Pair arm zeroes it. The restart already snapshots the pose (`Set::rewind`), so the
    bike fades from where it was at the switch;
  - the weight drawn is the ramp *before* this frame's step: 1 on the restart tick, then 0.9 down to
    0.1, and 0 on the tenth tick after.
- `Chassis::load` sets the bike's `rate_stunt` to the motion library's constructor default
  (`motion::Params::default().blend_rate`, 3). The body keeps `vehicle::params()`'s measured 4.
- `trick_chain.rs`: the bike is held within `JOINT_DEG` from `BIKE_IN` to the landing, with no
  window after the switch, and the "B10's gap is still there" assertion is gone. The two wheel
  joints are left out of the bike comparison (see below).
- `whip_tricks.rs`: `FADE` goes from 15 to 21 frames (the bike's 1/3 s at 60 Hz, plus the
  one-frame lag in what is drawn). `OVER` goes from 0.025 to 0.075, because Flatliner's whipped
  rider now comes 0.062 ft further apart than the whip at frame 225. Mid-fade, the body (rate 4)
  is further out of the whip than the bike (rate 3) it sits on. Both rates were read off the
  original.
- Nothing was taken from opentennis. It has no bike.

## What the original does

This is S217's reading of `context/capture/s217/run1.csv`, which the S217 journal covers. When a
trick switches in, the bike's motion object (rider `+0x68`) restarts its clip and fades linearly
from a snapshot over 10 ticks. It fades into a trick out of the air the same way. The tick-by-tick
comparison below pins the phase: the snapshot carries its full weight on the restart tick (TireRear
123.6° on the switch tick, 12.36° less on each tick after). Run at its own 0.9-first ramp, the port
was exactly one tick ahead on every joint, with the gap at 0.1 × the snapshot-to-clip angle on each
fade tick. That gap goes away with the lag.

## How it was proved

A PINE recording, compared tick by tick: S217's `run1.csv` (380 rows, one run). I copied it from
slot s1's `context/` into this one's.
`cargo test -p game-render --test trick_chain a_queued -- --nocapture` prints
`switch 26 ticks in, landing 68: the rider within 1.48°, the bike within 1.30° (1.03° over the switch's fade)`.
Before the change the bike was 12° off on the root on each fade tick and 50° at worst over the 24
ticks. While working I printed every joint for every tick. Once the wheel joints are left out,
every bike joint is within about 1° from tick 10 on, the switch included. Outside the first 10
ticks only the wheel joints (14 and 20) were off.

Also green, each run on its own target: all of game-render (`./kit check -- -p game-render`, 292),
which includes trick_chain's S214 chain test (worst 0.78/0.71 of its bound), whip_tricks, vehicle,
body and pack_clips. And openmx's `flip_tricks`, `roost_offset`, `ride`, `machine_packs`,
`foot_down` and `original_mode`.

## Not verified / not 1:1

- **The wheel joints in the snapshot.** The original's snapshot of TireFront/TireRear holds the
  rolled wheel angle. The port's holds the clip's, so those two joints are up to 124° off over a
  fade. They are not drawn: `Chassis::drive` replaces both rotations with the wheel spin before the
  tree is composed. Whether the original's wheel drive also overwrites them was not checked (as in
  S217).
- **The first 10 ticks of the fade-in out of the air.** The fade's timing matches: the residual
  falls linearly to 0 on tick 10, as the original's does. But the pose it fades from is the test
  harness's (a fall from 400 ft, with no lip), so joints 3 and 5 start 11° and 20° off. These ticks
  are not held. That is a harness artefact.
- **Rate 3 for the other bike states the gesture rate drives.** Only two fades are measured, a
  trick out of the air and a trick out of a trick. The no-hander and the pack states take
  `rate_stunt` too, so they also fade at 3 on the bike now. A pack 360's play cross is `1/rate`, so
  on the bike it is now 1/3 s instead of a quarter second. None of those was filmed. The bike's
  other rates (airborne row, landing, hold) are still the body's measured ones. The original's bike
  object probably runs everything at its single default of 3, but nothing has measured it.
- **The one-tick lag** applies to the bike's table states only (gestures, extras, no-hander). It is
  read off the joints and not off a field. The bike object's ramp field is still unlocated.
- **A trick the bike has no clip for, queued after one it has.** Under the restart, `Blend::One`
  with no clip is a frame that poses nothing. `Chassis` then fades the rest shape out of the
  snapshot by the blend's ramp, as it does for any clipless state. Not filmed.
- **One run, one pair** (flatliner into the Taunt), as in S217.
- **The whip interplay was not filmed.** Flatliner's 0.062 ft mid-fade overshoot comes from the
  two measured rates applied together. A whipped trick was not captured in the original.

## Next

none
