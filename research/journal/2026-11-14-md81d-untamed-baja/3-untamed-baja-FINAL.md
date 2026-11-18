# 3 — untamed baja (FINAL)

Task MD81d, reopened after MD163a landed. 2026-11-18.

## What I did

- Checked that the task's test is now met by the work its splits landed. No new code.
  - Diff of SLUS-21701's car step against SLUS-21104's, and the port behind `Load::untamed`: part 1
    (`tests/untamed_car.rs`).
  - Capture: MD164 (`context/untamed/md164`, spawn and air images, seven drives).
  - Pack, off by default: MD163 (`baja@untamed_cars`, `tests/untamed_baja_load.rs`).
  - Replay of the captured drives, bit-exact: MD163a (`tests/mxvatv_car_frame/untamed_baja.rs`).
- Ran the three tests on this branch:
  - `./kit check -- -p game-sim --test mxvatv_car_frame untamed_baja`: 25 passed.
  - `./kit check -- -p game-sim --test untamed_car`: 3 passed.
  - `./kit check -- -p game-sim --test untamed_baja_load`: 2 passed.
- "With the pack off every replay hash is unchanged": `Load::untamed` is false unless a pack car says
  `"step": "untamed"`, and the gate's replay tests (run by `./kit done`) are the check.

## How it was proved

- MD163a's replay: throttle, steer (with its wreck and put-back), brake, jump, preload and land, every
  frame of each, the port word for word against SLUS-21701's own frame in the interpreter, which itself
  matches every captured row bit for bit. Details in MD163a's journal.

## Not verified / not 1:1

- **The original's input step is not run in the replay.** The interpreter's frames take the input words
  from the port's `input_untamed`. The clutch, d-pad and unbound commands are not held against
  SLUS-21701's input step: no tape presses them (MD163a).
- **Reverse** (both pedals) is not driven by any tape (MD163a).
- **Body integrate's per-time angular damping** (Untamed's powf) is not ported. The baja's damping is
  `1, 1, 1`, so it leaves its drives unchanged; it would show only where a lane drops below 1 (part 1).
- **PostProcess yaw clamp and the class update's distance clause** are not ported. They differ only on a
  denormal limit or a −0 distance; argued, not tested (part 1).
- **The released frame-1 pad** after a load is read off the capture, not the load code (MD163a).
- **The preload cue** is matched as a handed-out call, not heard or seen (MD164, MD163a).
- Look, sounds and cameras of the baja: not compared (presentation).

## Next

- none
