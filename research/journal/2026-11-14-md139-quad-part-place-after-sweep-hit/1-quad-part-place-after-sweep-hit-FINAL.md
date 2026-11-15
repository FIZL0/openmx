# 1 — quad part place after a sweep hit

Task MD139. 2026-11-14. Slot s5.

## What I did

After a sweep hit, the record puts the quad out along the hit's normal. It sets the body's
position, then moves the scene node (`node_move`, the original's `0x00194b10`), then places the active
part (`place_part`, `atv_part_sync` `0x00114360`). With the Port pieces in `sim_frame`, both still
went to the original. MD118 named only the part place, but the node move ran the original too. Both
are now the port's, in `tests/atv_quad/sim_frame.rs`:

- **`WorldStep::node_set`**: `atv_moto::Scene::set_position` on the quad's node (it has no parent).
  The changed node words go to RAM. `node_point`'s write-back is now `nodes_write`, which also writes
  each node's own frame (`+0x30`).
- **The subtree in RAM (`out_of_date_in_ram`).** The original's move marks the whole subtree out of
  date (`atv_node_moved`): 16 nodes on this ride. The quad's wheel and rider nodes are past the 4
  nodes the port's scene holds. The first try left their bit 0 set, and 36 quad words differed after
  the frame. I found the 16 bytes by running the original's set on a RAM copy and diffing. The
  helper walks the same way as `Scene::out_of_date` (siblings `+0xf8`, children `+0xf4` only if the
  node was current), on the nodes the scene does not hold.
- **`WorldStep::part_place`**: the part read with `image::collidable` (it must be a ray probe on the
  quad's node). The scene brings node 0 up to date, then `atv_sync::part_sync` stands the part. This
  is what `quad::Steps` already does for `Call::PartSync`. Then the frame `+0x130`, centre `+0x50` and
  reach `+0x70` go to RAM, and `record_outside::spatial_move_in_ram` moves the database entry.
- **A check at the call.** The end-of-frame compare cannot see the part place: later in the same
  frame, the world step's collision sync syncs the same part again and moves its entry again. So at
  each call, the test runs the original's call on the RAM, keeps what it left, puts the RAM back,
  runs the port's, and compares every word outside the stack (`original_aside`,
  `is_the_originals`).
- `the_sweep_that_hits_the_palm_is_the_ports` now asserts that nothing was handed on (`> ` keys),
  and that the ride reached the node set and the part place.

No change in `src/`. The pieces were already ported: `Scene::set_position`, `Scene::update` and
`atv_sync::part_sync`. `quad::RecordOn` (MX Unleashed's worlds) has its own `place_part`, which is
correctly empty: its rays are stood at the node when they are searched. Nothing came from
opentennis: it has no vehicle physics.

## What the original does (fast palm ride, 130 frames)

One sweep hit (frame 90). The put-out ran once: one node set, one part place. The part has a
database entry, and the entry was moved.

## How it was proved

`./kit check -- -p game-sim --test atv_quad -- sim_frame::the_sweep_that_hits`: passes. It checks:

- the node set and the part place, each against the original's call, every RAM word outside the
  stack;
- every RAM word outside the stack against the original's whole frame, after each of the 130 frames.

Two mutations go red:

- without the database move: 3 words differ (the entry's last centre);
- without the subtree marking: 16 flag words differ.

The whole `atv_quad` suite passes (120 tests).

## Not verified / not 1:1

- **One hit, on a poked ride.** MD119's fast palm ride doubles the body's velocity before frame 87.
  It hits once. So the node set and part place are proved on one call each, on a ride no pad
  produced (MD119 has the reason no played ride makes the sweep hit on Desert Oasis).
- **Only the riding part was placed.** The wreck part (also a ray probe) and a part with no tree or
  no database entry never come up after a hit here. `atv_sync::part_sync`'s own oracle (MD75c2,
  10,000 generated parts) covers those.
- **The subtree marking is test glue, not product.** `out_of_date_in_ram` covers the nodes past the
  port's 4-node scene. The game's `quad::Ride` holds only those 4. The wheel and rider nodes there are
  drawn, not simulated, so nothing in the port reads their flags.
- **A put-out that returns early** (the two speeds facing each other) is not met on this ride. That
  branch is `record::sweep_back`'s and was already the port's. It just never reaches the node set.
- **The interpreter is the oracle, not PCSX2.**
