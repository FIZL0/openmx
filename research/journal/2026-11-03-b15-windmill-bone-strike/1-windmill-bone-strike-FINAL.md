# 1 — the windmill's strike on the port's bones (FINAL)

Task B15. 2026-11-03.

## Finding S167's state

- S167's slot 5 (copy 1) is gone. South3's image is not recording2's playback at its frame, 2734.
  I played recording2 to 2733 and found the bike at (4208, 186, 2903), nose-down.
- The image's human bike is pitched ~39° nose-up and rising at ~30 ft/s. That is S150's farmyard
  throw: recording2's rider teleported to (4642.5, 135.6, 3650) and thrown up.
- s1's `context/capture/s144/midback-2734` save state holds the same rider. Its bones' positions and
  velocities equal south3-before.ram's bit for bit, so it is the slot-5 state.

## The capture

- I copied that state to s3's `context/capture/b15/` and into copy 3's slot 8.
- `research/r14_windmill_bones.py` gains `ride=N` (default 30, R14's ride before the pause).
  `ride=0` pauses before the load and teleports on the loaded frame, as S167's script did.
- The run:
  `KIT_PCSX2=3 ./kit pcsx2 with -- python3 -B research/r14_windmill_bones.py PID 8 2439 140 4067.5 30 context/capture/b15/south3-bones.csv bones ride=0`
- Output: `context/capture/b15/south3-bones.csv`, 225 rows from frame 2734 with no gaps, plus the
  `-before` and `-after` RAM images. The script moved 95 triples.

## What the original does

- Rows 2–13 have the same chassis velocity as S167's south3.csv to its six printed figures.
  - Row 2: fall, vy −0.994.
  - Row 3: fall, vy −2.088.
  - Row 4: the throw, (7.79299, 18.6087, 133.41), with reason 4.
  - S167 put the bike back on its row 15; this run never does, because the bones went with the bike.
- On row 4 the body actor's riding word is still 1. The main bone goes at (0.79999995, 0.79999995,
  0.8) of the thrown bike. On row 5 riding is 0, so the wreck entry ran. This is R14/B16's one-row
  lag again.
- The bones ride with the wreck body at 107–149 ft/s and touch down from row 6 (touch 257, 32, 112…).
  No bone goes faster than 149.2 ft/s in 225 rows, and there is no reason 13.

## What the port does

- From south3-bones-before.ram, ticks 1–3's chassis velocity equals rows 2–4 bit for bit, and the
  wreck (4) comes on tick 3.
- Its bones enter on tick 3 from the kit's pose at rest. This test runs no riding ragdoll (B16).
  They stay at the windmill's south face, z ≈ 4064–4067, while the wreck body flies off.
- Bone by bone against the csv (port tick t, row t + 1; the bone order checked by the riding pose's
  shape on row 3, with the arms and legs in the same order):

  | tick | nearest–farthest bone | fastest port bone |
  |------|------------------------|-------------------|
  | 3    | 2.59–6 ft              | 3 ft/s            |
  | 4    | 5.78–10 ft             | 8 ft/s            |
  | 5    | 5.20–1639 ft (touched) | 106,464 ft/s      |
  | 6    | 30–3430 ft             | 74,995 ft/s       |
  | 8    | 439–4466 ft            | 38,809 ft/s       |

- On tick 5, bone collisions 0 and 7 touch the windmill (bone 7's normal is (0.05, 0.14, 0.99), its
  south face). The pushes are (2395, 6452, 44374) and (14683, 3803, 38155) ft/s. The joints carry
  that into bones 12–17. The cannonball follows on tick 9.
- The windmill's body has never stepped (`Bodies(0)`; S211 leaves the toy undrawn, as the original's
  was until the wreck camera saw it). So its mover answers its place × 30 (S167).
- Control: the same ride with the body counted as stepped twice from the throw on. Bones 0 and 7
  still touch the face, but no bone goes faster than 20 ft/s through tick 9.

## Why

- The port enters the bones a tick early, from the pose before the throw (B19). The original's bones
  ride one more tick on the thrown bike, so they leave the face at ~107 ft/s before the entry and
  never meet the windmill.
- The strike needs both:
  - bones left at the face;
  - a windmill body that has not stepped since the load.
- In the shell the windmill steps once it is drawn, so its mover is still. The strike is mostly this
  replay's set-up, but its root cause is B19.

## Checked

- `./kit check -- -p game-sim --test replay`: 119 passed, including the new test, S211's and R14's.
  The new test ran rather than skipped: its measurements printed with `--nocapture`.
- The new test asserts:
  - the original's run is S167's to six figures, with the throw's wreck, the one-row actor lag and
    the 0.8 carry;
  - the original's bones stay under 200 ft/s, with no put-back, and touch down;
  - the port's ticks 1–3 are bit-exact;
  - the port's bones are more than 2 ft off by tick 4;
  - the port's strike on tick 5 (touched, faster than 10,000 ft/s) and its put-back on tick 9;
  - the stepped-body control stays under 20 ft/s.

  The strike, the put-back and the 2 ft gap are pinned to fail when B19 lands, and the test is then
  B19's second proof.

## Not verified / not 1:1

- The entry itself (B19) is not changed, so no bone is compared bit for bit after tick 3.
- The original's ragdoll orientations and rates are not captured; positions and velocities only.
- Whether the original's bones would be flung by an unstepped body had they been at the face is not
  shown. Its bones never touch the windmill in this run.
- Only the south spot was re-captured. Apex was not.

## On the way

- `./kit pcsx2 start` without `KIT_PCSX2=N` starts the user's own emulator config. I first ran the
  recording2 playback there, and its `save_state(8)` replaced slot 8 in `~/.config/PCSX2/sstates`. I
  put the previous file back from PCSX2's `.backup` copy (11-02 06:27, byte for byte) and redid
  everything in copy 3.
- PINE's `pause()` needs the uinput pad. Pressing System → Pause over AT-SPI stops the VM about a
  frame after it is pressed.
