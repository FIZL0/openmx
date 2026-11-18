# 1 — truck pose against the original

Task S343. 2026-11-17.

## What I did

- Read the parked truck's node table, stored places and tire state out of first_recording's RAM image
  (`context/states/first_recording.p2m2_SaveState.eeMemory.bin`, car at 0x00e1e210, as S149a found it).
- Decoded the truck's pose step (`./kit fn 0x003e07b0`; the decompiler drops which vectors the `vsub`s take,
  so the drive line and swingarm part was read with `./kit dis 3e1180 3e1708`).
- Wrote it as `truck()` in `crates/game-render/src/car.rs`: hubs, tires and lower struts set afresh, the
  axles rolled by the pair's compression difference, the drive lines and swingarms given the original's
  look-at frame. `has_truck_rig()` gates it; a model without every node keeps the old aims (`truck_aims()`,
  unchanged), so a truck-like mod model is not broken.
- Before (old aims) against the image: FrontDriveLine 0.034, RearDriveLine 0.056 in a turn row, front lower
  swingarms 0.046, rear upper 0.015, rear lower 0.0077, front upper 0.0062.
- After: every node within 0.00049 in a turn row (the two axles, see below; the drive lines 0.00038, the
  swingarms 0.00021–0.00034) and 0.0018 ft in place, the size of the original's float noise on nodes
  nothing moves (struts, particle marks: 0.0005–0.0011 ft).
- Test: `crates/game-render/tests/car.rs` `against_the_original::the_trucks_pose_is_the_originals`.
- Nothing from `../opentennis`: it has no cars.

## What the original does

- Each hub: place = its bind place with y + its tire's compression; 3x3 replaced by a turn about y by the
  steer (the rears' steer × `RearTireSteerScale`, car +0xfd4). Each tire: 3x3 replaced by a turn about x by
  its spin, then its rows' y scaled by its stretch (the same clamp as `squash`). Each lower strut: bind place,
  y + its tire's compression.
- Each axle: place = bind + the pair's mean compression; 3x3 replaced by a turn about z by
  ±acos_table(w / √(w² + d²)), w = the front hubs' spread in x (car +0xfd0, 8.0627 = FR hub x − FL hub x),
  d = right compression − left, negated when d < 0. The table is S264's lerped 30000-entry acos.
- Each drive line (a child of its axle): dir = n(its own place in root space − its axle node's place),
  then `0x001b1e20(node, root, dir, y, 1,1,1)` → `0x0013e330`: rows [up' × dir, up', dir],
  up' = n(dir × (y × dir)), taken into the parent, place kept.
- Then each swingarm, upper four then lower four (FL, FR, RL, RR): dir = n(its swingarm node's place in
  root space − its own stored place), the same frame.
- Node table +0xe04..+0xe80: tires FL FR RL RR, hubs, lower struts, upper swingarms, lower swingarms,
  upper swingarm nodes, lower swingarm nodes, FrontAxle, RearAxle, FrontDriveLine, RearDriveLine. Stored
  places +0xe90..+0xfc0 (written in research/oracle.txt).
- The seat's four marks and the two axle nodes are never written: their worlds in the image are 1.37 ft
  and the root's tilt off their local on the root. No vertex sits on them; the test checks both.

## How it was proved

A save-state capture: one RAM image (first_recording's parked truck, springs settled 0.83 ft front, 0.90 ft
rear), every one of the 48 nodes under the root compared, 0.002 / 0.003 ft limits.

## Not verified / not 1:1

- **One still frame, parked: steer 0, spin almost 0, both sides of each pair compressed alike.** The
  steer's turn, the spin's and the axle roll's sign are read off the code, not seen in a moving frame. No
  driven truck image exists (S149a's car states are gone).
- **acos, not the original's table.** `std` acos stands for the lerped table. Near a level axle the table
  floors at its last entry, 0.00049 rad, so the original's axles always roll at least that; ours roll ~0.
  That is the largest difference left (0.00049 in a turn row). The table's generator was not found
  (no reference to its pointer), so the floor is not copied.
- **The stored drive-line and swingarm places are the model's bind places, not the original's.** The
  original's (+0xf30..+0xfc0) are the same places rounded (e.g. FL upper swingarm x −0.00537 against the
  model's −0.00571, y 6.38257 against 6.382577), so they come from elsewhere (not found). The difference
  turns a frame by under 0.0001 here.
- **The old relative rig stays for a truck-like model without every node** (`truck_aims`). Whether MX vs.
  ATV's monster truck (MD157) has the full rig, and so now takes the original's step, was not checked; its
  own game's pose code was not looked at.
- **At rest the truck is no longer exactly its model**: the drive lines and swingarms take the original's
  frames, which differ from the model's own turns by up to ~1.8° (places unchanged). Not seen on screen.

## Next

Nothing for S343.
