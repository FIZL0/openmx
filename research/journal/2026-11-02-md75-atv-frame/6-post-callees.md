# The vehicle post's three calls: spatial update, node bounds, collision sync (MD75c)

2026-11-03. Task: `./kit plan MD75c`.

## What the original does

Read with `./kit fn` / `./kit dis -p atv`. The field map is in research/oracle.txt under `# MD75c`.

- **Spatial update** (atv_vehicle_spatial_update, quad vt +0xe4). It runs only when the quad has an entry
  of its own in the world's spatial database (quad +8). It takes the box of the quad's node in the world
  (atv_node_world_box) and moves the entry to that box with a reach of 0 (atv_spatial_move).
- **Node box in the world** (atv_node_world_box). It brings the node's world frame up to date. If the
  bounds are stale (bit 7, plus either bit 3 or no bit 6), it gathers them again through node vt +0x94
  (atv_node_rebound) and clears bit 7. The box is the node's own (+0xd0 centre, +0xe0 half extent) carried
  into the world:
  - centre = `((p + 0) + x·c.x + y·c.y) + z·c.z`;
  - half = `(|y·e.y| + |x·e.x|) + |z·e.z|·1`;
  - xyz only. The w lanes stay the box's own, because the 1.0 the function first stores is overwritten.
- **Node bounds** (atv_node_bounds). It runs only when the node has a database entry (node +8). It
  computes the same box. Then:
  - With bit 2 of the second flag byte (+0x101), it hands over a box around the node's origin: its world
    position with w 1. The half extent grows by `|centre − origin|`, with the sign bits masked by `pand`
    on every lane. The reach is the length of that half extent (FPU `mula`/`madda`/`madd`, then
    `sqrt.s`).
  - Otherwise it hands over the box as it is, with the node's reach word (+0x108).

  The decompile hides that the bit-2 path's a1 is the origin, not the centre. `./kit dis` shows it.
- **Collision sync** (atv_vehicle_collision_sync, quad vt +0x124). It runs only when bit 0 of quad +0x44
  is set. Then:
  - the riding probe (+0xcc) is synced, unless it is the active one (+0xd8);
  - then the frame mesh (+0xd0) is synced;
  - then the active probe is synced, and the nearby list (+0xc4) is gathered around it with kind 3
    (atv_gather_nearby, already ported by MD76a1 as `atv_world`'s gather).

  Each sync (atv_part_sync) calls the part's vt +0xc8, then its box goes into its database
  (atv_part_box_move → atv_spatial_move).

In atv-r-spawn.ee.bin the quad and its node have no database entry (both +8 are 0). Bit 1 of the flag
byte is clear, so the node's bounds are not asked for. The collision sync is on. +0xcc and +0xd8 are the
same probe (0x0159c500), and +0xd0 is the frame mesh. Both parts have a database (+0x20).

## The port

- `atv_moto.rs` (part 4):
  - `Node` gains `spatial`, `centre`, `half`, `more` and `reach`;
  - `Parts` gains `spatial` and `Collidables` (sync, riding, frame, active, as `quad::Part`s);
  - the functions are `Scene::bounds_stale`, `Scene::rebounded`, `Scene::world_box`, `Scene::database_box`
    and `collision_sync`.
- `quad::Call` gains `NodeRebound`, `SpatialMove { node, centre, half, reach }`, `PartSync(Part)` and
  `Gather(Part)`.
- `quad::Steps` answers `SpatialUpdate`, `NodeBounds` and `CollisionSync`. `VehiclePost` now runs them
  inline (`self.call`), so the database move, the rebound, the part syncs and the gather are what go to
  `rest`.

## Proof

New hooks in `tests/atv_quad.rs`' `hook_post`:

- atv_spatial_move logs its entry (the quad or its node), both quadwords and f12;
- atv_part_sync logs which collidable;
- atv_gather_nearby asserts the list and kind 3, and logs the probe;
- a vtable laid at the quad's node (+4) sends +0x94 to a hooked address, which logs `NodeRebound`.

`lay_scene` writes the new words: quad +8, +0x44, +0xcc/+0xd0/+0xd8, and each node's +8, box, +0x101 and
+0x108.

- `0x0075_c001` (`the_quads_post_process_callees_are_the_originals`): 10,000 states under
  atv_quad_air_state, with the vehicle post and all three calls unhooked. Every region and the call log
  are equal. Each path below was seen at least 30 times:
  - the quad's entry moved / has none;
  - the node has no entry, its box around its origin, its box at its reach;
  - rebound;
  - sync off, no riding probe, riding is active, riding apart, frame / no frame, gathered / no active
    probe.

  The generator is MD75b1's scene with these on top: entries half the time; random boxes, w lanes
  included; random second flag bytes and reaches; bounds asked for half the time; the collidables drawn
  from four choices.
- `b102` and `b104` (the vehicle post and the whole PostProcess) now pick the three calls too, because
  they run inline. b104 uses the new generator. `post_paths` tallies "bounds" from bit 1 of the flag byte,
  since `NodeBounds` no longer reaches the log.
- `cargo test --release -p game-sim --test atv_quad -- post_process vehicle_post node_position`: all 5 pass.
  The whole file is in the commit message.

Mutation checks, one change each, caught by `c001`:

- the abs dropped on the z row;
- the bit-3 clause dropped from the stale test;
- bit 7 not cleared;
- the origin's w 0;
- the reach taken from the half extent before it grows;
- the centre's w 1;
- the riding probe synced even when it is the active one.

Not caught: dropping the world update before the origin is read. This is equivalent, because the box
step has just brought the node up to date, and nothing between the two can change the frame.

## Decisions

- **The database move, the rebound, the part sync and the gather stay calls.** atv_spatial_move is a
  portal walk over cells and polymorphic sub-databases (four virtuals each, a link pool, an overlap pass).
  atv_node_rebound gathers child bounds through four more functions. atv_part_sync calls one virtual per
  part kind, and two kinds are in use. Each one is a port of its own. MD75b1 did the same with the
  vehicle post's callees. Follow-ups: MD75c1 (the database move), MD75c2 (the part sync and the gather), MD75c3 (the rebound).
- **The quad's collidables are `quad::Part`s.** The sync compares pointers. The harness gives each `Part`
  its own address, so two equal `Part`s are the same pointer.
- **Nested calls run inline.** As in MD75 (memory `steps-nested-calls-run-inline`), a test that unhooks the
  vehicle post must unhook its three calls as well. `POST_STEPS` lists all four.

## Not verified / not 1:1

- Only the three bodies are proved. Their callees are only hooked: atv_spatial_move, atv_node_rebound,
  atv_part_sync, and the gather into the nearby list (proved alone in tests/atv_gather.rs, not under the
  sync). On the spawn frame the database move runs twice (once for the probe, once for the mesh), so the
  brief's whole frame still needs those callees ported (MD75c1, MD75c2, MD75c3).
- Node bounds is only ever asked about the quad's node (node 0), because the vehicle post's call always
  passes quad +0xe4.
- No generated lane is −0, a denormal or a NaN. The `pand` and `vabs` therefore differ only in ways the
  states never reach.
