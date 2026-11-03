# S256 — the braked bike's body rays meet only its list's cars

2026-11-03. Task: `./kit plan S256`.

**What shoved the port's bike.** The replay already seeded the ghost-gathered nearby list
(`seed_bike_search`: no path car in it, as in the image), and the wheel probe went through it. The
shove came from the body rays instead. `Sim::car_contacts` (`mover_contact.rs`, S137) ran them
against every path car in the world. It never read the bike's list, and the original's rays collide
only against what that list holds (S189, `the_parked_bike_never_gathers_the_train`). From tick 113
the train's cars reached the bike, and their contacts pushed it at 6.8 ft/s.

**The change.** `mover_contact::struck_among` takes the bike's list. The candidates are the cars the
list names (prop index after `flight_search::FIRST`), in the traffic's order as before. With no list
(a bike that has not yet gathered one) every car is met, as before. `car_contacts` passes
`ride.nearby`. A solid bike's list holds every path car: the port's across list holds all 21, and
the original's holds 26 entries in the ghost's category. So a riding bike meets the train as before.

**Results.**
- Braked agrees on every lane through tick 143 (was 112). The port's bike stays within 0.01 ft of
  the original's under the whole train, through tick 351. On 144 the two part by a creep of about
  1e-5 ft/s (chassis velocity, angular velocity). The test now also asserts that the port's braked
  bike never moves 0.25 ft and never goes down.
- Across still agrees on every tick, and with no body rays it still parts at 148.
- `tests/moving_vehicles.rs` parked the car 1000 ft off while the bike settled, then placed it
  beside the bike. The bike's list was gathered without it, so the rays met nothing. The test now
  clears the list once the car is placed, and the bike gathers a new one there. With the filter and
  no clearing, the wagon's rays struck nothing and the head ray ended up inside the car's box
  (in_box 9). With the clearing they strike it on 9 ticks of 90 and the bike is pushed back.

**Checked.** `./kit check -- -p game-sim --no-fail-fast`: everything passes except
`fly_and_collide`'s drawn lean, which also fails with this change backed out (filed as B18), and
`the_humans_tricks` (known red on C6's capture). replay.rs's car tests were run with s1's, s2's
and s3's contexts (`GAME_SIM_FIXTURES`): S129, S146 roof, S191, S252, the wreck and truck rides. All
of them passed. `-p openmx --test train`: 4 passed.

## Not verified / not 1:1

- Whether the original's list takes in a car that comes near after the gather, for a bike that
  stays inside its bound. The port's gather (S189) does not, and the yard test only works around
  it. No capture has a car arriving at a standing, solid bike whose list was gathered with that car
  far off.
- The 1e-5 ft/s creep at braked tick 144 was not traced. It is not the train's push: the bikes
  stay within 0.01 ft afterwards.
- Order: the candidates keep the traffic's order, filtered. The order the original's list hands
  them out in was not compared.
