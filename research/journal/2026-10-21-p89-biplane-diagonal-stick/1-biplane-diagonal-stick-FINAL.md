# 1 — biplane diagonal stick

Task P89. 2026-10-21.

## What was wrong

I logged the plane's pad, surfaces and attitude every tick on a held down-left. The plane was flying
level after thirty seconds of gas, under P69's default.

- **The keyboard's arrows were never the problem.** The arrows are the d-pad. Left+Down writes ±1 to
  the aileron and the elevator, and the plane pulls and rolls together on every tick.
- **A real stick is the problem.** The original reads each axis on its own. Each axis gets a ±0.3 dead
  zone (`AxisRange::PLAYER`, so the dead zone is a square) and is then cubed (`PlaneInput::from_sticks`).
  On a round gate the corner is about 0.71 on each axis. That comes out as **0.19 of the pull and 0.20
  of the roll**, against 1.0 straight back or straight left. A stick 17° off straight back has no roll
  at all. That is "as if the stick were read in four directions".
- **And the weak diagonal turns the pitch loop off.** At 0.20 the aileron is outside `AileronDeadZone`
  (0.1 on the disc). `biplane::autopilot` then drops its pitch-rate loop and flies the elevator
  directly: 0.19 × `MaxElevator` = −0.099. The loop's own trim, with the stick centred, was about −0.18.
  So the pull is *less* than no stick at all, and **the nose falls while the stick is held down-left**.
  The same thing happens at any part-tilt diagonal: as soon as the roll leaves the dead zone, the pull
  goes.
- I ruled out P69's fade lift. `min_elevator` 1 holds the fade at exactly 1 on every tick.

## What I did

Under the New scheme only, the plane's step (`vehicle::Plane::step`) now does three things. The new
`round_stick` flag turns them on (`PlaneScheme::round_stick`, `Garage::set_round_stick`). `main.rs` sets
it beside P69's `set_every_bank`, in both the live seat path and the `--play` path.

1. **`vehicle::round_stick`** replaces the stick's elevator and aileron in the input step's `Pad`.
   - It takes the stick's distance from centre and puts it through the same 0.3 dead zone, the same
     segment arithmetic and the same cube.
   - It then stretches the direction out to the square, so the edge of the gate is full travel at every
     angle. A held corner is the full pull and the full roll. A part tilt is the same fraction of each,
     in the stick's own proportion. Straight along an axis it is the original's reading, to within
     1e-6.
   - The d-pad still wins over the stick, one field each, as in the input step.
   - The centre byte reads −1/255 through the original's arithmetic. The stretch would carry that into
     the other axis, so an axis byte within one step of centre (0x7f or 0x80) counts as centred.
2. **The pitch loop never lets go.** `AileronDeadZone` is set to 2 for the step, which is wider than
   the stick's travel. So the elevator always asks for a pitch rate (`elevator × MaxPitchRate`) and the
   PI chases it. That gives a pitch response in proportion to the stick, with trim included, at every
   roll input.
3. **The loop's coordinated-turn feed-forward fades out as the aileron leaves the original's dead
   zone.** It is full inside the dead zone and gone by twice its width. Without the fade (I tried item
   2 alone first), a full corner held over the top had the feed-forward *pushing* once the bank passed
   about 110°: the command went from −0.52 to +0.52 for about 20 ticks. That was a new drop-out. With
   the fade, the stick inside the dead zone gets exactly the original's loop (feed-forward × 1).

Both autopilot values go back to the file's after the frame, the same way P69 restores `min_elevator`.
The simulation code is untouched. Under `Original` and `Screen` the flag is off and nothing changes.

I took nothing from opentennis. It has no flying vehicle and no radial stick reading.

## How it was proved

- `the_plane_reads_its_stick_round` (ride.rs, no disc needed) runs every pair of stick bytes (65,536)
  through the poll, then compares `round_stick` with the original's input step. It checks:
  - Along an axis, the two agree to within 1e-6.
  - Inside the radial dead zone, both values are 0.
  - Past the dead zone and off both axes (>40,000 pairs), neither value is ever 0. Both have the
    stick's signs, and elevator/aileron equals the stick's y/x to within 1e-5.
  - At every degree round a round gate's edge, the longer value is ≥ 0.95.
  - At a round gate's four corners, both values are ≥ 0.95, where the original gives < 0.25 for each.
  - The d-pad's bank and dive still win.
