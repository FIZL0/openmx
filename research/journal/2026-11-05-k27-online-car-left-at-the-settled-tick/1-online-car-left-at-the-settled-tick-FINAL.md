# 1 — online car left at the settled tick (FINAL)

Task K27. 2026-11-05.

## What I did

What the two shells raced: not the ride. Both machines' rides agreed (0 desyncs, the same state hash
before tick 300); the line that parted was the shell's `car: monster N ft from where it was taken`, and
that line was read off the **frame drawn**, not off the session's tick:

- `main.rs`'s online loop printed the car left from `seat.garage.car_up()` on the first drawn frame
  with no car, and the garage's car is whatever `Garage::follow` copied from the last drawn frame.
- On the machine whose seat did not switch (here the host, for the guest's truck), the frames drawn
  past the guest's switch-back are predictions: ggrs repeats the guest's last pad (gas off, no switch),
  so the truck coasts on for however many ticks the guest's real pad takes to arrive. The rollback
  then takes the truck away, and the line says where the prediction had coasted it to. Under load the
  pad comes later, the prediction runs longer: 807.9 ft on the host against 806.2 on the guest, the
  host the farther, as this predicts. `taken_at` had the same flaw at the switch-in (first drawn frame
  with the car, a few ticks into the drive on the remote machine).

The fix (`online.rs`): `Carried`, a wrapper round the session's `Modded` game, watches every tick the
session runs (re-runs too) and writes a `CarLog` keyed by (tick, seat): where a seat's car stands after
the tick it was taken on, and the `CarRide::report` line of the car as it stood before the tick that
switched off it. A re-run tick drops what it said before and says its own, so a prediction's leftovers
are gone by the time it settles. `Online::settled_cars` hands out the leave lines at ticks now
settled (`Netplay::settled`, K8's: every seat's own pads, re-runs done), and the shell prints those
instead of the drawn frame's. The offline switch's line (`switch_machine`) and the end-of-ride line are
untouched.

The other race in this test (P220's, why it was in `[flaky]`): `both_agree` held the `vehicle:` lines
to one order across seats, but two seats' picks a moment apart land on ticks each shell's wall clock
chooses, and a machine draws the other seat's switch a few frames late. It now compares them as a
multiset; each seat's own order is still held by its car's distance line. With both fixed the test left
`gate.toml [flaky]`.

`crates/stitch/worlds/world_mxu_free.start.json` taken from main as P295 re-keyed it (byte-identical to what this branch built), only so disc_worlds is green here.

opentennis: nothing taken; it has no machine switch and no line said at a settled tick.

## What the original does

Nothing to port: the original has no online play. The line is the shell's own report.

## How it was proved

`./kit check -- -p openmx --test online_car` green (140 tests), then the three two-shell tests
(`two_online_shells`) four times back to back at load 53-71 on 32 cores (the failure was seen at load
~47): 12/12 green, with the `[flaky]` rerun no longer covering the two-cars one.

## Not verified / not 1:1

- The failing run's own output is not on this disk (P294's gate log), so the mechanism above is inferred
  from the code and from the sign of the gap (the non-switching machine the farther), not read off that
  run's lines; I did not reproduce the old red on demand before the fix (it needs a late pad at the
  switch-back, which load gives only sometimes).
- 12 green runs under load is evidence, not proof that nothing else in these tests reads wall clock.
- A car leave at a tick that never settles before the shell stops (a ride ending within a few ticks of
  the switch) is not printed online now; before, it was printed off the drawn frame. No test switches
  off a car that late.
- The `vehicle: <machine>` lines are still printed from the drawn frame (`took`), so on a machine they
  can come a few frames after the tick; only their order across seats was loosened in the test.

## Next

-
