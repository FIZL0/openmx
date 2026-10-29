# 1 — shadows (PART)

Task V108. 2026-10-28.

## What I did

- `crates/game-render/src/shadow.rs` (new):
  - The `Shadows` setting, with its labels and words.
  - The original's numbers: the 128x128 map, the view-count ranges, the fade, and the `Tint` read off
    the scene's sun.
  - The sun's box for a shadow map (`light_box`: reversed depth, drawn relative to the view origin as
    MD15 has it), sphere-snapped cascade slices, and grouping actor spheres into racers.
  - The light maps' shadow levels (`light_map_levels`), from the paint's shadow tiles: the 5th percentile
    of red over the median.
- `scene.rs`: a light's `ShadowRGB` / `ShadowIntensity`, and `Scene::shadow_tint` from the first
  directional light.
- `crates/game-render/src/gpu/shadows.rs` (new): `SunMaps`, a depth texture array of up to 8 layers per
  view and its comparison sampler.
  - Each frame `plan_sun_maps` lays out the maps:
    - **Original:** one 128x128 map per racer, meaning the actors whose spheres touch (radius ≤ 20 ft,
      not ghosts), up to the 1P/2P+ range.
    - **Better:** two cascades along each view's look (0–45 ft and 36–300 ft, the fog's far end if
      nearer), at 2048² (1024² for more than two views, and on wasm).
  - A depth-only caster pipeline draws into them. Its instance list is built from the same walk the view
    draws: props (all of them, even off screen), actors, and plants after the cull. Only depth-writing,
    non-additive materials cast.
- `gpu.rs`: the view uniform's padding now carries the maps (mode, count, tint, per-map layer data,
  matrices).
  - Mesh shader, Better only: prelit meshes multiply toward the light maps' shadow level, and lit meshes
    lose their sun term.
  - Ground shader (`shadowed`):
    - **Original:** mixes toward the fogged tint at the strength.
    - **Better:** darkens to the light map's own shadow level (the ratio of the shadow level to the baked
      value where a light map exists, ambient over sunlit where not), with fog kept.
  - The 3x3 PCF in `sun_hidden`. The WebGL2 translation test covers the new bindings. In GL, an array
    with 1 or 6 layers changes type, so the layer count is rounded up to a power of two of at least 2.
- Menu (`menu.rs`, `menu_pages.rs`): the Shadows row, its help, and the Original master switch, which
  sets it to Original and back to Better. `main.rs` applies `--shadows`, the menu value and live changes.
  It prints `shadows: <mode> (the original's in r,g,b at a)`, or that the software renderer draws none.
- Test `crates/openmx/tests/shadows.rs`:
  - free06's start, shot from 34 ft straight above with each setting. Original and Better each have
    over 3,000 pixels more than 20 grey darker than Off's, and fewer than 300 brighter. Run: about 9,600
    and 11,800 darker.
  - Original's core is the blend of Off's pixels toward `ShadowRGB` at the intensity squared, within 8
    grey.
  - Original's core leaves 0.63 ± 0.07 of the dirt, PCSX2's measured share.
- `crates/openmx/tests/menu.rs`: the Settings literal, the Picture group, and a test that the default is
  Better and that the Original switch sets Original. Turning Shadows back from Original steps to Off
  (the row goes Better, Original, Off), which counts as "ours".

## What the original does

These are read with `./kit fn` and `./kit data` on boot. The addresses are in research/oracle.txt's V108
block.

- `shadows.res` holds `shadowcasterN`, `terrainshadowreceiverN` (with a lod) and `shadowreceiverN` .psm
  files, for N = 1..8. That makes eight projected shadows at most.
- The caster draws the model into a 128x128 target cleared to 0, blurred once (`filterTexture 1`,
  `filterPasses 1`). The receivers lay it with `(Cs-Cd)*As+Cd`, clamp, alpha test ref 1 and no z
  write: a colour pulled toward the shadow colour, over the ground.
