# 1 — database moves in rust (PART)

Task F46. 2026-11-05.

## What I did

- Measured what each call `posts_frame` hands on writes over ride-2 (the interpreter's `written` set,
  outside the stack), to size the task:
  - each **database move** (`> PartMove`, `./kit fn 0x0018d690`) writes 16 bytes, the record's last
    centre (`+0x10`), on every one of ride-2's 918 calls;
  - the **gather** (`./kit fn 0x001d6a60`) writes ~200 ranges a call: a halfword stamp in every object
    the quadtree query meets, a flag byte in each, the query's walk state in the tree and its pool, the
    +0xc4 list (`0xa8` bytes) and a global stamp;
  - the **sweep probe** (segment set `0x001b9290`, place `0x00114360`, near search `0x001151c0` on the
    probe at record `+0x204`) writes the probe's frame/box, its record, its segment tree's nodes and
    three globals (search counters);
  - the **part search** (`0x001151c0` on record `+0x200`) writes the part's record and, on 98 of 459
    frames, words in a candidate object (a pair test that runs and does not hit).
- That is four ports' worth of RAM plumbing, so F46 is split: **F46a** the moves (done here), **F46b**
  the gather, **F46c** the sweep probe, **F46d** the part search (`./kit plan F46b` …).
- F46a, in `crates/game-sim/tests/mxvatv_bike_ride.rs`:
  - `DbInRam::read` reads the database an entry is in out of RAM as `atv_spatial` models it: the
    record's cell and every cell its links reach (with their portals), each cell's sub-databases (a
    list slot by slot; anything else, here the quadtree, as `Sub::Other { kind }`, which the move
    refuses loudly if it ever needs it), the link pool off its global, the record and its extras.
    Entries are items numbered as met, the moved one 0.
  - `DbInRam::write` writes the outcome back where it changed: the record's cell, sub-database, place
    and centre, every list's changed slots, its handed-out count and free chain. What it cannot write
    (a new list block, the pool, the extras, the cells) it asserts did not change.
  - `spatial_move_in_ram` = read, `atv_spatial::spatial_move`, write. `PostsInRam::steps` answers a
    `PartMove` with it when the thread-local `IN_RUST` names `PartMove`; the count then has no `> `.
    `IN_RUST` is the knob F46b–d add their names to (`Gather`, `record sweep`, `record search`).
  - `replay_in_rust(name, names)`: C15's replay by `posts_frame` twice, the first handing everything on,
    the second with `names` Rust's; every row of the second against C15 and the two RAMs alike after
    every frame outside the stack.
- Nothing taken from opentennis: its world has no spatial database of this kind. The reader is
  `tests/atv_spatial.rs`'s `Reader` (MD75c1), cut down to a list-only, walk-from-the-record form.

## What the original does

State 4's database, as the move sees it: the riding probe's entry (`0xea63b8`) and the frame mesh's
(`0xea5988`) are both in nat05's one cell (`0x672cd0`, no links), in its second sub-database, the list
of moving entries (vtable `0x003b5510`, kind 2, one block of 1024 slots, 141 handed out); neither has
an extra cell. The cell's first sub-database is the quadtree the gather reads. With no portal the walk
never leaves the cell, so every move is the list's same-slot shift and the centre kept.

## How it was proved

- `state4s_database_reads_back_word_for_word`: both entries read (one cell, no portal, two
  sub-databases, the list's shape, no extras) and written back unchanged leave state 4's RAM as it was.
- `ride_replays_with_the_database_moves_in_rust`: ride-2, 459 rows word for word against C15 and RAM
  alike with the handing run after every frame; 918 moves Rust's.
- `flip_replays_with_the_database_moves_in_rust`: flip-1, 465 rows, wrecked on 125 and put back on 193,
  RAM alike every frame; 771 moves Rust's (the frame mesh out of the database on 159 frames).
- The check bites: with the write-back dropped, ride-2 row 0 fails, RAM differing at the two records'
  centres (`0x1efa360`, `0x1f3e4e0`).
- `spatial_move` itself is MD75c1's port, oracle-proved over 12,000 generated databases
  (`tests/atv_spatial.rs`).

## Not verified / not 1:1

- **Only the same-slot shift runs on C15.** nat05 has one cell and no portal, so the walk, a move into
  another cell, the overlaps and the link pool never run here; neither does a list's take-out / put or
  a new block. Those paths are MD75c1's oracle's (generated databases), not this replay's; and the
  write-back of a new block, the pool and the extras is not written at all — it asserts they did not
  change, so a track that needs them fails loudly rather than silently.
- **The quadtree sub-database is not read** (`Sub::Other`): the move never touches it here (the
  entries are in the list). An entry filed in the tree would panic in the port's `other`; reading it
  is F46b's (the gather reads that tree).
- The database is re-read from RAM on every move rather than held in Rust between frames: the
  lockstep harness keeps RAM as the shared state while the gather and the searches are still the
  original's.

## Next

F46b: the gather in Rust. Read nat05's quadtree (`tests/atv_spatial.rs`'s `read_tree` shape: nodes,
chunks off the tree's pool) and the collidables the query meets out of RAM, run `atv_world::gather`, and
write back what the measurement above lists (every met object's stamp halfword and flag byte, the
tree's query state, the +0xc4 list, the global stamp). Then F46c and F46d. Add each name to `IN_RUST`
and assert it in `replay_in_rust`, as the F46a tests do.
