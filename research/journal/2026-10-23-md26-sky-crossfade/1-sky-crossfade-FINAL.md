# 1 — sky crossfade (FINAL)

Task MD26. 2026-10-23.

## What I did

- `game-render/src/gpu.rs`: the sky bind group has two face arrays (bindings 0 and 2) and a 16-byte
  uniform (binding 3): the second sky's share, and whether each sky is there. The sky shader samples both
  and mixes them; a missing sky is the fog's colour, as before. `sky_bind_group` is split into
  `sky_faces` (the upload) and the bind group itself. `Renderer::set_sky` keeps its meaning (one sky,
  bound twice, share 0); new `Renderer::set_skies(a, b)` binds two and `Renderer::set_sky_mix(share)`
  writes the uniform only, so it is cheap to call every frame.
- `game-render/src/scene.rs`: `Looks::skies(x, z)` gives the two maps with the largest
  `Regions::shares`, lower-numbered first, and the second's share of the two. Inside a map it is that
  map twice with share 0. Ordering by index, not by share, keeps the pair still across the whole join,
  so the textures are uploaded once on entering a join and only the share moves after that. `Looks::at`
  is unchanged; it still names the sky shown in the `world sky: over …` log line.
- `openmx/src/main.rs`: under Per Map the ride binds `Looks::skies`' pair when it changes
  (`shown_skies`), and sets the share every frame. Global, and Per Map off any region, bind the whole
  world's one sky as before.
- Nothing was taken from opentennis, which has one sky per court.

## What the original does

Nothing to port. MX Unleashed draws one map under one sky. A world of several maps is the port's own
(MD8), and how its sky looks is rendering, matched by eye.

## How it was proved

- `game-render` `gpu::tests::the_sky_crossfades_between_two_skies` (headless GPU; it ran here and did
  not skip): two solid skies at shares 0, 0.25, 0.5, 0.8 and 1 give the linear mix in every pixel, within
  1.5 levels. A missing second sky mixes toward the fog colour. One sky set with `set_sky` stays the whole
  picture whatever the share.
- `openmx/tests/world_look.rs` `per_map_sky_crossfades_across_the_join`: on MD9's two generated tiles
  the pair is `[0, 0]` inside `a` and `[1, 1]` inside `b`. Across the join, in half-foot steps, it stays
  `[0, 1]`, and `b`'s share only rises, never by 0.01 or more in a step. It is about 0.5 at the middle
  and above 0.99 where `b` takes over.
- `every_shader_translates_for_webgl2` still passes with the new bindings (3 sampled textures in the sky
  stage, well under WebGL2's 16).
- By eye: `world_mxu_free`, `--world-sky per-map`, `--shot-pose X,400,3000,90,15,70` for X from 6816 to
  7350 in eight shots at 320x224. free06's grey cloud sky holds to about 7100. Then free10's blue comes in
  over it, with both cloud layers faintly visible around 7200, and from about 7250 it is free10's sky
  alone. No frame has a step.

## Not verified / not 1:1

- **Where three or more maps meet**, only the two largest shares' skies are blended. The third's sky is
  left out, and when it overtakes the second the pair is rebound with a small visible step. Two
  bindings is what the brief asks for. In the stitched worlds the corners where three maps meet are
  small.
- **Re-binding uploads both skies** each time the pair changes, on entering or leaving a join. That
  happens at share 0 or 1, so nothing pops, but the upload is two six-face textures on that frame. Not
  measured for a hitch. The faces are 256 square, so it should be small.
- **Split screen**: the first view's camera picks the skies and the share for every view, as MD9 already
  does for the light.
- **The software renderer** (`Sky::draw`, used only for `moment` shots) still draws one sky. Worlds are
  drawn on the GPU.
- **By eye only on one join** (free06 → free10). Other worlds' joins were not shot.
- **The mix is linear in sRGB-encoded values**, not in linear light. It looked right by eye; not
  compared any other way.

## Next

Nothing.
