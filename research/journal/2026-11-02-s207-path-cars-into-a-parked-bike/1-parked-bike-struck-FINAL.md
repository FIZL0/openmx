# 1 — a parked bike struck by path cars (FINAL)

Task S207. 2026-11-02.

## What I did

- **Found the sweep's cars again.** The port's sweep (`research/journal/2026-10-26-s153-idle-rollback-wreck/sweep/sweep.sh`)
  on today's HEAD still wrecks at every spot: free09 2800,4300 h0 tick 230, h270 234, h90 267 (was 275);
  free06 3700,3100 h180 82; free06 2500,4000 h270 194, h90 258. What strikes it: on free09 the
  north-east trailer truck (`free_trailercab` + its trailer, about 45 ft/s); at free06 2500,4000 the
  train, after the bike has rolled onto the rails (about 2447.8, 3991.4); at free06 3700,3100 the
  company truck `ctruck.psg`, `.toy` `Car22`, a `HasPhysics` car (S127), at 23–27 ft/s.
- **New scripts in `research/s207/`:** `cars.py` lists an image's path cars and when each passes a
  point; `park.py` loads a slot, runs until a car is LEAD game seconds from a clock, teleports the bike
  into its way (S76's method), lets it settle and saves; `watch.py` loads a slot and prints car, bike
  and wreck reason every 0.1 s; `look.py` summarises a capture; `slots.py` is S162's, any folder.
  `watch/s207-free09.toml` is s89-rails.toml's lanes with first_recording's rider and bike chains.
- **Four states in copy 3, parked at full speed and recorded at a quarter speed, no pad**
  (`/dev/uinput` is root's), `context/capture/s207/*.csv`, `context/states/s207-*.eeMemory.bin`,
  `context/s207/*.p2s` (s3 only). No row is torn.
  - **truck** — first_recording (free09, the 125), slot 32, saved at frame 8410: bike at 2800.1,
    103.4, 4299.6, facing −20°. 450 rows. The original wrecks on tick 105, reason 2.
  - **rails** — s89-along (free06, the 250), slot 33, frame 14711: bike on the rails at 2447.7, 135.5,
    3991.2; the engine meets it 34° off head-on. 300 rows. Reason 1 on tick 98, thrown at
    (−22.74, 4.17, −44.68).
  - **beside** — the same, 6 ft off the line, slot 34. The bike rolls back down the bank to 2453.7,
    3971 before the train comes; the train passes about 15 ft off. No wreck in 300 rows.
  - **ctruck** — s89-along, slot 35, frame 11992: bike at 3702.8, 234.6, 3100.2 on the truck's line.
    Struck on tick 36 and thrown to about 25 ft/s, no wreck in 300 rows. The truck stops dead on the
    strike (its node moves under 0.4 ft per 6 frames afterwards). A first try (slot 35 over-written)
    let the bike roll 10 ft down the slope first and the truck missed it by inches.
- **Replayed them** from each state (whole bike, wrecks on, the body rays carried, the original's pad)
  among the state's cars. `in_the_track_with_cars` is S129's free06 world on any track.
  `SaveState::traffic_with_trucks` adds the two `HasPhysics` trucks after the 21 path cars, the
  order of the original's moving list (tests/physics_cars.rs); `traffic` and its callers are unchanged
  (`path_cars` and `traffic` now call the new `cars_of` and `traffic_of`).

## Results

| ride | lanes agree through | original's wreck | port's wreck |
|---|---|---|---|
| truck | 104 (parts on 105, chassis vel.x 8.3815 vs 8.3199) | 105, reason 2 | 105, reason 2 |
| rails | 97 (parts on 98 by one ulp, origin.x) | 98, reason 1 | 98, reason 1 |
| beside | all 300 rows | none | none |
| ctruck | 34 (parts on 35: port vel.x 9.58, original −0.48) | none | 172, reason 1 |

The truck's strike velocity is (8.32, −1.86, 11.38) in the port against the original's (8.38, −1.53,
11.77): **B7**. The ctruck: the original's struck truck wakes its body and stops; the port's drives
on, meets the bike a tick early and pushes it again into a wreck: **B8**.

## How it was checked

`cargo test --release -p game-sim --test replay -- --nocapture`: 109 passed, 1 ignored. S207's four
rides all ran (none skipped); the skips are other tasks' captures this slot does not hold (S184,
S191, the C6 acceptance run).

## Not verified / not 1:1

- The bike's headings are not the sweep's (h0/h90/h180/h270): with no pad the bike cannot be turned,
  so each stands as the state left it (−20°, 1°, −4°, −8°), and the spots are where the original's
  car passes, not the sweep's exact points. The port's sweep wrecks were not themselves reproduced
  in the original; these are the same cars striking a parked bike.
- Only a still bike with nothing held. Nothing with a rider on the brakes or leaning.
- The truck's thrown velocity (B7) and everything about a struck `HasPhysics` truck (B8) are not 1:1.
  The ctruck row pins the port's current wrong wreck so a fix shows; the one-tick-early contact is
  not explained.
- The ctruck state's bike may not have the trucks in its nearby search at save; `bike_search_holds_cars`
  checks the path cars only (it is true for all four).
- The states and captures are in s3's `context/` only, not in the main checkout's.
- Nothing was taken from opentennis.
