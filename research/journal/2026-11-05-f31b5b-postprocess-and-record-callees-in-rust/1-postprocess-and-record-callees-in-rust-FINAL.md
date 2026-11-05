# 1 — postprocess and record callees in rust (FINAL)

Task F31b5b. 2026-11-05.

## What I did

- `atv_moto::Scene` gained the record's three node transforms: `turn` (`atv_node_rotate`, 0x195230),
  `tip` (`atv_node_point`, 0x1952a0) and `set_position` (`atv_node_set_position`, 0x194b10 with
  `a1 = 0`, no parent only; it answers `None` for a parented node).
- `handling::steps::read_post` / `post_words` read the bike as `read` does, plus what the
  PostProcess's spatial update and collision sync read: whether the bike (`+8`) and its node (`+8`)
  are in the spatial database, the sync's bit (`+0x44` bit 0), the node's box words, and the three
  collidables (`+0xcc` riding → `Part::Riding`, `+0xd0` frame → `Part::Frame`, `+0xd8` active →
  `Riding` if it is the riding one, else `Part::Wreck`), each a ray probe or a mesh by a vtable pair
  the caller passes in (`steps::Kinds`; the port has no game addresses). The database entries
  (`+8`, a collidable's `+0x20`) are pointers, so they are read only, never written back.
  `post_words` lays its masked `+0x44` write over `words`' (the same word holds the `+0x45` flag
  byte) through a small `Over` view.
- `steps::words` now marks the bike's node's children out of date when the node goes from current
  in RAM to out of date in the bike: down the sibling list, into a node's children only if it was
  current, as `Scene::out_of_date` does. State 4's bike node has 22 nodes under it (the rider's
  skeleton, all current, flags 0xa5), and every Place (each frame) marks them. Without this a
  Rust Place loses those 22 flag bytes. F31b5a's paths never make the node out of date, so its
  words are unchanged.
- tests/mxvatv_bike_ride.rs, section "F31b5b":
  - `PostsInRam` (the PostProcess's Moto/Posts): the spatial update, node bounds, collision sync and
    node position go to `quad::Steps` on `read_post`'s bike; Place goes to `Scene::place`. Handed to
    the original: `PartMove` → 0x18d690 (entry `+0x18`, centre, half, `f12` reach), `Gather` →
    0x1d6a60 (`bike+0xc4`'s list, the part, 3), and the body reset, respawn, out-of-world message and
    put-back (never reached on C15).
  - `RecordsInRam` (the record's Collides): the node's rotate, point, position and set go to the
    scene; the damping goes to `seed::body` (body `+0x30`, `+0x150`). The sweep probe, part search,
    place-part and velocity stay `RecordInRam`'s.
  - `posts_frame` is `steps_frame` with both of these. `replay_posts` runs C15 twice in lockstep:
    Ee A with F31b5a's `steps_frame`, Ee B with `posts_frame`.

## What the original does

- The node set (0x194b10): if `a1` is the node, it adds `p.xyz` to the position row (`+0x60`). If
  `a1` is 0 and there is no parent, it stores `p.xyz` at `+0x60` (0x194af0), then calls moved
  (0x193d10). With a parent it goes through 0x1953c0 into the parent's frame first. Only the
  `a1 = 0`, no-parent case is ported.
- The node rotate and point: update (0x193b30), then `vmulax`/`vmadday`/`vmaddz.xyz` of world rows
  0–2 (the point starts with `vaddax` of row 3). The `w` lane is the argument's.
- The body damping setters (body vt at `+0x110`, slots `+0x20` and `+0x68`) store the vector at
  body `+0x30` and `+0x150` and nothing else (the oracle, below).
- On state 4 both the bike and its node are out of the database (`+8` = 0), so the spatial update
  does nothing and the node bounds are never called (flag byte `+0x45` = 0x05, bit 1 off). The
  riding probe (0xea5970, a ray probe) is the active one, and the frame (0xea63a0) is a mesh. Both
  have database entries (`+0x20`), so each sync moves them (0x18d690) and gathers the nearby list.

## How it was proved

- `the_node_transforms_are_the_originals`: 10,000 draws of state 4's bike node, with frames and
  flags (current or not, bounds stale or not) at random, 2,500 each of rotate, point, position and
  set. Each answer is bit-exact, and the whole of RAM outside the stack matches the original's
  after the call (the set marked the 22 children in more than 500 draws).
- `the_damping_setters_are_the_ports`: 2,000 random `(d, d, d, 1)` pairs through the body's two
  virtuals. RAM outside the stack matches `seed::body_words` with the two fields set.
- `state4s_bike_reads_back_for_its_postprocess`: `post_words(read_post)` is state 4 word for word
  (1,361 words).
- `ride_replays_bit_exact_with_rust_callees` and `flip_replays_bit_exact_with_rust_callees`: every
  row of ride-2 (459) and flip-1 (465, wrecked on row 125, put back on row 193) matches C15 word
  for word. Both interpreters' RAMs are identical after every frame outside the stack (the 256 KiB
  under `ee.stack` and 1 KiB over it), scratchpads included.
  - ride-2 answers SpatialUpdate, CollisionSync, NodePosition and Place 459 times each, and hands on
    Gather 459, PartMove 918, record sweep 459 and record search 459.
  - flip-1 answers the four 465 times each and the record's node point 470 times. It hands on
    Gather 465, PartMove 771 (the riding probe's every frame, the frame mesh's on 306), record
    sweep 465 and record search 465.

## Not verified / not 1:1

- **The probe searches and the database half of the collision sync are still the original's**
  (F46). This includes the "C15's world shown to file nothing" part of the brief: the record's
  sweep and part search run 0x1b9290, 0x114360 and 0x1151c0 in the interpreter, and the sync's
  moves and gather run 0x18d690 and 0x1d6a60. On C15 neither search ever hits. The nearby list
  holds 17–33 entries, though, and up to 6 lie within a part's reach. So the narrow phase does run
  and finds nothing; it is not empty, and proving "nothing" in Rust needs nat05's objects built.
  That was too large for this split.
- **Never reached on C15, so only by oracle or not at all** (F47):
  - The record's damping and node move/rotate/position never run in either replay; the oracles
    above cover them.
  - The body reset, respawn, out-of-world message and put-back slot are not ported. They are handed
    to the original, and C15 never calls them.
  - The node bounds and the spatial update's database box are ported in `quad::Steps` (MD75c) but
    never run on a database-less bike. `read_post` refuses a node in the database with a `+0x10c`
    bounds list (state 4's has one), because the scene can't hold the other nodes it gathers.
- `set_position` with a parent is not ported (`None`).
- The in-world test was Rust already (`atv_moto::in_world`, through `vehicle_post`); nothing new.
- The gate's `disc_worlds` went red on this branch's base: world_mxu_free was "built from cold: its
  start file is stale" after MD95's layout merged next to P299's start files. That is not this
  task's change. I copied the kept start in (`crates/stitch/worlds/world_mxu_free.start.json`);
  only the key moved (f06b0c54375601ea to 8a2309e7bba75399). Main has since re-keyed the same file
  (MD61, 35d567334c1588cb), so the merge keeps main's line.
- `./kit audit` reports four leaks already on main (`atv_wheel.rs:62,66`, `plan/DONE.md:519,533`),
  none in this task's files.

## Next

F46 and F47, then F31b5c/d. F31b5 is noted to need F46 and F47 as well.
