# 1 — p a turn that keeps its speed (FINAL)

Task P24. 2026-10-23.

## What I did

I started again from main, as the user asked on 10-21. wip/P24 was read only.

**Which of P10's two suspects it was: the tire.** By the time this session started, everything
wip/P24 had ported was already on main:

- P33 brought the original's tire law, its contact frame and the fork's raked steering axis
  (`tire::force`, `tire::contact_frame`, `tire::steering_axis`).
- S58 brought the forward lane, the surface drag, the spin step and the stand-in bikes' skid keys
  (`tire::spin_integrate`, `tire::surface_drag`).

The other suspect, the roll pin, is the original's own behaviour. `mv::pose` sets the chassis' third
freedom flag to `!on_ground`, flag for flag, so the grounded bike has no roll freedom in the original
either. The stand-in tire's side force sat at its cap every substep and scrubbed the speed. That
stand-in is gone.

On main, the brief's turn (55 ft/s, a second at full lock, gas pinned) keeps **0.996** of its speed,
both ways. On wip/P24's pre-P33 HEAD it kept 0.884.

So nothing in `mv.rs` or `ride.rs` had to change, and the brief's other items are not needed:

- the forward lane and surface drag are S58's;
- the stand-in bikes' skid keys are S58's;
- there is no rescripted minute, because nothing in the ride moved.

This task adds proof only, all of it in `crates/game-sim/tests/ride.rs`:

- **`a_full_lock_turn_keeps_its_speed`** is the brief's test. It rides race125p, from the disc,
  with the 125_faisst model's wheels and fork, up to 55 ft/s on the gas. Then it holds full lock for
  30 ticks at 30 Hz, each way. It asserts the bike keeps ≥ 0.8 of its speed, that the mean rate is
  > 0.6 rad/s and that the peak is ≤ 2.5 rad/s, which is P15's band for the original's held-stick
  turns. It measures 0.996, with rates from 1.8 to 2.38 rad/s.
- **`a_held_turn_rides_like_the_originals`** checks the same port against **the original's own
  held full-lock turn**. That turn is S58's `turn.csv` capture, from `watch/s58-slip.toml`'s "hard
  turn at speed". It was recorded in slot s1 by S58's run; the spec is embedded in the capture's
  `.json`. I copied it to this slot as `context/capture/p24/turn.csv`, in a subfolder so
  `replay.rs` leaves it alone.
  - The capture: the gas goes on from about 0.7 ft/s, and 16 ticks later the stick goes hard right.
    Both are held until the bike leaves the ground at about 56 ft/s.
  - The test rides the same script on a flat floor, using the capture's own gas and stick ticks.
  - Over the band where the lean has settled (44 to 56 ft/s), it compares means:

    | Measure | Original | Port |
    |---|---|---|
    | Turn rate (rad/s) | 1.853 | 1.842 |
    | Front steer (rad) | 0.303 | 0.304 |
    | Up y | 0.742 | 0.747 |
    | Time from 44 to 54 ft/s (s) | 1.00 | 1.00 |

  - The bounds are ±10% on the rate, ±0.02 rad on the steer, ±0.03 on up y and ±20% on the time.
    In the original the bike keeps gaining speed all the way through a held full-lock turn, and so
    does the port.
- **The test can fail.** With the fork left on the chassis' y, which gives a stand-in-like contact
  frame, both tests go red:
  - the full-lock turn peaks at 2.85 rad/s;
  - the held turn never reaches 54 ft/s.
- **`the_minute_on_the_discs_bike`** (ignored) did not set the fork, which is why its two
  "speed kept" lines read 0.678. `ride_the_discs_minute` now sets the fork from the disc's model, as
  the shell does in `crates/openmx/src/main.rs`. Both lines now pass at 0.860/0.861 against the
  0.768 floor, and the rates are 2.03 rad/s. The test stays ignored for P21's and P23's lines, and its
  reason no longer names P24.

The brief names `mv.rs`; the file is `move.rs`. Neither it nor `ride.rs` needed a change.

From opentennis I took nothing: it has no tire or bike physics.

## What the original does

A grounded wheel's tire force is the per-surface law P33 ported. On race125p it is the parametric
curve, which falls away past its peak instead of pinning at a cap. The force acts in a contact frame
built from the forward turned by the steer angle about the fork's raked axis (+0x8c0, normalised
Handlebars − Shocks). The capture shows the original's side forces at the settled lean:

- the front carries ~575 lbf of side force on ~193 lbf of load, with a patch slide of −14.6 ft/s;
- the rear carries ~625 lbf on ~207 lbf of load.

The port has 564/619 at the same speed, with the same slides to 0.1 ft/s. Neither tire is flagged
sliding, so the forward drive still gets through, and the bike accelerates through the turn.

## How it was proved

- **A PINE recording compared with the port:** S58's `turn.csv`, 201 rows, the original's
  race125p/cr125_faisst on Free09. The band means are in the table above
  (`a_held_turn_rides_like_the_originals`).
- **The brief's acceptance:** `a_full_lock_turn_keeps_its_speed`, 0.996 ≥ 0.8.
- **A mutation (no fork):** both tests fail, as described above.
- **`./kit check -- -p game-sim --test ride`:** passed. The full gate runs in `./kit done`.

## Not verified / not 1:1

- **The full-lock turn from 76 ft/s has nothing to compare with.** P15's figure was a shell turn
  from 76. The port now keeps 0.738 of 76 ft/s over a second at full lock: it peaks at 2.55 rad/s
  and ends at 56 ft/s, where P15 had 35 in 1.33 s. No capture of the original enters a full-lock turn
  from a straight at that speed, so whether the original scrubs that much is unknown.
- **P15's "61 → 52 in 1.1 s" is not a full-lock turn.**
  - It is recording2's human turn. P15's speeds were in capture units, which are 0.8× the body
    speed, so it began at about 77 ft/s of body speed.
  - The stick was (−0.94, +0.94), diagonal and pulled back, and the gas may not have been held.
  - On the flat with that stick and the gas held, the port turns at 0.6–0.9 rad/s and *gains*
    speed, so the original's loss there is presumably terrain or the throttle.
  - Not reproduced here, because the capture's full terrain run is a replay task, not this one.
- **The comparison is of band means, not tick by tick.**
  - The original rode Free09's bumps: its wheel loads swing 140–400 lbf, against the port's steady
    ~200 on a flat floor.
  - The port's run starts from 0.04 ft/s, where the original's started from 0.67.
  - The early ticks match to a few thousandths of a radian of steer (0.164/0.165, 0.321/0.322), but
    no per-tick bound is asserted.
- **The capture was copied from slot s1 and is not reproducible from this tree.** S58's
  `watch/s58-slip.toml` spec is not committed on main (S58's journal is still PART). The spec text
  is embedded in `turn.csv.json`. In a slot without `context/capture/p24/turn.csv` the test prints
  "skipped" and passes.
- **Only the right-hand turn is checked against the original.** The capture has no left turn. The
  port's left and right turns are mirror images to 0.001 in `a_full_lock_turn_keeps_its_speed`.

## Next

None for P24. If someone captures the original entering a full-lock turn from a straight at about
76 ft/s, add it beside `turn.csv` and bound the port's 0.738.
