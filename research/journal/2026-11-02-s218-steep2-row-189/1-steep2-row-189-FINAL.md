# 1 — steep2 row 189 (FINAL)

Task S218. 2026-11-02.

## What I did

- Copied s1's `context/capture/s58-slip/` and `context/states/s117-steep*.eeMemory.bin` into this slot,
  then ran `cargo test --release -p game-sim --test replay the_slip_cases_replay_from_the_save_state --
  --nocapture` on HEAD (c94b711b). steep2: 251 ticks, **0 of 112 lanes part**. Every S58 case agrees on
  every lane of every tick, and the test passes with steep2's ratchet at `u64::MAX` (path and slip).
- Control: I removed the late mark (`("steep2", "s117-steep2", &[])`) and ran the test again. steep2 then
  has 90 of 112 lanes parting and the test fails with "steep2: the path parts at 189". That is S179's
  part. So the whole of it is the late row, and the sim does not drift there. I reverted the change.

## Why it was already closed

S177's commit 0f6c0e73 merged after S179's eaf7fbca. It takes the chassis' euler angles from the frame's
one measure, which is S179's fix as well. It also marks the late rows it found (steep2 189, nose 180,
roll 148). The comment on `S58_OWN_STATES` records it: "steep2's row 189 holds tick 190's ride, word for
word".

## Not verified

- I did not check the late row against the csv lane by lane myself. The guard does not allow a whole read
  of the file. The evidence is S177's note plus the control above: when only that row's samples are dropped,
  everything agrees, including the ticks after 189.
- Why the recorder read late on that one row (load, or the quarter-speed poll) was not looked into.
