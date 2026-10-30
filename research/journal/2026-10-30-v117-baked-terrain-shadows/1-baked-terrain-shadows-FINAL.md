# 1 — baked terrain shadows (FINAL)

Task V117. 2026-10-30.

## What I did

- `crates/game-render/src/terrain_shadow.rs` (new): under Shadows All, a map's hills are baked once
  more. From each texel of a 4096² grey map over the disc light map's own rectangle (1.4 ft a texel
  on a free-ride map, half the light map's 2.8; 2048² in a browser), the heightfield is marched
  toward the scene's sun (`Lighting::of(look).sun`, the same sun the GPU's sun maps use). The least
  clearance slope of the ray over the ground sets the lit share through a smoothstep of ±0.04, which
  softens the edge with distance to the occluder. Steps are half a sample near, growing with
  distance; the march stops at the field's top or 4000 ft.
- Tone: a lit texel keeps the light map's own value. Where the light map is more than 0.1 (13 levels)
  darker than a least-squares fit of it against our sun's Lambert term, it is lifted to that fit:
  those are shadows the disc's sun, or the disc's baked trees, cast and ours does not. Under All the
  props and plants cast through the live world map, so the lift also prevents double darkening.
  A shadowed texel goes to `min(lit, light_map_levels().0)`, the level the moving shadows darken to,
  so a bike's shadow on a hill's shadow adds nothing.
- Cache: `<mods>/cache/shadows/<track>-<xxh3 key>.grey`, an 8-byte magic plus the grey bytes. The key
  covers bake VERSION, size, sun, the field's heights and the light map's bytes, and an older bake
  of the same track is deleted. free09: baked in 2.0 s on 32 threads, read back in 0.25 s.
- GPU (`gpu/shadows.rs`): `set_ground_baked` uploads it next to the light map (`GpuGround::baked`).
  `ground_light` picks it under All only; `set_shadows` rebinds the ground's group when All comes or
  goes, so the shader is unchanged (same rectangle, same sampling).
- Shell (`main.rs`, `ground.rs`): `Ground::field` / `Ground::bake_hills`. The bake happens at load
  when the setting is All, and on the first switch to All mid-ride (blocking, a bake or a cache read).
  The software renderer reads `Ground::hills` in place of the light map when it was baked at load.
- Menu help for Shadows says All now bakes the hills' shadows too.
- opentennis: nothing to take (a tennis court has no terrain shadows).
- Filed V117a: the same for worlds (see below).

Findings on what the bake changes, found by fitting each disc light map's sun (Lambert correlation
over a 5°/10° grid):
- free09's light map is lit by nearly our sun (fit 35° elevation at azimuth 320°; ours 40° at 318°),
  and at 40° its gentle hills cast little beyond their own back slopes. Only 2% of its ground is in
  our shadow, mostly where the light map is already 64–95. Under All, free09 changes by up to
  8–21 grey levels in a few hills' lees, and the open ground does not change.
- free06's light map is lit from another side (fit 35° at 220°; ours 35° at 338°). There our bake
  darkens 5% of the map by more than 8 levels (patches up to 104) and lifts 18% where the disc's sun
  shadowed and ours does not. That is the map where All visibly changes the hills.
- free10: fit 35° at 30°, ours 39° at 35°; like free09.

## What the original does

Nothing like it: the original's only hill shadows are the disc's light maps. This is ours, matched by
eye (AGENT.md: rendering is ours).

## How it was proved

- `crates/game-render/src/terrain_shadow.rs` tests, on a synthetic ridge under a 45° sun. The lee
  is dark and over 40 levels darker than the same field flattened. The open ground keeps the light
  map's 140 within 2. Nothing goes below the shadow level. The ground past the shadow's reach is lit
  again. The edge is wider for a far occluder than a near one. A disc shadow our sun doesn't cast
  is lifted. A second bake comes from the cache, and another sun bakes afresh and replaces it.
- `crates/game-render/src/gpu.rs` `the_ground_reads_the_baked_hills_under_all_only`: the ground
  reads the bake under All only, swapping both ways, and a new ground drops it.
- `crates/openmx/tests/terrain_shadows.rs`: free09, `--headless --shot-pose 3617,700,3948,0,-89,40`
  under On and All with a scratch `--mods`. The hill's lee (pixels 288..352 × 192..256) is more than
  4 levels darker under All. Three open patches match On within 3 levels (measured 0 ±1). The second
  load says "read from the cache" and draws the same.
- The bake's orientation was checked against the light map: 0.985 correlation as placed, 0.14 with
  z flipped.

## Not verified / not 1:1

- Worlds (world_mxu, world_mxu_free and mod worlds) still use the disc light maps under All. A world's
  disc maps have their light baked into terrain pages at load (MD15), a different path. Filed as
  V117a, along with the one world sun (V116).
- Mod tracks with no disc light map (MX vs. ATV imports painted from pages) get no baked hills: the
  bake lives on the light map's rectangle. Their ground has no baked shadows to replace, so nothing
  regresses.
- Static props' and plants' shadows are not folded into the bake (the task's "optionally"). Under All
  they already cast through the live world sun map. Where the light map baked a tree's shadow and the
  lift removes it, only the world map's shadow of the tree remains.
- The "same spot with the hill flattened" comparison is proved on a synthetic ridge in the unit test,
  not on free09's own field: the shell has no switch to flatten a hill.
- On free09 the change is small (see findings), because its disc light map is already lit by nearly
  our sun. That is the result of using the GPU's sun as the task asks, not a missing feature.
  Darker hill shadows would need a shadow level below the light map's own, which On/All tone matching
  rules out.
- The software renderer reads the bake only when it was made at load. A switch to All mid-ride
  reaches the GPU only (the software renderer draws no other shadows, `shadow::drawn`), and a switch
  away mid-ride leaves the software ground on the bake.
- The first switch to All mid-ride bakes on the ride's thread: about 2 s once per map and sun, then
  the cache read.
- The browser build (2048², one thread, std::fs failing quietly into no cache) was not compiled or
  run: this machine has no wasm32 target installed.
- The disc sun fits above come from a temporary probe that was removed. Its numbers are recorded here
  only.

## Next

Nothing for V117. V117a carries worlds.
