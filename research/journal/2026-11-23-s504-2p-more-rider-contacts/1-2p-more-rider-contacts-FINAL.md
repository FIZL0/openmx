# 1 — 2p more rider contacts (FINAL)

Task S504. 2026-11-23. Three more 2p rider-vs-rider contacts in C10's 2p state, after S158's rear strike:
a head-on hit, a landing on a standing rider, and a side swipe. All three replay as S88's tests pass.

## What I did

- **The tapes.** `context/recordings/split/s504/{head_on,landing,swipe}.p2m2`. Each pad script is kept
  in `research/s504/<ride>.p1.pad` and `<ride>.p2.pad`; `research/s158/tape2.py` and `research/s158/make.sh`
  rebuild the two-pad tape from them.
- **Growing the tapes.** `research/s504/pilot.py` plays the tape so far, reads both riders over PINE, and
  picks the next ticks' pads from a policy in `research/s504/scenarios.py`. The policies were edited
  between steps, so the saved pads, not a rerun, are the tapes.
- **Aiming.** `research/s504/sweep.py` re-steers P's stick x over ticks A..B and plays each variant. Steering
  does nothing in the air, so the aim is set before takeoff.
- **The captures.** `context/capture/2p/s504-<ride>.csv`, made with `research/c10/play_capture.sh` on
  kit copy 3 at NominalScalar 0.05, FIRST 127882, N 440, 500 and 1210.
- **The images.** `research/c10/play_at.sh` saved states at contact −6, −4, −2, −1, 0 and +1, and
  `research/s158/rams.py` (now takes `KIT_PCSX2` and a TASK argument) turned them into
  `context/states/s504/<ride>-<frame>.ram`.
- **The tests** (`crates/game-sim/tests/rider_rider_capture.rs`):
  - six named contacts now, S158's rear strike among them, each through all four of S88's tests;
  - two pre-contact images each in the step-order test, 17 images in all.
- **The port.** `crates/game-sim/src/rider_contact.rs` gained `gather_in(seats, order, dt)`, which takes
  the search order as an argument. The port's sim itself still searches in player order.

## The three contacts

- **Head-on, frame 128255.** P1 at 38.7 ft/s, P2 at 44.4, both airborne. Each list holds two mirrors and
  two of its own rays. P2 wrecks (reason 1, over the bars). P1 stays up at 6 ft/s.
- **Landing, frame 128315.** P2 comes down at 57.6 ft/s onto P1, who is standing. P1 has one ray on P2;
  P2 holds its mirror. P1 wrecks (reason 7). P2 rides on at 45 ft/s.
- **Swipe, frame 129028.** P1 at 47.1 ft/s, P2 at 50.3, level and converging about 10 degrees. They sit
  about 1.3 ft apart across and 2.4 ft along. Each has one ray and one mirror. Nobody wrecks. The contact
  lasts 8 frames and recurs from 129052.

## What the original does

- **The search order.** The original searches a collision node's list in its own order, not player order.
  In the 2p state that is P2 then P1. In S93's split state it is P1 then P2, except in images 4661 and
  5074, where P2 comes first.
- **Why it went unseen.** Earlier contacts could not show the order in the 2p state, because only one
  bike's rays hit in them.
- **The landing rows.** The rows read at the same phase on all four recordings, and the four are
  identical. They hold P2 one frame on, because they are read between P2's update and P1's. The test
  therefore compares P2 before with the earlier row, and P2 after with image 128315 (the `after` field).

## How it was proved

Save-state capture and bit-exact replay in the interpreter (`cargo test -p game-sim --test
rider_rider_capture`):

- all six named contacts pass every one of S88's tests;
- the step-order test covers 17 images, two before each contact;
- the landing's `after` check uses image 128315 for P2, to take account of the one-frame lead in its rows.

## Not verified / not 1:1

- **The search order is a finding, not a fix.** The port still searches in player order. What moves the
  original's order is not known; S513 has it.
- **Only the three contacts above were recorded.** Other contact kinds and other 2p rides are not covered.
- **The Hyprland output was a workaround.** There is no monitor here, so a Hyprland headless output was
  created for the AT-SPI-driven copy and removed afterwards.

## Next

- S513: what sets the original's search order, and whether the port should follow it.
