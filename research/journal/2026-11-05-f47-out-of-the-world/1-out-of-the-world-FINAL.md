# 1 — out of the world (FINAL)

Task F47. 2026-11-05.

## What I did

- **`game_sim::handling::reset`** (`crates/game-sim/src/handling/reset.rs`, new). Word-level ports over
  a `Resets` trait (word, set, and an `Out` per call out):
  - `body_reset`: the bike's vt `+0x13c`.
  - `respawn`: vt `+0xe8`.
  - `post`: the event post the out-of-world message goes through.
  - Their helpers: `remove`, the three world-list removes (one code); `add_unique`, the riding
    part's list, growing through the allocator; `body_on`, the body's on-word virtual.
- **`game_sim::handling::node`**, the node's box for a bike or node in the spatial database, on RAM:
  - `bounds_stale`, `rebound` (the node's vt `+0x94`, gathering over its `+0x144` list), `world_box`,
    `bounds` (the node-bounds function) and `owner_box` (the spatial update's box).
  - `rebound` is a RAM copy of `Scene::rebound`, because the scene holds 4 nodes and state 4's node
    gathers 22. I rejected growing `Scene`: it would touch every reader of it.
- **`steps::read_sync`**: `read_post` without its refusal. `read_post` now calls it, then refuses as
  before. The collision sync and the node position read only node 0's own frame, so they are right
  without the refusal.
- `atv_moto::frame_inverse` is now `pub(crate)`.
- **Tests** (`tests/mxvatv_bike_ride.rs`, the F47 section at the end):
  - Six oracles; see "How it was proved".
  - The glue: `ResetInRam`, `log_hook`, `f47_compare`, `gathers_in_database` and `node_box_on_ram`.
- **Wiring.** `PostsInRam`, and so the ride/flip "Rust callees" replays, now serve:
  - BodyReset and Respawn with `reset::body_reset` / `reset::respawn` (the ground reset is
    `wreck::ground`);
  - OutOfWorld with `reset::post` (the listeners the original's);
  - PutBack with `wreck::put_back` (F31b5c);
  - SpatialUpdate and NodeBounds, when the bike or node is in the database and the node gathers, with
    the RAM node-box ports (the database move the original's);
  - CollisionSync and NodePosition through `read_sync`.
- `wreck_steps` (the ground reset's spatial update and node bounds) takes the same RAM path when
  gathering in the database, and reads with `read_sync`.
- `RecordsInRam`: with a parented node, the record's four node calls (turn, position, placement,
  move) go to F43's `node::rotate/position/place/set` on RAM, because `steps::read` refuses a parent.
- Addresses: research/oracle.txt `# F47`.

## What the original does

- **The body reset.**
  - With the body's word 0 clear, it does nothing.
  - Otherwise it takes three things off the physics world's three lists, each removed by
    first-match ← last, count − 1: the body, the bike's `+0xbc` word, and the bike's post element
    `bike+0x34`.
  - It turns the body off through the body's second vtable (`+0x110`, slot `+0xc`, a bare
    `sw a1, 0(a0)`).
  - It lets go of the collision record only when both the owner and the record are set:
    - with bit 7 of byte `+0x44` and a riding part, it clears the riding part's `+0xf8` and adds the
      bounds' `+0xa4` word to the riding part's list once. Add-unique reads the count once, then
      looks for the word, then for an empty slot (re-reading the array), else grows the array by
      one through the allocator's resize;
    - without bit 7, it hands the record back to its owner.
  - With bit 6 of `+0x44` it clears bit 1 of `+0x24`.
  - Every word is re-read where the original re-reads it (the body, the world, the riding part).
- **The respawn.** The ground reset, then `+0x31c = 1`.
- **The event post.**
  - The message goes on the poster's stack as `[kind, id, t1, t0]`.
  - The queue's registrations are searched (signed count) for the first that is live (word 0 clear),
    is the poster's (`+0x10`), and has the poster's tag (`+4` = the poster's word 0).
  - Then every live listener of that registration is told through `(*(l[0x10]+8))+0xc`, with the
    listener, the poster and the message's address. The listener count is re-read each pass, so a
    listener that changes it changes the walk.
- **The node's box.**
  - If the bounds are stale (bit 7, and bit 3 or no bit 6), the node regathers them: as one sphere
    (bit 4) or one box in its own frame, from its own (bit 2) and each listed node that has its own.
  - The box gather leaves two frames in the scratchpad's first 0x80 bytes: the gathering node's
    inverse, and the listed node's frame inside it. The oracle found these.
  - Then the node clears bit 7 and works out the world box.
  - The node-bounds function adds the reach round the origin when bit 2 of the second byte is set.
  - The spatial update moves the bike's entry with reach 0.

## How it was proved

All six are function oracles in `tests/mxvatv_bike_ride.rs` on state 4's RAM image. Every word of RAM
outside the stack, and of the scratchpad, comes out the same, and so do the logged calls.

- `the_body_reset_is_the_originals`, 10,000 draws:
  - draws: body on/off; the three lists' counts −1..6, with the entry present or not; owner,
    record and riding part there or not; bits 7 and 6; the riding part's list holding the word, a
    hole, or neither (so it grows);
  - the allocator is hooked to log;
  - the release is the original's.
- `the_respawn_is_the_originals`, 10,000 draws as F31b5e's ground-reset oracle draws them.
- `the_event_post_is_the_originals`, 10,000 posts:
  - 9 in 10 on queues drawn in free RAM at 0x90000: registrations live or not, the poster's or
    not, tagged or not; listeners hooked to log, some changing their registration's count;
  - 1 in 10 the out-of-world message on state 4's real queue, to the 11 real listeners (the
    camera controller etc.).
- `the_node_box_in_the_database_is_the_originals`, 10,000 draws:
  - the node's flag bits; the second byte's bit 2; own centre, half and radius; position;
  - how many of the 22 skeleton nodes the gather takes, each with or without its own bounds;
  - node bounds, the spatial update, or both; the database move is hooked to log.
- `the_post_process_out_of_the_world_is_the_originals`, 10,000 draws:
  - the original's PostProcess against `bike::air_state` with `PostsInRam`, and the real database:
    the bike and node are added to state 4's database (0x672cd0) by the game's own add functions;
  - the paths taken: 3,795 out of the world; 2,139 body resets and respawns; 4,426 node bounds and
    6,492 spatial updates gathered in the database; 3,795 put-backs and message posts.
- `the_records_parented_node_is_the_originals`, 10,000 draws:
  - the bike's node hung under a drawn parent (each current or not);
  - each of the record's four node calls 2,500 times, `RecordInRam` (the original's) against
    `RecordsInRam` (the ports).

`./kit check -- -p game-sim --test mxvatv_bike_ride`: all 69 tests pass. That includes
`ride_replays_bit_exact_with_rust_callees` and `flip_replays_bit_exact_with_rust_callees`, which run
through the rewired `PostsInRam`, and F31b5e's ground-reset oracle through the changed `wreck_steps`.

## Not verified / not 1:1

- **C15 never reaches any of these paths.** They are proved by oracle on state 4 alone; no recorded
  ride leaves the world, puts the bike or node in the database, or parents the node. The ride/flip
  replays only show that the rewiring broke nothing on the paths they take.
- **Calls left to the original:**
  - the record's release (`0x203f40`), the allocator's resize, and the post's listeners (the camera
    controller and the others: `Out::Tell`);
  - the spatial database itself (insert, remove, move: `0x18d690`, `0x18d540`, `0x18d590`). These
    are F46's / the database's, not this task's.
- **A sphere gather with nothing to gather** (bit 4, no own bounds, no listed node with its own) reads
  a stack word the original never writes. The port takes 0, as the scene's does, and the oracle does
  not draw that case.
- **The body-reset and event-post oracles use fake data.** The world lists, the riding part's list
  and the post queues are drawn in free RAM, not taken from a real game moment. Only the
  PostProcess oracle and the 1-in-10 real-queue posts use state 4's own structures.
- **Every database oracle uses one database**: state 4's, with the node added through the ground
  reset's own call `(node, db, 1, 1)`. I set state 4's bike `+0xb0` (0) to that database by hand
  for those draws, and tied `+0x45` bit 1 to the node being in it. I don't know what the game itself
  leaves in `+0xb0` for a bike in the database.
- The glue's scratch slots over the stack (up to `WRECK_SCRATCH`'s) are left out of
  `f47_compare`'s RAM compare. They are out-parameters the glue hands the original's callees.

## Next

Nothing for F47.
