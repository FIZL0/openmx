# 1 — truck lean cofg travel

Task R38. 2026-11-17.

## Finding

- The 3 ft is the centre of mass's travel (`atv_car.rs` `travel_cofg`, vt `+0x470`), not a bug.
- MD182's test set `State::lean` to the stick's value (both 1). The travel step slides the body's
  centre of mass back along the car's `z` by `CofGTravelBack` × lean, and `set_cofg` moves the body's
  position and last position by that change turned into the world, so the car's frame origin stays put.
- The body's position is its centre of mass, so a 3 ft slide of the centre is a 3 ft move of the
  position word, with nothing in the car moving.
- Only the truck has a travel: `monstertruck.phy` has `CofGTravelForward = 1.5`, `CofGTravelBack = 3.0`;
  the sandrail sets `CofGTravelBack = 0.0`, the others have no travel keys.
- The original does it: the travel step is oracle-proved against SLUS-21104's code by
  tests/mxvatv_car_frame/pieces.rs `travel_cofg_is_mx_unleasheds` (and the all-pieces runs), and MX
  Unleashed's own `car::travel_cofg` reads the same way.

## What I did

- No product change.
- New test `each_cars_lean_moves_its_body_by_the_cofg_travel_alone` (crates/game-sim/tests/mxvatv_car_mxu.rs):
  each car rigged on free09, lifted 40 ft at 30 ft/s, air control held off (touching), two frames with
  stick/lean at 0 and 1:
  - the steer moves no body word;
  - the lean moves no body word but position and last position;
  - the centre of mass's `z` moves by exactly `-CofGTravelBack`;
  - position and last position move by that, turned along the car's forward row (within 1e-3);
  - exactly one car (the truck) moves.
- Nothing taken from ../opentennis: it has no car.

## Not verified / not 1:1

- No capture of the original truck leaning in the air: the proof rests on the travel step's oracle test
  (the original's code over generated inputs), not on a live run of this exact setup.
- The test checks the move against the forward row to 1e-3 ft, not bit for bit (the port turns the
  change with the quaternion; the test has only the matrix row to hand).
