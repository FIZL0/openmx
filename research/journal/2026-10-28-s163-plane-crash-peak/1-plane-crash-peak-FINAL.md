# 1 — plane crash peak (FINAL)

Task S163. 2026-10-28.

## What I did

- Measured first. The brief's premise is the port's own 45° dive into flat ground
  (`flight_collision.rs`, `a_plane_crash_is_heard`), where the wheels bottom out a tick before the
  rays reach the ground and the clank comes 4 ticks after the wreck. That dive was never flown in the
  original. So I replayed the original's own nose-in instead.
- P164's capture (`context/capture/p164/crash-sound.csv`) has the wreck word, the touched flag, the
  shock peak, the body's position and its velocity. It has no orientation or rates. I copied it into
  this slot from s3's context (git-ignored, not committed).
- C7's acceptance capture (`context/capture/acceptance/biplane.csv`) is the same biplane.p2m2 playback.
  Its frame counter steps 2 a tick, and P164's 11664 is C7's 28630: the two read the same position, bit
  for bit, on every untorn row. C7 has the body (pos, vel, quat, rates), the surfaces, the engine and the
  pad. So the port is seeded from C7 (the same way `tests/replay.rs`'s `biplane_start` does it) and
  compared with P164's gates.
- New `crates/game-sim/tests/plane_crash_peak.rs`: the disc's plane with its rays, over free06's terrain
  with its props (all loaded) and its trees (`eco::Wooded`, `Trees::on_ground`). It is seeded 6 ticks
  before a wreck and flown on C7's pads. Two tests:
  - `the_nose_in_wrecks_on_the_originals_tick_with_its_peak`: frames 11659..11668. On every tick the
    wreck word and the touching flag are the game's, and each lane of the shock peak is within 0.15 ft/s.
    The clank gate (wreck, touching, |peak| ≥ 5) first passes on 11664, the wreck tick, in both.
  - `the_second_crash_wrecks_on_the_originals_tick_bit_for_bit`: frames 12493..12498, a belly skid at
    135 ft/s. The body, wreck word, touching flag and shock peak are bit-exact up to and including the
    wreck tick.
- No change to `flight.rs`: the port's Flight already does what the original does on its crash.
- Pointed `a_plane_crash_is_heard`'s doc comment at the replay.
- Filed S204: the second crash parts from the game on the tick after its wreck (below).

Nothing taken from opentennis: it has no plane or collision object.

## What the original does

On the nose-in (P164 11664), the nose's rays and the main gear meet the ground on the same tick. The
port's tire 1 is at 0.42 of its 0.5 compression, so it has not bottomed, and the strut's velocity kill
does not fire. The shock peak (2.72, 13.52, −8.05) is the collision response's. The plane wrecks that
tick, and the record is touching. 11665 touches again (peak 20.0), 11666 is off the record (peak 1.4),
11667 has nothing, and 11668 has a wheel's 4.0.

The port's synthetic dive is different. Placed 45° nose-down with its velocity along the nose at 113 ft/s,
its wheels reach the ground first and bottom out. The strut's past-full-compression branch takes the
whole closing velocity off the body in one go (the 91 ft/s shock), so when the rays touch on the next
tick there is nothing left to resolve.

## How it was proved

A PINE recording compared tick by tick: P164's crash-sound.csv, with the port seeded from C7's acceptance
capture of the same playback.

| tick | port wreck/touch/|peak| | game wreck/touch/|peak| | body off |
| --- | --- | --- | --- |
| 11663 | 0 / no / 0 | 0 / no / 0 | 0.003 ft, 0.04 ft/s |
| 11664 | 1 / yes / 15.947 | 1 / yes / 15.966 | 0.005 ft, 0.07 ft/s |
| 11665 | 1 / yes / 20.050 | 1 / yes / 20.009 | 0.006 ft, 0.06 ft/s |
| 11666 | 1 / no / 1.389 | 1 / no / 1.395 | 0.008 ft, 0.09 ft/s |
| 11667 | 1 / no / 0 | 1 / no / 0 | 0.011 ft, 0.11 ft/s |
| 11668 | 1 / no / 3.965 | 1 / no / 3.965 | 0.016 ft, 0.15 ft/s |

The second crash (12498): 0.0000 ft and 0.0000 ft/s off on every tick from 12493 to 12498, and a peak of
9.227 with bit-equal lanes.

How far back the seed sits matters for the nose-in: 1 tick back, the wreck tick is 0.70 ft/s off and its
peak 16.61. 3 back gives 0.12 ft/s and 15.90; 6 back gives 0.07 ft/s and 15.95. The wreck tick and the
flags are right at all three.

## Not verified / not 1:1

- **The nose-in is close, not bit-exact.** Its peak lanes are up to 0.09 ft/s off the game's, and the
  body is up to 0.15 ft/s off by 11668. C7's rows do not carry the body's last position or orientation
  (taken as the current ones), the engine's noise clock, or the pad's previous commands, so the seed is
  not the game's whole state. The second crash, seeded the same way, is bit-exact, so the residue is
  probably the seed and not the step. That is not proved.
- **The second crash after its wreck tick (S204).** On 12499/12500 the game's plane is thrown off a
  vertical face: C7's velocity goes from (135, −1, 15) to (100, 26, 60) in one tick, and P164 has a peak
  of (−8.65, 0, 8.75) on 12500. The port skims the ground with no impulse. free06's props (all loaded,
  none moving) and trees are in the replay, so what the game hits there is unknown. The test asserts
  only up to the wreck tick. P164's 12499 row is torn (it repeats 12498).
- **The port's synthetic 45° dive is not proved either way.** That the wheels bottom out first and the
  clank comes late was not flown in the original. The strut's velocity kill is the original's (S36's
  tire port), but no capture covers a dive that steep at that speed.
- **Only two crashes.** Both are biplane.p2m2's, both on free06. No wing-tip, tree or prop crash was
  replayed against the gates (S143's captures have the body but not the touched flag or the peak).

## Next

Nothing for S163. S204 for what the second crash hits.
