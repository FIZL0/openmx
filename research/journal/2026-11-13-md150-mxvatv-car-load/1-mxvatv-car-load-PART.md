# 1 — mxvatv car load (PART)

Task MD150. 2026-11-13.

## What I did

- Added `game_sim::atv_car::Load` (end of `crates/game-sim/src/atv_car.rs`), the car twin of MD122's
  `quad::Load`. It takes a `Veh\<car>.phy`, read with `car::Car::read_mxvatv` (MD39b) and, for
  `[Mass Properties]`, again with the rigid-body part reader (`handling::mass::Part`, as MXVehicle's base
  loader does). From the chassis model it takes the part reader's half box and the four tire nodes' points.
  It builds:
  - `keys()`: the whole `Keys`, with each tire's `TireKeys` and the `EngineKeys`;
  - `input_keys()`;
  - `car()`: what the load sets: riding, the yaw limit, the landing sound and sort rays, the body's centre of
    mass, each tire's mount (point + `TireWidth` in x, w 1) and radius;
  - `body()`: enabled, the part's mass, damping and inertia, top speed 300, rates 12, no accel limits, no pose;
  - `vehicle(drag)`: wrecks on, bounded, `[Wrecks]`, the kind word, inertia, look, record, gravity, flag byte.
  - `TIRE_LEVERS` (fronts Brake/Steer, rears BrakeForce/RearSteer).
  - `tire_points(model, frame)`: the quad's mount method with `car::TIRE_NODES`.
- New test `crates/game-sim/tests/mxvatv_car_load.rs`:
  - `each_car_from_its_files_is_its_spawn_image`: for all five cars it reads every `Keys`, `InputKeys`, loaded
    `Car` word, non-pose `Body` word and loaded `Vehicle` word off the image and compares them with `Load`'s.
    It also compares every `Part::words` word, masked, with the image.
  - `the_models_tire_points_are_the_images_within_two_thousandths` and
    `the_models_box_is_the_records_within_a_percent` bound what the model gives.
- `research/oracle.txt` `# MD150`.

## What the original does

The car's setup (`./kit fn 0x004c76f0 -p atv`) is the first call of each class's node finder (buggy
`0x004c4b00`, golf `0x004ceda0`, truck `0x004d64d0`). For each of the four tire nodes it:

1. finds the node under the car's container;
2. carries the node's point into the container's frame (`0x00194c20`, the quad's call);
3. builds a tire (`0x004b48b0`) and sets it up (`0x004b4d30`): model, car, body, the brake and steer lever
   pointers, the point with w 1, and no sweep.

The tire reader then adds `TireWidth` in x. The setup builds the engine (`0x004b3620`), hands it the car's gas,
clutch and brake-force words, and builds the sort's ray probe.

The base loader (`./kit fn 0x004a7e70 -p atv`) runs the part reader on `[Mass Properties]`, reads `[Wrecks]`,
and copies the body's inertia to the vehicle's own.

## How it was proved

- Each of the five spawn images (`context/mxvatv-ps2/md39a`, the five drives' images) against the car built
  from its `.phy` and handed-in model inputs.
- Word for word: all `Keys` (with 16 surfaces × 16 words per tire, the engine's tables, wings and shift
  points), `InputKeys`, `Car`'s load words, `Body`'s non-pose words, `Vehicle`'s load words and every
  `Part::words` word (masked).
- To be bit-exact, the main test hands in each tire point as the image's mount less its width (the float that
  adds back to the mount). It also hands in the half box solved from the record's inertia (`+0x60`) and the
  body's (`+0x130`), MD122's method.

## Not verified / not 1:1

- **The model inputs' frame.** The tire points and the half box come from the chassis model placed in a frame
  that no spawn image keeps, because the cars have settled since their load. The test hands both in from the
  image:
  - Tire points: `tire_points` at the origin, or at the image's current car matrix, lands within 2e-3 ft
    (tested). It is not bit-exact. The spawn/heading frames are ~0.2 ft off.
  - Half box: the model's box at the origin gives the record's inertia within 1% (tested). It is not
    bit-exact in any frame tried (identity at spawn, at the matrix origin, at zero; the heading frame).
  - The box solved from the record alone is not unique on the buggy (its body inertia was 1 float off). The
    test solves it from the record and the body together.
- **Constants read off the images, not out of the code:**
  - body top speed 300, rates 12, accel limits f32::MAX, inverse inertia w 1;
  - the tires' friction scale 1;
  - the flag byte's 0x06 bits.
  These are the same on all five images. I did not trace where the constructor sets each one.
- **The keys with a leave-alone getter** (`[Preload]` and the clutch's launch keys) fall back to zero when a
  file lacks them. No retail file lacks them, so the constructor's real value is unknown.
- **Pose and ride words are not built:**
  - the body's pose and motion;
  - the car's measured words (speed, matrix, ground, tires' contact state);
  - the Vehicle's scene, subtree and bounds;
  - the rumble and touching words.
  The test takes these from `Load`'s own values, so they are unchecked. The world's drag is handed in from the
  image, because it is the world's word, not the car's.
- **The collision files** (`m.col`/`f.col`) are not parsed. The test notes if one is missing. The collision
  record's inertia comes through the part reader's box, as above.
- **Only the stock liveries** in MD39's files were checked.

## Next

MD149: the car stepped on MX Unleashed's worlds as an `atv_car` Ride seeded from this `Load`. Finding the setup's
frame (a capture at the load, before the car settles) would make the model inputs 1:1.
