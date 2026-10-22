# 1 — mod ground textures (FINAL)

Task MD4. 2026-10-22.

## What I did

- `game-render/src/terrain.rs`: `Pages` — a terrain's own texture pages (MD1's `Terrain::textures`)
  through its palette, which page each leaf draws (`page_of`, by the original's lookup below), and
  where on the page a point of the world lands (`page_at`, with the pages' 8-pixel border). `paint` is
  the software renderer's colour; `with_detail` / `load_detail` / `detail_over` / `detail_multiplier`
  lay the map's detail texture over it.
- `game-render/src/gpu.rs`: `GroundPaint { Bare, Splat, Pages }` and `set_ground_painted`
  (`set_ground` now calls it, unchanged for its callers). A `Pages` ground gets its own shader,
  `ground_pages_wgsl`: the pages as one mip-mapped array, a leaf → page image (page + 1 in red, 0 for
  none), and the detail map. The pages are sampled with `textureSampleGrad`, the gradients taken from
  the continuous sample coordinate, so the jump from one page to the next is not a mip seam. The vertex
  stage is the old ground's, moved into `GROUND_VS_WGSL` and shared. Added to
  `every_shader_translates_for_webgl2`.
- `openmx/src/ground.rs`: `Ground::paged` and the `PAGED` cell tag; a cell with no splat whose middle
  has a page is sunlit and painted from `Pages::paint`. `main.rs`: only for a **mod** track (MD2) with
  no splat: load the detail map from the mod's main archive, read the scene's
  `DetailMapTilingMultiplier`, and print `ground: N terrain pages on M of K leaves, with/without a
  detail map`. MXU's disc tracks never reach this path (they have splats, and are not mods), so their
  ground is exactly what it was.
- Test `crates/game-render/tests/mod_ground.rs`: a generated one-level PLN v7, 8 x 4 leaves, two
  256x256 format-39 pages (page 0 red | green halves, page 1 blue), the root's table naming page 0 for
  the left 4x4 block of leaves and page 1 (word 2) for the right one, one leaf missing. Checks each leaf's
  page and colour on the CPU, the uv border at a page's first and last samples and in its fourth leaf,
  the detail blend and multiplier, and a headless GPU frame looked at from above: every leaf's middle
  is its page's colour, the missing leaf and the 64 leafless feet past the terrain are dirt. (I dumped
  that frame once to look at it: red, green, blue blocks and dirt where expected.)

Decision: the pages are only used for mod tracks with no splat. The brief says MXU's ground path stays;
MXU's nationals do carry pages too (S98) but are drawn the S-tasks' way, through their splats.

## What the original does

No MX vs. ATV executable is decompiled in this repo, so the lookup was read in MXU's terrain renderer,
which draws its nationals' pages the same way, and then proved on MX vs. ATV's and Untamed's files.

- `./kit fn 0x001d50b0` (decomp ~line 124558 of boot.c): per render node, byte `b = cx | cz << 4` is its
  place within its parent; the parent's sixteen-word table (node +24..+88, `Unread::table`) is indexed
  `(cz >> 2 & 3) * 4 + (cx >> 2 & 3)`, and the word is twice the page's index into the terrain's page
  array (terrain +0x560). The uv offset is `(cx & 3) * 0.25`, `(cz & 3) * 0.25`, a sample being 1/64.
- `./kit fn 0x001d3ef0`: maps that offset `x * 0.9375 + 0.03125` — the 8-pixel border.
- The terrain loader (`./kit fn 0x001d9930` region, decomp ~128089) sets terrain +0x78c to
  `min(DetailMapTilingMultiplier * 64, 360)`; the renderer sets a detail flag (0x200) on nodes whose
  metric (+100) is below it. What the VU1 microcode then does with the detail texture was not read.
- MX vs. ATV's `Env\<MAP>\TerrainPlain.psm` (loose copy, 4789 bytes): `Vu1TerrainRenderer.vuo`, two
  texture maps, pass 2 `alphaBlendingOnOff_2 = 1` with no inputs named (so the GS default
  `(Cs - Cd) * As + Cd`), `TerrainColor 128,128,128,128`, `DetailReplacementColor 109,109,109,128`. The
  detail map `<MAP>_d.pst` (SX01's: 512x512) is a constant 24,24,24 with the grain in its alpha
  (decoded 16..136, mean 53; the `.pst` decoder doubles alpha).

Nothing was taken from opentennis: it has no terrain.

## How it was proved

- The page lookup and the border, on the files (throwaway probe crate in the scratchpad, linking this
  branch's game-sim and game-render): all 56 MX vs. ATV maps, 220,256 table lookups, every word even and
  inside the page count; along every page seam the pixels either side agree exactly (mean difference 0.0)
  with the `0.9375 x + 0.03125` inset, 32.6 levels a channel without it. Untamed's 49 terrains: 253,808
  lookups all good, seams 0.8 with the inset against 30.4 without.
- The whole SX01 mosaic built that way is one coherent supercross picture; Nat13's (two levels) centre
  uses pages 0–31 and its outer level-1 nodes reuse a filler set, pages 32–47.
- `crates/game-render/tests/mod_ground.rs` (three tests, generated data, no disc): CPU and GPU.
- By eye, both renderers: `mxvatv_nat13` imported with openmx-mods' `import-mxvatv` into a scratch mods
  folder, `openmx --mods <dir> --track mxvatv_nat13 --renderer gpu|software --shot`: the asphalt road
  with its yellow centre line and white edge line and the dirt verge, the detail grain over it; the two
  renderers' frames match. Log line: `ground: 48 terrain pages on 6400 of 6400 leaves, with a detail map`.

## Not verified / not 1:1

- **No MX vs. ATV code was read.** The lookup is MXU's renderer's; it is proved on MX vs. ATV's and
  Untamed's data (seams, table words) but not against their executables.
- **The detail map's tiling is a reading, by eye.** It repeats once every `DetailMapTilingMultiplier`
  samples (V1's reading of the number). The original's use of the number that I did read is a distance
  (`min(m * 64, 360)` decides which nodes get detail); how its VU1 microcode scales the detail uv was not
  read.
- **Detail at distance:** the original swaps in `DetailReplacementColor` (109,109,109) past that distance;
  here the detail is drawn everywhere and its mip levels stand in for the far grey. Not read, not 1:1.
- **The blend's alpha scale** — `As = decoded alpha * 0.5 / 128` — follows the `.pst` decoder's doubled
  alpha and the GS's 0x80 = 1; not compared against a frame of the original.
- **Lighting** of a paged ground: sunlit like the rest of openmx's ground, with `TerrainColor`
  128 (= 1) as the only tint. MX vs. ATV's own lighting of it is not known. Not compared with a frame.
- **Mip bleeding** on the GPU: the coarse mips of a page can reach past its 8-pixel border into the
  wrong side of a seam far away. Not measured; did not show in the by-eye frames.
- **Pages smaller or larger than 256x256, or in formats other than 39**: none exist on the three
  discs; `Pages::of` takes any one-byte-a-pixel size but only 256 was ever drawn.
- **One-level terrains** (SX01 and the other supercross maps) are sized 16 samples by MD2's loader, so
  only their first leaf is drawn — that is MD12, not this task; the pages themselves draw on them
  (the probe's mosaic) but not in the shell yet.
- No PCSX2 run of MX vs. ATV: there is no copy of it set up in the kit.

## Next

Lands as FINAL with the task.
