# 1 — car hull online (FINAL)

Task F23. 2026-11-03.

## What I did

- **`net/rides.rs`, `Rides::step`.** The contacts are now built here rather than through
  `rider_contact::among_met`, so the cars can join the list. The hulls are the flying seats' planes,
  then each touching seat's car (`vehicle::lane_hull`), the same order `World::simulate` uses (F26).
  The riders' list leaves out a seat in a car as well as one on the plane. That matches split
  screen's `on` (`garage.up == Kind::Bike`) and the `on` that `Rides::step` already used for rivals.
  A bike's `Bump::planes` is the planes' `Flown` and then the cars' (`vehicle::lane_flown`). The
  write-back goes to the plane, or else to the car (`vehicle::lane_pushed`). A car seat gets its
  hull's mirrors as a `flight::Met` (`rider_contact::met_among` at the car's body), and
  `vehicle::drive_lane_met` steps the car on them (`CarRide::step_met`, F26).
- **`vehicle.rs`.** Added `lane_hull`, `lane_flown`, `lane_pushed` and `drive_lane_met`.
  `drive_lane` is `drive_lane_met(None)`. The hull mesh is a constant of the disc, so it comes from
  the seat's `cars` copy (`CarRide::hull`) and never goes into the state or its bytes.

## How it was checked

`tests/online_car.rs` `an_online_bike_is_stopped_by_the_other_seats_parked_car`: two machines
(host and g) run through `Netplay` over the in-memory wire with a 3-round lag, on a flat floor,
riding race125p's collisions and the bike's collision files. g's seat takes the buggy on its first
tick, 25 ft up the host's way, and leaves it parked. At tick 60 the host's bike is set to 30 ft/s.

| | bike ends at z | car's top speed in the second after | car moved by the end |
|---|---|---|---|
| hull in | 18.49 | 2.90 ft/s | 4.64 ft |
| no hull | 64.96 | 1.16 ft/s | 2.94 ft |

Both machines read the same numbers, and their hashes and pad logs agree on every settled tick.
Negative control: with `lane_pushed` stubbed, the car reaches only 1.40 ft/s and the test fails.

Also passed: `./kit check -- -p openmx --test online_car --test netplay --test markers_online
--test online_put_back --test online_wreck_sound --test car_switch --test plane_seat_props --test
join` (268), and `-p openmx --bin openmx -- split_contact net::` (14).

## Not verified

- None of this is held to a capture of the original; none exists (R22, filed by F26).
- The car's own rays on the bikes are not ported, offline or online (as F26 says).
