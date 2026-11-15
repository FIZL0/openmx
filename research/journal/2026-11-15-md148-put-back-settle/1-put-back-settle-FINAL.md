# 1 — put back settle (FINAL)

Task MD148. 2026-11-15.

## What I did

- Found what times race-jump-side's put-back. It is none of the three guesses in the task. It is the
  quad's +0x920, "the rider is settled", which the rider's ragdoll sets.
- tests/atv_quad/replay.rs now lays +0x920 after each row's frame, as it lays the put-back spot. The
  value comes from the row that holds that frame's rider copy (below). race-jump-side's upto goes from
  257 to 300.
- All 30 C28 replay tests pass (`./kit check -- -p game-sim --test atv_quad -- replays_by`).
  race-jump-side has no torn rows over its 300. s-jump still has only row 74.
- Filed R37 (low): why the interpreter's ragdoll leaves the original's at the wreck.

Nothing was taken from opentennis: it has no vehicle wreck clock.

## What the original does

- **The wreck clock** (`./kit dis 0x4e2c68 0x4e2d10 --program atv`, ported as `handling::wreck::clock`).
  With a reason set, it adds dt. If it is not counting yet and +0x920 is non-zero, then once the clock
  passes 0.5 s (2.5 s for reason 13) it starts counting from 0. Past +0xc88 (1.75 s here) it unwrecks.
  race-jump-side's capture arms on row 205 and is put back on row 258, as this rule says.
- **+0x920 has one writer**: the rider's update (`./kit dis 0x4f1210 0x4f1230 --program atv`, called
  from 0x4f28fc). It copies the rider's +0x37c into *(rider+0x378)+0x1f0, which is quad+0x730+0x1f0.
  The rider sets +0x37c to 1 when any entry of its bone list (+0x5b0 count, +0x5b4 array) has a
  non-zero +0x14, i.e. a ragdoll bone touches. Otherwise it clears +0x37c and +0x380
  (`./kit dis 0x4effb4 0x4f0020 --program atv`). Found with a temporary write watch on Ee::write: every
  frame of the replay has exactly one write, all from pc 0x4f1224, ra 0x4f2904.
- **Order in a frame**: the clock reads +0x920 before the rider's update copies it. So the clock
  arms on the frame after the one whose ragdoll touched. In the machine, row 203's +0x920 is 1 and its
  clock (0.70 > 0.5) does not arm until frame 204.
- **Why the replay put back a frame early**: the interpreter's ragdoll touches on frame 203, the
  original's on 204. The rider's copies in the quad (+0x870..+0x978) equal the capture bit for bit
  through row 182 and differ from the wreck's first row, 183 (+0x870 −225.1 against −200.2). Its first
  touch (rows 185–186) happens to land on the same frames; its second does not.
- **The recorder reads the rider at two phases.** The rider's update runs after the quad's frame:
  - On the race rides, rows hold the rider copy from the same frame. The machine's +0x930 equals row r's
    on every row before any wreck.
  - On the s- rides, rows hold it one frame late. The machine's +0x930 equals row r + 1's.
  - s-steer and s-steer-slow switch to same-frame for a single row now and then.
  - The two captures' clocks agree with this. race-jump-side arms on frame 205 off row 204's 1.
    s-jump arms on frame 90 while row 89 still shows 0: its row 90 holds frame 89's copy.
  - So laying row r's +0x920 for every ride broke s-jump: it was put back a frame late, from row 151.
    The replay now follows the phase: after each frame, it takes +0x920 from row r if the machine's +0x930
    equals row r's, and from row r + 1 if it equals that row's. If neither matches, as after a wreck where
    the ragdoll has left the original's, it keeps the last phase it saw.

## How it was proved

- The bit-exact replay fixture (tests/atv_quad/replay.rs), all 15 rides twice: the original's whole
  frame, and the port's pieces and tail beside it with every RAM word equal. race-jump-side matches the
  capture on all 300 rows.
- Without the laid word and with upto 300, race-jump-side fails from row 257, body+0x40 3189.29 against
  2817.82: the early put-back.
- Scratch probes, not committed: the clock, counting and +0x920 per row, machine against capture;
  the write watch; and the rider phase per row for every ride (race rides all same-frame before a wreck;
  s- rides one frame late).

## Not verified / not 1:1

- **+0x920 is laid from the capture, not made by the replay.** The interpreter's rider ragdoll differs
  from the original's from the wreck's first frame, so the replay cannot settle it on time. The clock's
  own arming and the put-back still run in the original's code and the port's, on the laid word. Why
  the ragdoll differs (scratchpad, VU0, or an animation pose the sim frame does not advance) is not
  known: R37.
- **The phase after a wreck is assumed to stay what it was before it.** After the wreck, the ragdoll
  copies match no row, so there is nothing to read the phase from. race-jump-side (same frame) and
  s-jump (one late) both pass on that assumption. A ride whose phase changed during a long wreck would
  lay the word a row off. No capture shows that.
- **That the rider's update runs after the PINE read on the s- rides** comes from the copies matching
  row r + 1. I did not trace the frame's timing in PCSX2.
- The other quad words that differ, and are listed rather than asserted, are unchanged (MD37b, MD120).

## Next

Nothing for MD148. R37 if anyone wants the ragdoll itself.
