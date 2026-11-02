# 1 — riding bike meets cars as toys (FINAL)

Task S213. 2026-11-02.

## What I did

- Found the code move already done: `ride.rs` `wheel_hits`, `Ride::sweep_back` and the body rays all
  search `Ride::nearby` through `nearby_scene` (`Scene::flown`) and gather it with `refresh_ridden`
  (`world_flown`), since S189. `prop_world.rs` searches `Scene::probed` (flown, cars as toys), since S200.
  The only `Scene::wooded` callers left are tests.
- `tests/traffic.rs` `the_riding_bikes_probes_search_the_planes_world`: reads every rider (actor type 2
  in the game manager's list, four in each state) in S171's `s171-thrown` and S146's `s146-roof` RAM
  images: the collision object (`+0x7d0`), its ray and sweep probes, their leave-out and category lists,
  and the kept nearby list (`+0x7bc`).
- `tests/traffic.rs` `a_bike_ridden_onto_the_trains_roof_meets_it_as_a_toy`: the port's stock bike, let
  go 2 ft over the back of the boxcar's roof at the wagon's velocity + 4 ft/s, coasting with no drag, 45
  ticks among the moving cars.
- Filed R16 (low): rolled off the wagon's front at about 35 ft/s over the train, the bike meets the next car
  and its chassis velocity goes −135 then +135 ft/s.

## What the original does

Each riding bike's (the human's and the AI riders') ray and sweep probe is placed in the same table as
the wreck bike's and biplane.p2m2's plane's. That table's moving list has the 21 path cars and the two
driving toys at its head, each one category 0x48. The leave-out list is on (mode 1) and holds three
collidables, the bike's own (categories 0x9a048 for its body collidable, 0x18000, and the other probe's
own: 0x8 or 0x18008). There is never a toy in it. The category list is empty, a lone 0, or 0x40 on a ghost
(S129). So a solid bike's kept list holds every toy (23 of category 0x48; the moving list hands them out
wherever the bike is), and a ghost's holds none. In s171-thrown 3 bikes are solid and 1 is a ghost; in
s146-roof all 4 are solid.

## How it was proved

- Save-state RAM: `the_riding_bikes_probes_search_the_planes_world` on s171-thrown and s146-roof (copied
  into this slot from s3's and s2's `context/states`) plus biplane.p2m2's state for the plane's list.
- Port: `a_bike_ridden_onto_the_trains_roof_meets_it_as_a_toy`. On every one of the 45 ticks, the kept
  list holds all 21 cars as toys (category 0x48). The wheels touch the roof on 33 ticks and name the
  wagon (object FIRST + its prop) as their vehicle on all 33. The bike stays on the deck at 58 ft/s on a
  55 ft/s wagon. In the control (cars' motion taken away), the same 33 touching ticks name no vehicle.
- `./kit check -- -p game-sim`: 978 passed.

## Not verified / not 1:1

- The ridden test is the port alone; no capture of the original riding a bike onto a moving roof (S146's
  replay covers a braked bike already on one).
- The lone 0 in some bikes' category lists (count 1, word 0) excludes nothing, so the port reads it as an
  empty list. Where it comes from was not looked for.
- R16: what happens when the bike meets the next car's end face (a velocity swing to ±135 ft/s) was not
  checked against the original.

## Next

Nothing for S213; R16 carries the end-face contact.
