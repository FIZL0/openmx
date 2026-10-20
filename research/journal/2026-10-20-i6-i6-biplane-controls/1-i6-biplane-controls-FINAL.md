# 1 — i6 biplane controls (FINAL)

Task I6. 2026-10-20.

## What I did

- `crates/game-sim/src/input.rs`: `PlaneCommand` (the eight commands, numbered as the binding table
  numbers them, each with its shipped control and its name) and `PlaneInput` (the five numbers the
  plane's input step writes), with `PlaneInput::from_sticks` (the step itself) and
  `PlaneInput::from_pad` (one pad through the shipped bindings). Also `AxisRange::PLAYER`, the
  ride's ±0.3 stick range (see below), and `Pad::axis_through`, which runs any axis through a range
  of the caller's choosing. It needs a new `Pad::axes_centred` flag so it can repeat the poll's
  bad-read branch; the flag is not hashed.
- `crates/game-sim/tests/biplane.rs`: `the_biplane_reads_the_pad` (against C7's capture) and
  `the_biplane_input_step_is_the_originals` (function oracle).
- `crates/openmx/src/pad.rs`: no new binding was needed. On a pad, the shell's report already *is*
  the game's own layout, because the plane reads it through the shipped table. On the keyboard, the
  bike's keys already reach all eight commands:

  | key | control | plane command |
  |---|---|---|
  | X / space | Cross (ramped) | throttle up |
  | Z | Square | throttle down |
  | A | L1 | rudder left |
  | D | R1 | rudder right |
  | ↑ ↓ | left stick y | dive / climb |
  | ← → | left stick x | bank left / bank right |
  | A + D | L1 + R1 | the plane's reset chord |

  The plane's own dive, climb and bank bindings are on the d-pad, and the keyboard has no d-pad.
  The d-pad writes ±1, and an arrow held for 0.18 s is a stick at its stop, whose cube is also
  exactly ±1. So nothing is out of reach. The new test
  `the_keyboard_reaches_all_eight_of_the_planes_commands` pins that, and `keyboard()`'s doc now says it.
- `crates/openmx/src/hud.rs`: a new `Hud::plane` flag. With it set, the control band shows two rows:
  - Top row: the plane's eight commands, as `PlaneInput::from_pad` read them. A cell fills by how
    much of the command was written: the throttle's pressure, or a surface's deflection on that
    command's side.
  - Bottom row: the control each command is bound to, as the pad holds it.

  The two rows differ exactly where the original's arithmetic makes them differ:
  - with both L1 and R1 held, only the left rudder fills (left wins);
  - half a stick fills an eighth of a bank cell (the cube).

  The left stick's cross and dead-zone box use the ±0.3 range, and the ramp column shows the five
  numbers. Nothing sets the flag yet: the shell has no vehicle switch, so P19 should set it when it
  puts the plane up. With the flag off, nothing about the bike's HUD changes.
- `research/oracle.txt`: `biplane_input`, `pad_control_copy`, and the shipped binding table. The
  entry sits beside the other pad names, not at the end of the file.
  - `pad_control_index` was already named, at the camera block; I put no duplicate in and only
    describe it.
  - MXBiplane's own input slot, `./kit dis 0x355f40`, is a bare `j` to the Biplane one, so the step
    ported here is the one the game runs.
- Plan: added **I7**, the bike's stick dead zone (below).

## What the original does

The plane never asks the pad for a button. It works in three stages.

**1. Copy the pad's controls.** Every tick the player record copies each pad's sixteen control
records into its own table (player `+0x334`, stride 0x18, value at `+4`): `./kit dis 0x134cc0`, the
pad's vtable `+0x1c`, a memcpy from the pad's `+0x328 + slot*0x18`. It also copies the left stick
into `+0x324`/`+0x328`.

**2. Look up each command.** The plane's input step (`./kit dis 0x351e20`; f12 = x, f13 = y, a0 =
plane, a1 = table, a2 = pad) looks each command up in the front end's binding map
(`./kit fn 0x11e2d0`) and reads that slot's value. Then, in this order:
- `+0x338` = the value of command 0x0e (throttle up), as it is. So it is the pressure,
  `(p+1)/256`, not a switch.
- `+0x33c` = the value of command 0x0f (throttle down).
- `+0x340` = `(float)(-sign(y)) * |y*(y*y)|` and `+0x344` the same of x. Sign is −1 below zero,
  else +1, so a centred stick writes **−0.0**.
- `+0x348` = −1 if 0x14 is held, else +1 if 0x15 is, else 0.
- Then the d-pad overrides the stick:
  - 0x00 → aileron +1, else 0x01 → −1;
  - 0x02 → elevator +1, else 0x03 → −1.
- "Held" is `value != 0.0`.

Shipped bindings, the same for pad 0 and pad 1 (read from the biplane and first_recording RAM):

| command | control |
|---|---|
| 0x0e | Cross |
| 0x0f | Square |
| 0x14 | L1 |
| 0x15 | R1 |
| 0x00 | Left |
| 0x01 | Right |
| 0x02 | Up |
| 0x03 | Down |

**3. The joystick step** (`./kit dis 0x352160`, R8's "controls" slot) makes these into the
throttle and the surfaces. It is not part of I6, but it decides the keyboard question:
- The throttle moves by `ThrottleIncrement·dt` whenever throttle-up is **nonzero** and
  throttle-down is zero (and the other way round). So the throttle-up's pressure only gates; its size
  is never read.
- On the ground, a throttle at or below 0 jumps to ReverseThrottle.
- The surfaces walk toward command × max at their own rate × dt.

**The stick range is not I2's.** In a ride, the player's four stick consumers (left x, left y,
right x, right y) carry centre 0, range −1..1 and a dead zone of **±0.3**. I2 measured ±0.2 on the
main menu's consumers. I found this by scanning both gameplay save states' RAM for every registered
axis consumer:
- first_recording (on the bike) has exactly these four, all at 0.3;
- biplane has exactly these four, all at 0.3.

So report bytes 90..=165 read zero, and the rest is 1/0.7 steeper than the byte. The bike port
still reads `Pad::axis` (±0.2). That is outside I6, so I did not touch it and filed it as **I7**.

## How it was proved

- **Function oracle**: `the_biplane_input_step_is_the_originals`, 12,000 calls of the original
  step over the biplane state's RAM (shipped binding table in place), both pads.
  - Inputs, all generated: sticks through the player's range, sticks past ±1, ±0.0, and random
    16-slot tables holding 0, 1.0 or pressure fractions.
  - Every one of the five outputs matched bit for bit. This is the only proof of the d-pad override
    and of a both-held pair, because the human never pressed either.
- **Capture, tick for tick**: `the_biplane_reads_the_pad`, all 1,717 ticks of C7's
  `context/capture/acceptance/biplane.csv`, run through the port's `Manager` and `PlaneInput::from_pad`.
  Each tick is checked against the original's RAM:
  - the 16 table values;
  - the 2 player sticks;
  - the 5 plane commands.

  All are bit-exact, with two kinds of measured exception, both asserted exactly rather than
  skipped:
  - **R1's table entry.** It reads 0 on every tick R1 is held (154 of 154), though the plane read
    it as held (rud_cmd = +1 on each). Something after the input step clears it.
  - **Six rows (150, 158, 162, 163, 177, 179)** whose sticks and both stick-derived commands are the
    *previous* tick's. The recorder read them before the step wrote them; these are C7's torn rows.
- Output: `the biplane reads the pad: 1717 ticks, 16 table values, 2 sticks and 5 commands each,
  bit-exact (6 rows with last tick's sticks, 154 with R1's entry cleared)`, and `the biplane input
  step: 12000 calls, both pads, bit-exact`.
- **Shell**:
  - `the_keyboard_reaches_all_eight_of_the_planes_commands` (pad.rs) runs a second of each key
    through `Pads::build`, the game's poll and `PlaneInput::from_pad`.
  - `the_plane_shows_its_eight_over_the_controls_they_are_bound_to` and
    `the_planes_stick_has_the_players_dead_zone` (hud.rs) check the plane HUD.
  - `./kit check -- -p openmx`: passed.
- **Full gate**: `./kit check` gives TESTS PASSED, but only with biplane.csv moved out of
  `context/capture/acceptance/`. With the capture there, air.rs's
  `a_flight_falls_and_carries_like_the_originals` reds ("biplane.csv: only 0 ballistic ticks"). C7's
  branch already makes that test skip plane captures, so I left air.rs alone. The two biplane tests
  pass with the capture in place.

## KeyAxis for an elevator and an aileron

**Verdict: it suits, and I changed nothing.**

How a held arrow behaves now:
- It crosses the ±0.3 dead zone in ~54 ms, about 3 frames, not the 35 ms `KEY_TO_EDGE`'s doc
  quotes for the bike's I2 range.
- After that the input is the cube of a ramp, so a held arrow is ~2% at 0.09 s, ~14% at 0.12 s, ~44% at 0.15 s
  and exactly 1 at 0.18 s.
- So a tap of a frame or two moves nothing, a short hold is a gentle nudge, and a held key is the
  full deflection the game's own d-pad bindings give instantly.

Why that is enough:
- The original made digital pitch and roll a designed control (the d-pad), so a key that ends at
  ±1 is how the game expects to be flown without a stick. The ramp only adds a ~0.1 s soft start
  in front of it, which is the feel the bike's arrows already have.
- The surfaces themselves are rate-limited by the joystick step, so the ramp is not the only
  smoothing.
- The throttle's `KeyRamp` makes no difference to the plane beyond crossing the pressure threshold
  on its first frames: the joystick step only asks whether throttle-up is nonzero.

## Not verified / not 1:1

- **Who clears R1's table slot after the step.** It was measured on 154 of 154 held ticks and is
  asserted in the test, but the writer is unidentified. The port does not clear it, because nothing
  the port has reads it after the plane does. If a later consumer turns out to depend on it, the
  port will be wrong there.
- **The second-pad merge into the player record is not ported.** When a second pad is bound to
  the same player, the player record takes its sticks where they are further out and its controls
  where the first's are 0. `from_pad` is the one-pad case, which is every player the port has.
- **Only the shipped binding table is carried.** A remapped controller profile in the front end
  would bind other controls; the port has no front end to remap with.
- **The d-pad override and a both-held pair are proved by the oracle only.** The human's recording
  never presses the d-pad (ctl12–15 are 0 on every row), so the capture cannot show them.
- **Six capture rows are proved against the previous tick, not their own.** This is the recorder's
  timing (C7's torn rows), not the port; the test asserts exactly which rows.
- **The bike still reads ±0.2.** RAM says the ride's stick consumers carry ±0.3, but I did not
  check the bike against a capture in the 0.2–0.3 band, and it is outside I6. Filed as I7.
- **The plane HUD and the plane bindings are not live.** The shell has no plane to fly until P19,
  so `Hud::plane` is never set and no one has ridden the keyboard layout on a plane. The keyboard
  verdict above is reasoned from the arithmetic, not flown.
- **No emulator run this task.** Everything rests on C7's capture, the two save states' RAM and
  the function oracle.

## Next

P19 sets `hud.plane = true` when the vehicle switch puts the plane up, and feeds
`PlaneInput::from_pad(pad)` to the plane's joystick step (S33–S36). I7 settles the bike's dead zone.
