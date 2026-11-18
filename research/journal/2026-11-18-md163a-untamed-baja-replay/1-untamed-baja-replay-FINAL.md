# 1 — untamed baja replay (FINAL)

Task MD163a. 2026-11-18.

## What I did

- **Scratchpads.** context/untamed/md164/{spawn,air}.spr.bin: each .p2s's Scratchpad.bin (python's
  zipfile; `unzip -p` wrote empty files). Copied to kept-context/md164 too.
- **tests/mxvatv_car_frame/untamed_baja.rs** (25 tests), MD81f's sandrail file's pattern on MD164's seven
  drives (throttle, throttle-2, steer, brake, jump, preload; land from air.ee.bin):
  - capture: SLUS-21701's own frame in the interpreter off the image with the tape's input, against
    every row; steer's wreck rows (car state 14 on rows 209–269) and preload's fire row (timer −1/120 on
    row 123) asserted.
  - beside: the port's `frame_untamed` and the port's Ride (`update.untamed`), every frame compared word
    for word after SLUS-21701's (car, body, pieces, vehicle, record, respawn).
  - on the port's terrain: the same through the Ride with Rocksbury's ground and the noise from the port.
- **Shared harness (untamed.rs), three changes:**
  - the tape feed asks drive.rs's button table, so Square (brake) and Circle (hand brake) reach
    `input_untamed` (it fed Cross alone; MD160's and MD81f's tapes hold nothing else).
  - frame 1 runs on a released pad (the pad at the load), not the image's input words.
  - `UDrive::before`: the recorder reads its rows before the frame (MD81f, MD164). The put-back spot
    the script writes is then laid in off row `j − 1`, not `j − 2`.
- No change to atv_car.rs: the port already holds every word of these drives. Scene::update_untamed is not
  routed for the car: no drive needed it (every frame matched).
- Nothing from opentennis: no shared code involved.

## What the original does

- **Released pad on frame 1.** land.csv row 1 has the gas pedal at 0 between rows 0 and 2 at 1. air.p2s was
  saved holding Cross and loaded with the pad released. The load's first frame reads the released pad;
  tape tick 0 lands on frame 2. MX Unleashed (S117) rode tick 1 on the saved controls instead. On spawn
  drives the released pad is the image's own input, so MD160's and MD81f's replays are unchanged by this.
- **Put-back spot.** Steer's put-back on frame 270 stands the truck at the spot Free Ride's script keeps
  (+0xd30..). This recorder reads before the frame, so the spot going into frame `j` is row `j − 1`'s.
  With `j − 2`, frames 270 on parted by a frame's movement of the spot (y 1.2169 against 1.2115).
- **The preload fire** posts its cue once on frame 123 (handed out as `PreloadCue`, the port's as the
  original's), with `[Preload]` off (zero push).

## How it was proved

- **Live capture, frame by frame:** every row of all seven drives is SLUS-21701's frame in the interpreter,
  bit for bit (body, engine, four tires): 370 + 370 + 280 + 340 + 370 + 370 + 135 rows. No row is late or
  torn.
- **The port beside the original:** all frames of throttle 370, steer 280 (61 wrecked, put back on 270),
  brake 340, jump 370, preload 370, land 135. Every word the port keeps equals SLUS-21701's after each
  frame, through frame_untamed, through the Ride, and through the Ride on the port's terrain and noise.
  Tire-on-object frames compared too: throttle 113 and brake 116–117.
- `./kit check -- -p game-sim --test mxvatv_car_frame untamed`: 58 tests pass (MD160's truck, MD81f's
  sandrail, MD234's terrain and these). No shipped code changed, so the gate's replay hashes are the check
  that nothing else moved.

## Not verified / not 1:1

- **The original's input step is not run.** The interpreter's frames take the input words from the port's
  `input_untamed` off the tape. The tape fed only Cross, Square, Circle and the stick, so these drives
  check the port's input step for the gas, brake, hand brake and stick. The clutch, d-pad and unbound
  commands are not held against SLUS-21701's input step: no tape presses them.
- **The released frame-1 pad** is read off land.csv row 1 (its gas pedal word) and the replay matching. It
  was not read in the code: the load path was not decompiled.
- **The preload's cue itself** is matched as a handed-out call, port against original, in the same frame.
  It is not heard or seen (MD164's limit: it rises and falls inside one tick).
- **throttle-2** is checked against the interpreter only, not beside the port (it repeats the throttle tape).
- **Reverse** (both pedals) is not driven by any tape.
- Look, sounds and cameras of the baja: not compared (presentation).

## Next

- none
