# 1 — split thrown rider meets shared truck (FINAL)

Task F75c. 2026-11-21.

## What I did

- `parked_wreck.rs`: the F73 lend and take-back are now free functions over a car slice,
  `lend_cars(rider, cars, n, world)` and `cars_back(rider, cars)`. `Sim::lend_parked` and
  `Sim::parked_back` call them. `Sim::parked_back` now does nothing when the Sim holds no cars, as
  `Sim::lend_parked` already did, so a list lent from outside the Sim survives its step.
- `main.rs` `Seat::pass`: on the bike, with split screen's shared cars, the seat's rider is lent the
  cars right after its wreck bike meets them (F75) and they are taken back after `pass_among`,
  waking each car a bone struck. Same order as `Sim::step`: wreck bike, lend, rider's frame, take back.
- The first seat with AI riders (F75b) is unchanged: its Sim holds the cars for its pass and lends
  them itself.
- No opentennis code applies (it has no parked cars).

## What the original does

The original has one copy of each parked car; every rider's ragdoll meets it (F73's finding, S484's
ride1.csv). Nothing new read for this task.

## How it was proved

- `main.rs` `a_split_seats_thrown_rider_wakes_the_shared_truck_as_a_lone_seats` (disc test): F75's
  ride with the thq1 ragdoll on. Truck awake on ticks 8,10,…,22 alone and as seat 1 of 2; 9,11,…,17
  with no ragdoll. Truck z within 1e-4 ft every tick.
- With the shell's lend switched off the test fails: seat 1's truck wakes on 9,11,…,17, the
  boneless ticks (bones passed through).
- game-sim replay tests filtered `parked` and `truck` (F73's lined-up ride among them) still pass.

## Not verified / not 1:1

- No capture of a split-screen wreck into a parked car exists; the proof is "seat 1 does what a lone
  seat's Sim does", and the lone seat's match to the original is F73's (with R55's known gap).
- Seat 1 matches the lone seat within 1e-4 ft, not bit for bit (as F75's test): the other seat
  standing 100 ft off is in seat 1's world, and I did not chase which bits move.
- The lent cars carry their bodies from before this pass's bike resolve, as in `Sim::step`; the
  shell writes that resolve's pushes back after the pass. Not checked against the original's order.
- AI riders in a later seat's Sim still keep their own copy (F75b's `parked_out` rule); untouched.

## Next

none
