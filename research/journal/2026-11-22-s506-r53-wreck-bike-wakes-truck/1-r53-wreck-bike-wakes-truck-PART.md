# 1 — r53 wreck bike wakes truck (PART)

Task S506. 2026-11-22.

## What I did

- `crates/game-sim/tests/c16_fly.rs`: a new R53 section. `r53_ride` seeds the Sim off
  `context/states/r53/r53-348.ram`: P1 in the plane (`seeded_plane_at` on 0x604740; the image has it
  flown, body awake), P1's rider standing at its bike, P2 down with reason 4 (`wrecked_p2`, then the
  image's wreck clock 0.0333, hands held while under the throw's hold time, not landed), free09's
  truck settled at its map spot and put as the image holds it (`seat_truck`, R48's seat with the
  truck's address as a parameter). Idle pads, the capture's frame times, the split free09.
- Three tests replace S486a's `a_wreck_bike_in_the_plane_lane_wakes_a_parked_truck` (reason 7, C16
  image, < 10 ft bound):
  - `r53_the_wreck_bike_rolls_at_the_truck_as_the_original`: rows 350..=357, the wreck body's place
    and velocity and the truck's body bit for bit, the truck asleep on both, plane > 390 ft off.
  - `r53_the_wreck_bike_wakes_the_truck_on_the_originals_row`: the truck wakes on row 358 on both; the
    original's one own contact (kind 0) is the wreck body's collision object (body + 0x1040); body to
    body 10.6 ft (Sim within 0.1); the wreck bike's `z` speed below zero on row 359 on both.
  - `r53_the_sim_parts_from_the_original_on_the_bounce`: pins row 359 as the first where the truck's
    awake word or the wreck body's velocity (0.1 ft/s) differs.
  - `r53_rows_beside_the_sim` (ignored) prints both.
- Small refactors: `standing_p2` calls a new `standing_at(ee, rider, ..)`; `r48_seat_truck` calls a new
  `seat_truck(ee, truck, ..)`. `ParkedTick` lost the `plane` and `wreck` fields only the old test read.
- Filed S510: where the Sim parts (the bounce and the truck's response), with the oracle route.

## What the original does

From the capture (research/r53/index.md has the run): the wreck bike tumbles at the truck 22.8 ->
16.9 ft/s (rows 350-357), touches it on 358 (truck awake, one contact), bounces back at -2.3 ft/s
(359), the truck takes (0.02, -0.09, -0.04) ft/s and sleeps on 364, and a second touch wakes it on 374.

Row phases: row 349 holds the image's own state (its wreck body is 1.6 ft, two ticks' travel, short of
row 350's), so tick t from the image ends on row t + 1 as in R48, and row 349 is a late read. The Sim
lands on row 350 bit for bit from the image after two ticks, which confirms it.

## How it was proved

A save-state capture compared row by row: `context/capture/r53/wreck-truck.csv` against the Sim
seeded off `context/states/r53/r53-348.ram`. Bit for bit on rows 350..=357 (wreck body place and
velocity, truck place and velocity); the wake row matches; the parting row is pinned. Checked the
tests really run by moving `R53_PARTS` to 360 (failed: got 359).

## Not verified / not 1:1

- After row 358 the Sim parts: its wreck bike bounces back at -1.34 ft/s where the original's does at
  -2.31 (row 359); its truck keeps its own settle's -0.25 ft/s down instead of the original's -0.09,
  sleeps on 359 and flickers awake on 360 and 362 (the original's stays awake 358-363); the original's
  second wake on row 374 does not happen in the Sim. Not fixed here: it needs the original's node
  update replayed on R53's image (as R48's was) to find what the plane lane's wreck-bike/parked-car
  resolve and sleep miss. Filed as S510.
- Row 358's wreck columns are torn (they repeat 357's), so the Sim's wreck body on the wake tick is
  held only by its distance to the truck (within 0.1 ft of 10.6), not bit for bit.
- Row 349 (a late read) and row 348 (before the load) are not compared.
- The wreck's rider (the ragdoll point, `holding`, `landed`) is not in the capture; it is seeded from
  the clock (holding while under the throw's hold time, not landed), not read from the image. The
  wreck body matching bit for bit to 357 says it does not touch the bike there.
- The Sim's wake is taken to be the wreck bike's because nothing else is near (plane 391 ft, P1's
  bike's chassis about 640 ft off in the image); the Sim's contact list is not inspected.
- P2's pad is `Report::missing()` and P1's idle, not the capture's pad bytes (P2 is down and P1's
  plane parked; the plane does not move on either).

## Next

S510: replay the original's node update on r53-348 and fix the bounce and the truck's response.
