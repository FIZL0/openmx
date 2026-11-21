# 1 — split ai riders share parked cars (FINAL)

Task F75b. 2026-11-21.

## What I did

- `game-sim`: new `Sim::parked_cars_held`. Raised, the Sim's step still has its bikes strike, push and
  wreck on `Sim::parked_cars`, but `parked_cars_frame` frames none and `parked_cars_sleep` sleeps none.
  It is the shell's, set per pass: not hashed, not on the wire (the snapshot decode keeps this machine's).
- `openmx` (`World::simulate`): `parked_out` now shares the cars when only the first seat's Sim has AI
  riders (the only Sim the shell gives race AI riders, `race_ai`). The shell frames them before every
  seat's bike as before; at the first seat's turn, when it rides the bike, the cars are lent into its Sim
  with `parked_cars_held` raised, and taken back after its pass; `parked_back` sleeps them after every
  bike. `Seat::pass` is handed no cars then, since the Sim's own step runs the downed rider's wreck bike
  against them (`Sim::wreck_meets_parked`), which would otherwise run twice.
- On any other machine up in the first seat (plane, car, quad, helicopter) the cars are not lent: its Sim's
  bike step does not run then, and the shell's paths (F72, F75a) handle them as with no AI riders.
- AI riders in a later seat's Sim still keep every seat's own copy (`None`), as nothing lends them there.
- Nothing taken from ../opentennis: it has no parked machines.

## What the original does

One truck, one dynamics world: the woken cars step before every rider's update and the object listener
sleeps those at rest after all of them (S485, S486). The shell's split screen is several Sims, so the
port has to hold the cars outside them to keep that order (F72); this task does it for the Sim that also
walks AI riders.

## How it was proved

- `split_seats_share_the_parked_truck_beside_ai_riders` (crates/openmx/src/main.rs, disc test): seat 1's
  bike rolls into the disc's settled monster truck at 30 ft/s. With an AI rider in seat 0's Sim 200 ft off,
  the truck's z is bit-identical tick for tick (45 ticks) to the run with no AI rider, which is F72's
  proven shared path; both seats hold one truck every tick. With the pre-F75b `parked_out` it diverges at
  tick 19 (z 24.972855 vs 25.00244), so the test catches the old per-seat copies.
- Second half: the AI rider itself rolls into the truck, seat 1 200 ft off: woken, pushed > 0.03 ft, both
  seats hold the one truck.

## Not verified / not 1:1

- Not checked against a PCSX2 capture: no capture of split screen with AI riders beside a parked car
  exists; the proof is equality with the F72 shared path, which is itself a port-side order argument.
- The AI rider pushing case is only checked for "woken and pushed, one copy", not against a single Sim
  ride bit for bit: the AI rider's bike sits at another index in a single Sim, so its walk order differs.
- AI riders in a later seat's Sim (not the first) keep per-seat copies: the shell never gives a later seat
  race AI riders, so that path is untested and left as before.
- Parked planes with AI riders (`planes_out`) are still stepped inside the first seat's Sim alone, not
  shared through the shell; not this task's scope.

## Next

Nothing; the task lands with this entry.
