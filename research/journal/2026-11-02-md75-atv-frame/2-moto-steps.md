# 2 — the bike's steps on the quad (MD75a)

`quad::Steps` is a `quad::Moto`. It answers the frame's calls by running the SLUS-21104 bike steps that are
already ported, on the quad's own fields:

| Call | runs | adapter |
|---|---|---|
| `InputLimiter(dt)` | `handling::step::input_limiter` | `input_limiter` |
| `Clutch` | `handling::step::clutch` | inline (speed = the contact velocity's z) |
| `Preload` | `handling::step::preload` (posts the cue via `PreloadCue`) | inline |
| `Backflip` | `handling::backflip` | `air_in` / `air_out` / `backflip` |
| `AirborneControl` | `handling::airborne_control` | `air_in` / `air_out` |
| `Suspension(i)` | `atv_wheel::run` | `wheel_in` / `wheel_body` / `wheel_out` |
| `TireForces(i)` | `atv_wheel::run` | same |
| `WheelIntegrate(i)` | `handling::step::wheel_integrate` | steer tail into/out of the wheel |

`quad::Bike` holds what the bike steps need and the quad does not lay out (keys, held state, the wheels'
`atv_wheel::More`, cofg, etc.). The calls it does not port, and the calls the ported steps make outward
(stunt start/end, wreck, surface drag, spin integrate, the ground query), go to `rest`. `rest` is any
`Moto`, and the tests use the call log.

## How it was checked

There are ten new oracles in `tests/atv_quad.rs`, in the "MD75a" section. Each runs `GROUND_HEAD` or
`GROUND_STEP` in the `Ee` off `atv-r-spawn.ee.bin` with every Moto callee hooked except the ones under
test, which run as the original's own code. The port runs `quad::ground_head`/`ground_step` with `Only`,
which sends the picked calls to `Steps` and logs the rest. The test compares all of these, word for word:
the quad, the body, the engine word, the four wheels and their steering tails, plus the call log.

| test | states | unhooked |
|---|---|---|
| `the_quads_input_limiter_is_the_bikes` | 10,000 | input limiter |
| `the_quads_clutch_is_the_bikes` | 10,000 | clutch |
| `the_quads_pre_process_runs_the_bikes_steps` | 5,000 | limiter + clutch together |
| `the_quads_preload_is_the_bikes` | 10,000 | preload |
| `the_quads_backflip_is_the_bikes` | 10,000 | backflip |
| `the_quads_airborne_control_is_the_bikes` | 10,000 | airborne control |
| `the_quads_suspension_is_the_wheels` | 5,000 | suspension (4 wheels) |
| `the_quads_tire_forces_are_the_wheels` | 5,000 | tire forces (4 wheels) |
| `the_quads_wheel_integrate_is_the_wheels` | 5,000 | wheel integrate (4 wheels) |
| `the_quads_force_body_runs_the_bikes_steps` | 5,000 | all six force-body calls together |

Each test also asserts minimum counts for the step's paths: limiter locks and gas cut, clutch
engage/slip, preload arm/fire, flip start, clip and end, arming and triggering at the lip, airborne
on/off, plus the inner calls (cue posted, stunt started, stunt ended, wrecked). The backflip and force
tests use `turning_flip` so that flips in flight and lips with the stick hard over make up half the states.

`./kit check -- -p game-sim --test atv_quad`: 26 passed. These are the 16 from part 1, still green after
`Call::InputLimiter` gained its dt and `Body` gained position and correction, plus these 10.

Mutations of the adapters in `quad.rs`: 18 tried, and 16 go red. Two survived:
- *Rebuilding `bike.stick` from `held` after the limiter.* The limiter stores the stick as given, so the
  rebuild was a no-op, and I removed it.
- *Not copying the quad's contact velocity into `More::chassis_velocity_local`.* Every tire-forces pass
  calls `contact_velocity`, which rewrites the field before anything reads it. The copy stays, because
  `wheel_out` writes that field back to the quad, and without the copy a path that skips the call would
  write a stale value.

## Decisions

- The quad is authoritative for every word it shares with `Bike` (the shared layout). The adapters copy
  quad → step type → quad around each step.
- SLUS-21104's limiter walks the front brake and copies it into the rear (`+0x12d0 = +0x12cc`). The
  ported `step::input_limiter` (MD63a) walks the rear from its own word. The adapter starts the rear at the
  front, which makes the two agree. Filed as **B17** to fix in the limiter itself (`tests/mxvatv_step.rs`
  pokes rear == front, which hides the gap).
- `rider_stunt` (+0x1524) has to equal the first balance word. The generator and `poke_ride` keep it so.
- The ground closure takes no wheel index. The test serves the answers in call order.

## Not verified / not 1:1

- Only `Ground::Still`: the contacts' owner/vehicle are 0. Moving and node grounds are left to
  `tests/atv_wheel.rs`, which proves `atv_wheel::run` on its own types.
- The stunt start is always answered "yes" (returns 1). A refused start is not exercised on the quad.
- `rest` cannot change `Bike`. Nothing that is hooked out writes the bike's keys or held state today.
- `Controls.front_brake`/`rear_brake` are passed as zero to the limiter. The quad's brake words go through
  `held`, as on the bike.
- `held.brake` and the flick ring are port-only state. The original has no word for them, so they are
  not compared.
- `quad::frame` does not use `Steps` yet. The frame oracle from part 1 still hooks every call. The
  whole-frame proof is still MD75's later part.
- Clippy was not run (see memory: it cannot lint game-sim). `quad.rs` keeps `deny(clippy::float_arithmetic)`.
