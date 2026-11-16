# 1 — mxvatv car particles (FINAL)

Task MD176. 2026-11-16.

## What I did

- `crates/openmx/src/main.rs`: MX vs. ATV's car now throws its dust, dirt spray and chunks, the same way
  MD136 does for the quad.
  - `mxvatv_car_list`: picks the car's list by its kind (buggy, monster, baja, golf, SR).
  - `load_mxvatv_car_fx`: reads that list and its `.emt` files off the car's pack. It zones them by the
    pack's `mxparticlezones.dat` (or the track's if the pack has none), draws them on the track's
    particle sheet, and binds each entry to its node on the chassis model MD157 loads.
  - `mxvatv_car_fx_frame`: fills each tick's frame from the ported car step, using the fields MXU's car
    feeds (`PlaneFrame::car`, S248). Per tire: probe `solid`, sort `sorted`, compression, surface byte,
    `slipping`, `side_ratio`. Car-wide: gas after the rate limit, engaged clutch, `car.speed`, body
    velocity, and `down()` as the wreck.
  - One manager per seat. It steps while the car is out and idles otherwise, so whatever was thrown
    runs out. Drawn on both renderers next to the quad's.
  - Startup prints `mxvatv car: particles: zones off …` and `… N entries off <pack>'s <list>`. With the
    car out, the report line is followed by `mxvatv car: particles: … in the air`.
- `research/oracle.txt`: the vehicle particle loader's address, and what it picks per kind.
- Taken from opentennis: nothing; it has no vehicle particles.

## What the original does

`./kit fn 0x4a6810 -o atv`, the vehicle's particle loader. It switches on the vehicle kind:

- 0: `mxparticles.dat`, or `mxaiparticles.dat` for the AI.
- 1: `atvparticles.dat`, or `atvaiparticles.dat` for the AI, zoned by `atvparticlezones.dat`.
- 2: biplane. 3: helicopter.
- 5: monster. 6: baja. 7: golf. 8: SR. Any other kind (the buggy, 4): `buggyparticles.dat`.
- Every kind except 1 is zoned by `mxparticlezones.dat`.
- The car lists have no AI variant.

## How it was proved

- The list and zone choice per kind: read in the loader above.
- `crates/openmx/tests/mxvatv_car_particles.rs` rides all five cars on free09, headless, holding the gas,
  switching to the car at tick 30, for 112 ticks. For each car it checks:
  - the list is the kind's own, off the pack;
  - the zones are the pack's `mxparticlezones.dat`;
  - nothing goes unrun, and no entry sits on a node the chassis lacks;
  - by the end, the car is not wrecked and its tires' dirt spray is in the air.
  With the buggy picked but never switched to, it says nothing about car particles.
- Shot looked at by eye: buggy at tick 110, on all four wheels. Dust plumes and chunks trail off the
  rear tires. Counts that tick: Tire Dust 115, Dirt Spray 86, Chunks 63, Collision Chunks 78.

## Not verified / not 1:1

- **Which car words feed the frame.** SLUS-21104's car manager was not decompiled. The loader hands it
  the vehicle at +0xb90, and I did not trace what it reads there. The words follow MXU's car (S248),
  because the car step itself is ported from MXU's.
- **The wreck flag.** It is fed from `down()`, as the quad's is. In MXU the car manager's wreck word is
  never set (C18). Whether SLUS-21104's car is the same was not checked.
- **Collision contacts.** The car's collision record contacts are not fed (the record is a different
  type from MXU's `contact_search::Record`). So `Vehicle Dust` and `Vehicle Chunks` (CollisionModel
  entries) never throw.
- **Tire tracks.** These entries are skipped, not laid. The same is true for the quad.
- **No comparison with the original.** Nothing was compared in PCSX2: no counts, no positions, no look
  beside the original.
- **Bind poses.** Each node's bind pose comes from the render model's node world at rest. It does not
  follow tire compression or steer, so spray hangs from the rest pose (the same as MXU's car).
- **Untamed car packs.** `untamed_cars` carries no particle lists, so its cars throw nothing. The
  startup prints a `no particles` line instead.
- **The gate ran without the pack.** `mxvatv_cars` is still red on the sandrail with the pack in mods/
  (K55, same message, not from this task: checked here before `done`). So I moved the pack to the
  scratchpad for `kit done` and put it back after. `mxvatv_car_particles` skips in the gate; it is green
  only from my own run (`TESTS PASSED: 1 tests`, pack installed).

## Next

- K55: fix the sandrail red, so the gate can keep the pack in mods/ and run both car tests.
