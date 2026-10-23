# 1 — world flight (FINAL)

Task MD14. 2026-10-23.

## What I did

- **Found the y −10⁶ put-back.** I rode MD8's 1722 × 572 ft world (`tests/tiled_world.rs`) with
  `--trace 3000 --trace-gas --trace-feel` from (800, 286) at six headings. Heading 270 reproduced it:
  thrown off the west edge at x 147 on tick 230, the bike flew across the whole world, the wreck bike
  touched down by the far edge, and `Ride::keep_back_thrown` kept the spot. The world's ground there
  said `NO_GROUND`, and from tick 575 the bike sat at (1721.8, −999997.9). The other headings showed
  a second fault: a throw loop. A bike thrown toward the narrow `z` side flies off the world and comes
  down on nothing. Its record still holds the spot by the edge, so every put-back is thrown again on
  its first tick (13 throws in 3000 ticks, all at the same spot).
- **What a put-back over no ground does in a world (decided):** the put-back record keeps nothing
  there. There is a new `World::keeps(at)`. It defaults to `true`, so one terrain keeps every spot as
  the original does. `Tiled::keeps` refuses a spot that no tile owns, and also one the edge test would
  throw from (within `off_map::MARGIN` of the rectangle's edge or of a hole). `Ride::keep_back` and
  `keep_back_thrown` ask it before keeping anything, and a refused spot leaves the whole record as it
  was, `up` included. A wreck that comes down on nothing is put back on the last spot the bike held
  on ground, inside the margin. A wreck that comes down on ground is put back where it landed, as
  before. The heading-270 ride is now put back at (1267.4, 39.5, 285.9). It is forwarded through
  `Land` and `eco::Wooded`.
- **The plane gets every tile's shapes.** `Tiled::collision` used to hand the plane's search no
  shapes. Now `Tiled::every_shape` reads each tile's shapes from the source once, places them in
  world feet in owner order, and keeps them. `Source::shapes` is a new defaulted method: it falls back
  to `load`, and the shell's `worlds::Files` reads the scene's collision meshes without the terrain.
  The set is built once and never streamed, because the plane's broad phase numbers what it gathered
  by place in the set (`FIRST + i`); a set that changed as tiles stream would aim old gathers at other
  shapes. The bike's own sweeps still go tile by tile through the loaded slots. The wreck bike reads
  `wooded()` too, so a world's first wreck now builds the set as well.
- **The plane over a hole** (the "ground-less sky"). `CrashContact` has two new fields that only a
  world sets, in `flight::Listeners::post`:
  - `holed`: `World::off_world(position, EDGE)`. The crash test throws the plane back from a hole as
    from the rectangle's edge.
  - `bare`: `!keeps(position) && y < 0`, meaning over no ground (or by an edge) and below every height
    a terrain can hold (heights come out of `u16`s and non-negative scales). A thrown plane with
    `bare` set counts as landed. Without it, a plane thrown into a hole never comes down on anything
    and falls for ever (the throw aims at the rectangle's middle, which can be the hole). I tried
    `!keeps` alone first: it fired mid-air at 408 ft near the hole and recovered the plane in the
    air, so I added the `y < 0`.
- Tests, in `crates/openmx/tests/tiled_world.rs`:
  - `a_narrow_world_ridden_through_its_edge_throws_never_puts_the_bike_on_no_ground`: three
    3000-tick rides at headings 270, 0 and 45. Every tick on owned ground within 8 ft of the relief.
    Every put-back that moved the bike lands inside the margin. Result: 34 throws, 1 moving put-back.
  - `the_plane_hits_a_shape_on_the_second_tile`: a 40-hull column (`biplanef.col`) on tile `b` at
    world x 900. Flown into it level from x 600, the plane's rays strike it and it crashes. With no
    column it flies past x 1000. I checked that the test goes red with `collision` handing `None`
    back again.
  - `the_plane_is_thrown_back_from_a_hole_and_recovers_from_falling_through_one`: a world of tiles
    `a` and `c` only, so x 504 to 1368 is a hole. The plane is thrown on tick 27 at x 354.4, falls to
    y −1130 through the hole, and recovers to its spawn on tick 394.

## What the original does

It has no worlds of several terrains, so none of this has an original to match. Its put-back record
(S66, S77) keeps any spot that passes the overhead test and the clearance ring. Its throw (S59, and
the plane's S37) aims at the terrain's middle with an arc for half its width. One terrain behaves
exactly as before: `keeps` is `true` and `holed`/`bare` are `false` there. The
`Crash::step_in` oracle test in `game-sim/tests/biplane.rs` passes them as `false`.

## How it was proved

Shell traces (`--trace-feel`, 3000 ticks) before and after, by hand for the −10⁶ case. The three
tests above, run through `./kit check -- -p openmx --test tiled_world`: all 8 tiled_world tests
passed, and the plane-prop test was red with the fix taken out. The full gate runs in `./kit done`.

## Not verified / not 1:1

- Nothing here has an original: what a world does with a put-back, a hole or the plane's shapes is the
  port's own rule. It cannot be compared against the disc game.
- The shell's camera panic on a NaN angle was **not reproduced**. Headless `--trace` and
  `--trace-camera` runs with the bike at y −10⁶ for 400+ ticks exited cleanly. The fix takes away its
  cause (no put-back on no ground), but I did not touch `main.rs`'s camera, and a camera fed some
  other non-finite position could still panic.
- One terrain still keeps a spot over `NO_GROUND`. `world::Ground::ground` answers −10⁶ off its field
  (S64's choice), so a small single generated map whose wreck comes down off the terrain can still put
  the bike on no ground. It was left alone on purpose, because changing it would change disc tracks'
  record and replay hashes. Disc maps are big enough that a throw lands inside them.
- In a world the throw still aims at the rectangle's middle and still overshoots a narrow world
  (S59's arc for half the width, whichever way the bike is thrown). Most of the 34 throws ended back
  at the edge spot, and only 1 put-back moved the bike. A ride along a narrow world's long edge
  therefore cycles: thrown, falls off, put back just inside the margin, rides out again.
- `bare`'s "below y 0" assumes every terrain's heights are ≥ 0. That holds for `u16` samples with
  non-negative height and vertical scales. I did not check it against every openmx-mods import.
- `Tiled::every_shape` reads every tile's collision meshes once and keeps them. On a big world (the
  ultra map) that is a one-time read of every scene's meshes on the first plane frame or first wreck.
  Its time and memory were not measured.
- The plane's spawn and `Flight::area` in a real world mod: the test world has no `machine.txt`, so
  the plane in a world was proved only in `game_sim` directly, not through the shell.
- `Tiled::ground` at negative `x`/`z` clamps the owner check but not the field query, which then
  answers the probe's own height. This is pre-existing and harmless now that `keeps` refuses those
  spots. I did not change it.

## Next

Nothing for MD14.
