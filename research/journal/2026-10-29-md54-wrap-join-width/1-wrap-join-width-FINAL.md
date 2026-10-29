# 1 — wrap join width (FINAL)

Task MD54. 2026-10-29.

## What I did

- **The cause.** stitch's `plan` (tools/stitch/src/build.rs, as C12 left it) ends a wrapped axis at the
  larger of two things: every wrap pair's `gap_for` gap, and `far + pad`, the open world's outer band
  (`pad` = SHOOTOFF + min_gap/2 = 210 ft on world_mxu_free). On a wrapped axis the band before the first
  line is half of every join across the wrap. That band is never under the first tile's cut, because
  openmx/game-sim take no tile origin below 0. So the pad's floor won: 792 ft (Dallas's 88-sample ridge
  cut at 9 ft) + 216 ft past Norfolk + openmx's 135 ft seam strip, which `--seams` reads as 1134 ft
  (MD51).
- **The fix (openmx-mods 3c2c2fd, branch `openmx-MD54`).** On a wrapped axis the floor is now `far` +
  two lattice cells instead of `far + pad`. openmx drops the lattice's last cell, and the last tile's
  edge needs a join cell outside it. With no margin at all (first try), openmx counted 11 holes along
  Norfolk's far edge and `stitch check` said "no join outside it". An open axis is unchanged.
- **`stitch check` through the wrap.** With the pad gone, Norfolk's edge taper (its last-cell slope,
  2.27 ft a foot) now sits in the first 90 ft past x 0 and z 0. check measured a cell's distance from
  each tile without wrapping, so it held the taper to max_slope and failed 4339 cells there. The build
  already counted those cells as near a seam ("2.271 near them", the same figure as C12's build). check
  now measures through the wrap's images too (`build::images`). A world that doesn't wrap is unchanged.
- `stitch build --wrap x|z|xz`, so one wrapped world can be built by hand. I rebuilt world_mxu_free from
  the five MX Unleashed tiles that MD53 had staged with C12's code (copied, not regenerated from the
  disc).
- I took nothing from opentennis: it has no stitched worlds.

world_mxu_free, C12's build → MD54's (openmx `--frames 1 --seams`, `world crossing:` lines):

| crossing | join ft before | after | blend after (a foot) |
|---|---|---|---|
| x mxu_free07 > mxu_free06 across the seam (Norfolk→Dallas) | 1134 | 927 | 0.186 |
| x mxu_free09 > mxu_free11 across the seam (the short row) | 6606 | 6399 | 0.044 |
| z mxu_free09 > mxu_free07 across the seam | — | 918 | 0.064 |
| z mxu_free11 > mxu_free10 across the seam | — | 918 | 0.026 |
| stitched joins (x and z) | 135 | 135 | 0.136–0.694 |

stitch reports the world as 17289 × 11808 → 17091 × 11610 ft; openmx, with its strip, as 17415 × 11934 →
17217 × 11736. The 927 ft breaks down as Dallas's 792 ft cut + openmx's 135 ft strip (+ the two cells).
Under openmx's world model (everything from 0, extent = the farthest piece) that is the narrowest it can
get. The rest is filed as **MD60**: let a tile cross the far seam, or stand at a negative `at`.

## What the original does

Nothing to compare. The stitched world is openmx's own (openmx-mods' stitch), and no retail code is
involved.

## How it was proved

- openmx-mods `tools/stitch/tests/wrap_width.rs`
  `the_join_across_the_wrap_is_as_wide_as_a_stitched_join`. It builds a generated wrapped 2×2 world and
  checks each axis's wrap gaps (`period − last.x1 + first.x0`): every gap is ≥ its pair's `gap_for` need,
  the tightest pair is within two lattice cells of its need, and none is wider than the widest stitched
  gap + a leaf. With the fix: x 477 ft against a 468.75 need, z 657 against 649.8. Without it, x is
  675 against 375: 300 over, so the test fails.
- openmx-mods workspace tests (`cargo test --workspace --exclude import-music`): 136 passed, 0 failed.
  That includes C12's tests/wrap.rs (0 holes, no step across a wrapped 2×2 world and a wrapped ultra map).
- On the rebuilt world_mxu_free:
  - `stitch check --mods --iso`: ok (join slope 0.343 away from the seams; wrap near/far worst
    0.00429 ft).
  - `stitch holes`: 0 holes and 0 points with no ground across the seams.
  - openmx's ignored `wrap_seam_look.rs` `dallas_and_norfolk_meet_like_two_stitched_neighbours` passes
    (`OPENMX_WORLDS` = a scratch mods folder holding the rebuilt world, via `./kit check -- -p openmx
    --test wrap_seam_look -- --ignored --nocapture dallas`): wrap slope 0.898 = the joins', blend 0.186.
- `stitch holes`' steepest step across the seams went from 0.148 to 0.837 ft in half a foot, at
  (17047.5, 4482). That point is inside Norfolk's kept ground: the probe starts 60 ft short of the strip,
  and with the pad gone that is the tile itself. At that spot the C12 build and the MD54 build read the
  same heights to the bit (238.615…244.471 over 3.5 ft), so it is Norfolk's own ridge face, not the
  seam.

## Not verified / not 1:1

- **The wrap join is still 927 ft, not 135 ft.** 792 ft of it is Dallas's ridge cut, which stitch can't
  shrink: game-sim's `Tiled::new` takes no `at` below 0, and openmx's extent is the farthest piece, so no
  tile can reach across the far seam. Filed as MD60. The task's "as wide as its stitched joins" is not
  met.
- **openmx-mods main does not have this.** C12 (which MD54 builds on) is still unmerged there: its merge
  ba6ea4b sits unpublished in openmx-mods-slots/merge. So I pushed branch `openmx-MD54` (C12 + this
  commit) and not main. Pushing main would have published C12 ahead of its own runner. Someone has to
  merge C12, then openmx-MD54, into openmx-mods main.
- **Not installed.** The rebuilt world is in /home/rycs/repos/openmx-mods-md54/out/worlds/world_mxu_free
  only. Not in ~/repos/openmx/mods (the main openmx checkout, which this task must not touch) and not in
  openmx-mods' out/mods (another session, MD53, has uncommitted stitch edits in that checkout). The other
  four worlds (world_mxu, world_mxvatv, world_untamed, world_ultra) were not rebuilt. Their wrapped axes
  shrink the same way, but I measured none of them.
- **openmx-mods' own gate `check.sh` fails to build import-music** against current openmx: its
  path-mounted audio.rs uses `crate::thread`. That has nothing to do with stitch. I ran the workspace
  without that crate.
- The short row's x wrap (mxu_free09 > mxu_free11, 6399 ft) is wide because the row's third cell is
  empty. That is the layout, not the pad, and is left as it was.
- No before/after `--shot` over the seam. The change is a narrower blend band, judged by `--seams`
  figures only.
- The rebuilt world used tile imports staged by MD53 from C12's code (target/md53/c12stage.mxu), not
  regenerated from the disc by me.
- The ultra map (`ultra.rs`) already starts at 0 with no pad and was not changed. Its wrap join was not
  measured.

## Next

Merge C12, then `openmx-MD54`, into openmx-mods main, and rebuild and install the five worlds. Then MD60.
