# 1 — race file load tail (FINAL)

Task S295. 2026-11-12.

## What I did

- **The walk was already there.** S293 ported the post-load walk from path 0 (flag 0x80 cleared,
  0x4 marks) as `TrackPaths::mark_reached`, and `TrackPaths::new` already runs it. The S13e image test
  already checks the image's path flags through it. This task adds what came after it.
- `TrackPaths::gate_spot` (crates/game-sim/src/race.rs): the track object's gate spot, with a private
  `flat_crossing` helper for the 2-D line crossing it calls.
- `TrackPaths::load(&TrackFile) -> Option<(TrackPaths, [Option<Spot>; 3])>`: the end of the race file's
  load. It runs `TrackPaths::new` (which does the walk), then each gate's spot in order (grid, line,
  third), with the path spot asked a second time when the gate spot fails, as the loader does. Then the
  lap length goes into `lap_length`.
- **Where the spots live.** The original keeps them at gate +0x94..+0x9c. `Gate` is `Copy`, used all
  over and wired into the snapshot, so I did not add a field to it. `load` returns the spots next to the
  paths instead.
- Three tests in crates/game-sim/tests/race.rs (proofs 33–35 in its doc), and the `track_gate_spot`
  entry in research/oracle.txt.
- Nothing from opentennis: it has no track paths.

## What the original does

The tail of `./kit fn 0x269750`, after the links are resolved:

1. `+0x4c` = path 0, and the walk (`./kit fn 0x26e9d0`, already ported).
2. For each gate that exists (`+0x58`, `+0x5c`, `+0x60`): height = 150 (`TrackFile::parse` already did
   this), then the gate spot (`./kit fn 0x26e6d0`). If that returns 0, the path spot
   (`./kit fn 0x26b410`, (track, centre, gate+0x94, 0, 1, 0), i.e. not holding-only) runs again.
3. The route search (`./kit fn 0x26cbe0`) runs from the line's spot with `along` = 0x38d1b717 (0.0001)
   to the same spot with `along` 0. The route out is null, the distance out is track +0x54, and kept-off
   paths are left out. With no line gate, both spots are path 0's first node.

The gate spot (`./kit fn 0x26e6d0`, with `./kit dis` for the call Ghidra drops):

- It walks depth first from +0x4c over the links using route-pool entries, the same way as the walk.
  Only flag 4 is checked, nothing else; a dry pool returns 0 at once, leaving the marks and entries.
- On each path, for node → next (node +0x54) while next exists, it calls `./kit fn 0x1346f0`. That
  function takes 8 float arguments: a = centre.xz − 0.5·(across.xz·width), d = across.xz·width,
  b = node.x/z, e = next − node.
  - It computes det = msub(mula(d.z, e.x), e.z, d.x), and returns 0 when det == 0.
  - t = madd(mula(e.x, b.z−a.z), e.z, a.x−b.x) / det.
  - s comes from the x lane, (a.x + d.x·t − b.x)/e.x, or from the z lane when e.x == 0.
- A crossing with t and s both in 0..1 (c.olt 0, c.ole 1) stores (path, node, s), and the loop moves on
  to the next path. So within a path the first stretch wins, but a later path's crossing replaces an
  earlier path's.
- After the walk the 0x4 marks come off every path. If nothing crossed, it calls the path spot at the
  centre and returns its answer.

The reader zeroes each gate's +0x90..+0x9c when it makes the gate (the 0x0f/0x10/0x19 records). So a
spot that neither search writes stays zero, which is the port's `Spot::default()`.

## How it was proved

- **Oracle, 20,000 generated courses and gates** (`the_gate_spot_matches_the_track_objects`). It runs the
  original gate spot in Ee on `t_paths` courses, comparing the answer, the spot, every path's flags,
  distance and best link, and the three pools. The gates cross a stretch, fall short of it, sit far off,
  have no width, or carry stray words; some pools run dry. 7,540 found a crossing (5,279 off path 0),
  10,650 fell back to the nearest spot, and 1,810 were refused.
- **Oracle, 10,000 loads** (`the_loaded_track_object_matches_trackpaths_load`). The track object's own
  loader runs with only the file reads and allocations hooked; the walk, gate spot, path spot and route
  search all run as the original's code. The files are generated loadable race files: `t_paths` courses
  written as records, with scale and offsets, kept-off and opened flags, gates stood on the course, and
  every tenth file has enough links to drain the pool. Compared word for word: every path's flags,
  distance and best link, the three pools, the three gates' spots (+0x94..+0x9c) and the lap length
  (+0x54). The run hit about 8,000 lap lengths found, about 1,700 routes that found no way, and more than
  13,000 gate spots inside a stretch. A hooked `free` asserts that the load never frees anything.
- **Save-state capture** (`sx01s_loaded_track_has_the_images_lap_length_and_third_gate_spot`). sx01.tdf
  off MX.PSR through `TrackPaths::load`, against S196's image. The lap length (4153.401) and the third
  gate's spot (path 2, node 64) match bit for bit. I checked that the test really runs, with a temporary
  panic.
- **Mutants.**
  - Caught: measuring `s` on the z lane first, and dropping the per-path `break` (so the last stretch on
    a path wins).
  - Equivalent: the determinant through plain mul/sub instead of mula/msub. The two forms differ only on
    a signed zero, and det is only compared with zero and divided by. The same holds for t's numerator,
    because t is only compared with 0 and 1 and multiplied into s.

## Not verified / not 1:1

- **The grid and line spots are not checked on the image.** The supercross loader moves those two gates
  onto props and asks the path spot again (`./kit fn 0x2f9f40`), so the image no longer holds the load's
  spots for them. They are proved only by the 10,000-load oracle. Spotting the moved gates belongs with
  the supercross override (S297/S299), not this task.
- **The image's paths' scratch words and pools are not compared.** By the time of S196's image, the
  race's own queries have run on them. They are proved only by the oracle.
- **A load whose line spot is never written is refused, not imitated.** When both spot searches refuse
  the line (the route pool run dry), the original measures the lap from a null path, reading low memory.
  `TrackPaths::load` returns `None`, and the oracle skips those files (fewer than 1,000 of the about
  10,400 generated). For the grid and third gates the zeroed spot is matched.
- **Files `TrackPaths::new` refuses are not loaded**: empty slots, links to empty slots, paths without
  the owned flag. The original's walks would read through null pointers there. The load oracle generates
  only files it accepts.
- **The determinant's and t numerator's accumulator ops are not told apart** by any test: the mutant is
  equivalent (see above).
- **The ground snap after the load** (`./kit fn 0x2f8b40`) is S296's, not ported here. It moves only
  heights, which none of this reads.

## Next

Nothing for S295. The open follow-ups are S296 (ground snap) and S297–S299.
