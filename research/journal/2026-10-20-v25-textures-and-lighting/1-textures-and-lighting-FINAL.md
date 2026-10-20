# 1 — textures and lighting (FINAL)

Task V25. 2026-10-20.

## What I did

- **Freeride ground: the track's own paint.** New module `game-render/src/splat.rs` reads
  `terrainscreenspacemesh.psm`, which is a list of shader passes (a base layer, then
  `<layer>_mask`-gated layers, then a multiplied detail). The port evaluates each pass per pixel at
  world (x, z) with the GS blend `(A - B) * C / 128 + D` that the pass's `inputColorA/B/D`,
  `inputAlphaC` and `fixedAlphaValue` name. A pass's texture is tinted by `ShaderRGBA / 128`. Its UV
  is the world position turned by `Angle` and scaled by `UScale`/`VScale`, and `_animator_` sections
  with `Algorithm = UVAnimation` are read too. A pass whose texture is missing is dropped rather than
  painting magenta.
- **Freeride ground: the baked light.** The ground uses the 16 `<track>_shadow_NN.pst` tiles (512²
  grey, 4×4, covering the field less a 64/768 margin on each side) as a per-pixel light map. Where
  the map covers the ground, the sun is not applied on top of it. Free09's third row of tiles is
  stored backwards; `Shadow::assemble` finds that by seam cost against the row above and turns it
  round. `openmx`'s `ground.rs` draws with this when the track has it, and keeps the old dirt colour
  when it does not.
- **Sky.** New module `game-render/src/sky.rs` decodes the `.cub` named by the scene's
  `[Environment] CubeFile` (`<stem>.ps2.cub`). The file is a VIF stream of 96 packets: 6 faces × 16
  patches in a 4×4 grid. Each patch is a Gouraud strip, a `0x66` unpack of (u, v) bytes followed by
  a `0x6a` unpack of RGB. The port paints each face into a 256² texture and fills every pixel no
  geometry reached with the cube's colour along that pixel's ray. All 53 track skies decode. The shell
  and `--moment` draw it after the clear.
- **Materials.** `scene::Material` now also reads the first pass's alpha test and reference,
  `processingMethodWhenFailedInAlphaTest`, blend inputs, `zWriteDisabled`, `ShaderRGBA` and
  `TextureAnimation`. `Material::shading()` sorts the microprogram into three classes:
  - *Prelit*: the vertex colours are the light, so the sun is not applied.
  - *Diffuse*: sun and ambient times the vertex colour. This was the formula for everything before.
  - *ConstantColor*: `ShaderRGBA`.

  `world::Surface` turns a material into the cut-off, blend, additive and depth-write settings the
  rasteriser now takes through `raster::Look`/`Frame::triangle_with`. A material that says nothing
  keeps the old 32/255 cut-out and stays opaque.
