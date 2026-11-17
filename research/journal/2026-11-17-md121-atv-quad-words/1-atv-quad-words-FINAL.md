# 1 — atv quad words (FINAL)

Task MD121. 2026-11-17.

## What I did

- tests/atv_quad/replay.rs now asserts the quad's own 0x1800 bytes on every row, beside the body,
  engine and wheels. All 30 C28 replay tests pass
  (`./kit check -- -p game-sim --test atv_quad -- replays_by`). No row of any ride is torn.
- The old "outside / inside" listing of differing quad words is gone. In its place:
  - `BETWEEN`: quad words that code outside the sim frame writes, each with the reason. Not compared.
  - `RIDER_BLOCK` (+0x870..+0x980): not compared from a wreck's first row on (R37, the ragdoll).
  - `READER` grew by 8 bytes: +0x12b8/+0x12bc are the pad reader's, so they are laid like its others.
- The instant search (A7's mid-frame read) now looks for one instant where body, engine, wheels and
  quad all match. That covers the task's "+0x414..+0x7f0 differ for the first rows and heal" and
  "+0x930..+0x938 a few ulps off" on the s- rides: those rows are read a few dozen writes before the
  frame's end, mostly at pc 0x4bb308 (s-still 298 of 300 rows there).
- The run summary (`$MD37B_LOG`) now says from which row a ride is wrecked and which rows were read
  mid-frame.

## What the original does

Each finding is the writer of a quad word that the sim frame does not write, or writes off a value
written outside it.

- **+0x44 bit 0x400.** Set by `./kit fn 0x494350 --program atv`, called from the quad's vt +0x108
  (0x4a4c10). That is reached through the thunk 0x4a51d0, which is the +0xc scene node's vt +0x48;
  the scene pass 0x1288b0 walks its children's vt +0x48. The frame's vehicle PostProcess clears it
  (`./kit dis 0x4941d0 0x4941f0 --program atv`, pc 0x4941e8). So rows (read after the scene pass)
  have it, the machine after the frame does not. A static scan of the overlay for andi/shift tests of
  the bit found no reader.
- **+0x6f8..+0x720** (ride time, top speed, wheelie and stoppie distances and their bests). The quad
  update `./kit fn 0x4ba9f0 --program atv` reads the player's score card at *(+0x680)
  (oracle.txt, the score card): +0x6f8 += dt unless card+8 & 0x200 (0x4bb070); +0x6fc = max with
  card+0x108 (0x4bb080); +0x70c/+0x710 += card+0x18 with bests +0x71c/+0x720 (0x4baeb0, 0x4baf7c).
  The card is written between frames, so race-stoppie-short went off at row 107 on +0x710/+0x720.
- **+0x464/+0x468**, the camera link list's head and tail (F48). The append 0x1b5710 takes its node
  from the allocator 0x1b7620. Other per-frame code allocates and frees between frames, so the
  machine's node is elsewhere (race-wheelie-launch row 80: 0x01d16ff0 against 0x00d12290).
- **+0xd90**, the last gather centre of the vehicle's sphere gather (`./kit fn 0x4aa980 --program atv`,
  via 0x4aa7d0; callers 0x4aa460, 0x4ce6d0, 0x4e6e98, 0x4e78a0). Not in the replayed frame. +0xd38,
  the squared distance since, sits in the already-laid spot.
- **+0x12b8/+0x12bc**, the stick's angle and length (`./kit fn 0x4e3c80 --program atv`: atan by
  table, clamped; returns a stick quadrant). Its one caller is the pad reader 0x4e36c0, which runs
  outside the frame. Found by `grep` for stores at offsets 4792/4796 in `./kit dis` of the overlay.
- **+0x870..+0x978 after a wreck, +0x940 one more**: the rider's block (rider+0x378 is quad+0x730).
  The interpreter's ragdoll leaves the original's at the wreck's first frame. Already R37 (MD148).

## How it was proved

- The bit-exact replay fixture, tests/atv_quad/replay.rs, all 15 C28 rides × 2 (original's frame
  alone; port's pieces and tail beside it), 300 rows each, every byte of body, engine, wheels and
  quad (but the masks above) equal at the frame's end or at one instant inside it.
- Before each mask was added, the run failed on that word: race-stoppie-short row 107 (+0x710,
  +0x720), race-wheelie-launch row 80 (+0x468). The other masked words differed on most rows.
- Writers found by `./kit dis`/`./kit fn`/`./kit xref` and by temporary `Ee::log` probes (removed).

## Not verified / not 1:1

- **+0x44 bit 0x400 is not compared.** The scene-node pass that sets it is not run. That nothing reads
  it is from a static scan only; an indirect reader would be missed.
- **+0x6f8..+0x724's card-fed words are not compared.** The score card's update is not run, so the
  replay cannot make them. Their arithmetic in the frame is not checked by this test.
- **+0x464/+0x468 are not compared.** They are allocator pointers; equal values would need the game's
  whole heap traffic between frames.
- **+0xd90 is not compared.** The sphere gather is not run.
- **+0x12b8/+0x12bc are laid from the capture, not made.** The pad reader is not run (as for its other
  stores).
- **The rider block +0x870..+0x980 is not compared from a wreck's first row** (race-jump 66,
  race-jump-side 183, race-wheelie-launch 80, s-jump 74, s-wheelie-launch 82). Why the ragdoll
  differs is R37. The rest of +0x730..+0xa80 is still compared after a wreck and matches.
- **Most s- rows match only at an instant inside the frame.** The search takes the first instant
  where every compared byte matches. It assumes the recorder's fields (body, engine, quad, wheels) were
  all read at that one instant. They are separate PINE reads; no row needed them apart.
- I did not check in PCSX2 when the scene pass, the card update or the gather run within a frame.

## Next

Nothing for MD121.
