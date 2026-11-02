# 1 — world disc sun (FINAL)

Task V116. 2026-11-02.

## What I did

Reproduced V115's finding first (release, DISPLAY=:0, headless 960x540, real mods): free09 alone at
`--trace-at 1872,4500,180` vs world_mxu_free at `12816,9972,180`, mean of the ground band beside the
rider (rows 30–55 %, left third / right third): alone 91 / 123, world (Global) 71 / 105, world with
`--world-sky per-map` 87 / 123. The log under Global: `sun -0.760,-0.572,0.310 ambient
0.424,0.443,0.471`, free06's; under Per Map free09's `-0.572,-0.639,0.514 ambient 0.533,0.529,0.549`.
So the darkening and the shadow were all the setting: under Global (MD9, the default) each view took
`Lighting::of(&look)`, the sky tile's whole light.

Decided: Global means one *sky* (skybox and fog), not one sun. Every map's ground carries light
baked by its own sun — a disc map's light map (`<track>_shadow_*`, and the modulate pages baked with
its scene's `Lighting`, MD15/MD35), a mod map's pages — so lighting it with another map's sun darkens
the lit sides, lights the lee sides and throws the bike's and props' shadows against the baked ones.
Per Map still differs from Global in the skybox and fog. This applies to every map in a world, not
only disc maps: a mod map's pages are baked by its own sun too.

- `Looks::sunlit_at` (game-render `scene.rs`): `Looks::at`'s light (sun, sun colour, ambient, mixed
  across a join) with a whole-world lighting's fog, and the map whose sun it is.
- main.rs's per-view world look: Global takes `sunlit_at` with the sky tile's lighting; the
  `world sky: over …` line now says `sun of <map>` and is printed again when the sun's map changes.
- The World Sky setting's help text and `WorldSky::Global`'s doc say the sun stays the map's.

## What the original does

Nothing to port: the original has no worlds (MD8/MD9 are ours). Alone, a map is lit by its own
scene's sun, which is what a world now does under each map.

## How it was proved

- `world_look::global_light_takes_the_maps_sun_and_the_worlds_fog`: two maps with different suns;
  `sunlit_at` is `a`'s light over `a`, `b`'s over `b`, `at`'s mix across the join, always with the
  whole world's fog.
- `world_look::global_keeps_one_sky_and_per_map_takes_the_tile_below` now also asserts that under
  Global, over `b`, the line names `sun of b` and `b`'s ambient (fog stays `a`'s). `./kit check -- -p
  openmx --test world_look`: 10 passed; the shell-driven ones re-run with `--nocapture` and none
  printed "skipped".
- `crates/openmx/tests/world_disc_roads.rs` (ignored; user's mods, disc, GPU, release) gains
  `light_ratio`: world mean over alone's on the ground band, free09 must be within 5 %. Run by hand
  with DISPLAY=:0: **free09 0.983** (on HEAD's lighting it was 0.826), free06 1.060; paint RMSE free09
  0.0161, free06 0.0234 (V115's check, still passing).
- By eye (shots alone / Global before / now, side by side): the bike's shadow now falls to the right
  behind as alone; before it fell to the left.

## Not verified / not 1:1

- free09 is still ~2 % darker in the band than alone and free06's world shot ~6 % lighter than its
  alone shot. Not chased: the near ground is lit the same (`sun`/`ambient` lines equal); what is left
  is the world's sky tint and fog (free06's 97,93,92 at 352..768 vs free09's 150,151,139 at 374..1000)
  and the shot's other parts, which Global keeps the world's by design.
- Where a view's eye sits over one map and the bike over another (a join), the sun is the eye's map's
  mix, as Per Map's already was; not looked at across a real join on screen.
- Props and riders are lit by the eye's map's sun, not the sun of the map they stand on (same as Per
  Map before).
- Not run in a browser or on a phone; the change is CPU-side per view and adds one `Looks::at` a view
  a frame under Global (Per Map already did it).

## Next

none
