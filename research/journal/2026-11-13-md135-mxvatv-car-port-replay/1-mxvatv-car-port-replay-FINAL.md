# 1 — mxvatv car port replay (FINAL)

Task MD135. 2026-11-13.

## What I did

- `tests/mxvatv_car_frame/drives.rs` (new): one test per MD107 drive (buggy, baja, sandrail, golf cart,
  monster truck). Each one replays the drive in the interpreter the way `tests/mxvatv_car_replay.rs` does
  (the image, then SLUS-21104's sim frame, with input fed off the tape through the port's input step). The
  world step is hooked. In the hook the test reads the car, body, pieces (engine and tire keys and state)
  and vehicle words off the machine, runs the original's world step from inside the hook, then runs
  `game_sim::atv_car::frame` on what it read. Every word the port keeps must equal the original's after
  every frame. This is the quad's pattern from `tests/atv_quad/replay.rs` (its WorldStep beside).
- `Pieces` and `atv_car::Own` answer what they port. The `Oasis` from `whole.rs` answers the ground
  queries (atv_ground on Desert Oasis, mc02.res) and the smooth noise. The sort's search is answered with
  the original's own post-step answer (`sort_answer`: the struck flag and each hit's ray, point, normal and
  record). Every other call is handed out, and must be in a fixed list of 17.
- The helpers in `mxvatv_car_frame` read the buggy's car only. They now read through a thread-local
  `THE_CAR` (`car_at()`) in `main.rs`, `own_calls.rs` and `own_hand_outs.rs`. It defaults to the buggy
  image's car, so the existing tests are unchanged.
- MD107's drive helpers (Drive, the five drives, image/tape/rows, `step`, and the rest) moved from
  `tests/mxvatv_car_replay.rs` into `tests/mxvatv_car_replay/drive.rs`, so that both test crates include
  them. MD107's test itself is unchanged.
- When the record holds contacts (count at record +0xf0 non-zero), the test reads them with
  `handling::record::held`, and also compares what the port's resolution writes (`record::held_words`).
  With no contacts it keeps the reader's empty list.
- Follow-up filed: MD156, to step the car's update beside each drive once the class's per-frame step is
  ported.

## What the original does

The car's world-step listeners, the same as MD39–MD106c. Nothing new was decompiled for this task.

## How it was proved

A bit-exact replay of the five MD107 drives beside the interpreter: 730 frames each, every frame
compared. Words compared in all (car + body + pieces + vehicle + record):

| drive    | words     | sort struck | contacts held | other world steps |
|----------|-----------|-------------|---------------|-------------------|
| buggy    | 1,539,570 | 16 frames   | 0             | 0                 |
| baja     | 1,537,380 | 17          | 0             | 0                 |
| sandrail | 1,541,482 | 9           | 112           | 0                 |
| golf     | 1,539,570 | 3           | 0             | 730               |
| truck    | 1,544,340 | 10          | 92            | 0                 |

Calls handed out on every drive, and nothing else: SortPlace, SortSearch, SpatialMove (730 each), PartMove
(1460), Gather (730), LandingSound (2–5), ShiftSound (4–12). No drive wrecked.

## Not verified / not 1:1

- **The car's update (vt +0x10c) is not stepped beside on the drives.** On these drives it runs as the
  original's. The class's own per-frame step is still a call (`atv_car.rs` header), so the port can't run
  the update whole. `class_update` is proven only off generated cases (MD106e). Follow-up MD156.
- **The sort's search is answered from the original's result**, not by a port of the sort. So the port's
  handling of a hit is proven, but which objects get hit is not. The same holds for the other handed-out
  calls (SpatialMove, PartMove, Gather, the sounds): the test checks that they are asked for, not what they
  do.
- **No drive wrecked**, and Struck, NodeRebound, Respawn, Cue, Rumble, WreckLook, LeaveWorld, RidingLetGo
  and ReleaseRecord were never handed out. The paths that would hand them out were not exercised by these
  drives.
- **The golf cart's image steps a second world with no listeners** each frame (730 steps). The test runs it
  as the original's and doesn't compare it, because it has no car on it.
- **Held contacts with another record or a mover** (where `record::held` returns None) never happened on
  these drives (every held frame resolved through the port), so that path is unproven here.

## Next

Nothing for this task. MD156 is the follow-up.
