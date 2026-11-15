# 1 — mxvatv car record on mxu (FINAL)

Task MD149b. 2026-11-15.

## What I did

- `atv_car::Load::record` builds the car's collision record from its files: the part's record words,
  the constructor's words, the riding rays from `<car>m.col`, and the held collider.
- `atv_car::Ride::rig` puts that record and the held contacts on a ride. `Ride::record_on` runs it
  each tick before the frame, against `World::solid` and `World::groups` through `quad::RecordOn`.
  So `Vehicle::touching` and the held contacts are set, and GravityRecord is answered.
- The heightfield word follows `car.tipped`, as the car's frame writes it.
- openmx: `MxvatvCarRide::with_record` reads the pack's `col` (mod.json `cars[].col`). The shell
  rigs the car when it has one.
- Tests in `crates/game-sim/tests/mxvatv_car_mxu.rs`:
  - `each_cars_record_from_its_files_is_its_images`
  - `each_car_rolled_over_wrecks_tipped_and_is_put_back`
  - `each_car_driven_into_a_prop_stops_on_it`

## What the original does

- The record words are the same in all five MD39a images: +0x20 0x3ff, +0x28/+0x2c/+0x30 1,
  +0x48–0x54 1.0, +0xf4 the ray count, +0x100 0x1000, +0x104 !0, class word +0x94 0x43806.
- The tipped wreck clause: under 10 ft/s, touching, one side down or not grounded, up.y < 0.8.
  Before this task `touching` was never set on MXU, so a car never wrecked tipped there.

## How it was proved

- Record from the files vs. each of the five MD39a images (buggy, baja, sandrail, golf, truck):
  record words, held collider, mass, riding rays bit-equal. Inverse inertia within 1%.
- Riding on free09, headless (no capture: the original has no car on MXU):
  - All five cars put on their roofs wreck tipped in 7–23 ticks. Each is put back upright within
    2.7 ft of the kept spot and rides on. The body never sinks under the ground.
  - All five driven head on into prop 270 strike it (struck = 271) and drop under 5 ft/s within
    9 ticks. Their bodies stop 1.4–7.9 ft short of the face.

## Not verified / not 1:1

- Nothing is compared with a capture. The original has no car on an MXU world.
- `quad::RecordOn` is not the original's part search. It sweeps plain ray bundles, and the sweep
  back is one ray. Taken from the quad (MD123) as it stands.
- Struck props get category 0, and props are numbered owner + 1. Nothing on MXU reads either.
- The record runs once per tick, before the frame. The order is not checked against the original.
- No wreck-ray swap. It is unknown whether the car swaps rays when wrecked. `<car>f.col` is unused.
- Inverse inertia matches the images only within 1%. This is the model-box gap MD150 already noted.
- A car rolled onto its side does not wreck tipped on MXU. Its tires read ground straight down
  (`quad::world_ground`), so both sides count as grounded and the suspension stands it back up.
  So the test rolls the cars onto their roofs instead. Filed as MD168.
- The golf cart, held on the gas against the prop, climbs the face and slides off round its side
  after it stops. The prop test only looks at the 30 ticks after the strike. Whether the original's
  cart does the same is not known.
- Untamed car packs have `col` "" and get no record. They ride as before.

## Next

Nothing for this task. MD168 is the side-tipping follow-up.
