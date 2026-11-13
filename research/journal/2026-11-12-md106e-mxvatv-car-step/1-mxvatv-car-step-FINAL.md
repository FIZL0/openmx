# 1 — mxvatv car step (FINAL)

Task MD106e. 2026-11-12.

## What I did

- `crates/game-sim/src/atv_car.rs`:
  - New `class_update`: the car classes' own update slot (vt +0x10c) around MD106d's `update`.
  - New `OwnUpdate`, a `Steps` that answers three of the update's calls:
    - `Call::Ghost` goes to the new `ghost_step`.
    - `Call::HazardClock` goes to `handling::tail::stuck`.
    - `Call::VehicleUpdate` goes to `handling::tail::base`.
    - Everything else, and whatever those hand out, goes on to the inner `Steps`.
  - New `Updated` struct for the words of the slot, the ghost and the vehicle base. The vehicle base is
    the bike's `tail::Vehicle`, because it is the same base class.
  - New `Call` variants: `GhostSearch`, `GhostAlpha`, `GhostOver`, `Prepare`, `Ride`, `Sync`,
    `Snapshot`, `Network`, `Parts`, `PutBack`, `CollisionOn`, `Unlist`, `Place` and `RideRumble`.
- `crates/game-sim/tests/mxvatv_car_frame/class_update.rs`: new oracle,
  `the_car_classes_update_slots_are_the_originals`. In `update.rs`, `up` and `generate_update` became
  `pub(crate)` so the new oracle can build on MD106d's cases.
- research/oracle.txt: new `# MD106e` block.
- **Split off the class step** (vt +0x488, 6 KB) as MD148. Most of it poses the buggy's model (node spins
  and positions, VU0 normalising of the suspension parts), so it is presentation, and it is as big as
  everything else here put together. It stays a hooked `Call::ClassStep`.
- Nothing was taken from opentennis: it has no vehicles. The shape follows `quad`/`tail` (MD64a, the bike's
  base update).

## What the original does

Read with `./kit dis -p atv 0x4c6ce0 0x4c7050` and `./kit dis -p atv 0x4a74e0 0x4a7600`. The full word map
is in research/oracle.txt `# MD106e`.

- **Three slots, one code.** The buggy's, golf cart's and truck's vt +0x10c are the same instructions.
  Each reads its own rodata copy of 0.3, 5.0 and 55.0, and the values are equal.
- **No vehicle state:** only the update runs. Otherwise the slot does these steps in order:
  1. **Odometer.** On the ground, the speed is the word at +0xc38. That is the z lane of the car's
     ground velocity (+0xc30). It is not a separate signed-speed word as on the bike, which my first test
     run showed. Off the ground, the speed is the body's velocity dotted (x and z only) with the node's
     ahead row. The ahead row's y must lie strictly between −1 and 1; otherwise the side row is used. The y
     is zeroed and the row VU-normalised. A negative speed counts as 0. Then distance += speed × dt, as an
     adda/madd.
  2. **Collision.** It goes on when the car is carried (an object whose node's flags have bit 1) or
     riding (mode 1); the off bit +0x44 & 8 is then cleared. Otherwise:
     - with the body in the world, +0x11bc is cleared, and the car is taken off its list if it is on one;
     - CollisionSync runs, and the off bit is set.
  3. **Place.** It runs when there is no player.
  4. **Rumble.** It runs when riding and not wrecked:
     - Shock = |body correction peak|. At 5 or below the level is 0. Above, the level is
       cvt.w(255 × shock / 55), and at least 125.
     - If the level is above 0, there is a rumbler, and the switch is on, the rumbler runs for 0.3 s.
  5. Then the update, then cue 4.
- **Ghost step.** It runs while the ghost clock is above 0:
  - If the car has a probe: the probe's mode switches to 0, the probe searches, and the mode switches back
    to 1. The clock counts down by dt unless the probe is inside something.
  - When the clock reaches 0, the vt "ghost over" call runs.
  - Otherwise the node's alpha is set. It is 0.1 if the length is 0. Otherwise it is lerp(0.1, 1,
    (t − len)/(0 − len)), clamped with max.s/min.s.

## How it was proved

Function oracle: `tests/mxvatv_car_frame/class_update.rs`, run with
`./kit check -- -p game-sim --test mxvatv_car_frame` (20 tests, all pass).

