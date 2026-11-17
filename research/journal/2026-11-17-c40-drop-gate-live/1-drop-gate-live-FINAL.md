# 1 — drop gate live (FINAL)

Task C40. 2026-11-17.

## What I did

- `research/c40/` (new): `boot.py` (C31's scripted boot, with the track and league as arguments),
  `spec.py` (the watch spec), `capture.sh` (boot, record, save the end) and `summary.py` (reads a capture).
  How to use them: `research/c40/README.md`.
- Booted sx17 as a 4-rider AI single race, 2 laps, on a fresh copy 4. I saved the start in slot 36 (copied
  to `context/states/c40-sx17-race-start.p2s`, RAM image `c40-sx17-race-start.ee.bin`), recorded 6000
  rows from `in_game == 1` into `context/capture/c40/sx17.csv`, then saved slot 37 at the end.
- `crates/game-sim/tests/drop_gate.rs`, two new tests (both skip without their files):
  - `sx17s_live_sink_replays_tick_for_tick`: every row of the capture through `Sink::tick`, using the
    captured cards (list order) and dt. The counter, armed, on, the clock bits and the height bits match on
    every row. Also checked: the heights from the firing row, and both pieces' matrices (placement before,
    identity from the firing row on, node word 0).
  - `sx17s_trigger_is_bound_as_the_image_holds_it`: sx17's race file off the disc and the start image.
    The port's path spot for `[LowerGate]` equals the one the image keeps. `Sink::bind` to the file's line
    gives the image's trigger bits, 2475.7615. The moved line would give 2477.9702. The two gathered
    collision objects carry the names `collision_names` makes.
- **The fix.** `Sink::bind` now takes the line spot to measure to (`crates/game-sim/src/drop_gate.rs`).
  `race_start::laps` also returns the race file's own line spot. `race_start::Start::file_line` keeps it,
  and `gate_sink` takes it. `main.rs` passes it. A headless `--race --track sx17` now prints
  `race: drop gate sinks 2475.76 from the line, 2 collision pieces`, the original's number.
- Filed S382 (low): race_start reads the `.toy` numbers with Rust's rounding parse. The game reads them
  with strtod and then a truncating double-to-float.
- Nothing from opentennis: it has no race gates.

## What the original does

- The gate object is at activity `+0x3ac` (activity = `@activity_manager -> 0x58 -> 0x0`). The loader
  (`./kit fn 0x002f9f40`) hangs it under the score manager node (activity `+0x50`) with flag 2. The
  score manager's tick (`./kit fn 0x0025fcd0`) runs its mode handler and card work, and then, as its
  last call, `0x00133500`, the child walk, which calls each child's vt `+0x38`. The gate's update is one
  of those calls. So in a frame the cards' line distances are updated first, and then the gate reads them.
- In the loader, the gate's bind (line 166 of the decompile, `0x002ff3d0`) runs **before** the block that
  moves the line (track `+0x5c`) onto the finish-gate prop and spots it again (lines 172–188,
  `0x0026b410` on line `+0x94`). So the trigger is measured to the race file's line spot.
- The collision mesh's vt `+0xac` (`./kit dis 0x001edb60`) copies the old `+0x1a0` matrix to `+0x1e0`,
  stores the new one at `+0x1a0`, and rebuilds `+0x120` from it. The name `0x002ff540` matches is at
  the collidable's `+0x8a`, collidable = list object `+0x34`.

## How it was proved

A PINE recording, compared tick for tick (`context/capture/c40/sx17.csv`, 6000 rows, frames 1–6002),
and a save-state RAM image (`context/states/c40-sx17-race-start.ee.bin`) with the disc's race file.

- Trigger 2475.7615 (bits match the port's bind to the file's line). The 2 pieces are listed, and the
  sink is armed from the first row.
- On frame 1005 the gate fired: count 0 → 1, armed 1 → 0, on 1, y −0 (0x80000000). AI card id 1's pair
  on that same row is kept 2476.1499 > 2475.7615 > now 2473.9717. The row before was
  (2478.3311, 2476.1499), which does not cross. **So the gate runs after the cards' update in the same
  frame.** That matches the static reading above and the port's order (`Sim::race_frame` ticks the sink
  after the manager).
- Heights from frame 1005: 0x80000000, 0xc1554fdf (−13.332), 0xc1b8df8d (−23.109), 0xc1f606af
  (−30.753), 0xc213b62b (−36.928), 0xc2282cea (−42.044), 0xc23970eb (−46.360), 0xc2483c25 (−50.059),
  then 0xc2c80000 (−100) on the 9th row, with on → 0 and t held at 0.29997. dt is 0.03333 (f32), not 1/30,
  so the values are a little off P313c's 1/30 list. The port matches these bits.
- Both collision pieces: `+0x1a0` = placement (turned 90°, at 424.05, 90.06, 91.07) up to frame 1004,
  the identity from 1005 to the end. `+0x1e0` holds the old one for one row. Node word `+0x230` = 0.
- Port-side, end to end: headless shell on sx17 prints the trigger 2475.76 and a lap of 4022.52 (the
  image's lap is 4022.5212).

## Not verified / not 1:1

- **Only sx17.** nat16 and nat20 (the task's other two candidates) and nat09/12/13/19/sx18 were not
  booted. nat12/16/19 spell the trigger `Position=` (trigger at the origin), and that path was not seen live.
- **The shell's paths are snapped to the terrain** before the load (`race_start::laps`, the port's own,
  S296a). The image test used the unsnapped load. The shell's printed 2475.76 shows the snap did not
  change the measure on sx17, but only to 2 decimals in the print, not bit for bit.
- **The trigger z is read 1 bit off** by the port (704.9 rounded; the game truncates to 704.89996). It
  lands on the same path spot on sx17, so the trigger is the same there. Filed S382; other tracks not checked.
- **The drawn gate** (the soultree pieces going down under the group node) was not compared on screen.
  Only the group's y was captured. The render is matched by eye, by policy, and I did not shoot it.
- **Rows read at one phase.** The order proof assumes a row's fields all come from the same frame's
  end. The steady 9-row sink and the crossing pair on the firing row agree with that, but the recorder's
  read phase was not checked against `Ee::log`.
- **Cards in list order.** The capture reads cards along the list, which re-sorts by place. Torn reads
  were possible on any row. None showed where it mattered (the replay matches every row), but that was
  not ruled out for every row.
- **Two rows missed** (frames 4419 and 4447, recorder under load), long after the sink. The replay skips
  nothing there that changes state.
- **Rollback, MoveGateDown false, a track with no paths:** still as P313c left them, not seen live.

## Next

Done.
