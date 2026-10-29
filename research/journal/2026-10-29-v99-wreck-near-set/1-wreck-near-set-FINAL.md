# 1 — wreck near set (FINAL)

Task V99. 2026-10-29.

## What I did

- `Crash::step` (crates/game-sim/src/ride.rs) hands the wreck body's search `world.wooded_near(bike
  origin)` instead of `world.wooded()`. `WreckBike::tick_among` makes a fresh nearby list and a fresh
  broad phase from every prop it is handed, each frame. On a world of tiles that was every tile's
  shapes, built every frame of a wreck. The body's rays reach a few feet, and the near set keeps every
  shape within 256 ft (`NEAR_MARGIN`) of the 128-ft square, so the search finds the same shapes. Unlike
  the plane's search (V96, `flight_search::near_world`), nothing is kept across frames. So there is no
  list to forget when the square changes.
- Test `a_wrecks_frame_asks_a_world_of_tiles_only_for_the_shapes_near_it` in
  crates/game-sim/tests/plane_world_cost.rs (it reuses MD34's posted generated world, its question
  counter and the disc's `wreckbike.phy`). A wreck at 40 ft/s, spinning, 3 ft over the ground, runs into
  a post. It is stepped 150 frames on three worlds:
  - 3 rows of tiles;
  - the same world 9 rows deep;
  - 3 rows that hand the whole world's set to `wooded_near` (the search before this task).

  Checks every frame:
  - The `Crash`'s `Debug` text matches to the digit across all three worlds. The one exception is the
    sweep back's `struck` index: it is a place in whichever set the search was handed (135 in the near
    set, 885 in the whole one at frame 6), so the test compares it by the struck prop's `to_world`.
  - Both worlds are asked the same questions.
  - Neither world is ever asked for `wooded`, `collision` or `heightfield`.

  Results: 141 of 150 frames touching. The widest near set was 199 of 7308 shapes. With ride.rs put back
  as it was, the test fails at frame 0 ("asked for the whole world's `wooded`").

## Timing on world_ultra (`--timings`)

The run: release build, `--headless --size 1920x1080 --mods ~/repos/openmx/mods --track world_ultra
--trace-at 88450.5,35186,180 --timings --play <600 frames of full throttle> --frames 600`.

The spot: S76's teleport point in free09 (4642.5, 3650), moved into world_ultra's mxu_free09 tile at
(83808, 31536), facing 180°. `--trace` says the bike wrecks there at frame 33. In the session it wrecks
twice: it is put back after the first wreck and wrecks again.

The two builds took turns, two runs each. The load average was 17–23 (not a quiet machine).

| build | sim, per-second average through the wreck seconds | sim's worst frame | session sim | session sim peak |
|---|---|---|---|---|
| before (`wooded()`), run 1 | 99–419 ms | 580 ms | 17.75 ms | 580 ms |
| before, run 2 | 20–47 ms | 71 ms | 6.07 ms | 71 ms |
| after (`wooded_near`), run 1 | 2.9–4.8 ms | 24.8 ms | 1.97 ms | 24.8 ms |
| after, run 2 | 0.9–1.4 ms | 13.2 ms | 0.73 ms | 13.2 ms |

Riding with nobody down, sim is 0.5–2.5 ms a frame in both builds. A wreck no longer costs a whole world.
One thing is left: a single frame of 13–25 ms around each wreck (2–3 a run). It is not attributed (see
below).

## What the original does

Nothing new. The original has one map, and its wreck body searches that map's tree. A world of tiles is
the port's own addition. The near set is a cut that does not change what the search finds (proved
above). It is not behaviour taken from the original.

## How it was proved

Bit-exact replay on a generated tiled world: near set against whole set, and 3 rows against 9.
`a_wrecks_frame_asks_a_world_of_tiles_only_for_the_shapes_near_it`, 150 frames. Timing as in the table.

## Not verified / not 1:1

- **The ragdoll's bone search still takes the whole world's set.** `ragdoll_ride::Down::step_posed_with`
  calls `world.collision()`, which is every tile's shapes. It runs `Statics::keys` over all of them on
  each regather, and `StillProp::near` over all of them. Its `statics` names props by their index in that
  set across frames, so changing it to the near set needs the same forgetting V96 gave the plane. That is
  more than this task asked for. Filed as V112.
- **The 13–25 ms single-frame sim peaks around each wreck are not attributed.** They show up 2–3 times a
  run in the new build. My guess is the ragdoll's regather over the whole set (above) or the wreck's
  entry, but I did not profile it.
- **The timing was taken at load 17–23**, not on a quiet machine. The two builds' numbers are an order of
  magnitude apart, more than the noise, but the absolute milliseconds are not a clean measure.
- **`struck` is compared by the prop's transform, not by its index.** The index means something different
  in a near set than in the whole set. That is search bookkeeping, and nothing in the simulation reads
  it as a number across sets.

## Next

V112 (filed): the ragdoll's whole-set bone search, and the peaks left.
