# 1 — ragdoll near set (FINAL)

Task V112. 2026-11-02.

## What I did

- `ragdoll_ride::Down::step_posed_with` (crates/game-sim/src/ragdoll_ride.rs) took `world.wooded()`
  twice a frame:
  - once for the bones' gather, which on a regather runs `Statics::keys` over every prop;
  - once for `StillProp::near_wooded`, which reads the listed props.

  On a world of tiles that is every tile's shapes. Both calls now ask `world.wooded_near`:
  - **The gather** asks where the first sphere stood when it regathers, and at `search.centre` (where
    the kept list was gathered) when it keeps the list. Before any sphere has stood, it asks at the
    bike's sphere, and the gather then does nothing.
  - **The scene's props** are asked for at `search.centre`. After the frame's gather, that is the square
    the list was gathered in.

  A one-map world hands its whole set to `wooded_near` (`near_set` `None`), so nothing changes there.
- **Not forgetting, as the brief suggested.** V96's `near_world` forgets the plane's kept list when the
  probe reaches another square's set, which forces an early regather. The plane's journal says that
  can change the order candidates come in. Here the kept list is always read in the set it was gathered
  in, so nothing has to be forgotten, and the rider does exactly what it does over the whole set.
- Why that set holds everything the search can list or touch:
  - A listed prop's reach is under the bound, which is at most `max(30 ft, sphere radius)` from the
    centre.
  - A kept list's probe is within the bound of the centre.
  - The near set keeps every shape within 256 ft of the centre's 128-ft square, in the whole set's
    order.

  So the near set holds every prop the search can list or touch, in the same relative order. The
  tree's walk inserts props in index order. The props it leaves out are never listed, and listing,
  the capacity and the bike's place in the list count only listed props. So the gather lists the same
  props, the bike at the same place, with the same centre and bound.
- `Statics::props`' doc says which set its indices are in.
- Test `a_wrecked_riders_bones_ask_a_world_of_tiles_only_for_the_shapes_near_them` in
  crates/game-sim/tests/plane_world_cost.rs. It reuses MD34's posted generated world and V99's wreck,
  and takes the ragdoll kit the shell builds (`ride_kit`, copied from tests/ragdoll_trees.rs).
  - Eight throws, 150 frames each: the bike wrecked at 25–60 ft/s into the posts at z 618 and 658,
    with the ragdoll entered (reason 5) and stepped after the bike each frame.
  - Each throw is run on two worlds, a near-set world and one that hands the whole set to every
    search. This is done for 3 rows and again for 9.
  - Every frame, the rider's `Debug` and the bike's match to the digit between the two. The listed
    props are compared by their `to_world`, because the indices are places in different sets.
  - The near world is never asked for `wooded`, `collision` or `heightfield`.
  - Results: 2,384 frames listing posts and 851 frames with a kept list read across a square's edge
    (z 640), 263 of them with a post in a sphere's reach. The widest near set was 208 of 7,308 shapes.
  - 3 rows and 9 do not throw the same rider, even before this change: the bones' tree is laid over the
    whole terrain's box, and its cells clip the search's bound (30 against 29.67 at frame 1). So the
    test compares near against whole at each size, not 3 rows against 9 as V99's test does for the bike.
  - `Whole` (V99's whole-set wrapper) now passes on `wheel_surface`, `groups`, `crowded`, `keeps` and
    `moving`, which the bones' ground query reads. Before, it answered them with the trait's defaults.
- Controls:
  - **ragdoll_ride.rs as at HEAD:** the test fails at frame 0 ("asked for the whole world's `wooded`").
  - **Scene props read in the set at the probe instead of `search.centre`:** five of the throws part
    from the whole set at frames 12–22, on 3 rows and on 9. The last five starts were chosen for that.
  - **The gather's `at` always the probe:** passes. A kept list builds no keys, so the gather never
    reads that call's props.

## Timing on world_ultra (`--timings`)

The run was V99's with the trace point moved. world_ultra's mxu_free09 tile is now at (82224, 31536),
not V99's 83808, so S76's free09 point is at 86866.5, 35186:

```
openmx --headless --size 1920x1080 --mods ~/repos/openmx/mods --track world_ultra --controls original
  --trace-at 86866.5,35186,180 --timings --play <600 ticks of Cross held> --frames 600
```

The bike wrecks at tick 34 and again at tick 146. For the attribution I added temporary prints, not
committed:
- the sim stage's time each frame;
- `Crash::step`, `Down::enter_from` and `Down::step_posed_with`, each when over 1.5 ms;
- each bone regather's key count.

The two builds took turns, two runs each, at load 8–26.

| build | sim frames over 8 ms | what they were | doll step mean / max | regather keys |
|---|---|---|---|---|
| before, run 1 (load 26) | ticks 36, 148, 155, 166, 178: 13–29 ms | each a bone regather | 3.55 / 28.6 ms | 19,041 |
| before, run 2 (load 14) | the same five ticks: 12.0–12.4 ms | each a bone regather | 2.12 / 12.1 ms | 19,041 |
| after, run 1 (load 15) | none in the sim's own timer; `--timings` peaks 9.9 ms | — | 2.53 / 4.4 ms | 33 |
| after, run 2 (load 8) | none in the sim's own timer; `--timings` peaks 10.5 ms | — | 2.09 / 6.5 ms | 33 |

- **The peaks V99 left were the ragdoll's regathers.** Each one ran `Statics::keys` (a tree key per
  prop) over every tile's 19,041 shapes and put them all in a cloned tree. A wreck regathers when the
  first sphere leaves the bound, five times in the two wrecks here.
- **The `Down::enter_from` entry costs 0.05 ms.** It is not a cause.
- **The after-build's `--timings` sim peaks (ticks 101 and 201) are not the wreck.** The sim's own timer
  put those frames at 5.1 and 6.8 ms. Every stage of those frames was slow (stream 2.6–5.0 ms, ground
  4.3–5.7 ms, against about 0.5 and 2), and they fall on no regather or wreck entry.
- **What a wreck still costs.** The doll's step costs 1.5–2.3 ms on most frames in both builds. That is
  the bones' own physics, unchanged.

## What the original does

Nothing new. The original has one map, and its bones' search walks that map's tree. A world of tiles is
the port's own addition. The near set is a cut that does not change what the search finds (proved
above), not behaviour taken from the original.

## How it was proved

- **Bit-exact:** near set against whole set on a generated tiled world, 3 and 9 rows, eight throws ×
  150 frames, with the controls above.
- **One-map worlds:** `./kit check` on game-sim's ragdoll, ragdoll_props, ragdoll_trees, ragdoll_cars
  and ragdoll_riding tests: all pass. These take the same path as before by construction.
- **Timing:** as in the table.

## Not verified / not 1:1

- **The state hash's `search_prop` values change on a world of tiles.** They are places in the near set
  now, not in the whole set. Netplay peers both use the near set, so they agree. No pinned hash rides a
  world of tiles, but a hash recorded on one before this change will not match.
- **No world of tiles with plants was tried.** A world of tiles hands no plants (`Tiled::wooded_near`'s
  trees are `None`), and the plants' indices follow the set's prop count, which the near set changes
  consistently. Only a future tiled world with an ecosystem would exercise it.
- **The timing was taken at load 8–26**, not on a quiet machine. The before-build's regather frames are
  12 ms even in the quieter run, and none appear after, which is well beyond the noise. The absolute
  milliseconds are not a clean measure.
- **The bones' path cars are still asked for by `World::moving`.** On a world of tiles, though,
  `moving` is the trait's `None` (`Tiled` has no cars), so it costs nothing there.
