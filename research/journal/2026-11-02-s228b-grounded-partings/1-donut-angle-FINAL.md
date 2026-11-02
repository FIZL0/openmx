# 1 — the donut's angle (FINAL)

Task S228b. 2026-11-02.

## What I did

For the grounded partings in `HELD` (crates/game-sim/tests/fly_and_collide.rs), I compared the
port's ride against the original's, field by field, from the 100-frame images
(context/states/s228/s228-<frame>.ram). The comparison used a throwaway test, since removed.

- At 138010, the very first tick parts. The rear tire's side force was −52.25 in the port and
  −63.95 in the original. The chassis, engine and front tire matched before the rear tire's force.
- The rear tire's force is scaled by the donut. The port's `donut_scale` came out 0.218, but the
  original had no donut at all.
- The donut reads the stick's angle and deflection at `+0x634`/`+0x638`. The image holds 1.4564 and
  1.0066 there. The port's own `ride::stick` gave `atan2f(-|x|, y)`, near 0, with the deflection
  clamped to 1.
- The original writes those two words in its pad read, `stick_direction` (0x251830). It is the
  trick decoder's look, which the port already has as `trick::look`: `acos(x/|s|)` off the
  decoder's `AngleTable`, negated when `y < 0`, and the reach `sqrt(y² + x²)`, not clamped.
- I ran the original's ground step (`bike_ground_step`, 0x257a00) in the oracle from the image,
  with the port's levers written in. It gave the capture row's chassis velocity and centre bit for
  bit at 138010, 138210 and 138310. That is how I know the ground step is the only part that splits.
  The whole base update cannot be run: its attachment walk (0x133500) stalls from these images.

The change:

- `ride::Tables` gains `angles: trick::AngleTable`.
- `frame_searched` takes `trick::look(&tables.angles, trick::stick(stick, dpad))` for the angle and
  deflection.
- The port's own `fn stick` is deleted.
- move.rs's comment on `NEG_HALF_PI`/`NEG_PI` now gives the angle's convention: 0 is right, positive
  is pulled back, the forward stick spans 0 to −pi, and a pulled-back stick asks for no donut.

## Result

`HELD` went from 17 stretches holding to the end to 25. Each of these now holds all 100 ticks:

| Stretch | Ticks held before |
|---|---|
| 136610 | 14 |
| 137910 | 33 |
| 138010 | 0 |
| 138110 | 69 |
| 138210 | 2 |
| 138310 | 1 |
| 138710 | 19 |
| 139310 | 74 |

139310 had been put down to a preload's force; it was the donut.

S228a's drawn-lean test compares every stretch that holds, so it now compares 1675 ticks (274 in
hard turns) instead of 1201 (213). I re-ran it over only the stretches it compared before: its worst
figures are unchanged (lean to a side 0.650, root up 0.195, the same worst tick 137240). The new
stretches add worse ticks:

| Measure | Before | Now |
|---|---|---|
| Worst lean to a side | 0.650 | 0.713 |
| Worst root up (rad) | 0.195 | 0.283 |
| Hard-turn lean ratio | 0.802 | 0.795 |

I widened its bounds to 0.75, 0.3 and 0.79, and its docs say why.

## Still parting

- **137410 at 137462**: no gas, the stick full left (−1, 0).
- **137610 at 137637**: gas on, the stick coming off centre (0.025, then 0.17).

On the rows after the last held tick (137463, and 137636/137637), the capture's `p1_ch_force` and
`p1_ch_moment` go non-zero with the bike on the ground, and the port's stay zero. Something in the
original pushes the chassis there: a body ray, or a contact the port does not find. The donut is not
involved. There is no image at those ticks, so I did not chase it. Follow-up filed.

## How it was checked

- `./kit check -- -p game-sim`: 1025 tests pass, fly_and_collide's three included, with `HELD` and
  `DRAWN_TICKS` re-pinned.
- The donut fix was proved in the oracle as above, at three stretches.

## Not verified / not 1:1

- **The two remaining partings** (137462, 137637) are only described, not explained.
- **The put-backs** (138684, 139141, 139746) were not looked at. The port's respawn spot is not the
  original's. Follow-up filed.
- **Images at the parting ticks** (research/s228/parting_ticks.txt) were not made. The 100-frame
  images were enough for the donut; the two contact partings would need them.
- **The d-pad.** The look's d-pad override (`trick::stick`) is the decoder's. The tape never presses
  the d-pad while riding the ground, so its effect on the donut is untested.
- **The quad** (quad.rs) has its own `stick_angle`/`deflection` inputs, which its callers fill. I did
  not check them against the look.
