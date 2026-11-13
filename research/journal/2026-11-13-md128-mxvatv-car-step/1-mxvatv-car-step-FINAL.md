# 1 — mxvatv car step (FINAL)

Task MD128. 2026-11-13.

## What I did

- `atv_car::Own` now answers most of what MD106b's five calls hand out, with ported code. Their own
  sub-calls go through `Own` too (`o.call`), so a call one of them makes is answered as well:
  - `OutOfBounds`: the map-edge test and throw (`out_of_bounds`), reusing `biplane::outside_mxvatv`
    and `throw_mxvatv` with the vehicle's `area` and its own `rng`.
  - `BodyReset` (`body_reset`): the three world lists, the body switched off, then the riding let-go or
    the record handed back, then flag word bit 1.
  - `SpatialUpdate` and `NodeBounds` (`node_box`, `Scene::database_box`).
  - `CollisionSync`: `atv_moto::collision_sync`, with `part_sync` on `atv_sync::part_sync`.
  - `GravityRecord`: `atv_contact::resolve`, when the vehicle holds a record whose list has no entry
    with another body.
  - `SubtreeOutOfDate`: the subtree's current bits (`Below`), walked as `handling::node`'s out-of-date.
- New `Call`s for what those hand out in turn:
  - `SpatialMove` (the database move of the car's entry or the node's) and `PartMove`.
  - `NodeRebound` and `Gather`.
  - `LeaveWorld(k)`, `RidingLetGo` and `ReleaseRecord`.
- `Vehicle` lost `Copy` and gained the words those read: `area`, `rng`, `spatial`, `collidables`,
  `reset_flags`, `flag_word`, `owned`, `held` and `below`.
- The calls still handed out: `Respawn` (filed as MD148), plus cue, rumble, wrecked look and landing
  sound (left out on purpose).
- Tests:
  - MD106b's `the_cars_own_calls_are_the_originals` now unhooks MD128's calls as well. Their reads,
    writes, generator and hooks are in the new `tests/mxvatv_car_frame/own_hand_outs.rs`.
  - `whole.rs` now runs the collision sync and the subtree walk. Only the sort's place and search,
    the spatial move and the gather stay handed out.
- Harness fix: MD127's sort-place hook is on the ray probe's stand, which every ray probe shares. It
  used to swallow the riding probe's stand in the collision sync silently. Now any probe other than
  the sort probe runs the original's own stand inside the hook (`run_inside`, as in
  `tests/atv_wheel.rs`). This applies in `main.rs`'s `hook_sort` and in `whole.rs`'s bare run.

## What the original does

Addresses are in research/oracle.txt `# MD128`.

- **Edge test** (car vt `+0x16c`): does nothing unless the car is held to the map. Otherwise the body's
  position is tested against the terrain extent with a 150 margin on each side (strict `<`). Outside,
  it runs the same throw as the biplane's: velocity and lift, the world's drag halved, three `rand`s
  into the rates. Returns 1.
  - On this image the extent is the same object as the world's in-world box (`+0xa8`). So the edge
    test's width and depth are the box's high x and z.
- **Body reset** (vt `+0x13c`): does nothing with the body off. Otherwise:
  - three world-list removals, then the body off;
  - with an owner and a record: reset-byte bit 7 lets go of the riding probe (if there is one),
    otherwise the record goes back to its owner;
  - reset-byte bit 6 clears flag word bit 1.
- **Spatial update** (vt `+0xe4`) and **node bounds**: the quad's code.
  - The node's frame is brought up to date and its bounds gathered again when stale (bit 7). The
    gathering, called the rebound, is handed out.
  - Then the world box goes into the database. The node's move can use the box round its origin
    (second flag byte, bit 2).
- **Collision sync** (vt `+0x124`): the quad's code. The riding probe is synced unless it is also the
  active probe; then the frame mesh; then the active probe and its gather.

## How it was proved

Function oracle: `crates/game-sim/tests/mxvatv_car_frame/own_calls.rs`,
`the_cars_own_calls_are_the_originals`.

- It runs off MD39a's `buggy.ee.bin` with MD106b's five calls unhooked, and MD128's as well.
- 3,000 generated states. For every one, these matched word for word:
  - car, tires and body;
  - MD106b's vehicle words;
  - the extent, the car's entry, flag word, owner and collidable pointers;
  - both collidables' blocks: entry, reach, centre, frame, and the mesh's two kept frames;
  - every one of the 44 subtree nodes' flag words;
  - node 0's entry, box and reach words.
- Also equal: the two call logs, including what the spatial moves, part moves and world lists were
  handed, and the random numbers drawn.
- The original wrote nothing in those blocks that the test didn't compare.
- All 35 branches were seen at least 30 times each, including:
  - edge asked: inside, outside, and on the margin (±1, ±0.5 and 0 from it);
  - car and node entries moved, node rebound, the node's origin box;
  - sync off, riding synced alone, gather;
  - riding let go, record handed back, outside with the record kept;
  - flag word bit cleared;
  - subtree marked, and subtree current but not marked.
- A mutation (edge margin 150 → 151) passed the test at first, because the generator never came near
  the margin. That is why the margin cases were added; "edge on the margin" was then seen 137 times.
  The mutant was not run again afterwards (see below).
- `whole::the_buggys_whole_frame_is_the_originals` passes with the collision sync and subtree walk
  run. The suite's other 17 tests still pass.

## Not verified / not 1:1

- **Respawn** (vt `+0xe8`) is still handed out: put-back with its ground reset, then the tires' and the
  engine's resets. That is too big for this task. Filed as MD148.
- **The node's rebound** is handed out. The buggy node's list is 45 nodes long, past what
  `atv_moto::Scene` holds (`NODES`).
- **The world-list removals, the riding let-go, the record's release, the spatial move and the
  gather** are handed out, as they were for the quad. The test checks what they were handed, not what
  they do.
- **The record's contacts** were only run with an empty list, which is what the image has. `held`'s
  `cofg = car.cofg` and a non-empty `resolve` are unproven for the car. `GravityRecord` stays handed
  out when the list has an entry with another body.
- **Edge test, low side:** the `x < 150` and `z < 150` sides were never generated. The body sits
  above 400 on this image, so only the far edges were tested.
- **The body disabled** (body `+0` clear) was never generated as an input. The port returns at once
  there; the body reset's switch-off was seen.
- **Cue, rumble, wrecked look and landing sound** are handed out only (sound, pad and model, not
  physics).
- **The margin mutant after the fix:** I didn't re-run it. Both tries timed out waiting for a
  `./kit check` slot, because the other checkouts held all three for over an hour. A ±0.5 case should
  flip the mutant's answer (inside for the original, outside for the mutant), but no run showed it.
- **Only one image:** the buggy (kind 4). The extent is assumed to be the same object as the in-world
  box (the test asserts it). A map where they differ would need separate generation.

## Next

Start MD148: port the respawn in `atv_car.rs`, then unhook it in `own_calls.rs`.
