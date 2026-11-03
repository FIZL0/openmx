# MD75c2: the collision sync's part sync and gather

2026-11-03. Task: `./kit plan MD75c2`.

MD75c (`6-post-callees.md`) ported the collision sync's branches but left its three calls hooked:
`atv_part_sync` for each part, the box move that call ends in, and the gather of the nearby list. MD75c1
ported the box move. This task ports the part sync in `crates/game-sim/src/atv_sync.rs`, which
`quad::Steps` now runs for `quad::Call::PartSync`. `quad::Call::Gather` is answered by the world through
the new `quad::Sweep::gather`, which `atv_search::Oasis` implements. Field notes are in `research/oracle.txt`
under "MD75c2".

## What the original does

`atv_part_sync(part)` calls vt `+0xc8` and then `atv_part_box_move`. The brief called vt `+0xc8` the record
clear, but MD86 found it is not: for both of the quad's kinds it is the same pair of steps.
`atv_node_frame` brings the part's node up to date and copies its world frame (the probe keeps its node at
`+0x174`, the mesh at `+0x240`). Then vt `+0xcc` stands the part at that frame:

- **Ray probe** (`atv_ray_probe_stand`). The frame becomes `+0x130`. The box comes from the probe's tree
  (`+0x17c`, centre `+0xc`, half `+0x18`), or, with no tree, from half the sum and half the difference of
  its bound (`*(+0x180)`). With a bound, all four lanes of `+0x50` are written, and `w` is the high corner's.
- **Mesh** (`atv_mesh_stand`). It keeps the last two frames (`+0x1f0` = `+0x1b0`, `+0x1b0` = the frame). Its
  own frame (`+0x130`) is its frame on the node (`+0x170`) times the node's. Its box is its mesh's (`+0x244`).

Both kinds then take the reach as the length of the half extent, with an FPU `mula`/`madda`/`madd`/`sqrt`
chain, and put the box's centre through the frame into `+0x50`.

**The mesh's centre goes through the node's frame, not its own.** The stand reads `a1` again rather than
`+0x130`. My first port used `+0x130`, and the generated test caught it. In the image the two are equal,
because the mesh sits on its node unturned.

`atv_part_box_move` runs only if the part is in a database (`+0x20`). It calls vt `+0xc4` (the box: the
centre through `+0x130`, half `(|r1·e.y| + |r0·e.x|) + |r2·e.z|·1`, both with `w` 1) and then
`atv_spatial_move(part + 0x18, ...)` with the reach.

## In the port

- `atv_moto::Collidables::states` holds the five parts' `atv_sync::Collidable`, indexed by `atv_sync::slot`.
  `Parts` is `Copy`, so the parts are as well.
- `Steps` answers `Call::PartSync` with `scene.update(0)` and then `atv_sync::part_sync` at node 0's world.
  Both of the image's parts sit on the quad's own node (`0x0159c290`), so the port assumes node 0.
- The box goes on to `rest` as `Call::PartMove`. `atv_spatial::Answers` answers it in its `Space`, which now
  has one record per part (`PARTS + slot`).
- `Steps` now runs the collision sync's calls through itself rather than through `rest`, as it does for
  the vehicle post's. A test that unhooks `COLLISION_SYNC` therefore has to unhook `PART_SYNC` as well, so
  `POST_STEPS` now includes it.
- `Call::Gather` goes to `Sweep::gather`. With no world, as with `()`, it goes on to `rest`. `Oasis` puts its
  gather probe at the active part's centre with the part's reach, gathers, and marks the list as gathered.
  Its sweep stops gathering for itself once that has happened. Before any PostProcess has run, the sweep
  still makes the list (MD86).

## Checked

- `tests/atv_quad.rs` `the_quads_part_sync_is_the_originals`, seed `0x0075c201`: 10,000 states.
  - Each part is laid whole in the harness, `0x400` apart (`lay_collidable`): a probe on a generated tree
    or bound, the frame mesh with generated frames and box, and a database entry half the time.
  - The vehicle post, its three calls and the part sync are unhooked.
  - Every region, the parts included, is equal, and so are the logged calls.
  - Paths: no sync 4,651 / probe on its tree 2,699 / on its bound 2,818 / mesh 3,999 / box moved 4,761 /
    no entry 4,755.
- `the_quads_post_process_callees_are_the_originals` and `the_quads_post_process_runs_every_ported_step` now
  unhook the part sync too, and pass.
- `image.rs`: `read_ride` reads the parts' states from the image, and the read-back compares them word for
  word. The sweep (vtable `0x003b6b40`) is neither kind. Its words are declared gaps, and the bound the
  harness lays for it is the harness's own.
- `tests/atv_quad/oasis.rs` `the_quads_collision_sync_on_desert_oasis_is_the_originals`: the 763 MD86 frames
  on Desert Oasis.
  - The image's own probe (`0x0159c500`, riding and active) and frame mesh (`0x01627370`) are used.
  - The probe's tree box is grown up to 8× on 388 frames, so the gather's reach changes.
  - The mesh is missing on 97 frames and out of the database a quarter of the time.
  - The original syncs and gathers into the image's list. The port's `Oasis` answers the gather.
  - All equal: the quad, the scene, both parts, the calls (763 probe moves, 485 mesh moves), and the list
    entry for entry with distance bits, bound and centre. 636 frames gathered something: a mesh on 635, a
    plant on 335.
  - The probe cannot be taken out of the database here. The gather queries the probe's own database, and
    with `+0x20` = 0 the original reads RAM 0 and faults.
- Mutations: the mesh centre through its own frame, the bound's centre with `w` kept, no `vabs` on a row, the
  mesh's frame before not kept, the node not updated, and the gather's centre or reach not set. Each fails
  at least one of the two tests. Swapping the `vadda` operands is the only mutation that does not fail, and
  addition commutes.
- `./kit check -- -p game-sim --lib --test atv_quad --test atv_spatial --test atv_search --test atv_gather
  --test atv_ground --test atv_wheel --test mxvatv_ride --test mxvatv_step --test mxvatv_air`: 137 passed.
- `./kit audit` shows only the pre-existing leaks in `atv_wheel.rs` and `plan/DONE.md`.

## Context files

This slot lacked `MC02.esb` and `capture/md37a`, so I copied them from s1 (`context/` is git-ignored).
