# 1 — anisotropic setting (FINAL)

Task V83. 2026-10-26.

## What I did

- **Renderer** (`crates/game-render/src/gpu.rs`): MD31's lone `ground_sampler` (and its
  `GROUND_ANISOTROPY`) is gone. Every model material and the ground now take their sampler from one
  cache, `Renderer::samplers`, keyed by `SamplerKey` (clamp u/v, one-mip-level or not) and made by
  `texture_sampler` with the renderer's anisotropy. `Renderer::set_anisotropy(n)` clamps `n` to
  `1..=anisotropy_most` (16 where the adapter has `DownlevelFlags::ANISOTROPIC_FILTERING`, else 1),
  remakes every cached sampler, then every live material's bind groups (each `GpuMaterial` now keeps a
  `MaterialSource`: view, sampler key, uniform buffers) and the ground's (`GpuGround` keeps its layout,
  shadow and texture views; `Renderer::ground_bind` is the one place the bind group is built). Same
  value again does nothing. `Renderer::anisotropy()` and `bound_anisotropy()` read it back for tests.
  A new renderer starts at the device's most, so anything not told otherwise (the web page, tools) gets
  16x.
- Before this, the disc tracks' splat ground, props, bikes and riders used the plain trilinear sampler;
  now they all filter at the setting. Clamped materials (palm fronds, bark) used to make one sampler
  each; they now share the cache.
- **Menu** (`menu.rs`, `menu_pages.rs`): `menu::Filtering` (16x, 8x, 4x, 2x, Off — ours first and the
  original's last, as Draw Distance lists them, so a Right press from Off is 16x again, which the
  generic every-departure test relies on). `Settings::filtering`, default 16x, written to settings.txt
  as `texture_filtering = off|2|4|8` only when not 16x (a file from before is written as it was).
  `Departure::Filtering` (P126): the Original switch sets Off, its own switch back sets 16x; it lives
  on the Graphics tab under Picture, after Picture Shape, as a five-choice row like Draw Distance is for
  Fog (`Departure::key` → `Key::TextureFiltering`). The row's help (`menu::FILTERING_HELP`) says the
  software renderer draws without it. The Original switch's help names texture filtering.
- **Shell** (`main.rs`): `--texture-filtering off|2|4|8|16`; the shell's ride takes the menu's setting;
  the ride applies it before its first frame and prints `texture filtering: <label> (anisotropy n,
  this device m at most)`; a pause change reaches the renderer through `set_anisotropy` like V57's
  foliage.
- opentennis: nothing to take — it has no anisotropy setting.

## What the software renderer does

Nothing. `raster.rs` reads one texel per pixel through each look's paint function; it has no mips and
no footprint to filter along. Said in the row.

## What the original does

Not applicable: the PS2's GS has no anisotropic filtering, so Off (plain trilinear) is the closer
picture and the Original switch picks it. Rendering is "ours, match by eye" per AGENT.md.

## How it was proved

- `crates/openmx/tests/menu.rs`:
  - `texture_filtering_defaults_to_16x_and_the_original_switch_turns_it_off` — default 16x and no line
    in a fresh file; Original on → Off and `texture_filtering = off`; the Graphics row reads Off, a pad
    Left steps it to 2x and leaves every other departure at the original's (Mixed); every choice
    round-trips `Settings::text`/`parse`; an unreadable word keeps 16x.
  - `texture_filtering_rebuilds_every_sampler_with_its_clamp` — headless GPU (it ran here, not skipped:
    RADV Strix Halo, most 16): a bare ground plus a three-material model (repeating, clamped, kept-
    coverage cut-out); for every choice both directions, `anisotropy()` is the choice clamped to the
    device and `bound_anisotropy()` is `[n, n, 1, n]` — each bind group made again, the cut-out at 1;
    a material uploaded after a change is made under it.
  - `the_settings_file_reads_back_as_it_was_written` now carries `filtering: X4`;
    `the_graphics_tab_has_each_graphics_setting_once_under_its_group` lists the new row under Picture;
    `original_on_sets_every_sub_switch_and_one_turned_back_leaves_the_rest` covers it through
    `Departure::ALL`.
- `crates/openmx/tests/original_mode.rs`: a ride with the Original switch on prints
  `texture filtering: Off (anisotropy 1,`, and off prints `texture filtering: 16x (anisotropy `.
- Shots (git-ignored, `context/shots/v83/` in s2): `openmx --track free09 --headless --frames 5
  --overlay none --size 1280x720 --trace-at 4620,2320,0 --texture-filtering off|16 --shot …`.
  `side.jpg` and `crop.jpg` (the far ground band) compare them: 16x keeps visibly finer grain toward the
  horizon, Off smears it. RMSE between the two frames 1.0 %; 802 pixels differ by more than 2 %.

## Not verified / not 1:1

- The change is modest on free09: its splat layers are low-frequency at their tiling, so much of the
  softness the user saw down the ground is the textures themselves, not the filter. Not chased (render
  differences are noted, not chased, per AGENT.md).
- Cut-out materials that keep their coverage (V58/V80's leaves) stay at anisotropy 1 under every
  choice: they read one mip level (`MipmapFilterMode::Nearest`), and wgpu refuses anisotropy unless
  every filter is linear. Giving them linear mips would undo V58.
- Not filtered by the setting, on purpose: the sky and the particle/sprite sheets and the upscale blit
  (`clamp_sampler`, drawn face on or full-screen), glows (`sampler`, additive columns), and the splat's
  road/rut meshes, whose sampler draws them top-down into the ground's targets (the ground then reads
  those targets through `clamp_sampler`, not anisotropically). No shot was taken to look at the roads.
- The live change at the pause was not driven in a running ride: it is the same call the tests prove
  (`set_anisotropy`), reached through the same `front.menu.changed()` block V57's foliage uses.
- A device without anisotropic filtering (most = 1) was not available; the clamp is
  `DownlevelFlags::ANISOTROPIC_FILTERING` read from the adapter, and the test accepts either 16 or 1.
- The web page (`web.rs`) applies none of the Settings' graphics choices; it draws at the renderer's
  default, the device's most (16x). Nothing in this task wires the setting there.
- Frame-time cost of 16x was not measured.

## Next

Nothing.
