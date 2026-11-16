# 1 — mxvatv car respawn (FINAL)

Task MD154. 2026-11-16.

## What I did

- Ported the car's respawn into `atv_car::Own`. It answers `Call::Respawn` when the new `Vehicle::back`
  (`atv_car::Back`) is set and the car's node has no parent. Otherwise the call goes on, as before, so MD149's
  Ride (`PutBack`) is unchanged.
- The port runs the put-back, its ground reset, the orient and the body's commit on the typed `Body` and `Scene`.
- What it can't do itself it hands out as new calls: `BackGround`, `GhostStart`, `TrailReset`, `NodeUnhook`,
  `NodeRehang`, `PartUnhook`, `PartOn`, `PartAttach`, then `TireReset(k)` ×4 and `EngineReset`.
- `Pieces` answers the two resets with the new `atv_car::tire_reset` and `engine_reset`.
- Tests are in `tests/mxvatv_car_frame/own_back.rs`. MD106b's frame test (`own_calls.rs`) now runs with the respawn
  unhooked and hooks what it hands out.
- Addresses are in research/oracle.txt `# MD154`.

## What the original does

- Respawn: `./kit fn -p atv 0x004cab10`. It calls put-back (snap 1, kind 1). Then, if the engine and all four tire
  pointers are set, it resets each tire and then the engine.
- Put-back: `./kit dis -p atv 0x004a6f70`.
  - Copies the spot's position.
  - Moves the run's spot when the run is there and its first two words are set.
  - With a terrain, it asks the ground at the position, lifts the position 0.3 along the normal, and takes the
    normal as the up. Without one, it takes the spot's forward and up.
  - Then the ground reset (the bike's), and the latch set.
  - Then the ghost's clock is set and the ghost starts, the trail resets, and cue 21 is posted.
- MD93's oracle note says SLUS-21104's put-back "drops the normal". That is wrong: it lifts and takes the normal.
- The orient's set-position marks the node's subtree out of date when the node was current. The first run of the
  test caught this: 12 subtree flag words differed.

## How it was proved

- `own_back::the_cars_respawn_is_the_originals`: the respawn called alone in the interpreter, over
  `md39a/buggy.ee.bin`, 2,000 generated states.
  - Compared: the car, body, tires, nodes, collidables, run words and respawn words, word for word.
  - The calls must match in order, and nothing else may be written in those blocks.
  - 17 branches, each seen 30 or more times: terrain or not, run moved / kept / absent, trail or not, parts or
    not, node bit on or off, sync on or off, no riding probe, no frame, and the frame and riding probe syncs.
- `own_back::the_respawns_tire_and_engine_resets_are_the_originals`: the tire reset on all four tires and the
  engine reset, 200 rounds of random words, every tire and engine word compared.
- `own_calls::the_cars_own_calls_are_the_originals` (3,000 frames) now runs the respawn unhooked from the unwreck
  and the outside-the-world path. A new branch, "respawn", is seen 30 or more times.
- The whole `mxvatv_car_frame` test passes (33 tests).

## Not verified / not 1:1

- What the respawn hands out is not run by the port: the ghost start, the trail reset, the database unhook and
  rehang, and each collidable's turn-on and attach. The tests check only that they are called, with the right
  arguments, in the right order. Reason: they are other objects' code (ghost, trail, spatial database, world
  lists), outside this task.
- The sweep collidable (`+0xd4`) is assumed to be there whenever the riding probe is, as the original does. The
  port holds no pointer for it.
- The test that calls the respawn alone clears only one of the five part pointers when parts are off (picked by
  the spot's x bits). The `&&` of all five is read from the disassembly, not tested one by one.
- A car node with a parent goes on to `rest` (Scene can't set a position under a parent). The image's node has
  none.
- Only kind 1 (ghost start) is ported, since the respawn always passes 1. Put-back's kind 0 branch (vt +0x39c)
  isn't.
- MD149's Ride respawn reset still writes w 0 where the original writes w 1. I left it alone to stay in scope.
  Filed as MD175.

## Next

Nothing; the task is done.
