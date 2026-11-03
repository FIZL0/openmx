# 1 — the fliers' follow camera, captured and replayed (FINAL)

Task MD82c. 2026-11-03.

## What I did

- **The states.** MD80a's states were gone from every slot's `context/`. MD80a had made them in PCSX2
  copy 1, though, and its save slots were still there. I matched them by vsync and model word with
  `research/md80a/ram.py` and copied them into `context/mxvatv-ps2/md80a/`. `ram.py` also wrote each
  one's `.ee.bin`.

  | state | copy 1 slot | vsync | flier | model word | follow camera | its target |
  |---|---|---|---|---|---|---|
  | options | 01 | 23611 | — | — | — | — |
  | biplane | 02 | 27579 | 0x014de810 | 1 | 0x01c398c0 | 0x014df1d0 (flier +0x9c0) |
  | biplane3 | 03 | 29999 | 0x01b13790 | 0 | 0x01446950 | 0x01b14150 |
  | kiowa | 04 | 27759 | 0x01ab7d20 | 1 | 0x0147c580 | 0x01ab86e0 |
  | kiowa3 | 05 | 30181 | 0x014c8f00 | 0 | 0x01c5aae0 | 0x014c98c0 |

  Each state holds one follow camera, with MD82b's vtable word 0x003cc6f0 at +4, and its target block
  is the player's flier +0x9c0. The fliers have no camera class of their own.
- **The captures.** `research/md82c/fly.sh NAME` finds the state's camera, builds the tape from
  `research/md80a/NAME.pad` (research/c15/tape.py, the same tape MD80a flew) and plays it with
  `research/md82b/play.sh` and `cam.toml`. I ran it in my own PCSX2 copy 1, with a scratch kit.toml
  (serial SLUS-21104, the MX vs. ATV iso, vsync 0x3fa6d8, elf SLUS_211.04) and a PATH shim for
  `agent-runner home slots` (T7). The captures are in `context/capture/md82c/` and the tapes in
  `context/recordings/md82c/` (git-ignored).
- **The camera files.** MD82a's `context/mxvatv-ps2/md82/cam/` was gone too. I copied
  `FC_BiplaneDefault.pb` and `FC_HeliDefault.pb` from this slot's flier pack (`mods/machines/mxvatv_fliers/Cam`).
  MD80d's modcheck compares those byte for byte with the disc. Desert Oasis's map (`md37a/mc02.res`)
  came from s3.
- **The tests.** `tests/mxvatv_camera.rs` now reads captures by folder and replays MD82c's four after
  MD82b's seven. `tests/mxvatv_camera_presets.rs` takes the four md80a images as well, which closes
  MD82a's "no state for the biplane or helicopter".

## The captures

`python3 -I research/md82c/summary.py` prints:

| capture | rows | vsync | target | preset | airborne | speed | under 29.33 | yaw | distance |
|---|---|---|---|---|---|---|---|---|---|
| biplane | 1625 | 27581..30829 | every row | 0 | 1155 | 0.50..138.76 | 384 | π | 32 |
| biplane3 | 2338 | 30001..34675 | every row | 0 | 1981 | 0.52..113.98 | 334 | π | 32 |
| kiowa | 1265 | 27761..30289 | every row | 0 | 0 | 0.56..76.93 | 342 | π | 60 |
| kiowa3 | 1380 | 30183..32941 | every row | 0 | 0 | 0.02..75.94 | 511 | π | 60 |

- The row spans are MD80a's own, row for row.
- On every row, in all four captures: no dropped tick, no held row, the still bit 0, snap 0, the pitch
  lock 0 and the near clip 1.0.
- The Kiowa's camera target never says airborne. The word is 0 on every row, even 400 ft up, so its
  heading is always the body's forward. The biplane's says airborne from row ~103 of its flight.

## How it was proved

- **Replay.** `cargo test -p openmx --test mxvatv_camera -- --nocapture`: each flight replays bit for
  bit. That covers distance, yaw, pitch, lift, the look lags and their taus, the position lags, the
  viewport eye and the fov.

  | capture | rows replayed | from row |
  |---|---|---|
  | biplane | 1624 | 0 |
  | biplane3 | 2337 | 0 |
  | kiowa | 1263 | 1 (row 0 torn) |
  | kiowa3 | 1379 | 0 |

  MD82b's seven skip here because their captures are gone from every slot.
- **Presets.** `cargo test -p openmx --test mxvatv_camera_presets -- --nocapture`: in each md80a image,
  the camera loaded `Cam\FC_BiplaneDefault.pb` or `Cam\FC_HeliDefault.pb`. Its 8 records match
  `Preset::read_layout` field for field, and the current preset is 0.
- **Mutations.** Each was reverted afterwards.
  - Turning off the airborne velocity heading fails biplane at row 103.
  - The look swing's −0.7 → −0.6, with only the Kiowa captures listed, fails kiowa at row 208.
  - `MOD_LAG_SPEED` 29.3332 → 29.0 stays green (see below).

## Not verified / not 1:1

- **The lag-tau speed scale is still unexercised.** Speeds under 29.33 are plentiful (334–511 rows a
  flight). But the yaw is π on every row, so the rear fade is 1, and the blend runs from 1 to 1
  whatever the speed. The mutation above stays green. The same holds for the look-ahead's rear fade
  and the swing fade. Only a camera swung toward the front would test them, and the tapes never touch
  the right stick.
- **The Kiowa never takes the airborne heading.** Its target's airborne word is 0 throughout. So the
  velocity-heading branch is proved by the biplane alone. Why the helicopter never sets it was not
  looked into.
- **Never set:** the still bit, snap, the pitch lock, a held row, a wreck (null target), the reverse
  swap (no row under 0.0001 ft/s), and presets other than 0.
- **Coverage:** only 1p 4:3, Desert Oasis Free Ride. The view direction was not compared, only the eye.
- **Not wired into the shell.** That is MD82/MD82d.
- **The states come from copy 1's save slots, not from a fresh run of `state.sh`.** They were
  identified by vsync, model word and flier positions, which match MD80a's table exactly.

## Next

Nothing for MD82c. MD82 still needs the shell wiring (MD82d). A right-stick tape would cover the fades.
