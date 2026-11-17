# 1 — mxvatv car driver (FINAL)

Task MD175. 2026-11-16.

## What I did

- `crates/openmx/src/mxvatv_car_draw.rs`: `seat_nodes` finds the car model's four seat nodes (feet, hands)
  under either spelling: `Dummy_Foot_L_pos` (buggy, baja, monster truck, sandrail) or `Dummy_Foot_L`
  (golf cart). `seat` is their middle plus `car::SEAT_FROM_LIMBS`, as MXU's `car::seat`.
- `main.rs`: `mxvatv_car_drivers`, a copy of `car_drivers` (S245) for the MX vs. ATV car: the rider's body
  posed by the plane pilot's clips (`biplane::pilot_palette`), root at the seat, limbs held to the four
  nodes. Drawn on the GPU (an actor) and the software path (skinned), and traced as `pilot:` lines.
- Found and fixed an MD157 bug: on the tick the car is taken, its scene node is still all zeros (the
  step has not run). The "before" matrix was that zero matrix, so the car (and now its driver) was drawn
  sliding in from the world's origin for the first two frames (w 0.47, then 0.97). `placed` now gives
  `None` for a node whose w is 0, so the first tick draws it where it is.
- Took from opentennis: nothing. It has no vehicles.

## What the original does

Not decompiled. SLUS-21104's string table holds both seat spellings (`Dummy_Foot_L_pos` and
`Dummy_Foot_L`, each with the hands), which agrees with the golf cart's model using the short one. The
cars are MX Unleashed's car family (research/mxvatv-pc.md, MXVehicleCar), so the driver is seated the way
S245 seats MXU's car driver.

## How it was proved

- `crates/openmx/tests/mxvatv_cars_drawn.rs` (the MD157 test, five cars, GPU + software for the buggy)
  now also checks, every drawn frame after the switch:
  - one `pilot:` line per car frame, none before the switch, none when the car is picked but never taken;
  - the driver's root is at the seat worked out in the test from the pack's own `.psg` (within 0.01 ft:
    print rounding and the slid rotation);
  - the body's middle is within 3 ft of the seat (in the cab).
- Green with the pack in mods/: `TESTS PASSED: 11 tests`.
- Looked at the shots (target/tmp/md157-mxvatv-cars-drawn/): the golf cart's driver is plainly in the
  seat under the roof; the buggy's suit shows in the cage; a white helmet shows in the truck's cab.

## Not verified / not 1:1

- The seated pose is the plane pilot's clips with the default lean, as S245's MXU car driver. Whether MX
  vs. ATV seats its car driver with those clips, or with a motion of its own out of the rider MLBs, was
  not read in the ELF: no decompile of the car's rider code this task.
- The steering does not feed the lean, and the hands do not turn with a steering wheel (as S245).
- The seat's offset from the four nodes is the plane's measured one (`SEAT_FROM_LIMBS`), carried over;
  never measured on an MX vs. ATV car.
- Which spelling the original tries first is not checked; the port takes `_pos` first, then the short
  one. No model has both.
- Nothing was compared in PCSX2 (no capture of a car's driver).
- The gate runs without the pack in mods/ (K55 still open: `mxvatv_cars` is red on the sandrail with the
  pack in, not from this task), so in the gate `mxvatv_cars_drawn` skips. It is green only from my own run
  above. The pack was copied from s3's mods/ for that run.

## Next

none
