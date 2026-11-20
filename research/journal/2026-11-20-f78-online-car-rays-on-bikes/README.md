# F78 — an online car's own rays strike the bikes, and the car steps first

F76 gave split screen's driven car its own rays on the bikes. Online drive lanes (`net/rides.rs`)
still built the car's met with `met_among` (mirrors only) and stepped the car seat in the bikes'
last-seat-first walk.

## What changed

- `net/rides.rs` (`Rides::step`): the car seats step after the planes and before every bike, as
  split screen's `World::simulate` since F76. A seat's car j gets `met_car_among` (named
  `n + flown + j`, the bikes' frames and collision files), and after its step its pushes go to the
  riders (`pushed_among`) and its mirrors into `struck`, which heads each bike's list
  (`struck_first`), as for a plane.
- `rider_contact.rs`: `met_car_among`, `met_car` on online's seats.
- `car.rs`: **the car's post regathered its nearby list without the bikes**, so `search_met` never
  found them after the car's first search (the broad phase's found list held no bike objects).
  This was true in split screen too: F26's `a_split_bike_is_stopped_by_the_other_seats_car` never
  had `met.pushed` set. F76's R22 test passed because it searches a fresh collision object, whose
  first search gathers. `refresh_met` regathers with the bikes in the world, as a flown plane's
  post does (`Flight::refresh` with `Bikes`); the post uses it when `strikes_bikes(met)` holds.

## Found on the way

- A bike rolling into a parked buggy's tail at 30 ft/s (the F23/F26 tests) is held by its own rays
  on the hull ~10 ft short of the car's origin; the car's own rays do not reach it even with the
  regather. A car driven into a standing bike does strike it with its own rays (kind-0 entries
  naming the bike).

## How it was checked

- New `online_car.rs` `an_online_car_strikes_the_other_seats_bike_with_its_own_rays`: g's buggy
  driven from 80 ft behind into the host's standing bike. On both machines the car's record holds
  kind-0 entries naming seat 0, the bike is shoved to ~59 ft/s (z ~90), where with the host's bike
  carrying no collision files it stays at z 0, and the machines agree tick for tick.
- With a temporary print before the `car.rs` fix the online car's `met.pushed` was never true, in
  this test and in F26's split test; after it, `pushed [true, ..]` and kind-0 entries from the
  first touch.