- `a_held_diagonal_pulls_and_rolls_together_every_tick` (ride.rs, needs the disc's `biplane.phy`)
  starts from level flight and flies 120 ticks in the shell's own order. It covers every corner of a
  round gate and of a square gate, the arrow pairs, the 0.71 tilts and the half tilts, in all four
  directions.
  - On every tick, the pad holds its values.
  - From tick 20 on, the aileron surface, the body's pitch rate and the body's roll rate keep the
    stick's signs on every tick.
  - The corners are full on both axes, and the tilts are equal on both.
  - The test first reproduces the bug under `Original`: on the round down-left corner, the nose falls
    on more than half of the settled ticks.
  - The slowest settled rates, in rad/s:

  | case | elevator / aileron | slowest pitch | slowest roll |
  |---|---|---|---|
  | round corner down-left | −0.982 / +0.993 | 0.3149 | 0.5571 |
  | square corner / arrows down-left | −1 / +1 | 0.3149 | 0.5608 |
  | 0.71 tilt down-left | −0.198 / +0.201 | 0.1796 | 0.1204 |
  | half tilt down-left | −0.023 / +0.023 | 0.0163 | 0.0134 |
  | round corner up-right | +0.993 / −0.982 | 0.5922 | 0.5588 |

  The other two corners are within a few percent of these.
- I checked that the test was not skipped. A temporary panic printed the table above from the disc's
  file.
- P69's `the_default_biplane_stick_pulls_the_nose_toward_the_planes_up_at_every_bank` now also sets
  `round_stick`, as the shell does. Upright, New and Original are still bit-identical. The roll-in to
  45° now keeps its pitch loop and arrives with the nose up (nose_y 0.14 where it was −0.06). From
  there the pulled stick turns the nose 0.098 where it was 0.005. The original's stick there has 0.16
  of the New's say instead of 0.02, so that bound went from a tenth to a fifth.
- `./kit check`: the one failure was netplay's `four_peers_and_eight_riders_on_a_bad_network` (the
  known load flake), which passed on a rerun alone.

## Not verified / not 1:1

- **The New scheme's stick is deliberately not the original's.** It has a round dead zone and a
  direction stretched to the square, where the original uses a square dead zone and a cube on each axis.
  The pitch loop runs at every aileron, and its feed-forward fades out between 1× and 2×
  `AileronDeadZone`. Those are the point of the task. Under `Original` none of it runs. The existing
  P50/P69 tests still hold `Original` byte-for-byte and field-for-field.
- **Along an axis it is the original's reading only to within rounding (1e-6), not bit for bit.**
  Through `sqrt` and the stretch's divide, a straight stick can land an ulp away. Upright, the P69
  flight came out bit-identical anyway.
- **The fade's width (one more dead zone's width) and the one-byte centre snap are my own choices.**
  They are not the original's: the original has no such fade.
- **A stick a few degrees off an axis now gives the other axis a little.** It gives the tangent of the
  angle times the major axis's value, where the original's square dead zone gave nothing. This is what
  reading the stick round means, but a pad held "straight left" now pitches slightly if it wobbles.
- **The roll can still stall on its own at part tilts.** At the 0.71 tilt, the plane stops banking at
  about 62° after about 3 s, even though the aileron is held. That is the plane's own aerodynamics
  (adverse yaw against the aileron), not the stick: the command holds on every tick. The test covers 2 s.
- **Not flown by a person, on a real pad, or against PCSX2.** The pad bytes for a round gate are
  computed (127.5 ± 127.5·cos/sin), not read off a device. The flight is the port's own model on a flat
  floor. The original has no New scheme to compare against.
- **A `--record` tape under New replays the same only under New** (seat 0's `--biplane-controls` on
  `--play`), as P69 already noted for the fade. Netplay still does not carry the plane.

## Next

none
