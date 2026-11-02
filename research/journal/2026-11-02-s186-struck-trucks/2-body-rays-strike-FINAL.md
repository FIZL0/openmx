# 2 — the body rays strike a truck, and the trucks update after the bikes (FINAL)

Task S186a. 2026-11-02.

## What I did

- **The struck listener, from the body rays.** In `mover_contact.rs`, the bike's body-ray search over
  the cars gives each car's collidable a listener. `struck_heard` returns the contacts and the cars
  whose listener heard a hit, in the search's order. `struck` is unchanged and wraps it.
  `Sim::car_contacts` (now `&mut self`) wakes each car it heard, after the search, through
  `Traffic::strike`. `Traffic::car_at` maps a prop index to its car. This follows the original's
  `collide_against_search` (0x0011a9d0): the narrow phase makes the hit, then the struck one's
  listeners (vt+0xc) are told. The car mesh's interface vt+0xc is a jump to `physics_car_wake`, so
  the contact stays and the car wakes after it.
- **The HasPhysics cars update after the bikes.** `Traffic::frame_held` steps only the path cars now.
  A new `Traffic::frame_late` runs each `HasPhysics` car's update: path step, or clock-only standing at
  the body, plus the zero-motion first frame back. It then marks their meshes' motion. `mark` takes
  `late` to choose its set. It is called after every bike's frame in all four `Sim` step paths, after
  the plane's frame, and at the end of `Traffic::frame`. The physics world node (`physics_frame`) stays
  before the bikes. Traffic with no physics objects is unchanged: `frame_late` returns at once and
  every car steps early, as before.
- **The replays carry the trucks' physics.** `SaveState::trucks_physics` in `tests/replay.rs` loads
  each `HasPhysics` car's `PhysicsCar` from the track's archives, at the terrain's edge.
  `in_the_track_with_cars` gives the traffic those objects. S207's ctruck row now reads agrees 36,
  port wreck `None` (was 35, `(172, 1)`). S219's rear row reads agrees 42, port wreck `(42, 4)` (was
  41, `(41, 4)`).
- Follow-ups: **S251** (the woken truck's f.col in the bike's frame, two-way, and the path mesh stood
  down) and **S252** (every other search's strike). The original task's text named both, plus a
  capture. The captures already existed (S207 ctruck, S219 rear), so I used them.

## How it was checked

Against the original's PCSX2 captures of a bike struck by a free06 `HasPhysics` truck, from their save
states. I copied the captures and RAM images from slots s3 (S207) and s2 (S219) into this slot's
`context/`.

| ride | before | strike only | strike + late update | original |
|---|---|---|---|---|
| S207 ctruck: parts on | 35 | 35 | **36** | — |
| S207 ctruck: wreck | (172, 1) | None | **None** | None |
| S219 rear: parts on | 41 | 41 | **42** | — |
| S219 rear: wreck | (41, 4) | (41, 4) | **(42, 4)** | (42, 4) |

- **The strike alone** stops the ctruck wreck: the woken truck stops instead of driving on into the
  bike again. Both rides still met the truck a tick early. **The late update** moves both strikes
  onto the original's tick. Each column is a mutation check of the other change.
- On rear, every lane agrees through tick 41, and both wreck with reason 4 on tick 42.
- `cargo test -p game-sim --test replay -- a_path_car_into_a_parked_bike a_bike_meets_a_moving_truck`:
  2 passed, with all 8 rides run (truck, rails, beside, ctruck, flank, slow, rear, trailer). None was
  skipped, and the other six hold their old rows with the trucks' physics loaded.
- Also green: game-sim `physics_car_struck` (6), `traffic` (18), `physics_cars` (4),
  `moving_vehicles` (3), `snapshot` (11), `hash` (16), `bike_col` (5), `prop_knock` (8).
  `cargo check -p game-sim -p game-render -p openmx --tests --bins` is clean.
- `./kit audit` reports only the two existing `plan/DONE.md` lines.

## Not verified / not 1:1

- **The update order is inferred, not traced.** Two captures say the truck a bike's rays meet is
  where it stood the tick before. The walk that runs the toy world's list after the bike world was not
  found in the ELF. Where the late update falls against the props' step and the other players is the
  port's choice: last in the tick.
- **The strike tick's push parts.** ctruck tick 36 vel.x: port 9.53, original 2.97. Rear tick 42
  vel.z: port 15.48, original 24.73. The port keeps the path mesh at the woken car's pose for the rest
  of the bike's frame. The original stands it down and files the `f.col` as a body (S251).
- **Only the riding bike's body rays strike.** The scenery search's own pass over the cars, the sweep,
  the wheels, the wreck bike, the plane and the props do not wake a truck yet (S252).
- **The wake's motion reads the mesh's `Moving`** as marked at the car's last update. The
  original's virtuals read the node after the wake's placement (squared rows). This was not compared
  on the strike tick.
- In the plane's world the trucks now update after the plane. That is the same inference; there is no
  plane capture with a truck.
- `B8` asked for this same ctruck row: agrees past 35 and the port's wreck `None`. It now does. I did
  not tick B8 because it is not this task. Its rear note (S219) is answered too.
- Nothing taken from opentennis: it has no counterpart.
