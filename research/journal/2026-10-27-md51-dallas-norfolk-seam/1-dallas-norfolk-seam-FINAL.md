# 1 — dallas norfolk seam (FINAL)

Task MD51. 2026-10-27.

## What I did

- **Looked first.** I used the C12 build of world_mxu_free from ~/repos/openmx/mods (17415 x 11934, wrap "xz").
  `--seams` reports 216 edges, 1,040,382 rays, no gaps, 0 holes, a steepest step of 0.302 ft (j006_005|j007_005), and
  0.033 ft across the wrap. Since C12, the shape and the stitch's joins are fine at the seam. I walked
  `--trace 1 --trace-at x,2700,90` over the seam: 17073 → 254, 17150 → 282, 17414 → 262, 50 → 259, 450 → 266,
  792 → 252. Norfolk's ridge rises onto the join's plateau, the plateau crosses x 0 with no step, and it comes
  down to Dallas's ridge at 792.
- **The paint was the problem.** game-render's `Regions::weights` measured the distance to each map's owned rect
  without wrapping. Across the seam, Norfolk's paint (owned x 11736..17073) therefore ran at full weight up to
  x 17415, and Dallas's (792..6129) began at x 0. The result was one straight grey/yellow line through the middle
  of the join. `mirrored` read the far map's ground from the unwrapped side as well.
- **The fix.** `Regions` now has a `period: [x, z]` field: the world's side on each wrapped axis, 0 on an open one.
  `distance_to` and `mirrored` first move a point by ±period to the copy nearest the rect (`nearest`). Both the
  CPU path (`terrain.rs`) and the generated WGSL (`gpu.rs` `near1`/`near`, used by `reach` and `mirror`) do this.
  scene.rs's `Looks` uses `shares`, so the light now blends across the seam too. `worlds.rs` `regions()` fills the
  period from the pieces' far edge on each wrapped axis.
- **A measure.** `--frames 1 --seams` now also prints a `world crossing:` line for each pair of facing maps,
  including pairs that face across the seam. It reports the join's width, the steepest height slope, and two
  steepest paint changes a foot over 16 lines: `blend` (map colours weighted by `shares`, no texture) and `paint`
  (`WorldPaint::paint` itself).

Installed world_mxu_free, before → after (blend and paint in colour units a foot):

| crossing | join ft | slope | blend before | blend after | paint after |
|---|---|---|---|---|---|
| x mxu_free07 > mxu_free06 across the seam | 1134 | 0.898 | 13.349 at x 0 | 0.169 | 6.219 at x 598 |
| x mxu_free06 > mxu_free10 | 135 | 0.898 | — | 0.246 | 4.771 |
| x mxu_free10 > mxu_free07 | 135 | 0.898 | — | 0.688 | 2.432 |
| x mxu_free11 > mxu_free09 | 135 | 0.898 | — | 0.246 | 1.329 |
| x mxu_free09 > mxu_free11 across the seam | 6606 | — | 8.679 | 0.042 | — |
| z, the two seams | — | 0.775 / 0.784 / 0.621 | 4.421 / 1.719 | 0.057 / 0.022 | — |

After the fix, the raw `paint` rate across the seam (6.219) is still above a join's (4.771). Its peak is at x 598,
inside Dallas's mirrored ridge and not on the seam. It comes from the ground texture that `mirrored` reads past a
map's edge, the same thing every stitched join shows, so the tests assert on `blend` for the real world.

Shots (headless GPU, `--overlay none --size 1280x720 --frames 3`, `--shot-pose 15600,1500,2700,90,-30,85` side and
`17000,4000,2700,90,-89,85` top, kept out of git in the session scratchpad as before_side/top.png and
after_side/top.png):
- before: a straight hard line where Norfolk's grey ground meets Dallas's yellow, running along x 0 through the
  join.
- after: the grey fades into the yellow across the join. Faint mirrored ghosts of a track oval show in the blend,
  as on every stitched join.

## What the original does

Nothing to compare. The stitched freeride world is openmx's own (openmx-mods stitch), and no retail code is
involved.

## How it was proved

- `crates/game-render/src/terrain.rs` `a_wrapped_worlds_join_blends_across_the_seam`: two maps in a 1000-ft
  wrapped row. Shares are continuous over the seam, about 50/50 at its middle, and the steepest change is under
  10 a foot (over 100 without the wrap). It also checks that `mirrored` across the seam equals `mirrored` at the
  wrapped point.
- `gpu.rs` `every_shader_translates_for_webgl2` now translates a wrapped world's ground shader.
- `crates/openmx/tests/wrap_seam_look.rs`, on a generated two-map world wrapped on x (flat colours, red/green):
  - `across_the_wrap_the_maps_blend_like_across_a_join`: across the wrap, slope, blend and paint are each ≤ the
    join's (blend 0.825 against 1.553; 32.428 with the fix taken out).
  - `the_gpu_paints_the_wrap_like_a_join`: top-down GPU shots over the join and over the wrap. The worst pixel step
    is 1 over the wrap and 2 over the join, and the wrap row goes from green to red.
  - Both tests fail with `period` zeroed and pass with it.
  - Ignored test `dallas_and_norfolk_meet_like_two_stitched_neighbours` (needs `OPENMX_WORLDS=~/repos/openmx/mods`):
    on the installed world, the Norfolk>Dallas seam's slope is ≤ the joins' steepest + 0.001, and its blend is ≤
    every join's and ≤ Dallas>mxu_free10's. It passes (7 s).

## Not verified / not 1:1

- The wrap join is 1134 ft wide where stitched joins are 135 ft. C12's stitch layout puts it there: the first
  line's cursor starts at Dallas's cut offset, so its kept edge sits at 792, and the wrap pair takes the far end's
  pad plus that offset. The blend now spans the whole 1134 ft, so it is softer than a join's (0.169 against
  0.25–0.69). It does not read as a narrow join, though. That is a stitch change in openmx-mods, filed as MD54 and
  not done here.
- openmx-mods main does not hold C12's merge. C12's commit is on origin/task/C12, but main sits at MD50's spawn
  commit. The worlds I measured are the C12 builds installed in ~/repos/openmx/mods (11:09 today); openmx-mods'
  own out/mods is still the 10-23 build.
- Raw paint across the seam (6.219/ft) is above the Dallas>mxu_free10 join's (4.771). Its peak is the mirrored
  texture inside Dallas's ridge at x 598, not the seam. Only the texture-free `blend` is asserted on the real
  world.
- The slope metric reads exactly 0.898 on every x crossing, the seam included. I take that to be the shared ridge
  geometry each crossing walks over, but I did not trace where the 0.898 sits.
- The real world's GPU look was judged by eye from the four shots. The GPU pixel test runs only on the generated
  world.
- On the real world the z seams were measured (blend 4.421/1.719 → 0.057/0.022) but are not asserted.
- The ignored real-world test needs OPENMX_WORLDS and the installed C12 worlds, so the gate does not run it.

## Next

MD54 (stitch: make the wrap join as wide as a stitched one), after C12 is merged to openmx-mods main.
