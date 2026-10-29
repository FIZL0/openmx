# 1 — reverse full lock (FINAL)

Task S184. 2026-10-29.

## What I did

- **Measured first, in the shell.** I rode V70's tape (`research/v70/tape.py`, lx 255) with
  `openmx --headless --track free09 --play TAPE --frames 445`, with temporary per-tick prints (since
  removed). The prints covered the pose's steer command, both wheels' contact and slip, the chassis
  yaw rate, and the seat's position and heading. The steering is all there:
  - the shell's stick is 1;
  - the lean reaches 1;
  - the front wheel's angle is the full lock, 0.4712 rad;
  - both wheels are down and none locks or slides;
  - the yaw rate reaches −1.09 rad/s, the same as game-sim's −1.13 from the save state.

  Yet the bike only turned 62° and ended at 4314,3313.
- **The cause is the tape.** Every line of V70's tape carries a frame length of `0x3c888889` (1/60 s),
  but `tape.py` schedules its 30 Hz ticks (`t / 30`). The shell ticks on the frame length a tape gives
  it (`played_dt`, main.rs `simulate`), so the whole schedule ran in half the game time:
  - 0.45 s of gas launched the bike 10 ft (to z 3326.7) where the original went 23 ft (to 3342);
  - the 1.25 s back-up turned about 60° where the original turned 130°.

  The shorter launch and the half arc happened to end near the original's bars-centred stop, which is
  what made it look like "backs nearly straight". None of the three suspects drops the lock: not the
  shell's reverse input, not the tape's stick, and not the ride's reverse steer.
- **The fix:** `research/v70/tape.py` now stamps each line `0x3d08850a` (0.03333, the 30 Hz cap a
  level ticks at). The shell then rides the original's arc on the same tape:
  - **Bars full right (lx 255):** the bike stops at z 3341.6 (the original's rear wheel stopped at
    3342), backs round to 4330.3,3329.4 at a heading of −150°, and ends at `end: 4330, 208, 3329`.
  - **Bars full left (lx 0):** the mirror, ending at 4293, 208, 3323.
- **Captured the original's back-up.** `research/s184/backup.pad` is the same schedule at a quarter
  speed (all times ×4), and `research/s184/capture.sh` records it with S68's recipe and lanes
  (`watch/s68-backwards.toml`, S68's `setup.py 0.25`, copy 2's slot 8 ← first_recording):
  - Output: `context/capture/s184/backup.csv`, 290 rows, frames 2775..3064, 0 torn and 0 gaps. It is in
    s2 only and not committed.
  - The first attempt had 5 counter gaps (29 frames) at load ~37, and `PadTape::from_capture` refuses
    a gap, so I recorded it again.
  - Over the 2.5 s of reverse at full lock the original turns from −10° to −144°, and backs from
    4309,3342.9 round to 4328.5,3329.8.
- **Test** `crates/game-sim/tests/replay.rs` `a_full_lock_back_up_rides_the_originals_arc`: it plays
  the captured pad from the save state, one tick behind its row as in S58/S68, then checks every
  captured tick:
  - the bike's ground position within 0.25 ft and its heading within 0.5°, against the nearer of the
    row and the next one;
  - no wreck;
  - more than 120° turned on the full-lock reverse rows, in both games.

  Measured: 289 ticks, at most 0.088 ft and 0.07° off; on the lock the port turns 129.9°, the
  original 129.8°.
- **Follow-up S219:** `research/v40/tape.py` and P72's `capture/port-tape.py` have the same 1/60 s
  stamp on a 30 Hz schedule.
- **Taken from opentennis:** nothing; it has no reverse or tape-length question.

## What the original does

Backing up at full lock it turns hard: about 130° in 2.5 s at about −12.5 ft/s. The front wheel holds
the full lock (`f_steer` 0.4712), and the steering fade at negative speeds is `steer_scale_1` (0.9).
The port's ride already does all of this: its steering command and the turn agree with the capture
tick by tick. There is no port change.

## How it was proved

- **A PINE recording compared tick by tick:** `context/capture/s184/backup.csv` (290 rows) and the
  test above (0.088 ft / 0.07° worst over 289 ticks).
- **The shell on the corrected tape:** run by hand. With lx 255 it ends at 4330,3329, heading −150°;
  with lx 0 it ends at 4293,3323. The 1/60 s stamp was shown to be the whole cause by changing only the
  stamp: the same tape with `0x3c888889` → `0x3d08850a`, and nothing else, went from 4314,3313 to
  4330,3329.

## Not verified / not 1:1

- **The shell run is not compared tick by tick with the original.** The shell spawns a fresh rider
  at the track's spawn (4310,3316), not at the save state (4309.6,3319.6), so its ride starts 3.6 ft
  from the capture's. I compared the end point and the heading only:
  - shell 4330.3,3329.4 at −150°;
  - game-sim from the save state 4329.9,3330.3 at −146° (stick 127 of 128);
  - the original's rear wheel in V70's wall-clock run, 4330.7,3334.5.

  No test drives the shell over this tape. The shell's pad path is covered only by this hand run.
- **V70's own original-side numbers came from a wall-clock pad** (`./kit pad script` at full speed),
  not this capture. Its end point (rear wheel 4330.7,3334.5) was compared by hand, not in a test.
- **The capture releases every button after the back-up**, where V70's pad holds the brake. The
  coasting tail after the lock (ticks ~225–289) is the original's coast and not V70's brake. It agrees
  as well, but the brake-held tail was not captured.
- **The capture is at NominalScalar 0.25 (S68's recipe), not full speed.** S58/S68 already rely on
  the quarter speed riding the same ticks. It was not re-proved here.
- **The test skips** outside a slot that has `context/capture/s184/backup.csv` (s2 only), and also
  without the acceptance capture, the save state or the disc. A skip passes.

## Next

none
