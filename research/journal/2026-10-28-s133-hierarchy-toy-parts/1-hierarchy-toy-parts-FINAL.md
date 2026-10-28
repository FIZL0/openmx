# 1 — hierarchy toy parts (FINAL)

Task S133. 2026-10-28.

## What I did

- **Made the two save states the task needed.** No state of free10 or nat11 existed in any slot. I
  used R7's scripted boot: cold-boot the disc, and over PINE, until a scene exists, write
  `start_mode = 2`, the event's league, type and track name, one racer, `all_ai = 0`, 250cc and Race.
  - `Free10`: league 2, type 5 (freestyle).
  - `Nat11`: league 1, type 1 (practice), which is what the TrackName autodetect sets for a `nat` name.

  Each state was saved about 8 s after `in_game`. The script is `boot.py`, beside this entry. Run it
  as `./kit pcsx2 with -- python3 boot.py <Track> <league> <type> <slot>` on a freshly started copy.
  Its `except` catches the `RuntimeError` PINE raises before the disc has booted.
  - free10 is slot 3 of copy 1; its RAM is `context/states/s133-free10.ee.bin`.
  - nat11 is slot 4; its RAM is `context/states/s133-nat11.ee.bin`.
  - Both RAM images are in **s1's** context only.
  - Shots of both (bike at the start, the expected HUD) were checked by eye and not kept.
- **Wrote `crates/game-render/tests/toy_parts.rs`**, which proves the parts against those states (see
  below). It needed no change to the port: `flatten`, `Spec::at` and `Toys::frame` were already right.
- **Added names to `research/oracle.txt`:** `kinematic_body_vtable` and `kinematic_body_update`, plus
  an S133 comment block on the parts' fields.

Nothing was taken from opentennis: it has no animated toys or hierarchy collision.

## What the original does

- **The parts list.** `./kit fn 119070 -p boot` (`animated_toy_collision`) runs on the toy's motion
  object, not on the toy:
  - the parts array goes to `+0x300` and its count to `+0x304`;
  - a mesh `.col` (kind 0) is one part;
  - a hierarchy (kind 1) is split by `collision_hierarchy_split`.
- **Each part's body.** Each part gets an 800-byte kinematic body (`./kit fn 1dd580 -p boot`, vtable
  `kinematic_body_vtable`). The body's vt+0x74 (`./kit dis 0x158da0 0x158db0 -p boot`) is a single
  store: body `+0x254` = the part's `+0x230`. By then `+0x230` is a node of the toy's skeleton:
  - one of the motion object's joints' `+0x34` nodes for a hierarchy part;
  - the root node `+0x30` for the mesh part.

  The part's `+0x7c` is then set to the body.
- **The body's update.** The body's update (`kinematic_body_update`, `./kit fn 1dd9b0 -p boot`)
  copies `+0x2a0` to `+0x2e0`, then the node's world matrix (`+0x80..+0xbc`) to `+0x2a0`. Its
  velocity and spin are the path car's `path_car_velocity` and `path_car_spin`.
- **What the states show:**
  - free10 has 2 animated toys:
    - the windsock: a mesh `.col`, one part on its root node. Its body is still identity, so it was
      never updated, as with free09's rustic windmill.
    - shovel_01: 71 joints and 10 parts, on joints 0, 1, 4, 5, 8, 9, 10, 11, 13 and 16.
  - nat11 has 8 gondolas, each placed at identity, with 2 joints and 2 parts (joints 0 and 1).
  - On every hierarchy part, body `+0x2a0` equals its node's world matrix. So the bodies are updated
    after their toy's nodes in the frame.
  - The clips are 10 s for the shovel and 3.33 s for the windsock, both looping. All eight gondola
    clips are 125 s and looping.

## How it was proved

All of it is in `crates/game-render/tests/toy_parts.rs`. It builds each track's toys from the disc
(`Animated::load` over `<track>.res` and `<track>_2.res`) and checks them against the state's toys,
which it finds by the vtable scan.

