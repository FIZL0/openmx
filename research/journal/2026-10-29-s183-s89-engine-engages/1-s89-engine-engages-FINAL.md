# 1 — s89 engine engages (FINAL)

Task S183. 2026-10-29.

## What I did

- Measured first. The parting S183 was filed about (S123: tick 31 along / tick 16 across, original
  engine.torque 8.350148 and wheel_torque 379.33 where the port had -0.7397749 and 0) was already gone
  before this task started. S124 (f47aaed6) found the cause: the test put each pad on its capture *row*,
  so the gas came a tick late. It now puts the pads on the *ticks* (`save_state_inputs`: PAD_LEADS, plus
  the state's own first-tick controls), and the engine drives the rear wheel on the original's tick.
  S124 raised the ratchet to [236, 141], where both rides parted in the air on the angular history's z.
- Then S179 (euler angles measured once a frame, after the physics) and S180 (airborne control reads the
  roll the post-physics walk measured) landed. With them, `the_dallas_rail_rides_replay_from_their_save_states`
  on s1's fixtures (`context/capture/s89/{along,across}.csv`, `context/states/s89-*.eeMemory.bin`, all
  present, so the test does not skip) prints "agrees on every tick" for both rides and for every single lane,
  the four engine lanes included.
- Raised `S89_AGREES_THROUGH` from [236, 141] to [u64::MAX, u64::MAX]. Also raised `S89_LANES_AGREE_THROUGH`'s
  four lanes still below MAX (front/rear slip.locked 571 and ray_hit 517/518, along) to MAX, because the test
  asked for it. Silenced the "raise" message when the ratchet is already MAX. Updated the two doc comments.
- No port code changed. Nothing taken from opentennis; the task did not need it.

## What the original does

Nothing new. The engine's first drive of the rear wheel on along's tick 31 and across's tick 16 is the gas
the pad gives on that tick. The port already does it, once the pad goes on the right tick (S124).

## How it was proved

Bit-exact replay fixture: `crates/game-sim/tests/replay.rs` `the_dallas_rail_rides_replay_from_their_save_states`.
It is the original's quarter-speed PINE capture of both S89 rides from their save states, compared on every
watched lane. Along: 599 ticks, across: 299 ticks, every lane equal on every tick. Run with `--nocapture` in s1.

## Not verified / not 1:1

- The fixtures exist only in the slots that have S89's capture and images (s1 has them). In a slot without
  them the test prints "no …" and passes unchecked, so a green gate elsewhere proves nothing about this ride.
- I did not look again at which of S179 and S180 removed the 237/142 air parting. Both landed between S124
  and this task, and the ratchet now holds whatever the cause was.

## Next

Nothing. Done.
