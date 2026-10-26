# 1 — a lone terrain's put-back keeps no spot with no ground (FINAL)

Task MD22. 2026-10-26.

## What I did

- **Reproduced it.** `crates/openmx/tests/lone_map.rs` writes a track mod with one generated terrain:
  16 × 16 one-level cells at 2 ft, which is 510 ft a side, under `TerrainWidth=1`. It rides that map
  through the shell headless for 3000 ticks from the middle. I traced eight headings. At 270° the
  bike is thrown from x 148 on tick 67. The second throw's wreck comes down at x 512–513, past the
  field's last sample (x 510). `world::Ground::ground` answers `NO_GROUND` there, `World::keeps` was
  `true`, and the record kept the spot, so from tick 410 to tick 727 the bike sat at y −10⁶. The
  other seven headings never reached −10⁶.
- **Decided:** a lone terrain refuses only the spots its own ground query answers `NO_GROUND` for.
  It keeps every spot by an edge, as the original does. `Ground::keeps` is `Ground::owns`, a new
  private helper that `Ground::ground` now uses too, so the two can never disagree. `Place` passes
  `keeps` through to its ground. It does not take `Tiled::keeps`'s margin rule (refusing spots the
  edge test would throw from): the original keeps those, and on a disc map a put-back by the edge is
  the original's record.
- **Why this cannot move a disc ride.** The original's ground query has no "no ground" answer: off
  the field it leaves the point where it was (S64). `NO_GROUND` is this port's own answer. Before
  this change, any spot the new rule refuses would have put the bike back at y −10⁶, a state that no
  capture of the original can contain. So a refusal can only fire where the port had already left
  the original. On a disc map the edge test throws a riding bike long before it leaves the field
  (the scene's rectangle is one sample wider than the field, and the margin is 150 ft), and every
  throw lands well inside.
- Comments in `ride.rs` (`keep_back`) and on `World::keeps` say the rule is now every world's, not
  only a tiled one's.
- Took nothing from opentennis: it has no terrain edges or put-back.

## How it was proved

- `lone_map.rs`'s `a_lone_terrain_smaller_than_a_throw_never_puts_the_bike_on_no_ground` rides six
  headings (270, 0, 90, 180, 45, 315) for 3000 ticks each, 54 throws in all. On every tick the bike
  is within 20 ft of the field and above y 30 (the relief lies between 38.5 and 41.5). With
  `Place::keeps` put back to `true` the test is red: `270°, tick 410: at y -1000000 under the map
  at (513.566, 257.863)`. With the fix it is green.
- Disc replays: `./kit check -- -p game-sim --test replay` gave 87 passed, 1 ignored. The only skips
  were S68's `s68-backwards/{flat,slope}.csv`, which this slot does not have; they are about riding
  backwards on a flat and a slope, with no put-back in them. `platforms.rs`'s pinned hashes ride
  `Flat`, which keeps every spot as before. The full gate runs in `./kit done`.

## Not verified / not 1:1

- No original to match. A lone generated map smaller than a throw's arc does not exist on the disc,
  and `NO_GROUND` is the port's own answer, so the refusal is the port's rule and cannot be compared
  against a capture.
- "Disc replay hashes unchanged" is proved by the gate's disc tests passing, plus the argument above
  that a refusal needs a spot that already put the bike at y −10⁶. I did not ride every disc track's
  edges looking for a refused spot. `back_search.rs`'s put-back query tests skip in this slot (no
  `context/s66/free06.eeMemory.bin`), but they test the clearance query, which this change does not
  touch.
- Spots a foot or two short of the field's `x = 0` / `z = 0` edges are still kept. `Ground::ground`
  clamps the owner check there, and the field's query answers the probe's own height (MD14 noted
  the same in `Tiled`). The 0° ride is put back at z −1.4 and rides on from there. I left this as it
  was, because it is ground by the port's own query and changing it would change `ground`.
- The plane now sees it too. `flight::Listeners::post`'s `bare` is `!keeps && y < 0`, so on a lone
  map a thrown plane below y 0 off the field now counts as landed. Before, it fell for ever. On a disc
  map the plane is thrown from the edge before it can leave the field, so I expect no change there,
  but I did not ride the biplane off a disc map's edge to prove it.
- The bike on a small lone map still cycles through throws. The throw's arc overshoots it, so most
  wrecks come down off the field and are put back on the last kept spot, often by the edge it was
  thrown from (54 throws in 18000 ticks). That is the throw's arc (S59), the same as MD14's narrow
  world, and I did not change it.
- Seen on the way, not changed: MD8's generated tiles (`tests/tiled_world.rs` and others) write
  `TerrainWidth` as the count of 16-sample cells. A tile loaded *alone* as a track then has its
  edge-test rectangle 16× past its ground: `md8_b` alone was never thrown, and fell off the field
  for ever. Inside a world the rectangle comes from the tiles, so the worlds are unaffected. Filed as MD33.


## Next

Nothing for MD22.
