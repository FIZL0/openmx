# 2 — mxvatv car step (FINAL)

Task MD107. 2026-11-11/12.

## What I did

- `crates/game-sim/tests/mxvatv_car_replay.rs` (new): each of MD39a's five car drives (buggy, baja,
  sandrail, golf cart, monster truck) replayed in the interpreter from its RAM image through
  SLUS-21104's per-frame sim update (0x00204040, the frame MD106 ported the car step of). Every one of
  the 730 rows has its car object (0x1800 bytes), body, engine and four tire blocks compared word for word.
  Nothing from the capture is laid on the machine. The only input is the tape's: each frame, the car's
  input words come from `drive.p2m2.ticks.csv` through the port's own input step.
- `crates/game-sim/src/atv_car.rs`: the car's input step (0x004ca010) ported as `atv_car::input`, with
  `InputKeys` (the three `[Joystick]` keys it reads) and `Input` (the words it writes). It is proved on
  the game's own step by `the_car_input_step_is_the_ports`: 10,000 generated cases on `buggy.ee.bin`
  (stick, every command's strength, speeds and slip angles around the two keys' thresholds).
- `research/oracle.txt` `# MD107`: the input step, the keys' values, the words outside the frame, and how
  images and rows line up.
- Filed MD134: the same five drives by the port's own frame, stepped beside the interpreter.

The brief named `car.rs` as the file. The car port lives in `atv_car.rs` (MD106 put it there), so the
input step went there. `car.rs` (MX Unleashed's car) is untouched.

## What the original does

- **Input step** (0x004ca010, a0 car, a1 record `[player, controls+0x18, stick x, stick y]`; full logic
  in oracle.txt `# MD107`):
  - It returns at once unless car +0xbc4 is set and its state is 1.
  - Stick x/y go to +0x11c8/+0x11cc, and the controls' pad word goes to +0x1124.
  - Each command's strength comes through the bindings lookup (0x00118e10).
  - Gas is analog (strength / MaxAnalogThrottle, clamped by adda/madd/max/min) at low speed or a large
    side-slip, and digital otherwise.
  - The brake is 1 when pressed, or before the gas has ever been touched (+0x1128). Hand brake 0x18 and
    clutch 0x10 are each 0 or 1. The d-pad overrides the stick.
- **Images vs. rows**: the buggy's and baja's images are their row 0. The sandrail's, golf cart's and
  truck's are the frame before row 0. Every row 0's vsync is the image's + 2. With the brief's lags
  (2 / 1), frame `j` reads tape tick `j - 2` on all five drives, and the test asserts both facts.
- **Words outside the frame**, masked in the comparison:
  - car +0xd38..+0xd70, the put-back spot (as the quad's, MD37b);
  - +0xd90..+0xd9c, a position breadcrumb dropped every so far driven;
  - bit 0x400 of +0x44.
- **The tire step clears before it writes**:
  - Tire +0xb0 (contact flag): cleared at 0x004b5d10, set at 0x004b5740.
  - Tire +0x230..+0x23c: cleared by one `sq` at 0x004b6444, then x written at 0x004b73c8 and z at
    0x004b73e0 (or x alone at 0x004b6e10).
  - I found these writers with a temporary write watch in `Ee::write`, since reverted.

## How it was proved

`./kit check --package=game-sim --test=mxvatv_car_replay`: all six tests pass (the input-step oracle and
the five drives). Each row is classed against the interpreter's frame `f` (the frame its vsync names):

| drive    | rows | equal at frame end | torn (read mid-frame) | stale | between a tire's clear and write |
|----------|------|--------------------|-----------------------|-------|----------------------------------|
| buggy    | 730  | 730                | 0                     | 0     | 0                                |
| baja     | 730  | 730                | 0                     | 0     | 0                                |
| truck    | 730  | 716                | 14                    | 0     | 0                                |
| sandrail | 730  | 662                | 64                    | 1     | 3 (554, 686, 727)                |
| golf     | 730  | 39                 | 691                   | 0     | 0                                |

- **Torn rows**: every word equals the machine's value at some moment about frame `f`. The moments are:
  - frame `f-1`'s end, frame `f`'s end and frame `f+1`'s end;
  - within frames `f` and `f+1`: the start and end of the vehicle's force (0x004a4ef0 → 0x00493f60),
    each tire's step (0x004b5ca0) and its three parts (0x004b5cf0, 0x004b62c0, 0x004b6400), the aero
    (0x004cb200), and each rigid-body force add (0x00494000) and moment add (0x00497530).
  - The torn words are the body's force/moment accumulators, mid-substep tire words, and so on.
- **Golf's torn rows**: almost all of them are only car +0x930..+0x938 (a position copy), read one frame
  late.
- **Sandrail from row ~612 on**: the recorder reads mid-frame on most rows (MD37b saw the same on the
  quad's s-stoppie-short and s-wheelie-launch). Rows equal at a frame's end keep turning up between the
  torn ones up to the end (row 728 is one), so the replay never leaves the drive.
- **The three sandrail rows** hold a 0 that no hook point sees:
  - Rows 554 and 686: tire3 +0x230/+0x238. Row 686 holds the new x with z still 0, a read between the
    two stores at 0x004b73c8 and 0x004b73e0.
  - Row 727: tire3 +0xb0.
  - They are accepted only if every unheld word is one of those cleared tire words and at least one
    still holds the 0. Any other unheld word fails the test.
- At most rows/20 rows may be stale or late (the sandrail has 1).

## Not verified / not 1:1

- **The port's own frame is not stepped beside the interpreter.** The replay runs SLUS-21104's frame,
  with only the input step (`atv_car::input`) from the port. `atv_car`'s whole frame still needs the
  car's update hand-outs (MD106e, MD128), and `tests/mxvatv_car_frame.rs`'s helpers take the buggy's car
  address only. Filed as MD134.
- **Clutch and the pad word are not exercised by the drives**:
  - The tape has no clutch (0x10) button, and the replay feeds the pad word as 0.
  - Both are proved only by the input-step oracle's generated cases.
- **Torn rows are proved against moments, not exactly**:
  - A torn row only shows that each word is held at some moment about its frame, not that all its words
    are held at one moment.
  - Those rows are the recorder's reads during the frame (as MD37b's). Rows equal at a frame's end
    bracket them on every drive.

## Next

MD134 (port-side replay, after MD106e and MD128). MD39b can close on this, and MD39c (the cars in the
game) can follow.
