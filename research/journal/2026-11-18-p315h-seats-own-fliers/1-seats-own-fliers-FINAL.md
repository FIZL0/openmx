# 1 — seats own fliers (FINAL)

Task P315h. 2026-11-18.

## What I did

Seats 3 and 4 (and any past them) now fly and wear their own Fliers-tab picks. Before, the ride built
plane and helicopter files, lift, stock files and drawn models one a livery (two), so seat 3 rode
P1's and seat 4 P2's.

- `vehicle.rs`: `Plane::livery` and `KiowaRide::livery` are now `flier`, the seat whose flier it is
  (the copy's own). That was all the field was ever used for (an index into the drawn lists and the
  lot's lift and helicopters). Which skin it is stays on the seat's `Fliers`: the pack's, or
  `Fliers::original` for the disc's livery. That is the livery vs. skin split the task asked for.
- New `vehicle::flier_slot(copy, built)` and `vehicle::own_flier(list, copy)`. A seat gets its own
  entry. A seat past what the ride built (one who joined after the load) gets its livery's entry,
  then the first. Every per-seat list goes through them: `Lot::lift` (now a `Vec`, with
  `Lot::lift_of`), `Lot::kiowa`/`kiowa_of`, the drawn plane and Kiowa lists, plane files, stock plane
  files, sounds, cameras and the Kiowa's own particle lists.
- `Lot::plane`, `plane_anywhere`, `heli_for` and `bay_plane` set `flier` to the copy. Each bay's
  see-through plane now shows its seat's own skin.
- `main.rs` `fliers_for` builds `players.max(2)` fliers, one a seat (`seat_flier_picks` already read
  seat n's picks from `args.others`). The drawn lists load one model a seat in the ride
  (`flier_seats`). `refit_fliers` (the pause's mid-ride switch, P315g) rebuilds by seat.
- `vehicle::tell_apart` now works two by two: seat 2 against seat 1, and seat 4 against seat 3. On
  the same skin, the second of a pair wears the other skin of the model, or the disc's other livery.
  The old defaults stay as they were: no picks gives P1–P4 biplane, biplane2, biplane, biplane2, and
  `--fliers mxvatv --flier-skin 4` gives 4, 3, 4, 3.
- `menu.rs` `OWN_FLIERS` is `SEATS_MAX`, so every seat's pause Garage offers its own flier skins.
  Before, only the first two seats did.
- Online is unchanged. Its per-tick reset `plane.flier = vehicle::livery(d)` puts a remote seat's
  drawn index back where it was before this task.

Nothing to take from opentennis: it has no machines.

## What the original does

The original has two players at most. Player 2's machine is parked in the alternate livery. It has
no flier picks per player. Seats 3 and up, and picks per seat, are the port's own front end.

## How it was proved

- `crates/openmx/tests/mxvatv_fliers.rs` `every_seat_of_four_flies_its_own_picks`: two headless
  four-seat rides on free09, each reading its own `garage.txt`.
  - Ride 1: P1 pack biplane3 + disc kiowa, P2 disc biplane2 + kiowa2, P3 pack biplane (model 1) +
    pack kiowa3, P4 disc biplane + pack kiowa4. The test checks every seat's `biplane:` and `kiowa:`
    drawn line, P3's and P4's `fliers:` lines, and `4 of 4 liveries loaded`.
  - Ride 2: P1, P3 and P4 all on biplane4. P4 is drawn in biplane3; P2, with no pick, rides the
    disc's biplane.
  - The first run failed on an expectation of mine, and the log showed the rides really ran (pack
    and disc were present), so the pass is not a skip.
- Unit tests in `vehicle.rs`:
  - `four_seats_on_mx_unleasheds_first_skins_alternate_the_liveries`
  - `every_seat_finds_its_own_flier_and_a_late_one_its_liverys` (own_flier fallbacks, lift per
    seat, `flier` per copy)
- Changed asserts in `each_player_takes_their_own_plane`, `tests/bays.rs`, `tests/machines.rs` and
  `tests/machines_everywhere.rs`: `flier` is the copy.

## Not verified / not 1:1

- Seats 3 and 4 flying their own `.phy` is shown by the ride lines and by `flight_for` reading
  `own_flier(files, seat)`. No test flies a third seat's plane and hashes it (no four-pad tape).
- A seat that joins after the load (drop-in) still flies its livery's flier: P1's or P2's. Its own
  picks are only built once the ride loads again, or when its pause switch rebuilds them. The
  switch's rebuild for a seat past the built list is not tested.
- Particles: the plane's emitter binds are still read once off the first seat's drawn model, as in
  P315b. Not checked on screen.
- Online with fewer local drawn models than remote seats can still draw a remote seat in an online
  pack entry (`plane.flier = livery(d)` landing at or past `online_drawn_from`). This was there
  before this task, and I did not touch it (P318's area).
- No on-screen look at four seats' planes or helicopters. Checked by ride lines only.
- Mid-ride pause switch for seats 3 and 4 (now offered by the menu) is checked only by the existing
  menu tests passing. No headless pause-switch ride for seat 3 was run.

## Next

Done.
