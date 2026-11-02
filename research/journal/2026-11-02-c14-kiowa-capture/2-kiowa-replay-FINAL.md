# 2 — kiowa replay (FINAL)

Task C14. 2026-11-02. Continues `1-kiowa-capture-PART.md` once S220a and S220b landed.

## What I did

- `crates/game-sim/tests/kiowa_capture.rs`, `the_flight_replays_bit_exact`: the capture's flight through
  `Kiowa::tick`, from the take-off to the row before the touchdown.
  - **Constants** come off the tape's start state `context/states/c14-kiowa.ee.bin`:
    - the controller, rotor and aero file values and the curve table, read as S220's tests read them;
    - the collective filter's time constant;
    - MinWreckSpeed, WreckTime and the clock;
    - gravity (+0x140), the gravity and throw scales (+0x190/+0x194);
    - the terrain's area.
  - **The seed** is row 1238, the first row with the collision record's touching flag (the capture's
    `pilot_flag`) down after the take-off:
    - the pad, commands and loop words from `heli_a`;
    - the filter's value is the filtered collective (+0x35c), asserted equal on the image;
    - the rotor and aero states;
    - the whole body from `body_raw`.
  - **The measure** is made again from the seed's body by `Measure::flight`, with beta's last value and
    the forward/up rows' `w` taken from the row. All 23 captured figures (speed, alpha, beta and the five
    rows) are asserted equal to the row's first, so the seed is checked as well as used.
  - **The pad** is polled from the capture's start off `pad_a`. A row's input step reads the previous
    row's buffer, then goes through `Pad::from_pad`, the shipped bindings.
  - **The surroundings** are the air's:
    - the record's response adds nothing (it has no entries);
    - touching is false, with no entries;
    - the rotor strikes nothing.
- Steps 1239..=1815, 577 ticks. Each tick it compares, bit for bit:
  - the five pad words;
  - the six commands;
  - the loop: speed command, last stick, hold height and timer, integral, filter value;
  - the rotor state and the aero state;
  - the measure's 23 captured figures;
  - every body word: 21 quadwords (4 lanes, 3 for the two accelerations), enabled and inverse mass.

  It also asserts that the crash test asks nothing, the capture's wreck word and touching flag stay 0,
  the port draws no `rand()` in the air, and every one of the five pad words is non-zero somewhere.

## What the replay shows

- **The world step flies the previous row's pad words.** The first run put the input step before the
  world step, as `Kiowa::tick`'s `Some(pad)` does. It was bit-exact through the climb, then went red at
  1325, the first row with Cross released. There the game's controller still took the climb branch (hold
  timer reset to 2, integral restarted, filter heading to 1), while the row's climb word was already 0.
  So row k's world step uses the words row k−1 left, and the input step that writes row k's words comes
  after it. The test calls `tick(.., None, ..)` and then sets `kiowa.pad = Pad::from_pad(..)`. Two
  orders fit the rows equally well, since one read a row cannot tell them apart:
  - the original runs the helicopter's input slot after its dynamics world;
  - the row is read between the next frame's input and its world step.

  For the port both give the same rule: the world step reads the pad the last input step left.
  `Kiowa::tick`'s docs still say "the input step, then the world"; a caller replaying the original has
  to pass `None` and run the input step after. I did not change `tick`'s signature: S220b's frame
  oracle drives it with the pad first, and both orders are one frame of the same loop.
- **The pad buffer lags a tick.** As C14's first part found, the Cross bit is in `pad_a` from 1234 and
  in the climb word from 1235. The controller first sees it at 1236.
- **The capture is bit-exact through the port**: the climb to y 254.6, the hover's overshoot to 294.6 and
  settle, the stick forward and the speed command's snap-back, the right rudder, the bank left and turn,
  and the descent to y 123.4 at 1815.

## How it was checked

- `./kit check -- -p game-sim --test kiowa_capture --test kiowa`: 12 passed.
- `cargo test -p game-sim --test kiowa_capture -- --nocapture` printed "577 ticks (1239..=1815)
  bit-exact, every pad word used", so the test ran and did not skip.
- Mutants in `kiowa.rs`, each red, then restored:
  - the rotor's blade-pitch share one bit off: red at 1239, the rotor's state;
  - the speed command's snap to the ground speed dropped: red at 1491, the speed command.
- The input step before the world step: red at 1325 (above).

## Not verified / not 1:1

- **The ground at both ends is not replayed.** The take-off (1204..1238) and the touchdown and bounces
  (1816..2057) need the collision record's response on the terrain: the helicopter's collider and the
  search that fills the record each substep. Neither is wired for the helicopter. Filed as C14a.
  Because of this, three things are still untested against the capture:
  - the touchdown's `rand()` draw;
  - the crash test's MinWreckSpeed branch;
  - the controller's touching branch (the collective held at 0 on the ground with climb up).
- In the air the surroundings are assumed, not ported. The response is given nothing to do, and the
  rotor's strike answers no. The capture backs both: the touching flag and the wreck word are 0 on every
  replayed row, and the body matches word for word. But the original's strike test (the ground under
  the rotor's spheres, the rotor's contact search) is not run.
- The rows are read at quarter speed after the tick. `flight-run2.csv` has 7 torn rows (part 1), so the
  test runs on `flight.csv` only.
- The capture's generator moves (323 values) through draws that are not the helicopter's. The port's
  frame is only checked to draw nothing.
- Whether the input slot runs after the world step, or the row is read between them, is not told apart
  (above).
- The rotor angle and the presentation copies are not captured or compared.
- One flight in one place. No wreck, no edge throw, no climb past MaxAltitude, no d-pad, no pressure
  values (the tape's buttons are 0/1).
