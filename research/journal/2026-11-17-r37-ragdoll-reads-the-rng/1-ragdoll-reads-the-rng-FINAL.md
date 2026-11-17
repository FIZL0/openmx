# 1 — ragdoll reads the rng (FINAL)

Task R37. 2026-11-17.

## What I did

- Found what the interpreter's rider ragdoll lacks at the wreck: **the RNG's state**. The wreck's
  first frame draws the rider's spin with `rand()`, and the original has made more `rand()` calls by
  then than the replayed sim frames have. Not the scratchpad, VU0 or an animation pose.
- tests/atv_quad/replay.rs no longer lays MD148's "a bone touches" word (+0x920). It lays the RNG
  instead: before a listed frame it steps the LCG's state by a listed number of draws, on both
  machines (`RNG_LAG`). race-jump-side: 41 before frame 183 and 1 before frame 226. s-jump: 33 before
  frame 74.
- The replay now also holds the rider's ragdoll copies in the quad (+0x870..+0x88c, +0x930..+0x974)
  against the row that holds them, and lists the rows that differ. It asserts them for race-jump-side
  (all 300 rows equal); on the other rides they are only listed.
- All 15 rides still pass both ways. race-jump-side's put-back now lands on row 258 with nothing laid
  but the RNG.

Nothing was taken from opentennis: it has no ragdoll.

## What the original does

- **The spin.** On the wreck's first frame the rider sets its spin, rider+0x650, to
  `base + spread × (2 × rand() × 2^-31 − 1)` per axis, three draws in a row
  (`./kit dis 0x4edb40 0x4edc60 --program atv`; another wreck kind's branch is at 0x4edea0..0x4edfe0,
  with a fourth draw that flips x's sign). race-jump-side's branch has base (0, 400, −200) and spread
  (0, 50, 50) at rider+0x690 / +0x6b0; s-jump's has (−400, 100, −50) and (50, 50, 50) at +0x6d0 / +0x6f0.
- **The copy in the quad.** The rider's update writes the spin rotated by a quaternion to rider+0x660
  (`./kit dis 0x4efe58 0x4efef0 --program atv`: the ragdoll body's +0x190 on one branch, quad+0x8f0 on
  the other). The quad's +0x870 is that copy (`./kit dis 0x4f2b08 0x4f2b34 --program atv`), and +0x880
  is the ragdoll body's velocity. The spin then moves every bone, so +0x920 ("a bone touches") follows.
- **rand()** is newlib's LCG (`atv_rand` in research/oracle.txt): 64-bit state at
  *(0x393a90)+0xa8, state × 0x5851f42d4c957f2d + 1, the draw is the high word & 0x7fffffff.
- **Later draws.** race-jump-side's row 226 draws once for the rider's idle variant, quad+0x86e =
  65 + int(5 × draw) (`./kit dis 0x4eb54c 0x4eb594 --program atv`). Row 258 draws once more
  (caller 0x4eb65c); with the lags above it already matches.

## How it was proved

- **Same formula, other draws.** I inverted race-jump-side's captured +0x870 on row 183 through the
  interpreter's own quaternion. It gives (0.000002, 435.845, −218.848). x is 0, as the spread says, and
  the length matches (487.70 against the interpreter's 487.76 from other draws). So only the draws differ:
  0.858449 and 0.311524 for y and z, against the interpreter's 0.727 and 0.067.
- **Where in the stream.** Stepping the LCG from the interpreter's state before frame 183, those two
  draws are draws 43 and 44 (to 2e-6). The interpreter's are draws 2 and 3. So the original is 41 draws
  ahead. Over 4 billion draws only four other pairs come within 2e-5, all past draw 89 million.
- **s-jump the same way.** Its wreck branch rotates by quad+0x8f0 as the frame wrote it. Inverting row
  75's +0x870 gives the draws 0.107034, 0.291150, 0.968427. They are draws 34–36 from the interpreter's
  state before frame 74. The interpreter's are draws 1–3, so the lag is 33.
- **The replay** (tests/atv_quad/replay.rs), all 15 rides by the original's frame
  (`./kit check -- -p game-sim --test atv_quad -- replays_by`). With the lags and no +0x920 laid:
  - race-jump-side: body, engine and wheels equal all 300 rows. The ragdoll copies equal bit for bit on
    all 300 rows. +0x920 equals the capture on every row, so the put-back is on row 258.
  - s-jump: the ragdoll copies equal on every row but 49. Row 49 is before the wreck: a one-row phase
    flip, as MD148 saw on the s- rides.
  - Without the 41, race-jump-side's copies part at row 183, as R37 said. Without the 1, they part at
    row 226.
- Scratch probes, not committed: the frame's write log for who writes rider+0x650/+0x660 and the
  quaternion, the rand() callers per frame (the return address `rand` stores), the inversion
  (numpy), and an LCG stream search in C.

## Not verified / not 1:1

- **The lags are found from the capture, not made by the replay.** Nothing in the capture records the
  RNG. Each lag was found by inverting a captured spin, or by trying lags until the copies matched.
  Where the original's extra `rand()` calls come from is not known. They could be outside the sim frame
  (sound, particles, camera) or in a frame the recorder skipped. I did not trace them in PCSX2.
- **race-jump-side's lag of 1 at row 226 is the smallest that fits, not a proven count.** That draw
  only picks one of five variants. Any lag that picks the same variant matches. 0 does not match; 1
  matches rows 226–300.
- **The lags are laid at the frame that first reads the draws.** The original may have drawn earlier.
  Draws in between (the dust draws on frames 52–164) would then see other values. They do not change
  any captured word: body, engine and wheels match either way.
- **Rides still unlagged:** race-jump, race-wheelie-launch and s-wheelie-launch. Their ragdoll copies
  part from their first contact (rows 66, 80 and 82). Their bodies match all 300 rows without a lag.
  - I scanned lags 0–90 at those rows. The scan was wrong for the race rides: a failing test writes no
    log, and the script read that as "no rows differ". With its picks (race-jump 11, race-wheelie-launch
    1) the copies still part at once and the bodies fail after the put-back (rows 142, 159).
  - s-wheelie-launch with 53 at row 82 did pass, and its copies then equal rows 82–131. So 53 is likely
    right, with a second lag near row 132. Not added: unproven, and the spin there is not a wreck's.
  - These rides may draw at first contact by another path, not the wreck spin. Not traced. Filed as a
    follow-up (R41).
- **Rows where the copies differ by phase.** On the s- rides the copies are listed, not asserted. A
  single row can hold the frame's own copy instead of the frame before's (s-jump 49, s-jump-side 48
  and 69, s-steer and s-steer-slow a few rows each). MD148 saw the same. I did not trace the
  recorder's timing.

## Next

- Find the lags of race-jump, race-wheelie-launch and s-wheelie-launch, or better, where the original's
  extra rand() calls come from, so no lag is fitted (R41).
