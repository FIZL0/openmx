# 1 — untamed golfcart replay (FINAL)

Task MD170. 2026-11-19.

## What I did

- **Scratchpad.** context/untamed/md81e/spawn.spr.bin: spawn.p2s's Scratchpad.bin (python's zipfile, as
  MD163a; `unzip -p` writes an empty file). spawn.ee.bin is the .p2s's eeMemory.bin byte for byte.
- **tests/mxvatv_car_frame/untamed_golfcart.rs** (18 tests), MD163a's baja file's pattern on MD81e's
  five drives (throttle, throttle2, analog, steer, jump), seeded from spawn.ee.bin (cart 0x01be4740,
  golf class vtable 0x003f1370):
  - capture: SLUS-21701's frame in the interpreter with the tape's input, against every row. The hop
    (all tires off rows 272–324), analog's first gas row (33) and the jump's wrecked rows (state 14 on
    rows 234–294) asserted.
  - beside: the port's `frame_untamed` and Ride, every frame compared word for word (car, body, pieces,
    vehicle, record, respawn). Throttle2 beside through the Ride only (the throttle tape again).
  - on the port's terrain: throttle, analog, steer and jump through the Ride with Rocksbury's ground and
    the port's noise.
- **Shared harness (untamed.rs), two changes, neither moving an old drive:**
  - `u_frames`: the frame each row was read after, from its vsync (2 a frame). A row the recorder missed
    moves the rows after it on; the capture check and the beside loop step those frames unread, and the
    put-back spot comes off the last row read before the frame. Every MD160, MD164 and MD81f capture has
    vsyncs 2 apart throughout, so for them this is the old `r + 1` / `r`.
  - the tape feed reads `button@byte` presses (research/c15/tape.py's pressure words): a press counts
    from byte 27 (`U_PRESS_LEAST`) and is fed at a strength of one. Old tapes hold no `@`.
- MD81e's recorder reads its rows after the frame (`before: false`, as MD160's), not before as MD164's.
- No change to atv_car.rs: the port already holds every word of these drives. The golf class's own step
  and Untamed's baja class update (research/oracle.txt `# MD81e`) needed nothing new.
- Nothing from opentennis: no shared code involved.

## What the original does

- **The press threshold.** analog.csv's gas comes on in row 33, read after frame 47. Frame 47 reads tape
  tick 45, the first `cross@27` tick. Bytes 21, 23 and 25 leave the gas at 0. MD81e's journal said "from
  about byte 25": it counted rows as frames, and the recorder missed 13 frames before row 33 (see below).
  Byte 26 is on no tape, so the threshold is 26 or 27. Above it, any press is the whole gas (the cart's
  `MaxAnalogThrottle` is 0.1), so no part strength shows.
- **Missed rows.** analog.csv's vsyncs jump by 4 at rows 13 and 126 and by 26 at row 22: 211 rows over 225
  frames. Stepping 13 frames between rows 21 and 22 matches row 22 (no other count from 1 to 40 does), so
  these are rows the recorder did not read, not frames the game skipped. The tape keeps one tick a frame
  through them.
- **The jump's wreck** runs 61 frames (235–295) and puts the cart back on frame 296 through Untamed's
  respawn, as MD164's baja steer did.

## How it was proved

- **Live capture, frame by frame:** every row of throttle (370), analog (211), steer (280) is SLUS-21701's
  frame in the interpreter, bit for bit (body, engine, four tires). throttle2 has 11 rows read a frame
  early and 9 torn, jump 1 and 2: the recorder falling behind (throttle, the same tape, reads none; MD81e
  saw the same 20-odd rows differ run to run). The rows after each match again.
- **The port beside the original:** every frame of throttle 370, throttle2 370, analog 225 (the missed
  rows' frames too), steer 280, jump 370 (61 wrecked, put back on 296). Every word the port keeps equals
  SLUS-21701's after each frame, through frame_untamed and through the Ride, and through the Ride on the
  port's terrain and noise. Tire-on-object frames compared too: throttle 113–118 and 274, analog 143–149,
  steer 268–274.
- `cargo test --release -p game-sim --test mxvatv_car_frame`: the whole binary passes (MD160's truck,
  MD81f's sandrail, MD163a's baja, MD234's terrain and these). No shipped code changed, so the gate's
  replay hashes are the check that nothing else moved.

## Not verified / not 1:1

- **The original's input step is not run**, as MD163a: the interpreter's frames take their input words
  from the port's `input_untamed` off the tape. So the press threshold (27) is the harness's reading of
  the pad, found from the capture; the pad layer (PCSX2's pressure byte, libpad, the game's input
  manager) is not ported or checked, and byte 26 is untested.
- **Part analog gas** is not seen: any counted press is over the cart's 0.1.
- The class update's `!(d <= 0)` distance clause stays unported (MD81e): no drive reached a `-0` distance.
- Look, sounds and cameras of the golf cart: not compared (presentation).

## Next

- none
