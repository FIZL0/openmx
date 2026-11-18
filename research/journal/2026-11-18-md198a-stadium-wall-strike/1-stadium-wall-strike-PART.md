# 1 — stadium wall strike (PART)

Task MD198a. 2026-11-18. Blocked: tick 256's body is still off. Two real fixes landed on the way.

## What I did

- Read the quad record's contact list, flags and wreck words for hazard-time rows 253–258 from MD224's
  capture (context/capture/md224/hazard-time.csv) and MD147's image, beside a throwaway probe test that
  printed the port's record, wreck, body and wheels each tick.
- Fix 1: the port never gave the record its heightfield flag. `Ride::record_on` / `record_among` now set
  it from `Quad::tipped` (what the last air state left), as the original's PostProcess writes record
  `+0x24`.
- Fix 2: the part swapped in by the wreck keeps its last-synced frame for one pass. `Ride::swap` syncs the
  outgoing part's frame and sets `Recorded::kept`; the next pass stands the world rays at the part's own
  frame (`RecordOn::stood`) instead of the node's. On tick 256 the port now files 0 contacts, like the
  original (it filed 7 before).
- `Recorded` grew a bool (`kept`), so the snapshot wire grows by one byte.
- The test stays at row 254: the body on row 256 is still 35 words off.

## What the original does

- PostProcess (`./kit fn 0x004dbdd0`, via 0x004f1270 / 0x004e9790) writes record `+0x24` = up.y ≤ 0.25
  every tick. The record's next pass reads it to turn the heightfield rays on.
- A part's frame (collidable `+0x130`) is synced only while it is active. The wreck rays (0x01c45ec0)
  swapped in on tick 255 still hold their identity frame at (395.8, 3.76, 155.9) on tick 256's pass, far
  from the quad: no contacts.
- Wreck entry `./kit fn 0x004fa390`: vt +0x4a0, vt +0x4a8(−1), vt +0x398(0) (chassis mesh out of the
  world), vt +0x484 (swap to the wreck rays), zeroes, then the base wreck 0x004cbc60. The base wreck's
  damping change needs quad `+0xbac` ∉ {0, 1}; it is 1 here, so nothing changes.
- Wreck slow motion: dt per tick after the wreck falls from tick 257 (0.0324, 0.0315, 0.0306, 0.02967, …;
  read as obj0 `+0x448` deltas). Ticks 255–256 use the full dt.

## How it was proved

- Bit-exact replay fixture: `untamed_quad_replays_md147_hazard_time_through_the_put_back` still passes
  to row 254.
- Tick 255 (the strike): the wall contact and the record's push-out agree with the captured list; hit
  points differ by ≤ 2e-5 along the normal. Row 255 is a mid-tick read, so the body is not compared there.
- Tick 256: record contact count 0 on both, after fix 2.

## Not verified / not 1:1

- Tick 256's body: 35 words off. Port pos (213.99, 3.15, 128.23), vel (10.58, 7.35, −25.12); original
  (214.68, 3.64, 128.13), vel (11.79, 6.00, −26.38). The original gets more push, mostly in x, during
  the substeps. Not the record (empty on both) and not the tire forces (dropping them makes it worse).
  Best lead: the wheels' own sweep against a struck object (wheel `+0xd4`/`+0xdc`), which MD247 found
  unported for the bale. Also unchecked: the chassis mesh leaving the world on the wreck.
- The wheel suspension spring differs on row 256 (382.2 vs 377.15): only a result of the body speed
  being off, not a cause.
- Per-tick dt after the wreck is not fed to the replay yet: needed to run past row 256.
- Tick 255's hit points differ by up to 2e-5 along the normal: too small to move the push-out words seen.
- MD224's hazard-time capture differs from MD147's on 79 rows; this work used MD224's.
- Heightfield tips are still stood at the node's frame, not a stale part frame: no case needed it yet.
- Knocked records (bales) never use the stale frame (`stood: None`): not checked against the original.
- The riding part's frame on an unwreck is the node's frame at swap time: an approximation, not traced.

## Next

- Capture tick 256 per substep in the interpreter (MD147's image, the quad world's step) and read the
  four wheels' `+0xd0..+0xdc` and contact points against the port's. Port the wheel sweep's hit on the
  wall if that is where the push is.
- Then feed per-tick dt (obj0 `+0x448` deltas) and extend the test to row 369.
