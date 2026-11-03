# 3 — every search's strike on a truck's path mesh, and a parked plane wakes ctruck (FINAL)

Task S252. 2026-11-03.

## What I did

- **One mark on the mesh.** In the original, the struck listener belongs to the car's path mesh,
  not to any one search. `collide_against_search` (0x0011a9d0) tells it about every hit, whichever
  probe made it. So the port's world `Prop` now carries the listener: `heard` (an `AtomicBool`, so
  `Props` stays `Send + Sync`), `Prop::strike_heard` and `Prop::take_heard`. Only a moving prop (a car
  mesh) marks.
- **The searches that go through `flight_search::Scene`** now mark the prop on a hit, in `Scene`'s
  prop pair. That is one place, and it covers the bike's in-frame scenery search
  (`rider_contact::scenery`, `ride::nearby_scene`), the wheels (`ride::wheel_hits_in`), the sweep
  (`sweep_back`), the wreck bike, the plane's ray probe and sweep (`flight`), the knockable props' and
  the woken trucks' own searches (`Scene::probed_among`, `prop_world`), and car.rs and kiowa.rs.
- **The ragdoll** searches copies of the cars (`ragdoll_scene::PathCar`). Each copy now holds its
  prop index (`prop`) and a `struck` flag, which `WreckBikeScene::touching` sets when a sphere's
  record takes the car. After the ragdoll's step, `ragdoll_ride` marks those props.
- **`Traffic::heard`** takes the marks at the start of `Traffic::frame_late`: after the tick's
  searches, before the `HasPhysics` updates. Each marked car wakes there through `Traffic::strike`,
  in car order, the same place S186a's body-ray strike lands. The body rays' direct strike
  (`Sim::car_contacts`) is unchanged; a second strike on a car that is already physical does
  nothing.
- **The capture.** `research/s252/aim.py` parks biplane.p2m2's plane on free06's company-truck road.
  It is a version of S241's put-back aimed at a truck. A first try, 80 ft along the truck's heading,
  missed: the road bends, and the plane rolled down the bank. So I gave it `AT=x,y,z`, a point the
  first capture showed the truck pass (3933.0, 201.6, 2609.2). The state came from the biplane
  state in slot 8 and went into slot 21 of copy 3.
  `research/s252/strike.py` records `watch/s252-truck.toml` from there and saves states around the
  wake. Its output is `context/capture/s252/plane.csv`, the states `context/s252/plane-*.p2s`, and
  their RAM in `context/states/s252-plane-*.eeMemory.bin` (from `research/s252/slots.py ram`).
  **The original's truck turns physical (+0x588) on 11092.** That is the tick the plane's contacts
  come on (8). Its +0x470 stops at its 11091 point, so the strike came before that tick's update,
  as S186a inferred for the body rays. The plane is pushed off at about 1 ft a tick.
- **The test** is `tests/replay.rs`
  `a_parked_plane_wakes_the_truck_driving_into_it_as_the_original_does`. It takes the 11072 state
  (the capture's 11073 row): the plane's body, tires and engine come from `off_the_save_state`, and
  free06's cars with the trucks' physics come from the state. The run has no pad.
  - The truck's path point matches the capture to the printed digits on every tick (clock to the
    bit).
  - With the plane's body put on the capture's row each tick before the wake, the port's ctruck wakes
    on tick 19 (11092). It stops at the original's 11091 point, and the plane's push after it is
    within 0.07 ft of the original's.
  - With the port's own plane, the plane creeps a quarter foot short of the original's on its tires by
    11091 (0.05 against 0.06 ft a tick). The truck then meets it two path steps later, on tick 21, and
    still wakes. The test holds both outcomes.
  - Mutation check: without the mark in `Scene`'s prop pair, the truck drives on through the plane
    and the test goes red on tick 19.
- **`tests/truck_wheel.rs`** part 3 looked for a spot where a wheel stands on the sleeping truck's
  path mesh *and the truck stays asleep*. Now the wheel's search strikes the mesh, as the brief says
  the original's does, and the truck wakes at the frame's end. The test now asserts that wake. Its
  other parts are unchanged.

## How it was checked

- `./kit check -- -p game-sim --lib --test hash --test physics_cars --test physics_car_struck
  --test prop_plane --test prop_world --test ragdoll_cars --test ragdoll_wrecks --test ragdoll_riding
  --test snapshot --test traffic --test truck_wheel --test two_p_truck --test wreck_bike`: 141 passed.
  For prop_plane I copied S241's states from s2's context, so its four tests ran rather than skipped.
- `cargo test --release -p game-sim --test replay`: 117 passed, 1 ignored (the one ignored before).
  `a_path_car_into_a_parked_bike` and `a_bike_meets_a_moving_truck` hold all 8 rides' rows unchanged.
  `the_plane_flies_frame_for_frame_like_the_original` and `the_biplane_acceptance_run` are green.
- `cargo check -p game-render -p openmx --tests --bins` is clean.

## Not verified / not 1:1

- **Only the plane's strike has a capture.** The wreck bike, the ragdoll, the wheels, the bike's
  scenery search and the props' searches are wired through the same mark. Only the wheels have a
  port-only test (truck_wheel part 3). No PCSX2 capture shows a downed bike or a ragdoll waking a
  truck (follow-up filed).
- **The wake is late within the tick.** The original's listener wakes the car inside the search,
  standing its path mesh down at once, so a later search in the same tick meets the f.col. The port
  wakes at `frame_late`, and every search that tick still meets the path mesh. The two differ only
  when two searches strike the same truck in one tick, and that is not captured. S251 owns the
  woken truck's f.col in the bike's frame.
- **Cars wake in car order**, not in the order the searches struck them. Two trucks struck in the
  same tick would wake in the original's search order. No capture has that.
- **Tiled worlds** (`tiles.rs` copies the props near the rider) have no traffic, so their copies'
  marks are dropped. No tiled track has a `HasPhysics` car.
- **The parked plane's creep** after a put-back is a quarter foot short of the original's over 18
  ticks: the plane's ground roll with a fresh port surface/pad state, not this lane (follow-up filed).
  The free run's wake is two ticks late because of it.
- Nothing taken from opentennis: it has no counterpart.
