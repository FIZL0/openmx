# 1 — SLUS-21104 bike replay (blocked on the port)

Task F31. 2026-11-03. Slot s3.

## What I did

### Re-recorded C15 (s3, copy 3)

- State: `SLUS-21104 (95BB1901).04.p2s` from ps2-decomp-kit-slots/pcsx2/s9, md5 73389cd1… (C15's).
  `context/mxvatv-ps2/state04.ee.bin` + `.spr.bin` were extracted from it (a .p2s is a zip; eeMemory.bin).
- The disc's files, through `ps2kit.disc.Disc` and `tools/mxvatv_overlay.py`:
  `context/mxvatv-ps2/{SLUS_211.04, GAME_REL.BIN, atv_game.elf}`. `context/elf/atv.elf` (a copy of
  atv_game.elf) is what `./kit dis --program atv` wants, with a KIT_CONFIG that has `[other.atv]`.
- Tapes: `python3 -I research/c15/tape.py <570-byte header of the main checkout's animandpump.p2m2>
  research/c15/<t>.pad context/recordings/c15/<t>.p2m2` (+ `.ticks.csv`, `_SaveState.p2s` = the state).
- Captures: C15's recipe (its journal) in PCSX2 copy 3 with a scratch KIT_CONFIG (serial SLUS-21104,
  the MX vs. ATV Unleashed iso, vsync_addr 0x3fa6d8). Results: `context/capture/c15/ride-{1,2,3}.csv`
  and `flip-{1,2}.csv`.
  - ride-2 == ride-3 and flip-1 == flip-2, byte for byte.
  - **ride-1 tore on row 1** (vsync 13111: obj0, body, record, wheels and engine were read mid-update).
    Use ride-2.
  - Rows, vsyncs and events are as C15 describes (459 rows 13109..14025; 465 rows 13109..14037).
- Backup (git-ignored data): `~/repos/openmx-slots/kept-context/c15/{capture,recordings,mxvatv-ps2}`.

### The original's frame in the interpreter

- The physics world in state 4 is at 0x00665a4c (MD37a's 0x0065bc0c is another image). It was found by
  scanning for (1 iteration, 115/s) with the bike's body in its list.
- The world step alone is not the frame. The per-frame sim update is the sim object's vt +0x40
  (0x00204040 on the object at world − 0x6c), and it reproduces row 0. The calls inside it are in
  research/oracle.txt `# F31`.
- dt: 1/30 left ulp-level misses in the body. A search over dt bits (body block vs row 0) gives
  0x3d08850a, the 30 Hz tick MX Unleashed's port already uses.
- Hooks the frame needs: DIntr/EIntr (mfc0); every syscall stub (WaitSema from the heap allocator,
  GetThreadId); and the bike's network snapshot send (vt +0x544), which reads the T0 counter and writes
  nothing of the bike's.
- Chained over the rows, poking from each row only:
  - **the pad reader's stores** (the reader 0x004e36c0 runs outside this frame; its store list was read
    off its disassembly);
  - **the put-back spot** +0xd38..+0xd6c, row r's into frame r + 1.
- Steps on the way:
  - Poking +0x308..+0x318 and +0x12c0..+0x12cc (the limiter's outputs too) parted the ride at row 311,
    where the brake comes on. Raw inputs only: no parting.
  - Without the reader's +0x12b0 (the flip hold) the flip parts at row 79, where the backflip fires.
  - Without the spot, the put-back on row 193 lands on state 4's spot.

## How it was proved

`cargo test --release -p game-sim --test mxvatv_bike_ride` (debug too; 3 tests, ~4 s):

- `ride_replays_bit_exact`: 459 rows; body, node, collision record, front and rear wheel and engine
  are all word-for-word equal every row, and the bike never wrecks.
- `flip_wrecks_and_is_put_back_bit_exact`: 465 rows the same. The interpreter's wreck state (collision
  record +0xcc = wreck part +0x1434 or riding +0x1438) equals the row's on every row, and it changes on
  rows 125 and 193 only.
- `ticks_land_one_row_later`: for every tick t of both tapes, row t + 1's reader words are tick t's pad.
  Cross is gas 1.0 and square is brake 1.0. lx/ly are the stick at +0x12a8/+0x12ac. R1 is trick bit 4.
  While wrecked (row t's state), gas and trick read 0. The same check against row t fails.

The port's code is untouched (only a test and comments were added), so Original rides keep every replay
hash.

## Not verified / not 1:1

- **No Rust replay.** The acceptance's "C15's ride replays bit-exact on Handling::MxVsAtv" needs the port:
  - a Ride seeded from SLUS-21104's bike object (params from its files, state from the image);
  - SLUS-21104's frame driver and substeps around the clauses handling::step already has;
  - MD63e's mass properties, MD63f's wreck gravity, the put-back;
  - the ground the bike rides (state 4's world, through the atv collision).

  The interpreter replay is that port's per-row oracle; filed as follow-ups.
- **Outside words.** Some obj words differ from the rows and stay wrong: +0x44 bit 0x400, +0x6f8/+0x6fc,
  +0x790.., +0x870..+0x898, +0x908/+0x14e4, +0x930..+0x978, +0x940, +0xd90..+0xd98, +0x12b8/+0x12bc.
  They are written outside this frame, and nothing in the frame reads them: the physics stays bit-exact
  with them wrong. Who writes them (rider, camera, effects) was not traced.
- **The reader is fed, not run.** Its outputs come from the rows. The test ties them to the tape, but the
  reader itself (0x004e36c0, pad record through 0x00118e10) was not run on the tape.
- **Who writes the put-back spot** each frame was not traced. It is fed from row r into frame r + 1,
  which the replay shows is right.
- **Audit:** `./kit audit` fails on HEAD's own leaks (crates/game-sim/src/atv_wheel.rs, plan/DONE.md);
  this task adds none.
