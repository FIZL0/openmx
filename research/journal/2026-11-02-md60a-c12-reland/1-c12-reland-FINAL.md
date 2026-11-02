# 1 — c12 reland (FINAL)

Task MD60a. 2026-11-02.

## What I did

- In a fresh openmx-mods worktree (`~/repos/openmx-mods-md60a`, branch `openmx-MD60a`, from origin/main
  65db95d) I ran `git merge origin/task/C12` (b36970a). The conflicts were the four `build.rs` hunks and
  `notes/stitch.md`, and I kept both sides of each:
  - `Plan` carries MD57's `relief`/`relief_scale`/`room` and C12's `period`, and `plan()`'s literal sets
    all of them.
  - The blend's two early returns are C12's image loop with MD57's `relief: 0.0`.
  - `notes/stitch.md` has MD53's "Rock at a cut" first, then C12's "Wrapped worlds", plus a bullet on the
    relief.
- The relief across the wrap:
  - `Plan::relief_at` builds one term per (candidate, image) from `images(self.period)`. Its nearest
    distance, gate clearance, 3×-nearest cut and 1/d² weights are all taken over those terms, and each
    image folds its own shifted point into the keep.
  - `Plan::relief_cands` filters on `wrapped_gap`, and its `far` bound is the farthest corner from the
    nearest image. That is still an upper bound on any point's nearest distance, so the list stays a
    superset.
  - The relief stays 0 on the world's own edge, where openmx's strip closes the wrap (MD57's choice,
    kept).
- tools/stitch/tests/wrap.rs: in 600 ft bands along both far edges of the wrapped 2×2 world,
  `relief_at(Some(relief_cands(band)))` equals `relief_at(None)` to the bit at 402 points (122 carry
  relief).
- Pushed d9e38b8 to openmx-mods main (fast-forward from 65db95d), and to branch `openmx-MD60a`. Its
  journal is journal/2026-11-02-openmx-md60a-c12-reland.

## What the original does

Nothing: the stitched worlds are generated ground (openmx-mods), not the original's.

## How it was proved

- openmx-mods `cargo test --release -p stitch`: every non-ignored test passes, wrap.rs (2), rock.rs and
  relief.rs included. Control: with `relief_cands` back on plain `rect_gap`, wrap.rs fails.
- openmx-mods `./check.sh` (openmx linked to this worktree, 6c649058): `check: ok, 148 passed, 0 failed`.
- world_mxu was built alone from the merged tree with `stitch --mods ~/repos/openmx-mods/out/mods --out
  out/stage`, the other four specs moved aside for the run and then put back. Results:
  - The build: 721 pieces, 37792 × 31960 ft, join slope 0.498 away from the seams.
  - `stitch check`: ok. Worst seam step 0.00299 ft; the near edges against the far ones, 0.00403 ft.
  - `stitch holes`: 0 holes, and 0 of 629584 points across the seams have no ground. Steepest step
    0.229 ft in half a foot.
  - `openmx --headless --no-sound --track world_mxu --mods <that> --trace 1 --seams` (release): no gaps,
    steepest step 0.700 ft at j023_011 | mxu_nat20, 0.100 ft across the wrap. The brief's limits were
    0.75 ft and 0.2 ft; the installed world reads 7.977 ft.

## Not verified / not 1:1

- Only world_mxu was rebuilt and walked. world_mxu_free, world_mxvatv, world_untamed and world_ultra are
  MD60's. Nothing was installed into ~/repos/openmx/mods.
- The relief's `Room` lattice does not wrap: its sweeps stop at the world edge. The relief is 0 there
  anyway.
- No ride across the wrap on the new build.
- openmx's crates/stitch (disc-built world_mxu / world_mxu_free, both `"wrap": "xz"`) has no wrapped
  field. I did not change or measure it; it is filed as MD60b.

## Next

MD60: rebuild the five worlds from openmx-mods main (d9e38b8 or later), put MD66's `"category": "World"`
line back into their mod.json files, install them, and run `--seams` on each. Then MD60b.
