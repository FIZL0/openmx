# 1 — atv model load (FINAL)

Task MD134. 2026-11-17.

## What I did

- Found the frame the part reader reads the chassis box in. It is the mounts' frame (`quad::spawn_frame`),
  but at the start spot the quad was made at, not where it has settled since.
- `Load::read_model` now reads the box and the mounts in that one frame. `quad::heading_frame` is gone
  (MD129's stand-in; nothing else used it).
- `the_models_box_is_the_c28_records_but_one_float` is exact: the box is the one the record needs, and no
  loaded word differs from the image's. Name kept because the plan names it. The test reads the start
  point from RAM 0x669e40 (`start`), the heading from quad +0x60 as before.

## What the original does

- The constructor's loader places the scene node from the start spot (`./kit fn 0x4de920 -o atv`): point,
  unit heading, up (0, 1, 0).
- The init (`./kit dis 0x4b9db0 --len 0x580 -p atv`) reads the placement back, then the load path
  (`./kit fn 0x4922a0 -o atv` -> `./kit fn 0x492ca0 -o atv`) calls the place method
  (`./kit fn 0x492e50 -o atv`). It re-squares up against the heading with two cross products and makes the
  heading and up unit length again. The already-unit heading moves by one float in x and z
  (3f7458cc/be98b77f -> 3f7458cd/be98b780). It puts the body at the point and the node back on it.
- The mounts and the phy load (part reader + box) both run after that, at the end of the init. So both see
  rows_facing(start point, unit(unit(start heading))), which is `spawn_frame(start point, quad +0x60)`.
- MD129's guess used the image's quad +0x50 (the settled point, ~0.06 ft away) with the heading not
  re-normalised. That missed z by one float only by luck of the two errors nearly cancelling.

## How it was proved

- Save-state capture: C28's four images (`context/mxvatv-ps2/md37a/c28-atv-{r,s}-{spawn,air}.ee.bin`).
- With that frame the box is 400833cd 3ff98919 40410f77 on all four, the only box near that gives
  rec +0x60, and the four mounts stay bit-exact (`the_models_mounts_are_the_c28_wheels` still uses the
  settled point; a scratch check gave the same mounts from the start point on all four images).
- `./kit check -- -p game-sim --test atv_quad load::` passes (4 tests), every `Load::read_model` word equal.

## Not verified / not 1:1

- 0x669e40 being the factory descriptor's point/heading (+0x350/+0x360): it is heap RAM with no static refs.
  Its point and heading match what the creation copies (unit(heading) = quad +0x60 bit for bit, and the
  frame from it gives box and mounts exact), but I did not trace the descriptor's address.
- The place method's body offset (body +0x1a0 turned by +0x190) is taken as zero when the node is placed:
  the exact frame comes out only with the node at the start point itself. Not checked by reading the
  body's state at that moment.
- Other callers of `Load::read_model` (vehicle.rs, untamed and mxu tests) pass whatever spot and heading
  they had. Their box now uses the unit heading; no image proves those spawns.

## Next

- Nothing for this task.
