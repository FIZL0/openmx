# C32 — atv retear

2026-11-12. Task: `./kit plan C32`.

I re-recorded the three ATV rides that MD37b's replay stops early on (s-wheelie-launch, s-stoppie-short,
race-jump-side), with the same tapes and the copy at NominalScalar 0.06. The torn rows came back byte
for byte the same as C28's at 0.12, and race-jump-side's whole csv is identical. So these are not tears a
slower re-record can fix.
- The s- rides' rows are real mid-frame states, read at a fixed point in emulated time.
- race-jump-side's stop is a divergence in the replay: the original's frame puts the quad back a frame
  early.

The new csvs were not kept, and the upto values stay as they were. The test's comment is corrected.
Follow-ups: MD148 (the put-back) and A7 (mid-frame rows).