- **Every material that lost its texture, and why** (`cargo run --release -p game-render --bin
  materials -- --all`):
  - Bike and rider liveries all resolve (`--bike cr125_faisst thq1`).
  - On the tracks, the untextured ones were all `.atm` animated textures: `raindrops`,
    `nat08_windowshadow1`, `nat08_awindowshadow1`, `nat12_water`, `nat12mud`, `flag`, `flagr`,
    `seats_red`/`seats_green`. Their `TextureMap_1` names no `.pst`. The pixels are in
    `tex\<name>.atm`: a 12-byte header (1, frame count, 0) and then that many whole `.pst` files.
    `world::texture_files` now tries `<texture>.pst`, then `<texture>.atm`, then the `.atm` named by
    `TextureAnimation` (`unique` meaning the material's own name), and `Texture::parse_animation`
    decodes frame 0.
  - The one left is `jumbo_pic`, the stadium screen. Its `TextureMap_1 = RenderTarget` means the
    game draws a live camera into it, so it has no file to find.
- Tests:
  - `world::an_animated_material_that_came_out_untextured_now_resolves`
  - `world::a_lit_triangle_comes_out_the_colour_worked_by_hand` (diffuse, prelit, constant)
  - `world::a_blended_texel_mixes_over_the_frame_by_its_alpha`
  - `scene::a_material_says_how_it_is_lit_and_laid_over_the_frame`
  - the four in `splat` and the three in `sky`
- From opentennis: nothing taken. Its look comes from Bevy's PBR pipeline on the GPU, and this
  renderer is a software rasteriser fed by the PS2 material files, so no code or approach carried
  over.

## What the original does

- The sky: the cube builder is `./kit fn 0x0011fa40`. It builds a ±25600 ft cube of 6 faces, 5×5
  corners each, in world axes. That is why the port's faces need no turn or mirror. The port
  centres the cube on the camera.
- The ground program: read from `terrainscreenspacemesh.psm` and the GS's own blend equation, not
  from the VU1 microcode that runs it.

## How it was proved

Looked right beside a screenshot: free09's start moment from the human's recording state
(`research/shots/side_by_side.py`, the original on the left, `openmx --moment` on the right).
Differences that remain, as mean RGB per crop of the 640×480 frame:

| Crop | Original | Port |
|---|---|---|
| near ground 180x90+420+250 | 87,85,67 | 94,91,75 |
| left ground 150x60+100+260 | 68,66,53 | 84,82,66 |
| far ridge 200x40+20+170 | 125,122,103 | 143,142,124 |

- The ground is a little brighter in the port, by 7 to 18 levels, and the hue matches.
- Before V25 the sky was the flat fog colour (149,150,138 against the original's 134,140,134). It
  is now the track's own clouds, and the cloud shapes line up with the original's.
- Trees and bushes are missing in the port (the ecosystem `.esb` is not loaded), and so is the
  rider (the moment has none). Both were missing before V25.
- The bike reads darker in the port. In the original the rider hides most of it, so the visible
  parts (rear tyre, fender) are about the same.
- Supercross (`first-draw --track sx01`): the stands' seats now carry their `.atm` texture, and
  nothing that was textured before lost its texture.

## Not verified / not 1:1

- **Supercross dirt.** The ground of a supercross track is still one colour. Its `.trn` colour map
  (a 1 MiB image and a 256-colour palette) is not decoded, because how a cell indexes into it is
  unknown.
- **Ecosystem foliage.** Trees and bushes (`EcosystemFile` `.esb`) are not drawn, because no reader
  for the format exists yet.
- **Extra shader passes.** Only a material's first shader pass is drawn. Specular maps, glossy
  passes and second texture maps (`TextureMap_2`) are ignored, because the second-pass blends are
  not worked out.
- **`jumbo_pic`.** It is a render target in the original. In the port it is plain white times its
  light.
- **The terrain blend.** The terrain's blend is read from the `.psm` and the GS equation, not traced
  through the VU1 program, so any rounding or clamping the VU does is not matched.
- **Free06's light map.** Its placement fits worse (seam score 0.42) than free09's. The margin
  (64/768 of the field) was fitted by eye on free09 and assumed for every track.
- **Row reversal.** Detecting a backwards light-map row by seam cost is a heuristic, not the
  original's rule; the file may carry a flag the port does not read.
- **Splat orientation.** The direction `Angle` turns a layer's UV in was chosen by eye.
- **Ground sampling.** The ground is not mipmapped, so it shimmers with distance where the PS2
  filters. The ground's per-pixel screen-space target resolution and its fog are not modelled the
  way the original's screen-space mesh does them.
- **`free09_d`.** It is listed in the scene but not used.
- **Vertex alpha.** `DiffuseAndVertexAlpha` water and mud are drawn without their vertex alpha,
  because `raster::Shade` carries no alpha.
- **`.atm` animation.** `.atm` textures show frame 0 only: no animation, and
  `animatedMapFramesPerSecond` is not read. UV animators on props (`uVelocity`) are not applied
  either.
- **Alpha test.** A failed alpha test with `processingMethodWhenFailedInAlphaTest = 1` (colour only)
  is approximated by not writing depth for the whole material. `alphaTestMethod` 5 (≥) and 6 (>)
  are both drawn as ≥. A material whose `.psm` names no alpha keys keeps the old 32/255 cut-out,
  which is a guess.
- **Prop fog.** `foggingOnOff` is ignored: every prop is fogged.
- **Diffuse lighting.** The diffuse formula (`(ambient + sun · lambert) · vc / 128`) is the port's
  guess at what `Vu1StripDiffuse` computes; the VU program was not read.
- **Prelit lighting.** `vc / 128` was chosen because the baked vertex colours (mean 103 on free09)
  only look right that way. That was not proved against the microcode.
- **Sky size.** The sky is drawn at infinity around the camera rather than as the original's
  25600 ft cube around the world origin. Near the field's edge the horizon could sit slightly
  differently.
- **Not compared in the emulator.** No new captures were taken for this task. The comparison is
  one moment on free09 plus a supercross still with no original beside it.

## Next

- Load the ecosystem foliage.
- Decode the supercross `.trn` colour map.
- Draw the second shader passes (specular).
- Animate `.atm` frames and UV animators with the shell's clock.
