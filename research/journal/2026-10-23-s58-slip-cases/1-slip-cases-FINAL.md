# 1 — slip cases (FINAL)

Task S58. 2026-10-23. Reopened after S64 landed: "do what can be done now, the parts that wait on
another task go to it". Part one is `../2026-10-21-s58-tyre-slip/1-tyre-slip-FINAL.md`.

## What I did

- **Recorded the brief's cases in the original**, from first_recording's save state (slot 8), at a
  quarter speed, with a pad script played over the recorder (`watch/s58-slip.toml`, the slip lanes on
  both wheels). Scripts, pads and a reader are in `../2026-10-21-s58-tyre-slip/capture/`
  (`setup.py 0.25` with the copy stopped, then `./kit pcsx2 with -- bash capture.sh CASE ROWS`). The
  captures are in `context/capture/s58-slip/` (not committed):
  - `launch` (240 rows): full gas from standing, up the jump face ahead of the start.
  - `turn` (200 rows): gas, then the stick hard right.
  - `brake` (200 rows): gas, then both brakes at about 67 ft/s; the front locks.
  - `traverse` (200 rows): gas with the stick 0.4 right. The bike meets the jump face at an angle
    (up to 0.37 of the normal across the bike), and later crosses a side slope at 83 ft/s (0.23 across,
    ticks ~168–185). Not a held traverse of a steep face; see Not verified.
