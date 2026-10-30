# 1 — bike tyre own textures (FINAL)

Task MD67. 2026-10-30.

## What I did

- **Found the cause.** I added a throwaway trace in `Assets::texture_of` to print every archive that answers
  `<material>.psm`. On `untamed_nat_sdca` the bike's tyre material `tire` came back as
  `["0:tex\red_tire.psm", "1:bike\tex\tire.psm"]`. The ride's archive list puts the track first, and
  `Res::find_suffix` falls back to a bare ending when nothing in an archive matches the whole name. So
  `tire.psm` matched the map's `red_tire.psm` before the search ever reached the bike's own archive. The tyre
  then got the map's `red_tire` texture, which draws as a yellow-olive tyre. It was not the map's light or
  vertex colour, and no texture in the map has the tyre's exact name.
- **Which other maps do it.** These maps ship a member whose name ends in `tire.psm`:
  - Every Untamed national and supermoto map with `red_tire`/`white_tire`: am_cs, nat_az, co, dvca, in, ma,
    md, nc, ny, or, pa, sdca, ut, sm_as, cf, hs, mb, of. These hit the bug.
  - `rusty_tire`, `bulldozertire`, `Progressive_tire`, `airstream_tire`, `nat01_trucktire` and
    `buggy_spare_tire`, on Untamed am_jy, am_gfuc, sm_s1, all sx_*, mxvatv_nat02 and sm03.
  - The worlds that hold these maps as tiles.

  Any of them could paint the tyre whenever its archive is searched before the bike's.
- **Fix (`crates/game-render/src/disc.rs`, `world.rs`).**
  - `Res::find_whole`/`read_whole` match a name only at a folder boundary.
  - `world::read_member` first tries every archive for the whole name, and only then falls back to a bare
    ending. `Assets::load` and `Assets::texture_of` both use it.
  - `Assets::own(map)` covers the time the ride's own models load. During it, the first `map` archives (the
    track's) are searched last. The material and texture answers it remembers are kept under their own keys
    (`own:`), so a map prop that loaded a `tire` of the map's own first is never handed to the bike, and the
    reverse is true too.
- **`crates/openmx/src/main.rs`.** The bike's chassis and parts, the rider's body, and the two places that
  repaint plates from the ride archives (another seat's parts) now load between `assets.own(1)` and
  `assets.own(0)`. `scene.rs` and `track_mods.rs` did not need changes. The importer is left alone, since
  the fix on the reading side covers every map already imported.
- opentennis: not looked at for this one. It has no track mods, and the bug is in this repo's own archive
  search order.

## What the original does

Not applicable. The original never loads another game's map beside its own bike archive. On its own
disc, its texture lookups are by whole name/hash.

## How it was proved

- Unit test `world::tests::a_track_mods_tyre_texture_leaves_the_bikes_own` (game-render). It sets up folder
  archives: a map with `tex\red_tire.psm`/`.pst` (yellow), a second map with its own `tex\tire.psm`/`.pst`
  (yellow) that a prop loads first, and a bike archive with `bike\tex\tire.psm`/`.pst` (black).
  - In both cases the bike's tyre comes out black.
  - The prop keeps the map's yellow `tire`.
  - With no whole name anywhere, the bare ending is still found as the last resort.
- Shots, `openmx --headless --track untamed_nat_sdca --mods ~/repos/openmx/mods --size 960x540 --overlay
  none --frames 40 --shot-frame 30 --shot-pose 2645,399,2559,-90,-10,50` (side) and `2636,399,2568,180,-10,50`
  (behind). They are in s3 `context/md67/before_{a,b}.png` and `after_{a,b}.png`, with crops `*_crop.jpg`.
  - Before: both tyres are yellow-olive.
  - After: both tyres are black rubber with knobs.
  - The log's `loaded:` line goes from 44 to 45 textures, the extra one being the bike's own `tire`.

## Not verified / not 1:1

- **Other imported maps are not shot.** I only rode West Junction. The other maps listed above are fixed by
  the same lookup but were not shot one by one: each shot takes a debug build ride, and the cause is the
  same file-name match.
- **Other loaders still use the old search.** Loaders outside `Assets` still search the ride archives
  track-first with `read_suffix`: bike FX, sky, splat, wreck parts' `.phy`/`.col`, and creatures. A map
  member whose name ends in one of their file names could still win. They were not part of this task, and
  no clash was seen.
- **Models of the same name are not separated.** `Assets::load`'s early return on a model already loaded is
  keyed by the bare model name. A map prop with the very name of a bike part's `.psg` would still stand in
  for the bike. I found no such name, so I left it unchanged.
- **Disc tracks are not shot.** The whole-name-first search also changes how the disc tracks' props resolve
  where a name was matched only by its ending in the track archive and whole in another. Only the gate's
  render tests stand in for that, and I took no disc-track shots.

## Next

None.
