# 1 — sim thread in worlds

Task F25. 2026-11-16.

## What I did

- **The picture's copy of a world** (`crates/game-sim/src/tiles.rs`):
  - `Tiled::picture_copy()` builds a second `Tiled` over the same tiles and wraps. Its source reads
    nothing (`Unread`), so it never loads a tile itself.
  - `Tiled::copy_into(&mut copy)` copies the slot table: each loaded tile's terrain bytes, shared; the
    tiles out stay out, the failed stay failed. It clears the copy's asked marks and log, and copies the
    near-set cut count (the first frame's `--timings` line reads it).
  - To share the bytes, a loaded tile's terrain is now `Arc<Vec<u8>>` (it was `Box<[u8]>`). `Arc::new`
    of the source's `Vec` does not copy it.
  - The copy holds no shapes. Only the tick asks for them (sweeps, near sets, moving shapes).
- **The ride loop** (`crates/openmx/src/main.rs`):
  - The place a held picture reads (`drawn_land`) is, for a world, `Land::Tiles` of the copy, made once
    after the load. Before, it was `None` for a world, which kept every tick on the loop.
  - When a tick goes off to the thread, the copy is brought up to date from the real world first. Tile
    streaming runs in the tick's second half on the loop, after the world is back, so the copy is never
    read while the real world changes.
  - `--trace-hash` now also says, at the end, how many ticks ran on the thread
    (`sim thread: N ticks ran there`), so a test can tell the thread was used.
  - The `--sim-thread` help and the `Mode` doc no longer say "not in a tiled world".
- I did not need to change `world_dress.rs`. The dressing's draw reads ground heights through the place
  it is handed (`car_ground`), which is now the copy. Its pieces are taken and dropped in the tick's
  second half, on the loop, as before.
- `../opentennis` has no sim thread and no tiled worlds, so there was nothing to take from it.

## How it was proved

- **A world ride, tick for tick** (the brief's `--trace-hash` comparison). This was world_mxu_free from
  the main checkout's install (symlinked, read only), with `--players 2 --steady-clock --trace-hash
  --headless`. Seat 0 played a 1800-tick tape: idle for 30 ticks, then Cross held. Seat 1 stood idle.
  - With `--sim-thread on`, 1798 of the 1800 ticks ran on the thread (not the first, and not the last).
  - All 1800 `tick N hash` lines, both seats, are byte-identical to the run with `--sim-thread off`.
    The ride hash is 0x07f4dae081b087c4 in both.
  - Seat 0 ended 3,600 ft west of the start, after a wreck (reason 5) and its get-up. The same 39
    `world: … loaded/dropped` lines came in the same order in both runs, so tiles streamed while
    ticks were on the thread.
  - Both players' end positions are the same. Their triangle counts differ a little (522195 against
    521913): the held picture is the last tick's end, not a blend (N23a).
- **The software renderer** (`--renderer software`, which draws a world's ground through
  `world_dress::draw_ground` over the copy): the same ride for 300 ticks, 298 on the thread. All 300
  tick-hash lines match the run with the thread off, ride hash 0x2a0e9f2ade539ac6 in both, and the
  triangle counts are within a few (55590 against 55600), as on the GPU.
- **A gate test**: `crates/openmx/tests/tiled_world.rs`
  `a_worlds_ticks_on_the_sim_thread_are_the_same_ride`. It runs a two-player md8_world ride (300 ticks
  on the gas, 60 at rest), thread off and on. It checks all 360 tick-hash lines, the ride hash, and at
  least 300 ticks on the thread. A temporary panic showed it really ran: 360 ticks the same, 358 on the
  thread.
- **The copy itself**: `crates/game-sim/tests/tiles.rs` `a_pictures_copy_answers_what_the_world_had_loaded`.
  Over 400 points the copy's ground is bit for bit the world's, and it reads no terrain. After the world
  drops a tile, the copy keeps it until `copy_into`, then has no ground there and does not read it back.
  Once the world reads the tile again, the next `copy_into` gives it back to the copy.

## Not verified / not 1:1

- **A tile the picture asks about that is not loaded has no ground in the copy.** On the loop, the
  picture's question would have loaded it. This touches only the picture: a start or marker portal's
  height, the sun flare's sky test, and a dressed car's ground far out. The tick's tiles are the ones
  near the riders (6000 ft load reach), so in practice the picture asks about loaded tiles. I did not
  find a case where it matters, and I did not look for one on screen.
- **The picture no longer marks tiles as asked.** On the loop, a tile the picture asked about stayed
  loaded through the next stream. With the thread, only the tick's questions keep a tile. The module
  note says streaming never changes an answer, and the hashes held over the world ride, but the set of
  tiles in memory can differ by a tile in some ride.
- **The GPU ground pieces and the paint were built a tick or two apart** between the two runs (for
  example j004_002 at tick 680 against 681, a paint at 1149 against 1167). Those builders run on wall
  time, so two runs with the thread off would differ the same way. I did not run two threaded-off rides
  to show that. One `world sky` line differed in its sun too, which is the picture's camera at that
  moment.
- **Not tried windowed, and not on a phone or a Deck.** Only `--sim-thread on`, headless. Whether
  `auto` engages in a world on a slow device was not measured (R20 covers the Deck for N23a).
- **Not tried:** a wrapped world crossing a seam with the thread on (world_mxu_free does not wrap),
  the plane, helicopter, car and quad in a world, and three or more players.
- **Cost of `copy_into`:** a slot table walk and an `Arc` clone per loaded tile, each threaded tick. I
  did not time it; for world_mxu_free's 57 pieces it is a few hundred bytes. A world of thousands of
  joins walks thousands of slots each tick.

## Next

Nothing. F25 is done.
