# 1 — riding overlay (FINAL)

Task P6. 2026-10-17.

## What I did

`crates/openmx/src/hud.rs`, new: the riding overlay, behind `--hud` (and H toggles it while riding).
A panel in the bottom-left corner, in three bands:

- **the numbers**, two columns of four lines: speed in feet a second and in miles an hour, rpm, gear,
  the ramped clutch and whether the clutch is locked or slipping, grounded and touching, roll and
  pitch in degrees, the ride state machine's phase (with the shell's own freeze beside it and the
  reset clock while one counts), the last trick, and the last wreck reason as its code plus the port's
  name for it;
- **the sixteen controls**, in the order the input map holds them, each a cell whose fill is
  `Pad::strength` — full for a digital control, the pressure byte for the four analogue ones — with a
  yellow border on the ones that went down *this frame* and the name drawn white while held, grey
  while not;
- **the two sticks**, each drawn twice over: a white cross where `Pad::axis` put it (after the dead
  zone and the two segments) and a blue dot where the raw report byte is, over a grey box that is the
  dead zone in the raw dot's own space. Beside them the three commands that ramp — gas, brake, clutch
  — each a bar of what `ride::Held` is actually holding with a yellow tick at what the pad asked for
  this frame.

Every value is read off the port at draw time: nothing is recomputed, latched or smoothed, except the
two things that are events rather than state (the last trick and the last wreck), which the shell
latches into the overlay from the same `rider::Frame` it already prints.

It draws with no triangles: a 2-D blit straight into `Frame::color` after `draw_scene`, a 3×5 bitmap
font at five bytes a glyph, scaled by whole pixels. That is the cheapest thing the rasteriser allows,
because it does not use the rasteriser at all — the overlay has no depth, no texture and no clip
beyond the frame's own edges.

Two small changes outside it:

- `main.rs`: the flag, the H toggle on its own edge (the window keeps key state, not key events), the
  draw call between `draw_scene` and `present` so the window and `--shot` get the same picture, and
  **`--shot-frame N`** — `--shot` wrote the first frame of a ride, which is the one frame where
  nothing has happened yet, so there was no way to photograph the overlay with anything in it.
- `window.rs`: `key::H`, which was the only unbound letter near the others.

### A bug the overlay found on its first frame

With no joystick on the machine the shell held the **clutch and the front brake at 43%, for ever**.
`Pads::open` starts every joystick axis at 0, which is the middle of an axis's range — right for a
stick and for a hat, wrong for a trigger, whose rest position is the bottom of its travel.
`trigger_pressure(0)` is `(0.5 − 0.12) / (1 − 0.12) = 0.43`, and that went into the report as L2's and
R2's pressure byte, so `Controls::from_pad` asked for 43% clutch and 43% front brake with nobody
touching anything. One line in `pad.rs`: the two trigger axes start at `i16::MIN + 1`. A pad that
opens sends an initial event for every axis and overwrites both within a frame, so this only ever
showed on a machine riding the keyboard — which is every unattended run in this project.

This is exactly the task's own justification ("a person riding cannot otherwise tell a dead key from a
bike that ignores the key"), so the fix is in this commit with the overlay that found it.

## What the original does

Nothing: this matches nothing on the disc. The original's own HUD is a front-end page with its own art
and none of it is ported; the overlay is the shell's debugging furniture and is drawn in the shell's
own way on purpose. The only thing it borrows from the disc is the names, and those it takes from the
port rather than inventing: `Control::name` for the sixteen, the stunt table's `display_name` for a
trick, and the state machine's own reason code for a wreck (named here for the five the port has
names for — 7 thrown, 11 attitude, 12 trick, 13 cannonball, 14 no control — and shown as a bare number
for anything else).

## How it was proved

By eye, as the brief asks, plus pixel tests for the parts an eye cannot hold still.

