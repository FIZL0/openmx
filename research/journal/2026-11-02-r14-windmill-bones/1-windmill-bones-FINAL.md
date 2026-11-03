# 1 — the windmill's bones (FINAL)

Task R14. 2026-11-02.

## What I did

- `research/r14_windmill_bones.py` is research/s167_windmill.py's run with more columns: the
  chassis' cofg, the published main bone (rider +0xd40), the wreck reason (+0x748), the body actor's
  state words, the nine bone collisions' touched flags, and all 18 bones' +0x60 position and +0x70
  velocity. The body actor is the vehicle − 0x400 (vtable 0x00458eb0), with its parts at +0x2f4
  (stride 0xb0, body at +0xa4) and its collisions at +0x624.
  - With `bones`, the teleport also moves the triples near the bike in the actor's neighbourhood,
    the parts and their bodies, and the collisions.
  - After the load it rides 30 frames before it pauses. A previous run leaves the VM paused, so it
    presses Pause to resume first.
- S167's own slot-5 state is gone (overwritten on 10-29). I ran from recording2's state instead,
  copied over slot 8 of kit copy 3, and teleported to S211's south spot (2439, 140, 4067.5):
  - `context/capture/r14/south.csv` (+ `-before.ram`, `-after.ram`): without `bones`, frame 2373.
  - `context/capture/r14/south-bones.csv` (+ images): with `bones`, frame 2403.
- `tests/replay.rs`:
  - `s211_ride_in` is `s211_ride` from any image.
  - `r14_capture` reads a csv by column name.
  - New test `the_windmill_wrecks_the_bones_with_the_bike`.
  - S211's doc comment and pins now say the original's ticks are the teleport's.

## What the original does

- **south (S167's teleport).**
  - Row 1: every bone is still at the old spot (4309, 211, 3314).
  - Row 2: the main bone is on the bike. Bone 1 is 1,166 ft away, strung along the teleport line.
  - Row 4: the main bone's velocity is (−14118, −464, 5802) ft/s, along the delta.
  - The main bone runs west past x = 150, and reason 13 appears on row 12. S167's south3 is the
    same thing on row 14.
  - So the original's 14 and 85 in S211 measure how far the teleport moved the bike, not the
    windmill's mover on the bones or the bone search. I did not log apex's bones, but it is the
    same teleport from the same state.
- **south-bones (bones moved).**
  - Wreck 4 on row 3, the windmill's from-rest throw: (7.80110, 18.8411, 133.375).
  - The bones follow the wreck at ~107–130 ft/s, with no touches early on.
  - No reason 13 in 188 rows. The main bone's x stays within 2439–2551.

## What the port does

- **From south-bones-before.ram.**
  - It wrecks on its tick 1, so port tick `t` is csv row `t + 2`. That is one tick early against
    S211's `t + 1`. The wreck velocity matches row 3 (7.80083 against 7.80110).
  - The main bone is 0.93 ft from the original's on tick 1. It drifts ~0.35 ft a tick, and is
    9.43 ft away on tick 30.
  - The bone velocity on the wreck tick is the bike's (7.77, 17.94, 133.47). The original's main bone
    is at (6.14, 15.08, 106.70), about 0.8 of that (cf. the 0.8 of captured bike_vel).
  - No cannonball over 40 ticks. The bones touch from tick 34.
- **From south3-before.ram (S211's).**
  - Wreck on tick 3. On tick 5, bones 0 and 7 touch, and bones 0 and 12–17 leave at
    10,000–100,000 ft/s. The main bone is at (2487, 171, 4316) with velocity (3703, 3576, 12908).
  - That fling is the port's tick-8 cannonball.
  - Without the windmill there is no wreck, so the strike is from the windmill or the ground
    after it. There is no original to set it against: the original's bones in that capture were
    left behind, and R14's bike, from another state, is not struck this way.

## Checked

- `cargo test --release -p game-sim --test replay`: 116 passed, 1 ignored, including the new test
  and S211's.

## Filed

- B15: the port's tick-5 bone strike from S211's south3 image: re-capture the original with `bones` from a
  state that wrecks the same way.
- B16: the port wrecks a tick early from R14's south-bones image, and its bone's wreck-tick velocity is
  the bike's, not ~0.8 of it.
