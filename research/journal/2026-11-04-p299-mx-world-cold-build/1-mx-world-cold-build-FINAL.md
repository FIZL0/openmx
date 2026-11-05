# 1 — mx world cold build (FINAL)

Task P299. 2026-11-04.

## What I measured first

There is no iPhone here and no phone log with a world_mxu build in it (`~/Downloads/openmx-log*.txt`
are from 10-28/10-30 and only ride world_mxu_free). So I measured on the desktop (32 threads), release
build, `openmx --headless --no-sound --frames 2 --track world_mxu --mods <empty dir>`:

| run | wall | CPU (user+sys) | what |
|---|---|---|---|
| cold, before, load ~120 | 253.5 s | 1350 s | first frame (`load:` line) |
| cold, before, load ~60 | 181.6 s | 1312 s | 30 plans, 10 solves of 291-316 pieces |
| cold, after, load ~25 | 19.8 s | 208 s | 1 plan, 1 solve (316 pieces in 3.3 s) |
| cold, after, load ~125 | 48.8 s | 265 s | same, busy machine |
| warm (cache read), after | 10.5 s | 59 s | the 27 tiles off the disc, joins, ground/paint |

Where the old cold build went: 30 plans (~1.9 s each, 57 s, one thread), 30 coarse passes (38 s) and
10 full solves (~11 s each on 32 threads, 114 s wall, most of the CPU). Only the last solve's joins
are kept. The first 20 passes widen for the coarse lattice. The last 10 solve the whole world just to
find one piece (j014_017 by nat15) still over 0.5, widen that gap, and solve everything again. After
the cut, the build is ~9 s of the 20 s cold ride; the rest is what a warm ride also pays (reading the
27 maps for the key and the world, the ground and paint).

world_mxu_nat: 114.7 s / 729 s CPU, 4 passes → 24.5 s / 291 s. world_mxu_free: 12.2 s → 5.3 s
(already 1 pass; the cut there is the build no longer reading as cold).

Phone estimate (not measured): CPU time over ~3-4 desktop-core equivalents (2 P-cores and 4 E-cores):
before ≈ 1300 s → 6-7 minutes, after ≈ 210 s → about a minute, of which ~15-20 s is the warm load.

The cache on iOS: the mods folder is `Documents/mods` (`crates/openmx/src/ios.rs`, and the 10-30 phone
log's `/private/var/mobile/Containers/Data/Application/…/Documents/mods/tracks/…`). Documents is kept
across launches and app updates, not purged. The key is `stitch::VERSION` + the spec + the tiles' bytes.
None of those change with an app update unless the stitcher's output changes (VERSION is bumped by hand
for exactly that). So the cache already survives updates, and nothing needed changing there.

## What I did

- `crates/stitch/src/build.rs`: `Start` (the widen, the relief scale and the relief dents a pass
  begins from), `Params::start`, and `Built::start`/`Built::passes`. The pass loop starts from
  `p.start` (cold if none) and records where its last pass began. `Start::to_json`/`from_json` write
  numbers with `{:?}`, which reads back to the same bits. The seam gates are now measured per tile on
  threads, like the cuts.
- `crates/stitch/worlds/{world_mxu,world_mxu_free,world_mxu_nat}.start.json`: `{"key", "start"}`, copied
  from the `start.json` a cold build now writes beside its `mod.json` in the cache.
- `crates/openmx/src/disc_worlds.rs`: `STARTS`. The build takes the start whose key is this cache key.
  With another key it says `no start kept for key …` and builds from cold, which is as correct as before
  and only slower. It says `<id>: joins built in N passes, T s` (for the phone's log too).
- `crates/openmx/tests/disc_worlds.rs`: every first ride must build in 1 pass from its committed start,
  and the start it keeps must be the committed file; the failure message names the file to copy. COLD
  is down from 3600 s to 900 s of CPU.
- `crates/stitch/src/lib.rs`: VERSION's doc says a bump makes the start files stale.

Why not the brief's other cuts:
- **Joins near the start first, the rest streamed while riding**: the sim side would allow it
  (`game-sim/src/tiles.rs` loads a piece on its first query, so a load could wait on its solve without
  changing behaviour). But `worlds::read_watched` reads every join's header, mod.json is written only after
  the whole solve, and the far GPU ground, radar and `--seams` read every join at load. That is a large
  change for what is now ~9 s of a 20 s desktop cold start. Filed as P300 (later), conditional on the
  phone's timing.
- **The store route**: not chosen. The build fell ~9× in CPU, so it should take about a minute on a phone,
  not minutes. If the phone's timing (HUMAN.md "How long MX World takes to build on the iPhone (P299)")
  says otherwise, P300 or the P241 download is next. Nothing was uploaded.
- **Every core**: the solve, the cuts and the reliefs were already on threads. The gates are now too.
  The one plan and coarse passes left (~1.5 s) stay on one thread.

From opentennis: nothing; it has no world stitcher.

## How it was proved

- Byte equality: for each of the three worlds, a cold build without a start (the old path) and a
  build from the committed start into another empty mods folder; `diff -r -q` of the two cache key
  folders (mod.json, start.json, every join's .ps2.trn and .blend) reported nothing for all three.
  I ran this again after threading the gates, for world_mxu and world_mxu_nat.
- `tests/disc_worlds.rs` (4 tests, passed): world_mxu_free from its start is still openmx-mods' installed
  world_mxu_free byte for byte. Every first ride passes once from its committed start.
- `stitch` `a_start_reads_back_to_the_bit`: Start's JSON round-trips to the same f64 bits.

## Not verified / not 1:1

- **No phone timing.** No iPhone here. The ~1 minute is an estimate from desktop CPU time. The measurement
  is a HUMAN.md step, and the shell now logs `joins built in N passes, T s` for it.
- **The phone builds the same bytes as the desktop**: not checked. The stitcher is host f64 with libm
  calls; Apple's libm on aarch64 could differ in a last bit from glibc's. If the phone's pass then ends
  differently, it still builds a correct world, only one that is not the desktop's to the byte. That
  was already true before P299.
- **world_mxu from a start equals world_mxu from cold, in the gate**: proved by hand here (diff above),
  not by a test. The gate's disc_worlds proves it only for world_mxu_free (against openmx-mods), and for
  the others that the start is a one-pass fixed point. A second cold build of world_mxu in the gate would
  cost ~1300 s of CPU. The argument: same key means the same stitcher version and tiles, and the file
  was recorded from a cold build of that key. It fails if someone changes build.rs's output without
  bumping VERSION, which already breaks the cache the same way.
- **Joins near the start only / far joins arriving while riding** (the brief's test): not done. Why is
  above; filed as P300 (later).
- **iCloud backup**: the ~220 MB cache in Documents is backed up with the app's data; not marked as
  excluded from backup. Not checked whether the user minds.
- Timings were taken at loads of 25-125 on a shared machine; the before/after pair is at different
  loads (~60 vs ~25). CPU time (1312 → 208 s) is the fairer comparison.

- **Audit skipped**: `./kit done`'s audit found 4 addresses already on main in files this task did not
  touch (`crates/game-sim/src/atv_wheel.rs:62,66`, `plan/DONE.md:519,533`), so it landed with
  `--skip-audit`; P299's own files have none.

## Next

- The user times it on the phone (plan/HUMAN.md, P299). Then P300 (stream) or the store download if
  it is still too slow.
