# 1 — mxvatv car update replay (FINAL)

Task MD156. 2026-11-16.

## What I did

- New `tests/mxvatv_car_frame/update_beside.rs`. It hooks the drive's car's update slot (vt `+0x10c`).
  Each time the slot runs on the player's car:
  1. It reads the update's words, the slot's and base update's words, the car and its body off the
     machine, and runs `atv_car::class_update` on them. A call list (`Vec<Call>`) collects what it
     hands out.
  2. For the buggy class, when the port calls `ClassStep`, it runs `atv_car::class_step` over the RAM
     the slot started on (MD151's `Over`).
  3. It copies RAM and the scratchpad, then runs the original's slot once with MD106e's `hook_class`
     hooks in place: every hand-out is logged and none runs. The buggy class's own step runs for
     real (logged too); the golf cart's and truck's are logged only.
  4. It compares: every car, hazard, slot and base-update word; the calls in order with their
     arguments; the probe's modes; the buggy class's part-node words (`compare_nodes`); and that the
     original wrote nothing else in the car (0x1500), its body or its tires.
  5. It puts RAM back and runs the original's slot for real, so the drive goes on as the original's.
- `drives.rs`: `beside()` installs the hook on the `Through::Frame` runs (the five `…_is_the_ports`
  tests). It checks the update ran once a frame and prints a summary line per drive.
- The helpers it reuses read the drive's car (`car_at()`) instead of the buggy image's constant:
  `class_update.rs`'s `hook_class`, `read_class` and `class_words`, and `class_step.rs`'s
  `read_parts` and `read_posing`. `car_at()` defaults to the buggy image's car, so MD106e's and
  MD151's tests are unchanged.
- `hook_class` now skips the rumbler's hook when the car has no vehicle state or no rumbler. Its
  "the image has a rumbler" check moved into MD106e's test body.
- No change to `atv_car.rs`. The class step the task asked to port was ported by MD151, for the
  buggy class only.
- Split off MD156a (the golf cart's class step) and MD156b (the truck's).
- Nothing taken from opentennis; it has no car.

## What the original does

Nothing new decompiled. What I checked per drive image (read off the vtables):

- Buggy, baja and sandrail share the buggy class's vtable: update slot = MD106e's buggy slot, class
  step = MD151's.
- The golf cart's and the truck's update slots are MD106e's other two copies. Both call the same
  update (`./kit dis -p atv 0x004d0250 0x004d05c0`).
- Their class steps are different functions: `./kit fn 0x004cf9b0 -p atv` (golf cart, 2196 bytes) and
  `./kit fn 0x004d6f20 -p atv` (truck, 1456 bytes). They pose other parts (car words from +0x14a0 up,
  nodes from +0x12d4 up) and use node calls that aren't ported (a matrix build, axis-angle
  quaternions, and others).
- Their prepare (vt `+0x3e4`) and snapshot (vt `+0x37c`) slots are their own too. These are hand-outs,
  hooked through each car's own vtable.

## How it was proved

Bit-exact replay of MD107's five drives, each update beside the interpreter (`./kit check -- -p
game-sim --test mxvatv_car_frame`, 31 tests pass):

| drive    | updates | words compared | class step parts compared | off the ground | rumbles |
|----------|---------|----------------|---------------------------|----------------|---------|
| buggy    | 730     | 223,380        | 730                       | 168            | 10      |
| baja     | 730     | 223,380        | 730                       | 175            | 9       |
| sandrail | 730     | 223,380        | 730                       | 158            | 4       |
| golf     | 730     | 223,380        | 0 (its own step, logged)  | 56             | 2       |
| truck    | 730     | 223,380        | 0 (its own step, logged)  | 201            | 10      |

Every update handed out the same calls as the original, in order: CollisionOn, ClassStep, Prepare,
Ride, Sync, Snapshot, Network, Parts and Cue on every frame, plus RideRumble on the frames counted
above. The ghost step, the hazard clock and the base update run as the port's (`OwnUpdate`), not as
calls.

Mutants (each restored with cp + touch):

- the slot's least rumble level 125 → 124: red (buggy frame 128, truck frame 408; the rumble's level
  in the calls).
- the class step's swing factor 1.25 → 1.2500001: red (buggy frame 15, 27 part words).

## Not verified / not 1:1

- **The golf cart's and truck's class steps aren't ported or compared.** They are their own
  functions (see above). On those two drives the step is logged in the copy's run, and its writes
  aren't checked. Follow-ups MD156a and MD156b.
- **The ride is checked one update at a time, not run whole by the port.** Each update starts from
  the machine's words, as MD135's frame does. The port's output is never fed into the next frame.
- **The hand-outs are checked as calls, not as what they do.** Prepare, Ride, Sync, Snapshot,
  Network, Parts, CollisionOn and Cue run as the original's in the real run. The copy's run checks
  only that they are asked for, with the same arguments, in the same order.
- **Branches these drives never reach:** all frames ride in vehicle-state mode 1, so the collision
  is always on. Never seen: Unlist, CollisionSync, Place (every car has a player), the ghost (every
  image's ghost clock is below 0), the hazard clock's put-back, a hazard surface (on a hazard on 0
  frames), the still camera and a missing vehicle state. MD106e's 10,000 generated cases still
  cover all of these.
- **The rumbler isn't rung in the copy's run.** The rumble switch's hook answers "not ready", so
  only the level handed out is compared. The real run rings it as the original does.
- **The update's tire-surface byte is used unmasked.** `class_update` indexes the 16-entry table with
  the raw byte. Every byte on these drives was below 16, so nothing panicked. A byte of 16 or more
  isn't covered (the shell's `Ride` masks it with `& 15`).

## Next

Nothing for this task. MD156a and MD156b port the other two class steps.
