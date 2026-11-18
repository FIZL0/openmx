# 1 — s89 rides remade (FINAL)

Task C39. 2026-11-18.

## What I did

- **Recorded along.csv** from R32's s89-along state, with `watch/s89-rails.toml` and
  `watch/pads/s89-along.pad`.
  - `/dev/uinput` is root's, so `./kit record --pad` cannot drive the pad. New `research/r32/ride.sh`
    turns the wall-clock pad script into game time (sleeps ÷ 4, the quarter speed it was written for),
    makes it a from-savestate tape (C15's `tape.py`) on the state's `.p2s`, plays it over AT-SPI (C6's
    recipe) and records a scratch copy of the spec.
  - The scratch spec drops `state = 9`, adds `start = frame_counter > <state's frame> … and
    rider_type == 2`, and drops `required = true`: the chain reads null on the load's first frame and a
    required field ends the capture there, before `start` arms. ride.sh checks every row's
    `rider_ctl_src` is 1 instead.
  - 600 rows, frames 11912..12511, no gaps (at NominalScalar 0.12; at 0.25 under load 40–90 it
    dropped 3 frames, which the test refuses).
- **Remade s89-across** with R32's `make.sh`/`along.py`, now taking an optional spot and clock
  (`make.sh PID 9 3900 136.7 4560 34.8`: S89's across spot).
  - The ground there slopes down to the bed. Saved at R32's clock (39.95), the bike had rolled the
    47 ft to the bed after the tape let go of the brakes.
  - Saved at 34.0 (braked, settled), the stale brake stayed in the older half of the pad buffer, and
    the test's row-0 rule read it as row 0's pad ("the pad read 0x0082 but the game held 0x0000").
  - Saved at 34.8 instead: frame 11729, 0.66 ft past the spot, rolling at 3 ft/s, both pad halves
    empty.
  - Installed at `context/states/s89-across.eeMemory.bin`; RAM image and `.p2s` kept in
    `~/repos/openmx-slots/kept-context/r32/`.
- **Recorded across.csv** from it: 300 rows, frames 11730..12029, no gaps (NominalScalar 0.06, load
  ~130; at 0.12 it dropped 31 frames).
- **The test picks free06 by name.** On the new across ride, `recorded_on` took Free11 as well as
  free06 (2 of 300 rows buried more than 2 units there, none on free06). Added
  `in_the_world_named` (and split `in_the_area` out of `in_the_world_on`, unchanged for its other
  callers). The S89 test uses it with `Tracks\free06.res`, the world both states were made on.

## What the original does

Same as S89's lost rides, row for row where it can be compared:

- along: on the bed rows 0–107 (108 rows, S89's 108), at most 1.31 ft/s across its heading (S89's 1.24).
- across: on the bed rows 55–58 at about 50 ft/s (S89's 55–58 at about 50), at most 0.31 (S89's 0.51).
- Both wheels' surface byte on the bed is 0 or 12, never 1.

## How it was proved

- `./kit check -- -p game-sim --test replay the_dallas_rail_rides_replay_from_their_save_states`:
  passed, and ran (not skipped): "s89 along from its save state: agrees on every tick" (599 ticks) and
  "s89 across from its save state: agrees on every tick" (299 ticks).
- The ratchets hold on the new rows: `S89_AGREES_THROUGH` is every tick for both, and so is every lane
  of `S89_LANES_AGREE_THROUGH`. Wheel mounts, the front's surface-0 grip table and the wheel probe
  match the image word for word.
- The full gate in `./kit done`.

## Not verified / not 1:1

- **Not S89's captures or across state.** Those are lost. These are new rides from new states (along
  is R32's). The rows, speeds and bed counts match S89's journal, but no number can be compared bit for
  bit with S89's runs.
- **The across state is not settled.** It was saved 0.3 s after the brakes let go, rolling at
  3 ft/s, 0.66 ft past S89's spot, so the pad buffer holds no stale brake. S89's settled before its
  save. The test seeds from the image, so this does not reach the replay.
- **The pads came from a tape, not `./kit pad`.** The script's wall-clock sleeps were divided by 4 and
  rounded to ticks. So the button edges fall on whole ticks, not where a live pad would put them.
- **The test's row-0 pad rule (S237) was not checked against a stale pad half.** Both rides start with
  nothing held in either half. A state saved holding a button would trip it, as the 34.0 across did.
- **free06 is named, not measured** for these rides: `recorded_on` cannot tell free06 from Free11 on
  across.
- **The recorder's `required` check was turned off** in the scratch spec. Every row was checked
  afterwards for `rider_ctl_src` 1 instead.

## Next

Nothing.