- The light reader (`./kit fn 0x001a81b0`) reads three keys:
  - `CastsShadows` (default 1).
  - `ShadowIntensity` (default -1). Above 0 it sets a global to intensity × 128, which otherwise stays
    85.
  - `ShadowRGB` (default "none", which is black).
  
  free06's and free09's sun is 25,25,25 at 0.7.
- The draw range by view count is set at `./kit fn 0x00250c60` from a table: 1P draws to 100 ft and
  fades over the last 40; 2P and more draw to 65 and fade over 10.
- ProjectedShadow's methods (the vtable is in oracle.txt):
  - It makes its target lazily.
  - It culls by camera distance.
  - It takes the caster's bounding radius times a scale field as its map's extent.

## How it was proved

Looked right beside a screenshot, plus one measurement.

- PCSX2's picture of free06's start comes from V77 (`context/shots/v77/side_all.jpg`, left column; copy
  2, git-ignored). It shows the rider's shadow behind the bike, soft-edged. Its core is 74 grey on dirt
  of 104–121, about 0.63 of the dirt.
- A plain blend at 0.7 would leave 0.3 + 25·0.7/117 ≈ 0.45. The intensity squared (0.49) leaves 0.62,
  so Original uses alpha² (`SunMaps::fields`). With it, the shell's Original at the start reads 74 in
  the core (`ride_original2.png` in the session scratchpad).
- The edge was a one-texel 3x3 at first, visibly crisper than PCSX2's. The filter now steps two texels.
- Better, by eye from above the start (`--shot-pose x,y+34,z+2,0,-85,60`): the bike's full silhouette,
  the bushes' shadows, and the bike's shadow a little darker than Original's.
- Tests: `crates/openmx/tests/shadows.rs`, the menu tests, the shadow.rs unit tests, and game-render's
  WGSL/WebGL2 translation test.

FRAMETIME

## Not verified / not 1:1

- **The PCSX2 comparison is not at the same frame.** It uses V77's shot (a teleported free06 bike
  state, with the camera behind the bike), not a new shot of the test's top-down pose. The original's
  camera cannot be posed there. Only the core's share of the dirt and the edge's softness, by eye, were
  compared; the shadow's shape and offset were not.
- **The intensity squared is fitted, not read.** One shot fits it. I did not find where the second
  factor comes from in the code (a caster alpha of intensity × 128, and the receivers' vertex alpha, is
  the guess).
- **The fade with height is a guess.** Our Original's map reaches 40 ft under the caster's sphere and
  fades over its last 40%. The original's depth clip and height fade were not read.
- **The map's extent is not the original's.** It uses the caster's sphere as is. The scale field the
  original multiplies the radius by was not read.
- **The original's blur is approximated** by a 3x3 PCF at a two-texel step. Its filter kernel was not
  read.
- **The distance fade's curve** is assumed linear over the range's fade band.
- **Which actors cast in Original** (radius ≤ 20 ft, ghosts left out) is our rule, not read. So is the
  limit of eight maps per view beyond `shadows.res`'s eight.
- **Slopes:** the shadow is projected along the sun onto whatever ground is under it, so it lies on
  slopes as a projection does. The original's receivers were not compared on a slope.
- **What receives in Original:** only the ground. The original's `shadowreceiverN` suggests that props
  marked to receive also take it; that is not done.
- **Better's darkness** is matched to the light maps' 5th-percentile/median levels. The two were only
  compared by eye at free06's start, not measured across tracks.
- **Plants** cast only when the view draws them (after its cull), so a plant just off screen casts
  nothing into view.
- **The software renderer draws no shadows.** The brief allows Original's "at most"; none were ported
  there.
- **The browser (P48/P205)** was not run. The WebGL2 shader translation test passes, and wasm uses
  1024² maps, but nobody measured the browser's fps.

## Next

Nothing.