- **Replayed each from the same save state** (`the_slip_cases_replay_from_the_save_state` in
  `tests/replay.rs`), with S64's seeding. The quarter-speed rows skip the recorder's `stamp_*` columns
  (`quarter_ticks`), and a row's pad drives the port's next tick (`PAD_LEADS`, measured on the
  launch: one tick later and the rear took the gas's first torque a tick early). `S58_TRACE` prints
  the original and the port side by side for a tick range.
- **Four fixes, each found at a case's first parting tick:**
  1. **The levers** (ride.rs `Held::step`, new `rise`). The original's limiter (`./kit fn 0x251a50`)
     lets the gas and the brakes *rise* at their rates and drop to the lever at once; only the clutch
     is held back both ways. The port walked them down too, so a released gas kept driving and a
     released brake kept braking. The brake case is bit-exact through the gas release at tick 92
     (engine torque −4.290465 both sides).
  2. **The airborne rear borrows the front's load** (ride.rs `wheel_integrate`). Inside every substep
     the ground step (`./kit fn 0x257a00`) sets the rear's `+0xc4` when the front touches and the rear
     does not, and `./kit fn 0x25c530` then runs the rear's tire at its probe end on
     `OtherTireNormalLoadScale` × the front's load, with the contact velocity redone. That is how the
     original's rear keeps driving and braking over a bump. Part one listed this path as never taken
     (`+0xc4` is 0 in every save state); it is set each substep and cleared by the next
     `wheel_clear`. In the brake case the port's speed at tick 140 went from 23.07 to 47.68 (the
     original's 47.38), and the rear's force is bit-exact at the tick it first borrows.
  3. **The bike's speed is the wheel's view of the chassis** (ride.rs, the frame-end air state and
     `forward_speed`). `./kit fn 0x251d20` sets the bike's velocity to the front wheel's `+0x190` when
     the front touches, else the rear's, else the chassis' own; its z is the speed the steering fade,
     the lean and the rest of `r#move` read. The port handed the air state the contact velocity
     instead, and read `chassis.local_vel.z` as the speed. The save state's bike velocity equals the
     front's `+0x190` bit for bit. With it the turn's steer at tick 47 is within an ulp, and the
     launch's and the brake's positions agree to ticks 124 and 106 (from 59).
  4. **The fork's rake** (tests/replay.rs `seed_wheel`). The port's steered wheel turns about the
     wheel's `steer_axis`, which `Wheel::of` leaves straight up until whoever has the model sets it
     (the shell does). The replay never set it, and the original's front axis in the save state is
     raked, (0, 0.8808, −0.4735). The turn's front had parted sideways at the first tick it steered
     (front lateral velocity −0.727 vs −0.818). A seeding gap in the test, not in the ride.
- `research/oracle.txt` has an S58 block with all four.

## What the original does

- The limiter: `+0x650` (gas) and the brakes step toward the stick only upward, at their `.phy` rates;
  a lever let go is 0 the same tick. The clutch walks both ways. `+0x654` is copied from `+0x650`.
- The ground step writes the steer command into `+0x8ac` once per frame, before the substep loop. Each
  substep: clear both wheels, the 0x257820 step, both suspensions, both tire forces, the share flag
  `rear+0xc4 = front touches && rear does not`, then both spin integrations.
- The share branch (`+0xc4` set and the wheel itself not touching): load point = probe end, normal
  load = `+0xc8` × the other wheel's, the contact's owner and vehicle (`+0x1f0`/`+0x1f4`) copied from
  the other wheel, the contact velocity redone, then the contact frame and the drive force as usual.
- The bike's velocity `+0x430` is the touching wheel's `+0x190`: the chassis' velocity less the
  ground's, in the chassis' frame. Its z, `+0x438`, is the "speed".

## How it was proved

- Save-state captures of four cases (above), replayed from the same state, lane by lane:
  `the_slip_cases_replay_from_the_save_state` reports each case's first parting tick per lane.
- The fixes were each found by tracing the first lane that parts, reading the function, and checking
  that the lane agrees after the fix (numbers above).
- The `+0x430` = front `+0x190` and the raked `+0x8c0` facts were read off the save state's RAM.
- Where each case stands now (`S58_CASES`' ratchet; the path is `rider.ride.origin`, bit-exact):

  | case | path bit-exact through | slip flags agree through | what parts them |
  |---|---|---|---|
  | turn | 82 | 146 | ulp drift until 129, then the original's quaternion sign flip at 130 (S112) |
  | launch | 96 | 126 | the landing off the jump face at 124 (noted on S67) |
  | brake | 105 | 130 | the same landing at 123; the front then locks at 131 in the port, 133 in the original |
  | traverse | 82 | 123 | the same landing at 124 |

  Before this session's fixes the launch's and the brake's positions parted at 59 and the turn's steer
  at 47. In the turn the chassis' velocity holds to about 1e-4 ft/s through tick 129, four seconds
  into a full-lock turn at 50 ft/s with the tyres at their side-force limits.

### Ruling out the shell

The brief asked to rule out P52's dt and the 0.8 `bike_vel` scale. Neither one touches these replays.
The port runs at the capture's own 30 Hz dt, `quarter_ticks` checks it against the rows, and the
NominalScalar of 0.25 only slows the emulator's clock, not the game's tick. The comparison is lane by
lane on the chassis' own words (`+0x260` velocity, the path), not on the 0.8-scaled `bike_vel`. So the
four divergences above are the simulation's own. Whether the shell's frame pacing adds anything of its
own on top is not tested here.

### Tests that moved, and why

- `tests/replay.rs` `the_human_recording_is_a_switch…`: part throttle on 27 ticks of the minute, not
  54. The gas no longer walks down after a release, so only the nine presses have a ramp.
- `tests/modifiers.rs` `engine_power_is_quicker_off_the_line`: the "still rides" check is at 400%,
  not 500%. With S58's speed 500% loops the bike over at tick 33. Without it, 450% did the same:
  the band from 450% to 500% is a cliff, and a tick's lag in the speed decides which side it falls.
  400% rides at a 12° nose either way.
- `tests/ride.rs` `md25_easy_three_sixty_after_a_sweep_back`: the ride-out heading bound is 0.35, not
  0.2. The landings are all within 0.04 of take-off; the drift after them on a centred stick went
  from 0.14 to 0.29 ("right, a little up"). Both the borrowed rear load and the lagged speed move it.
- `tests/ride.rs` `md25_mx_vs_atv_rides_as_before`: both folds re-pinned. Every ride moved.
- `tests/platforms.rs`: ten consts re-spliced from `print_the_vectors` / `print_the_riders_vectors`.
  `HUNDRED_TICKS` is re-pinned in all three places (lib.rs, hash.rs, platforms.rs).

## Not verified / not 1:1

- **Bit-exact from tick 1 is still not reached.** Every case parts by an ulp on the chassis' angular
  history at ticks 1–3. That is S107's drift from the save-state start. The ratchets above are first
  parting ticks with that drift in.
- **The landing off the jump face** (ticks 123–124 in three cases) is not fixed: a position push-out
  at a bottomed-out fork. It belongs to S67 (landings replay bit-exact through touchdown), and a note
  with the recipe is on S67.
- **The turn's quaternion sign flip** at tick 130 is not explained. The original's chassis quaternion
  is negated after the integrate's derivative, by something the port does not have. Filed as S112.
- **The traverse is not a held traverse of a steep face.** The capture crosses the jump face at an
  angle (up to 0.37 of the normal across the bike) and a 13° side slope at 83 ft/s. No steep side
  slope is near first_recording's start, and every motocross state is the same spot on free09. Filed
  as S113.
- **The brake's front lock** comes two ticks early (131 vs 133), but only after the landing has
  already parted the path. It is not a separate finding until S67 lands.
- **The share branch's `+0x1f0`/`+0x1f4` copy** (the contact's owner and vehicle) is not carried. The
  port's ground does not move, so they would only matter on a moving body.
- **The shell is not ruled out by a run of its own.** The reasoning is under "Ruling out the shell":
  the replays use the capture's dt and the chassis' own velocity words, so neither P52's dt nor the
  0.8 scale is in them. Nobody compared a shell frame with a capture row here.
- **The fork rake is a test-side seed.** The ride itself was always right once the model sets
  `steer_axis`, and the shell does. Any other `Ride` built without a model (tests, `stock`) still steers
  about straight up. That is the stand-in's choice, not the original's.
- **Bounds moved without proof against the original:** the 400% engine power, the 0.35 ride-out
  heading. Both are hand-made expectations of mod knobs, which the original does not have.
- From part one, still standing: the hinge-torque path (`+0xc0`) and `+0x224`/`+0x228` are not
  ported; the stand-in bike's tyre numbers are tuned, not the disc's.

## Next

S112 (the quaternion flip), S67 (the landing), S113 (a held traverse) and S107 (the tick-1 drift).
Each raises `S58_CASES`' ratchet in `tests/replay.rs`.
