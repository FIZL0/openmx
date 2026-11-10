# 1 — mxvatv car step (PART)

Task MD107. 2026-11-09.

## What I did

- Nothing in the code. MD107 replays the five drives *through MD106's car frame*, fed C29's tape and
  csvs. On main at a200878f, neither has landed:
  - **MD106** (SLUS-21104's car physics frame, bit-exact on one frame of `buggy.ee.bin`) is `[ ]` with no
    branch: there is no car frame in `crates/game-sim/src/car.rs` to replay through.
  - **C29** (the remade golf cart / sand rail states, `drive.p2m2` and the five drive csvs) is `[ ]` on
    main; its work sits on `origin/task/C29` (fc0ee161), not yet merged.
- Writing a replay harness now would mean writing MD106's frame inside this task, which is MD106's work
  and would collide with its edits to `car.rs`. So the task is blocked `--after MD106,C29`.

## What the original does

Not read this session (MD106 reads it).

## How it was proved

Not proved: nothing was ported.

## Not verified / not 1:1

- All five drive replays: not attempted — MD106's car frame and C29's captures are not on main.

## Next

Once MD106 and C29 have merged: seed a `car.rs` car from each drive's `.ee.bin` RAM image (field map in
`tests/mxvatv_car_fields.rs`), feed `drive.p2m2`'s inputs at row lag 2 (buggy, baja) or 1 (truck, golf cart,
sand rail), and compare every csv row bit for bit.
