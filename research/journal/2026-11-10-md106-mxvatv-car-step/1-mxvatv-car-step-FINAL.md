# 1 — mxvatv car step (FINAL)

Task MD106. 2026-11-10.

## What I did

- MD106 is big (three listeners, about 15 callees, the update, and SLUS-21104's own pieces), so I split it
  the way MD75 was split. This is part 1. It covers the frame driver only: the world step over
  MXVehicleCar's three listeners, with every callee handed out as a call. The rest is filed:
  - MD106a: the calls answered by the MXU pieces.
  - MD106b: SLUS-21104's own calls.
  - MD106c: the brief's whole-frame proof, with only the ground query hooked.
  - MD106d: the update.

  MD107 was blocked after MD106. It now waits on MD106c and MD106d instead, so ticking this part does not
  start the replay early.
- New `crates/game-sim/src/atv_car.rs`:
  - `Car` and `Tire`: what the listeners read and write, not the layout.
  - `Call`, plus the `Steps` trait that runs the calls.
  - `pre_process`, `force` and `post_process`.
  - `frame`, over `biplane::World::step` with one body and no constraints.

  The listeners' own code is ported in full. That covers the shock-peak clear, the contact forget, the
  lone-front flag, the hand-off to the rears, the yaw-rate hold, the grounded and one-side flags, the
  landing test, the speeds, frame and angles, the tipped test and the latch.
- New `crates/game-sim/tests/mxvatv_car_frame.rs`, the oracle (see below).
- research/oracle.txt `# MD106`: the buggy image's world, the car's and tire's words, and each listener's
  calls in order. It also settles MD39's "listener at this-48 / this-60": the first is the PreProcess, the
  second the Force body.

## What the original does

Read with `./kit dis -p atv` (Ghidra stops the post at the ground query and drops the landing call's third
argument).

- **PreProcess** (`./kit dis -p atv 0x4cb0c0 0x4cb200`):
  1. The vehicle base clears body +0xc0 (the shock peak) to the rodata (0,0,0,1).
  2. Controls (vt +0x34c), then the cofg travel (vt +0x470).
  3. Per tire, the contact is forgotten: +0xa4, +0xa8, +0x19c, +0x1a0.
  4. The contact sort runs, then each tire's probe.
  5. Both fronts' +0xb4 = (a front down) && (no rear down), where down = +0xa4 || +0xa8.
- **Force** (`./kit dis -p atv 0x4cbb60 0x4cbd80`):
  1. Vehicle force, then anti-slide (vt +0x478).
  2. Only when the state +0xc80 is 0: preload, launch and the engine step. MX Unleashed has no such gate.
  3. The FL and FR tire steps.
  4. Each rear gets +0xb8 = 1 and +0xbc = `max.s`(FL load, FR load). +0xc0/+0xc4 get the left front's
     ground if it is down, else the right's. All of them are zeroed when neither front is down.
  5. The RL and RR tire steps, then aero (vt +0x474).
- **PostProcess** (`./kit dis -p atv 0x4cbd80 0x4cc4e0`):
  1. The vehicle post, then the yaw-rate hold. The hold tests `c.ole.s` and branches on false, so a NaN
     rate would be held too.
  2. Shift, then the car's own wreck timer (vt +0x484).
  3. The grounded flags, then one side (measured only while grounded).
  4. On touching down with a landing sound object: the shock (or the velocity, when the shock is still
     (0,0,0,1) bitwise) goes into the node's frame, and the sound is played with it.
  5. Speed: sqrt of mula/madda/madd. Then the local velocity, the ground velocity (the first grounded
     tire's, else the body's), the world rates, alpha and side slip, and the Euler angles.
  6. The frame: `atv_moto::quat_frame` at position − turn(q, cofg). Its forward and up rows are copied
     into +0xbe0/+0xbf0 with w kept.
  7. The ground query under the position, then record +0x24 = dot(up, normal) <= 0.70700002.
  8. The latch, then the node's position into +0xc40.

## How it was proved

Function oracle: `tests/mxvatv_car_frame.rs`, `the_cars_frame_is_the_originals`. Run it with
`./kit check -- -p game-sim --test mxvatv_car_frame` (it passes).

- **What runs on each side.** SLUS-21104's world step runs in the interpreter, off
  context/mxvatv-ps2/md39a/buggy.ee.bin, over the game's real world. That world is the car's body and its
  three listeners, plus the idle buggy and two other preforce listeners. The port runs `atv_car::frame`.
- **Hooks.** Every call the car's listeners make is hooked to log and play a shared script: probe flags,
  per-step grounded/load/ground/contact velocity, the ground normal and the node position. The port's
  `Steps` plays the same script.
- **Cases.** 3,000 generated states, poked into the car, its tires and its body:
  - state 0, 5, 13 or 14;
  - random flags, yaw limit and cofg;
  - velocity, rates (some past the limit), quat, position, force, moment and correction;
  - "still" cases, held level so the up row is exactly (0,1,0), against a normal on the tipped edge;
  - ±0 loads;
  - dt 1/30 or random (2 to 6 substeps).
- **Checks.** Every compared word of the car, the tires, the body and record +0x24 is equal bit for bit.
  The call logs are equal (Debug). Every word the original wrote inside those blocks is a compared word.
  The image is put back from a second, pristine copy after each case.
- **Branches.** 19 labelled branches, each seen at least 30 times. A flipped bit of the result goes red in
  exactly that word.
- **Mutants.** 9 of 10 go red. The survivor, `!(|y| <= limit)` → `|y| > limit`, differs only on a NaN
  yaw rate, and the integrate before the post does not produce one from the generated states.
- **A real fix.** The oracle's signed-zero loads found that the hand-off is a `max.s`, not MX Unleashed's
  compare-and-select. `max.s` gives +0 for (+0, −0) either way round. The port now uses `F::max`.

## Not verified / not 1:1

- **Every callee is hooked.** None is answered by ported code yet, so the listeners are proved only as the
  order and arguments of their calls plus their own inline code. The MXU pieces are MD106a. SLUS-21104's
  own calls (wreck timer, vehicle force/post, landing sound, node position) are MD106b.
- **The brief's proof is not done.** That is one whole frame off buggy.ee.bin with only the ground query
  hooked, with engine words compared. This part never compares the engine, because it is only touched by
  the hooked engine step and shift. That proof is MD106c.
- **The image is not read as a whole car.** The cases are generated states poked into the image's car.
  That read is part of MD106c.
- **Other listeners are not ported.** The idle second buggy's PreProcess and the two vt 0x3d04dc listeners
  in the world's preforce list run in the original but are not part of `frame`. The idle buggy's calls are
  dropped by the hooks (they filter on the player's car), but its own inline writes and the two listeners'
  writes are not checked. They lie outside the compared blocks.
- **NaN yaw rates were never fed.** The hold's NaN behaviour is ported from the instruction, not seen run.
- **The update is not ported.** The car's update (vt +0x10c) runs outside the world step. It is MD106d.
- **Only the buggy was run.** The golf cart and truck (other vtables, the truck's post wrapper) were not.
  The baja and sandrail share the buggy's listeners as far as MD39's vtables show, but were not run.
- **The world step is shared.** It is MX Unleashed's (`biplane::World::step`). It is proved here only for
  this world: one body, no constraints.
- **Clippy was not run.** It cannot lint game-sim (see memory). atv_car.rs uses only ps2-float ops, under
  `deny(clippy::float_arithmetic)`.

## Next

MD106a: run each `Call` through car.rs's pieces on state read from the image, and unhook each one in turn
under this oracle. That needs a reader from the image into car.rs's `Machine` fields through # MD39b's
field map.
