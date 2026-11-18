# 1 — baja pose against the original (FINAL)

Task S342. 2026-11-18.

## What I did

- Made a driven-baja RAM image: `context/states/s342-baja-free07.ee.bin` (and its `.p2s` beside it).
  - S149a's boot parks the baja on free07: `boot.py free07 2 5 21 5`.
  - `research/s342/drive.sh`: sets the cheat flag (`cheat.py`), plays `drive.pad` as a from-state tape
    (C34's switch, then 1.5 s of gas and a coast with the stick centred), saves, unpacks the RAM.
  - `research/s342/play.sh` is C28's `tape.sh` with C34's System → Reset before Play.
- Added `against_the_original::the_bajas_pose_is_the_originals` to `crates/game-render/tests/car.rs`.
  - The buggy's test body is now a shared helper (`posed_as_the_original`); both cars call it.
  - The "image is posed" check now counts nodes *turned* off the model (> 0.05 in a turn row, at least
    20) instead of nodes *moved* over 0.2 ft. The coasting baja moves only 14 nodes that far; it turns 32.
    The buggy turns 29.
- No change to `car.rs`: the port's pose already matched.
- Nothing from `../opentennis`: it has no cars.

## What the original does

- Nothing new: the baja runs S264's step (`./kit fn 0x0039edb0`), as S264 read off the shared vtable.
- A parked baja that was never switched to is not stepped: its tire objects are fresh (compression,
  spin all 0). It has to be driven for the image to show the pose step.

## How it was proved

A save-state capture: one RAM image of the driven baja, coasting on free07, steer 0, springs at FL 0.26,
FR 0.31, RL 0.71, RR 0.62 ft. All 45 nodes under the root compared to the port's pose with the buggy's
limits (0.002 in a turn row, 0.003 ft in place). Largest gaps: 0.0016 in a turn row (front hubs and
tires), 0.0024 ft in place (FL tire). The front hubs' stale marks pass the same check as the buggy's.

On the "0.68 ft low at rest" note: the rear tires here, at 0.71 ft rear compression, sit 0.029 ft from
the model's place. So the model's rear tire place matches a ~0.7 ft compression, and at 0 they drop by
about that much. This agrees with the code, but it is shown at one compression, not at rest.

## Not verified / not 1:1

- **One still frame, steer 0, as for the buggy.** The steer's turn and the stale hub marks under a steer
  are still read off the code only.
- **Rest itself is not seen.** A baja at 0 compression never shows in the game (a stepped car sits on its
  springs; an unstepped one has never run the pose step). The 0.68 ft drop at rest follows from the step
  matched here at 0.71 ft. It has not been seen on screen.
- **asin, not the acos table** (as in S264). Inside the limits here.
- **The tape ran at the copy's NominalScalar 0.3**, and the VM ran on a few seconds after the tape before
  the save. The image is a coasting car some frames after the tape, not a set tick. That does not matter
  for a still pose.

## Next

Nothing for S342.
