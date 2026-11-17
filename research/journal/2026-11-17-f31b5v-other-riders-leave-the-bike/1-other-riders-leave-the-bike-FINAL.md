# 1 — other riders leave the bike (FINAL)

Task F31b5v. 2026-11-17.

## What I did

- Found the test already on main: `the_other_riders_pre_process_leave_the_bike_alone` in
  `crates/game-sim/tests/mxvatv_bike_ride.rs`, landed with F31b5's part commit 2bf04ca5c. F31b5's
  journal (2-ports-and-what-is-left) still called it "not started", which is why F31b5v was filed.
- Ran it and proved it is not skipped. No code change.

## What the original does

State 4's world has six preforce listeners: the bike's PreProcess and five others. All five are
elements 0xec24f0, 0xf04bc0, 0xf46590, 0xf88e10, 0xfcb750, running the same PreProcess
(`./kit fn 0x4de0c0`).

## How it was proved

- The test replays C15 ride-2 and flip-1 (through the wreck on row 125 and put-back on row 193) frame by
  frame. The replay is F31b4's port_record_frame and stays bit-exact on every row.
- Around each of the five listeners it puts the port's body into RAM, then snapshots the bike object
  (obj0+obj1's length) and its body, node, record, both wheels and engine block before and after. It
  asserts no word changed and the port's body is equal.
- `./kit check -- -p game-sim --test mxvatv_bike_ride the_other_riders_pre_process_leave_the_bike_alone`:
  1 passed.
- A temporary `panic!` after flip-1 confirmed both replays ran (not skipped) and listed the five
  listeners above for each. Probe removed.

## Not verified / not 1:1

- Only the bike's words are watched, not the rest of RAM. A listener could write somewhere else the
  bike reads later. The bit-exact replay after each frame covers that for these two rides only.
- Only C15 ride-2 and flip-1. Other rides (other tracks, other rider counts) are not shown.

## Next

Nothing for F31b5v. F31b5 can drop F31b5v from its blockers.