- **What runs.** Each of the three slot addresses, chosen per case, on md39a/buggy.ee.bin. The ghost step,
  hazard clock and base update run unhooked. Hooked to log are:
  - what those three hand out: the probe calls, the alpha, ghost over, prepare, ride, sync, snapshot,
    network, parts and put-back;
  - the slot's own calls: collision on/sync, unlist, place, the rumble switch, the rumbler and the cue;
  - the class step.
- **Cases.** 10,000 generated cases on top of MD106d's update cases. Each one also pokes:
  - the ghost clock and length (±0, equal, negative);
  - probe present or absent, inside or not;
  - the node's two axes, with y exactly ±1, beyond ±1, or 0, and x and z sometimes 0;
  - the ground velocity's z (±0, flipped);
  - the distance;
  - the carrier: none, without a node, node without bit 1, node with bit 1;
  - the collision word, list and flags byte;
  - the player;
  - the car's wreck state;
  - the rumbler and the switch's answer;
  - the base update's clocks, free/stuck, where it was last free, and the NO_STUCK run flag;
  - the body: in or out of the world, velocity, position, correction peak.
- **Checks.**
  - Every compared word of the car, its tires, +0xc20, and the slot's and base's words is equal bit for
    bit.
  - The call logs are equal, with arguments.
  - The probe's mode sequence is [0, search, 1].
  - The rumbler is rung exactly when the switch says so, with time 0.3, softness 0 and the level.
  - Every word the original wrote in the car, body or tire blocks is a compared word.
  - A flipped bit goes red in exactly its word.
- **Branches.** 30 labelled branches, each seen at least 30 times. They cover the three slots, every
  odometer path, every collision path, place, the rumble rung, switched off, under its shock, with no
  rumbler and wrecked, the ghost counted, held, without a probe, over, at its floor and fading, the base
  update with and without a state, and the hazard clock's put-back and stuck count.
- **Mutants.** 10 of 10 go red, each restored with cp + touch:
  1. side-axis test `<=` changed to `<`
  2. the speed's clamp at 0 dropped
  3. carried dropped from collision-on
  4. the body-in-world test dropped
  5. the rumble's floor of 125 dropped
  6. the inside hold dropped
  7. the alpha's upper clamp dropped
  8. the body-position sync before the base update dropped
  9. the flattening (y = 0) dropped
  10. the place test inverted

  Mutant 8 first passed, because the test pre-set that field itself. That line was removed, and then it
  went red.

## Not verified / not 1:1

- **The class step is still a call.** Vt +0x488 (6 KB, model posing) is hooked and logged as
  `Call::ClassStep`; MD148 ports it.
- **Only the buggy image was run.** The golf cart's and truck's slots were run on the buggy's car. Their
  code and constants are equal to the buggy's, but no golf cart or truck image was used.
- **These stay handed out, not ported:**
  - the slot's place, collision on and CollisionSync bodies, unlist, the rumble switch and the rumbler;
  - the ghost's probe search, the node alpha and ghost over;
  - the base update's prepare/ride/sync/snapshot virtuals, network step and parts.

  The test proves they are called, in order and with their arguments, but not what they do inside.
- **The base update only ran as the car hits it.** `handling::tail::base` runs only as the buggy's car
  reaches it. Its vehicle-state branches (state ≠ 0) were proved on the bike, not here. In the oracle,
  "free kept" is seen, but which of the bike's other paths the car reaches was not tallied.
- **Unused or duplicated fields.** `Updated.vehicle.speed` and the bike-only `tail::Vehicle` fields are
  unused here. The ground speed is `Car::ground_velocity.z`, on the ground is `Car::grounded`, and the wreck
  reason is `Car::state`. `vehicle.state` must be 0 exactly when `Update::vehicle_state` is None. That
  contract is documented; it is not checked in code.
- **Inherited from MD106d:** surface bytes stop at 15, and only the image's one vehicle state object was
  used.
- **The PC exe was not read.** The PS2 ELF is the oracle.
- **Clippy was not run.** It cannot lint game-sim. The new code uses only ps2-float ops, under
  `deny(clippy::float_arithmetic)`.

## Next

MD148 (the class step), then MD106a/b/c leftovers and the MD107 replay.