- **Matching.** Each of the port's toys is matched to exactly one state toy by two things, both bit
  for bit:
  - the placement (the root's local matrix);
  - the clip: every rotation and position channel's name and every key word.

  This is needed because the eight gondolas share one placement, so the clip is what tells them apart.
- **The binding**, on free10's 10 parts and nat11's 16:
  - The part count equals the state's.
  - Part `p`'s joint, through `flatten`, is the node of the state's part `p`. This proves bone 0 is the
    toy's root, siblings are in `.psg` order and the bones are clamped as the loader does. Every
    part's node is distinct, so it also proves the part order.
  - Each part's body is of the kinematic class and is on that same node.
  - The windsock's mesh is one part, on its root node.
- **The motion.** The test starts from the state's clocks and channel hints. Each tick, the
  interpreter runs, for each toy, `motion_advance(model, dt)`, `transform_rebuild` on every joint node
  and `kinematic_body_update` on every part's body; the port runs `Toys::frame(dt)` over a props-only
  world. Then, for each part, these are compared bit for bit:
  - the prop's `Moving.pose` against body `+0x2a0`;
  - its velocity against `path_car_velocity(body)` (xyz);
  - its spin against `path_car_spin(body)` (xyz).
- **The frame's dt** is the state's `frame_dt`. The test asserts that the port's `1/dt` (one division)
  equals the app's word after it (`app+0x348`), which the body's virtuals read.
- **Length of the run.**
  - free10: 360 ticks (12 s), past the shovel's 10 s loop and its wrap.
  - nat11: 3,900 ticks (130 s), past the gondolas' 125 s loop and its wrap.
- **The tests really ran.** A temporary print showed:
  - free10: 2 toys and 10 parts, with 2,254 of 3,000 part-ticks moving in the first 300-tick version.
  - nat11: 8 toys and 16 parts, moving on all 4,800 part-ticks.

  The final tests assert free10 moved on more part-ticks than it ran ticks, and that every gondola
  part moves on every tick. Both pass in about 9 s.
- **Tick 0 checks the state's own bodies.** The body's update moves the state's `+0x2a0` into
  `+0x2e0`, so the first tick's velocity and spin compare the port's pose at the state's clock against
  what the state already held.

## Not verified / not 1:1

- **The parts' meshes are not compared with the state's split collidables.** Not checked: their
  triangles, vertices, transforms and surface. Only each part's node, body and order are. Reason: the
  0x250-byte mesh collidable's fields past `+0x230`/`+0x7c` are not mapped, and MD5 parsed the `.col`
  hierarchy class against the disc already.
- **Where the body's update sits in the original's frame is inferred, not read.**
  - What is known: both states hold `+0x2a0` equal to the node's world matrix, so the update follows
    the toy's.
  - What the test does: it runs it right after its toy, in the interpreter, and not from the physics
    pass's own loop.
  - Not checked: a contact resolved between the toy's update and the bodies' would see the old matrix.
- **Contacts on the parts are not exercised.** Not exercised:
  - the bike against a moving shovel or gondola;
  - the body's local velocity (vt+0xb4);
  - the push-out through `Moving::mover`.

  Reason: no capture of a bike touching either toy. Those paths are the path car's, proved under
  S142/S145.
- **The windsock's mesh part stays a static prop in the port.** The port gives it no `Moving`. In the
  original its body is never updated, so `path_car_velocity` would answer the node's position × 30
  against a still-identity `+0x310` (oracle.txt's free09 note). Whether a contact ever reads that is
  not checked. Reason: out of this task's parts. It is the mesh toys' case, already listed under V43.
- **AnimateWhenNotVisible is still not ported (V71).** The interpreter and the port step every toy
  every tick. The original skips a toy that writes False and was not drawn.
- **One state per track, from a scripted boot rather than the front end.**
  - The freestyle course fields are R7's defaults.
  - nat11 runs as a practice session.

  The toys' load does not read either, but that is from the decompilation only.
- **The two states are only in s1's `context/states`.** Elsewhere both tests skip and still read as
  passing.

## Next

Nothing for this task.
