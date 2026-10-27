# 1 — road paint filtering (FINAL)

Task V91. 2026-10-26.

## What I did

- `gpu.rs`: a `CLAMPED` `SamplerKey` (clamped u and v, linear mips) in V83's sampler cache. A ground
  with roads and ruts makes it at the renderer's anisotropy and hands it to `GpuMeshes::new`.
  `Renderer::set_anisotropy` passes the rebuilt one to `GpuMeshes::set_sampler`.
  `bound_anisotropy` now also lists each near target's group-2 anisotropy, after the ground's.
- `gpu/ground_mesh.rs`: group 2 of the ground's pipeline gets binding 3, `mesh_smp`. `APPLY_WGSL` reads
  `mesh_near` and `mesh_far` through it instead of group 1's `clamped`. That is the old `clamp_sampler`,
  with anisotropy 1 and Nearest mips, and it stays for the shadow. Every near target's bind group, which
  also holds the far target, is built by one `ground_bind`. `set_sampler` rebuilds all of them. Off is
  anisotropy 1, which is plain trilinear.
- Nothing was taken from opentennis: it has no ground paint targets.

## What the original does

Not applicable. This is rendering, and the GS has no anisotropic filtering.

## How it was proved

- `crates/openmx/tests/menu.rs` `texture_filtering_rebuilds_the_road_targets_sampler` (new sibling of
  V83's test): a headless GPU sets a meshed splat ground under Off (bound `[1, 1]`), then steps through
  every Texture Filtering choice twice. Each time it checks that the ground's group and the near
  target's group 2 were made again at that choice's anisotropy (clamped to the device). It really ran
  here: a temporary panic on the skip branch did not fire.
- `ground_mesh.rs` `each_view_gets_a_near_target_and_views_in_one_square_share_it` now also checks that
  `set_sampler` reaches both near targets' groups when there are two.
- Shots (git-ignored, `context/shots/v91/` in s3). The command was `openmx --track free09 --headless
  --frames 5 --overlay none --size 1280x720 --texture-filtering off|16`, with the binary built before
  (HEAD~ sources) and after:
  - `--trace-at 1872,1900,0` (`rut-*`) looks down a rutted track whose mesh tiles lie at x
    1836–1908. A build with the mesh paint turned off differs by 10 % RMSE, so the ruts are the mesh
    paint. A build with a forced Nearest `CLAMPED` sampler differs by 0.36 %, so the new sampler is the
    one the ground reads.
  - `--trace-at 1872,4500,180` (`road-*`) looks down the same road to the horizon. Before vs after:
    16x 0.31 % RMSE (72 pixels differ by more than 2 %), Off 0.10 % (no pixel differs by more than 2 %).
    Off vs 16x is 0.80 % in both binaries; that is the splat's V83 change.
- Why the change is so small. The disc's road and rut textures are coarser on the ground than either
  target. Measured over every free09 mesh triangle (texel area ÷ world area): 1.7 / 2.5 / 2.7 / 3.8
  texels a foot at the 10th / 50th / 90th / 99th percentile. The rut under the shots is a 128×256 texture
  at 1.26 texels a foot. The near target holds 4 a foot, and mips 0 and 1 already carry all of the
  source's detail. The softness the user sees is the disc's own texels magnified, not the filter. So I
  did not raise NEAR_PER_FOOT or NEAR_SIZE: it would add memory and no detail.
- The far target. A build with `FAR_SIZE` 4096 (0.72 texels a foot over free09's 5,678 ft, against
  0.36 at 2048) differed from 2048 by 0.05 % RMSE, 1 pixel over 2 %, on the `road-*` view. Kept at 2048.
  Cost of the raises (RGBA8 with mips, ×4/3): far 4096² is about 85 MiB against 21 MiB, drawn once per
  ground. A near target at NEAR_PER_FOOT 8 / NEAR_SIZE 4096 is about 85 MiB per view, 683 MiB for
  eight split-screen views, and four times the fill on every 64 ft square change. WebGL2's downlevel
  limit is 2048 per side as well.

## Not verified / not 1:1

- The look is not matched to the PS2. This is rendering, matched by eye: the PS2 reads the paint with
  no anisotropy, which is Off here.
- The disc's road and rut textures, at 1.3–3.8 texels a foot, set how sharp the roads can look. No
  sampler or target size fixes that; only higher-resolution replacement textures would. Not chased (see
  AGENT.md on render differences).
- The hard mip bands of the old Nearest-mip sampler were not singled out in a shot. The before/after
  difference above includes them, but no frame was taken with a band in view, side by side.
- The live change at the pause menu was not driven in a running ride. It goes through the same
  `set_anisotropy` the test proves.
- No device without anisotropic filtering was available. As in V83, the test accepts a device maximum
  of 16 or 1.
- Frame-time cost of the anisotropic road read was not measured.
- Split screen with several near targets was proved only by the unit test (`set_sampler` over two
  targets), not by a shot.

## Next

Nothing.
