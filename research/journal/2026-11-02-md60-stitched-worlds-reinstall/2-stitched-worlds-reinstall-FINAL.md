# 2 — stitched worlds reinstall (FINAL)

Task MD60. 2026-11-02.

## What I did

- Built stitch (release) in the openmx-mods worktree `~/repos/openmx-mods-md60a`, at origin/main
  d9e38b8 (MD60a: C12's wrapped field over MD57's relief). Its `openmx` link points at this slot, which
  has MD60b. Then I rebuilt all five worlds from the spec files:
  `DISCX_ISO=~/repos/openmx-mods/iso stitch --mods ~/repos/openmx-mods/out/mods --out out/stage`.

  | world | tiles | join pieces | join slope away from / near seams |
  |---|---|---|---|
  | world_mxu | 27 | 721 at 4.5 ft | 0.498 / 5.196 |
  | world_mxu_free | 5 | 38 at 9 ft | 0.448 / 1.671 |
  | world_mxvatv | 27 | 704 at 4.5 ft | 0.500 / 3.335 |
  | world_untamed | 23 | 840 at 4.5 ft | 0.498 / 2.795 |
  | world_ultra | 3 worlds, 77 tiles | 2265 + 625 new at 4.5 ft | 0.454 / 0.448 |

- Put MD66's `"category": "World"` line back into each mod.json, after `"name"`. openmx-mods' stitch
  still does not write it, but openmx's `crates/stitch` does.
- Installed the five into `~/repos/openmx/mods/tracks/` (git-ignored, so the main checkout's tree stays
  clean). I also installed them into `~/repos/openmx-mods/out/mods/tracks/`, as MD57 did, so the next
  ultra check reads the same worlds. The replaced copies were moved, not deleted, to
  `~/repos/openmx-mods-md60a/out/old-installed/` and `out/old-outmods/`.
- No code changed in either repository. openmx-mods main already had everything (d9e38b8).

## What the original does

Nothing: the stitched worlds are generated ground (openmx-mods), not the original's.

## How it was proved

- `stitch check` on each world: all five `ok`, with near edges against far edges at 0.0035–0.0067 ft.
  The first world_ultra check failed only on "joins not byte for byte world_mxu's", because it compared
  against the *old* world_mxu in `--mods`. Against a mods folder holding the new worlds it is `ok`. The
  installed (MD57) world_ultra fails its own wrap check (35064 samples, worst 416.2 ft): it embedded
  pre-C12 worlds.
- `stitch holes` on each: 0 world holes, and 0 points across the seams with no ground. Steepest step in
  half a foot: 0.229 / 0.156 / 0.181 / 0.203 / 0.225 ft.
- `openmx --headless --no-sound --track <w> --mods ~/repos/openmx/mods --trace 1 --seams` (release, this
  slot), on the installed worlds. All five read `world holes: none` and `no gaps`, and none rebuilt its
  joins.

  | world | steepest step | where | across the wrap | before (part 1) |
  |---|---|---|---|---|
  | world_mxu | 0.700 ft | j023_011 \| mxu_nat20 | 0.100 | 7.977 / 1.616 |
  | world_mxu_free | 0.276 ft | j006_005 \| j007_005 | 0.040 | 0.276 / 0.057 |
  | world_mxvatv | 0.679 ft | mxvatv_nat11 \| j017_025 | 0.046 | 11.991 / 2.395 |
  | world_untamed | 0.419 ft | c20_002_000 \| untamed_am_jy | 0.072 | 17.640 / 3.522 |
  | world_ultra | 0.700 ft | mxu_j023_011 \| mxu_nat20 | 0.078 | 17.347 / 3.486 |

  These match MD53's C12-build figures (0.700 / 0.276 / 0.679 / 0.419). world_ultra no longer has
  nat17's rock seam (MD53's 4.1 ft step): its worst step is world_mxu's own.
- `cargo test --release -p openmx --test disc_worlds`: 3 passed. The world_mxu_free comparison ran (no
  "skipped" line), so the disc build is now byte-equal to the installed world, category line included.
- The brief's red openmx-mods `./check.sh` (import-music) is already fixed on main (44cb382, 65db95d).
  `cargo build --release -p import-music` at d9e38b8 builds.

## Not verified / not 1:1

- No ride across the wrap or over the new joins in the shell. The checks are the seams walk, `stitch
  check` and `stitch holes`.
- world_ultra's new ground between whole worlds still gets no relief (openmx MD70, unchanged).
- I did not run openmx-mods' full `./check.sh` or `./build.sh --check` here. MD60a ran `./check.sh`
  green at d9e38b8.