- **By eye, in a ride**: `./target/release/openmx --hud --no-wrecks --shot-frame 180 --shot <png>
  --seconds 3.5` on `free06`, and the frame read: speed 2.8 f/s / 1.9 mph, rpm 4500, gear 1, clutch
  0.00 slipping, ground yes, roll 0.6, pitch −2.5, state riding. Those are the shell's own numbers for
  a bike rolling off the start line, and the once-a-second line it prints agreed with the panel's
  speed. The run held 59.7–59.8 fps with the overlay on, which is the same rate the same run holds
  with it off.
- **By eye, with a pad in it**: a still rendered from a hand-built report (cross at pressure 0x60,
  L1, R3 and up down this frame, left stick down-left, right stick hard left, a landed trick and an
  attitude wreck latched) — the partial fill on cross, full fills on the other three, the yellow
  pressed border on all four, the stick dots off-centre with the crosses under them, and the gas bar
  short of its marker while the ramp catches up. Both stills are in `context/` (not committed).
- **Pixel tests** in `hud.rs`, through the port's own input layer rather than a hand-filled struct —
  `Manager::connected()` polled with a `Report`, so what the overlay draws is what the game read:
  the flag off leaves the buffer untouched; the flag on draws inside the panel and nowhere else; a
  held control's cell is `HELD` and an untouched one's is `EMPTY`; pressed-this-frame is a border
  that is gone the next frame while the button is still held; inside the dead zone the cross is dead
  centre and the dot is not; the font has a shape for every character the overlay writes; and the
  panel fits the smallest window the shell's help offers.
- **The cost**: the timing test draws 200 frames at 640×448 and reports the average — **47 µs** in
  `--release` (3% of a 60 Hz frame's budget, against the brief's 1 ms) and ~155 µs in the unoptimised
  test build. The test's bound is the brief's own millisecond so that it passes in the slow build; the
  number a rider pays is the fast one. With the flag off a frame is one branch, and the test holds
  that under 2 µs.
- `./kit check`: TESTS PASSED (483).

## Not verified / not 1:1

- **Nothing here is 1:1 with the disc, by design.** The layout, the colours, the font, which numbers
  are shown and the five wreck names are all this file's own; the original's HUD is a front-end page
  that is not ported. There is nothing to compare against and no capture was taken.
- **The values are trusted, not re-derived.** The overlay reads `ride.chassis.speed`,
  `ride.engine.rpm/gear/clutch_locked`, `ride.held`, `ride.air.on_ground`, `ride.touching`,
  `ride.chassis.euler`, `rider.phase()` and `rider.tricks` straight out of the port. If one of those
  is wrong in the simulation it is wrong on the screen too — which is the point — but it does mean a
  number reading right here is not evidence that the field is right against the original.
- **Roll and pitch are the chassis' euler angles, in degrees.** The port keeps them in radians and
  nothing in the ride's step reads them (`chassis.rs`'s measure writes them for the record), so they
  are shown as degrees for a reader. Which of x/y/z is pitch, yaw and roll is taken from S22's trace
  columns, not measured again.
- **The trick line shows the port's own progress**, not the original's HUD, which shows a scored
  trick name. `tricks.progress` and `tricks.safe` are simulation state being exposed, not UI being
  ported.
- **The press marking cannot be driven by a human in an unattended run.** Nothing on this desktop can
  send a keystroke to the window, so held and pressed-this-frame were proved by the pixel tests and by
  the hand-built-report still, not by a person pressing a key and watching the cell light up.
- **The trigger fix is checked by its arithmetic and by the overlay, not against a pad.** There is no
  joystick on this machine, so `trigger_pressure` at rest was proved from the numbers (0.43 before,
  0.00 after, read off the overlay in a real ride) and the "a pad sends initial events for every axis
  when it opens" claim is the comment `Pads::report` already carried, not something I could test.

## Next

Nothing outstanding. If a feel task wants more in the panel, add lines to `Hud::numbers` — the second
column has room, and the panel's height is computed from its bands.
