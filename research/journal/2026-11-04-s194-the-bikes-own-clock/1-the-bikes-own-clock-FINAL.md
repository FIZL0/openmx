# The bike's own clock — FINAL

## The stamp

- `0x0027c890` (the player update) ends with `lwc1 f0,0x13fc($18); add.s f0,f0,f20; swc1 f0,0x13fc($18)`,
  where f20 is the frame's dt. This runs after the base update's physics, on every tick.
- So a row's `+0x13fc` says which tick its bike is at, wherever the recorder's read falls.
- Address: `@game_manager -> 0x34 -> 0x34 -> 0x8 -> 0x8 -> 0x8 -> 0x0 -> 0x13fc` (the rider, then +0x13fc).
- Found by `hunt`-style scans between two RAM images two frames apart: 195.774 → 195.841 in
  s129-braked-11.
- Other per-frame counters (rider +0x17d0, engine (rider +0x7b8) +0x184) were not used.

## The rule (`replay.rs` `clock_ticks`)

- From row 0's clock, step `clock = F(clock) + F(dt)` until it reaches the row's. The two must be bit
  equal, and the step count is the row's bike tick, relative to row 0.
- The offset is the largest frame tick − step over all rows. Then every row's bike sits at its frame's
  tick or the next one, and the function checks that.
- Captures without the column keep the old frame rule.
- The train and the pad stay on the frame's numbering (`frame_ticks`): the cars are frame-paced.

## Recording (no /dev/uinput, root-only again)

`research/s194/capture.sh` plays a held-buttons .p2m2 (`make_tape.py`, P21's method) from a kit copy's
save slot, with C6's AT-SPI playback scripts. Everything was recorded on copy 1:

| capture | slot | buttons | first frame | rows | speed | late rows |
|---|---|---|---|---|---|---|
| s146/roof | 43 | square,r2 | 11933 | 270 | ¼ | 0 |
| s146/roof-full | 43 | square,r2 | 11933 | 270 | 1 | 268 |
| s129/braked | 40–42 | square,r2 | 16343 | 360 | ¼ | 0 |
| s129/across | 40–42 | cross | 16364 | 300 | ¼ | 0 |

`clock_rows.py` lists a capture's late, repeating and held rows.

## Results (`cargo test --release -p game-sim --test replay`)

- `a_bike_parked_on_the_trains_roof_rides_it_as_the_original_does`:
  - roof and roof-full agree on every tick.
  - Numbered by frame, roof-full parts at tick 1 (the control).
- `the_bike_meets_the_dallas_train_as_the_original_does`:
  - braked and across agree on every tick. Braked was 143 before; it now starts from frame 16342.
  - Across with no body rays now parts at 89, not 148, because this ride reaches the train sooner.
- `the_parked_bike_never_gathers_the_train`: passes.

## Not proven

- The old torn S146 capture is gone from this worktree, so its own rows (161-162, 179, 198, 211) were not
  re-run. The new quarter-speed captures tear nowhere; roof-full is the stand-in, and it is torn
  everywhere.
- Captures and states live only in s1's `context/`. They can be remade with `capture.sh` from copy 1's
  slots 40–43.
- Other watch specs do not record `bike_clock` yet.
