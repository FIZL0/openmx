# 1 — s13 sx01 data (FINAL)

Task S13e. 2026-11-09.

## What I did

- Ported the track object's race-file reader as `race::TrackFile::parse` (crates/game-sim/src/race.rs), plus
  `TrackPath`, `PathNode`, `Link`, `Checkpoint`, `CheckPart`, `Refused`, and `Gate::new` (the gate's
  constructor values).
- Ported the supercross loader's gate override as `Supercross::place_drop_gate` (grid + start gate, off the
  start-drop prop) and `Supercross::place_finish_line` (the line, off the finish prop), taking a `race::Prop`.
- Three tests in crates/game-sim/tests/race.rs (proofs 11–13 in its doc). New oracle.txt names after the
  score_race_tick block.
- **The brief names the wrong file.** sx01's race data is `tracks\sx01\sx01.tdf` inside
  `Tracks\sx01_2.res` (in MX.PSR), not `sx01.res`. The test reads it from there.

## What the original does

The race activity's load (`./kit fn 0x2f8610`) builds a track object (`./kit fn 0x2693c0`) and has it read
`<track>.tdf` (`./kit fn 0x269750`):

- The file is a stream of records, each a tag byte and then LE words. The read stops when a tag byte is the
  file's last byte.
- Header records give an x/z offset, a scale and a word. Points are `add(offset, mul(raw, scale))` on x/z
  and `mul(raw, scale)` on y (FPU mul.s then add.s).
- Paths (tag 5) hold nodes (three points each). A node's span is `sqrt(madd(mula(dx,dx), dz, dz))` after a
  VU subtract, and the path's length sums the spans.
- Link records (0xd count, 0xe pairs) are bounds-checked with an off-by-one (`c > count` rather than `>=`).
  After the load, values in 0..count become path pointers.
- Checkpoint records (0x1d) and their parts (0x22) chain through "current record" / "last part" hooks.
  Tag 5 and tag 0x1c reset the record hook, words 0x23/0x25/0x26 land on the last part made, and point tag
  0x24 sets the previous part's span.
- There are three gate records. Each is constructed (`./kit fn 0x246530`: unit quads, reach −1); the normal
  is kept raw with w = 1, across goes through normalize, width = mul(w, scale), flags = 3, and height is
  150 after the load.
- After the load, a walk from path 0 (`./kit fn 0x26e9d0`) clears flag 0x80 on every path it reaches. Gate
  and path spots are then snapped to the ground.

The supercross loader (`./kit fn 0x2f9f40`) then overrides the gates from scene props:

- **Grid gate**: centre and across are whole quads from the drop prop's frame, normal = cross(across,
  (0,1,0,1)) keeping across.w, width = 2·half.x, height = half.y.
- **Start gate**: flags |= 2, centre/across/normal xyz from the frame, height 1000, then centre −= normal·4.5.
- **Line**: centre/across from the finish prop, width/height from its bound, normal =
  normalize(cross(across, cross(ahead, across))), where ahead is the path direction at the line's path word.
- Its tail fills the lane table.

## How it was proved

- **Oracle, N = 12,000 generated race files** (`the_race_file_reader_matches_the_track_objects_loader`):
  the loader runs in Ee with file reads, allocations, the fail call, the post-load walk and the spot calls
  hooked. 11,103 files read and 897 were refused; across them, 11,356 gates, 29,265 checkpoints,
  18,646 parts and 21,560 links were compared word for word.
- **Save-state capture** (`sx01s_race_file_off_the_disc_is_the_track_object_in_the_image`): sx01.tdf read
  off MX.PSR, against the track object in S196's sx01 RAM image. The header, 2 paths, 96 nodes (x/z/w of
  every point, spans, words) and the third gate match bit for bit.
- **Save-state capture** (`the_supercross_loader_stands_sx01s_gates_on_its_props`): the image's grid and
  start gates are bit-exact from the image's own drop prop (bound by the original's bound function in Ee).
  The line's centre, across and normal are bit-exact (ahead from the original's path function in Ee).
- A mutant (`F(y.0 * s)` for the FPU mul) was caught. Two others (madd order, mul_q(1.0)) turned out to be
  equivalent.

## Not verified / not 1:1

- **The ground snap is unported.** Node heights and gate centres are snapped after the load
  (`./kit fn 0x2f8b40`, the ground query), so the image's node y values are not compared.
- **The post-load path walk and spots are unported**: the flag 0x80/4 walk (`./kit fn 0x26e9d0`), the
  gate/path spot calls (`./kit fn 0x26e6d0`, `./kit fn 0x26b410`) and the start-spot builder
  (`./kit fn 0x26cbe0`). The image test masks flag 0x80 for this.
- **The prop lookup and bound are unported.** The port takes a `Prop` the caller fills. The scene's name
  lookup and the bound function are not in game-sim.
- **The line's width/height are not proved on the image.** No finish prop in the image still carries the
  load-time bound, so the test feeds the image's own line width/height back in.
- **The gate override is proved on one image only.** I did not run a generated oracle of the supercross
  loader: it needs the scene's name lookup, the prop holders and the string helpers faked in Ee. Filed as
  a follow-up.
- **Refusal paths are not imitated in detail.** `Refused::Stray` / `Truncated` / `PastLinks` stand for the
  original's off-by-one link write past the array, a short read and reads of address 0. The port refuses
  where the original writes or reads garbage.
- **A link count of 0** (an allocation of 0 bytes) is not generated.
- **Checkpoints are proved only by the generator.** sx01 has none.
- **Unknowns**: the meaning of the five tunables and of the unnamed words (`word_*`) is unknown. The header
  offsets start uninitialised in the original and 0 in the port (the test zeroes them before the load).
- **The second track file** the activity keeps is not read.

## Next

Follow-ups S295–S299 were filed for: the post-load walk and spots, the ground snap, the second track
file, the finish prop's bound, and a generated oracle of the supercross gate override.
