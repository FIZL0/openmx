# 1 — stitched worlds reinstall (PART)

Task MD60. 2026-11-02.

## What I did

- **C12 is still not on openmx-mods main**, which MD60 waited for. `git merge-base --is-ancestor
  origin/task/C12 origin/main` is false. The only C12 merge, ba6ea4b in openmx-mods-slots/merge, was
  never published.
- **C12 no longer merges.** I ran a test merge of origin/task/C12 into origin/main (95541ff) in a
  scratch worktree (`~/repos/openmx-mods-md60`) and aborted it. It conflicts in `tools/stitch/src/build.rs`
  (4 hunks) and `notes/stitch.md`. MD57's far-join relief added fields to `Plan` and a `relief` to every
  `Point` the field returns. C12 changed those same returns to loop over a source's images, and changed
  `w` to per-term weights. Resolving this also means the relief's distance to the nearest map has to be
  read across the wrap. That is C12's own work and needs a full re-check, so I filed it as MD60a and did
  not resolve it here.
- **MD57 already reinstalled all five worlds (11-02 00:20), built from openmx-mods main without C12**
  (its worktree openmx-mods-md57 is at 95541ff, and b36970a is not an ancestor). So MD53's edge-slope
  blur and MD57's relief are installed, but C12's wrapped field is not, and the steep wrap is back.
  Release `--seams` on the installed worlds (binary from this slot at 1819dd49):

  | world | steepest step | where | across the wrap | MD53's figure (C12 build) |
  |---|---|---|---|---|
  | world_mxu | 7.977 ft | j003_035 \| seam_z_13 | 1.616 | 0.700 / 0.106 |
  | world_mxu_free | 0.276 ft | j006_005 \| j007_005 | 0.057 | 0.276 |
  | world_mxvatv | 11.991 ft | j000_029 \| seam_z_0 | 2.395 | 0.679 |
  | world_untamed | 17.640 ft | j008_031 \| seam_z_37 | 3.522 | 0.419 |
  | world_ultra | 17.347 ft | u00_02_09_03 \| seam_z_40 | 3.486 | not measured |

  All five: `world holes: none`, `no gaps`. Each worst step is at a `seam_z_*` piece, which is openmx's
  wrap strip. That is the MD53 journal's "steep wrap" from a stitch without C12 (8.05 ft on world_mxu
  then).
- **openmx-mods' build was red again, for a new reason.** The brief's `crate::thread` failure was fixed
  by 44cb382 (10-29). Since openmx 42eeed3b, audio.rs uses `cpal` directly (DeviceSound), and
  import-music failed with E0433 on `cpal`. Fixed in openmx-mods 65db95d (pushed to main):
  import-music's Cargo.toml takes `cpal = "0.16"` on Windows and Linux, as openmx's does.
- Nothing installed, no openmx code changed.

## What the original does

Nothing: the stitched worlds are generated ground (openmx-mods), not the original's.

## How it was proved

- `openmx --headless --no-sound --track <world> --mods ~/repos/openmx/mods --trace 1 --seams` on each of
  the five installed worlds (release build), giving the table above.
- openmx-mods `./check.sh` at 65db95d: `check: ok, 146 passed, 0 failed`. Before the fix, `cargo build -p
  import-music` failed with 7 × E0433 `cpal`.
- The C12 conflict: `git merge --no-edit origin/task/C12` on 95541ff, then `git merge --abort`.

## Not verified / not 1:1

- I did not ride across the wrap on the installed worlds to see whether a rider is thrown at the 8–18 ft
  steps. The figures are the seams walk's only.
- I did not resolve the conflict or check how hard it is beyond reading the hunks (MD60a).
- openmx's own `crates/stitch` (the copy that builds disc worlds) has no wrapped field either. I did not
  measure a world it builds.

## Next

MD60 is blocked on MD60a (C12 re-landed on openmx-mods main). Then do what the brief says: rebuild the
five with stitch from openmx-mods main (`stitch --mods … --out …`), put MD66's `"category": "World"`
line back into each mod.json as MD57 did, install them into `~/repos/openmx/mods/tracks/`, and run
`--seams` on each (the table above is the before).
