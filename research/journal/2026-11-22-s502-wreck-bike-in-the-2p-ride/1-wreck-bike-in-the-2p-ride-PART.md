# 1 — wreck bike in the 2p ride (PART)

Task S502. 2026-11-22.

## What I did

- New module `crates/game-sim/src/ridden_wreck.rs`, wired into `Sim::step_all` (lib.rs):
  - `Sim::wreck_meets_riders(i, …)`: a downed rider's wreck bike with a riding bike in reach (20 ft past
    the bike's frame radius) is ticked by S134's `WreckBike::tick_ridden`, just before its rider's frame,
    where S486's parked-car lane ticks it. It gets every player's bike (where it stands nearest the wreck
    bike; in the world while its rider is up, no ghost and carries `bikef.col`), chassis and record object,
    as the frame before left them. The rider's frame then skips the tick (`WreckBike::flown`).
  - A wreck bike the parked-car or flown-plane lane ticked already is left alone. One out of every bike's
    reach is ticked by its rider's frame as before, so rides where no wreck bike comes near a bike do not
    change.
  - `ridden_wreck::file_struck`: the mirrors go on each struck rider's list for its frame, ahead of its own
    entries (as the planes' mirrors do, `struck_first`), up to the 11-entry capacity. Each names a copy of
    the wreck bike's body, put after the woken cars' bodies in the bump. Kind-1 mirrors push no other
    side, so nothing is written back.
- New test `crates/game-sim/tests/ridden_wreck.rs`.
- Nothing taken from ../opentennis: it has no rider-against-wreck contact.

## What the original does

From S134's run of the original in the interpreter (S93's short split-screen ride): P2's wreck bike's
four substeps run before P1's record's, with P1's chassis as the frame before left it. P1's record holds
the kind-1 mirrors naming the wreck bike's record after the frame. The wreck bike's resolve leaves P1's
chassis velocity as it was.

## How it was proved

- `p2s_wreck_bike_puts_mirrors_on_p1s_list`: P2 down 1.5 ft beside P1, its wreck bike entered sliding at
  6 ft/s into P1. Over 10 ticks of `step_all`, P2's wreck bike struck P1's bike on 5 ticks and put 9 kind-1
  mirrors on P1's record, none on its own. Filed on P1's list, they lead it and name the wreck bike's body.
- `p2s_wreck_bike_pushes_p1_in_the_ride`: the same ride moves P1 0.56 ft. With P1 out of the bikes' world
  (no collision files) it moves 0.0006 ft.
- The tick itself is S134's proof (bit-exact against the original for 42 ticks). This task's wiring is a
  port-side test, not a comparison with the original.
- rider_contact, two_p_rider, prop_wreck, prop_strike_paths, wreck_bike, seat_rivals, rider_rider_capture
  still pass.

## Not verified / not 1:1

- No end-to-end replay of S93's short ride through `Sim::step_all`: the port's 2p ride is not seeded from
  that image, so P1's frame with the mirrors on it is not compared with the original's.
- Where the mirrors sit in P1's list: put first, as the planes' mirrors are. The S134 test filters P1's
  record for them, so their place among P1's own entries is not measured.
- The wreck bike is ticked just before its own rider's frame. The walk goes from the last player down, so a
  wreck bike's mirrors reach only lower-numbered riders (P2's reach P1, as in the capture). A mirror on a
  rider framed earlier this tick goes nowhere, and that rider's chassis is this tick's, not the last's.
  The original's order for P1's wreck bike striking P2 was not captured.
- The chassis the wreck bike's resolve might push are not written back: S134 measured the original leaving
  P1's chassis as it was, on one wreck only.
- The ticking moves the hands-off check after the wreck bike's tick, for a wreck bike in reach of a bike
  (the parked-car lane does the same). Not compared with the original.
- Only `Sim::step_all`. The plane lane (`step_flown_riders`) and the shell's seat Sims (split screen and
  online, one Sim per seat) still tick the wreck bike without the other bikes: follow-up filed.
- The 20 ft reach is the parked-car lane's margin, not a number from the original: the search itself
  decides what is struck. Outside it the bike is ticked by `tick_among`, as before.

## Next

The seat path (follow-up). Then a 2p capture where P1's wreck bike strikes P2.
