# 1 — ground screen space meshes (FINAL)

Task V67. 2026-10-23.

## What I did

- `scene.rs`: a model whose `Type=` is `TerrainScreenSpace` (any case) goes to `Scene::ground_meshes`,
  not `Scene::models`, so it is no longer a prop buried at height 0. Shadows were already drawn by name
  from the `.pst` tiles (`Splat::shadow`) and still are; their 16 `<track>_Shadow_NN` placements are in
  `ground_meshes` too but `ground_mesh::is_light_map` skips them.
- New `game-render/src/ground_mesh.rs`: `GroundMeshes::load` reads each placed `.psg`, each material's
  `.psm` (first pass, through `Splat::parse`, so texture/blend/colour come out the same way as the
  splat's), and lays every triangle flat in world x/z with uv, vertex colour and pass, in scene order,
  with a 32 ft lookup grid. `paint(x, z, frame)` runs the triangles under the point through their GS
  blends as the splat does; `affine(x, z)` gives the stack as `frame * keep + over`.
- `splat.rs`: `Splat::load` reads `<track>.scn` and sets `Splat::meshes`; `Splat::paint` runs them after
  the passes. That covers the software renderer, `world_dress`'s baked splat pages (worlds), and anything
  else that calls `Splat::paint` — `crates/openmx/src/ground.rs` needed no change.
- New `game-render/src/gpu/ground_mesh.rs` + hooks in `gpu.rs`: the meshes drawn from above into two
  `Rgba8Unorm` targets holding `over` in RGB and `keep` in alpha (clear (0,0,0,1); blend colour
  SrcAlpha/OneMinusSrcAlpha, alpha Zero/OneMinusSrcAlpha), each mip level drawn directly. Far: 2048²
  over the meshes' bounds, drawn once at `set_ground_*`. Near: 2048² at 4 texels/ft (512 ft) round the
  first view's eye, redrawn when the eye crosses a 64 ft square (`render`). The ground shader (splat
  only, not pages/world paints) samples both and does `frame.rgb * keep + over * 255` after the passes,
  before the light map. Two more sampled textures: the device-limit check counts them, and
  `every_shader_translates_for_webgl2` translates a meshed ground with `max - 3` layers and the mesh
  drawing shader.
- Test `crates/game-render/tests/ground_meshes.rs` (disc; skips without it): on free06 and free09 every
  `Type=TerrainScreenSpace` line of the `.scn` is a ground mesh and none is a prop; at the road mesh's
  most-covered triangle centre the CPU paint moves to the road's own colour (free06 163,136,101 →
  111,99,83, road 106,95,81; free09 143,141,121 → 95,95,85, road 88,88,80), a point ≤120 ft away no
  mesh reaches is unchanged; a headless GPU frame looking down agrees (free06 on 117,103,87 vs expected
  121,108,92, off unchanged 171,143,107; free09 on 110,110,101 vs 113,113,100, off unchanged).

Data seen: screen-space models per scene — free06 242 (Ruts/Rut_*, Road01, Erode, Shadow_01..16),
free07 249, free09 253 (12 road pieces), free10 236, free11 17 (16 shadows + 1). All meshes have y = 0;
vertex alpha fades their edges (road 0..96); textures 128x256; every material is `Vu1StripPrelit`, one
pass, modulate `[0,1,0,1]`, wrap 0. Shadow placements are 1440 ft squares, consistent with the existing
`Shadow` margins.

## What the original does

- The string `TerrainScreenSpace` (`./kit xref 0x42b1e0`) is read by the model-type factory
  (`./kit fn 0x1a35f0`), which for it constructs a TerrainScreenSpaceStaticModel (`./kit fn 0x1aa3e0`,
  vtable 0x44fe90).
- `./kit fn 0x1d9930` loads the terrain's five psms: TerrainScreenSpace (+0x8cc), UseScreenSpace (+0x8d0),
  Mesh (+0x8d4), Prologue (+0x8d8), Epilogue (+0x8dc).
- Order, deduced from the data rather than traced through the draw loop: the prologue psm targets
  `RenderTarget` with camera `TerrainScreenSpace`, a 512x832 target and `clearTargetBeforeUse=0`; the
  splat program's first pass has no blend (it overwrites), so the meshes cannot go before it; the shadow
  tiles are screen-space models too with blend `Cd * As` `[1,2,0,2]`, which only makes sense over a
  finished paint; the epilogue has `TotalShaderPasses=0`. So: the whole terrainscreenspacemesh.psm
  program (detail pass included), then the screen-space models into the same target.

## How it was proved

Looked right against the textures, not against the original: the test above checks the colour the port
computes from the disc's own textures/vertex colours/blends lands on the road and nowhere else, on both
renderers. No PCSX2 screenshot was compared.

## Not verified / not 1:1

- Order of the meshes against the detail pass and among themselves: deduced from the psm data (above),
  not traced in the draw loop. Among models the port uses the scene's order.
- In the original the shadow tiles are drawn in scene order with the other meshes (free06: 540–555, the
  road at 556), so there a road placed after them may be unshadowed; the port lights everything with
  the light map after the meshes, as before.
- The original's 512x832 screen-space target (its resolution, its camera) is not modelled: the GPU uses
  a 512 ft near cascade at 4 texels/ft and a far one over the whole mesh rectangle (≈2.8–3.4 ft/texel);
  the CPU paints per point exactly. Split-screen views other than the first see the near cascade only
  where it happens to reach, the far one elsewhere.
- GPU draws only modulate (`[0,1,0,1]`) meshes — every one on the disc; the CPU runs any blend.
- Texture wrap/clamp per material ignored (all are wrap 0 on the disc; GPU uses repeat).
- Clamps to 255 after the stack only (the GS clamps per blend); with modulate the result stays in range.
- Mod tracks painted from pages and MX vs. ATV world paints get no meshes on the GPU (the `World`/`Pages`
  shaders were not touched); disc maps inside worlds get them through the baked pages.
- No by-eye comparison with PCSX2 frames.

## Next

Nothing.
