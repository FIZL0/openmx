# 1 — p8 full throttle loops out (FINAL)

Task P8. 2026-10-17.

## What I did

Pinned the throttle on the start line and reproduced the brief exactly, then found the one line that
caused it: `ride::spin_integrate` was putting the tire's drive and side force into the chassis **at
the contact patch**. The patch is about two feet under the centre of gravity, so every pound of drive
came with two foot-pounds of nose-up couple and nothing to balance it. The original does not do that.
It crosses the force with a vector it loads straight off the wheel, and that vector is the `.phy`'s
`BodyTireForcePosition` — a quarter of a foot under the body origin on the disc's 125sx, used exactly
as it stands as an arm about the centre of gravity.

Four changes, all in `crates/game-sim/src/ride.rs`:

1. `spin_integrate` takes the arm as an argument (`tire.body_tire_force_position`) and pushes on it.
   `push` takes an arm rather than a point, because its two callers no longer take theirs where
   `wheel::tire_forces` takes the suspension's.
2. `surface_drag` adds a force and no couple at all, which is what the original's own version does.
3. The side force's stability bound (P4's) still measures its effective mass on the **contact
   patch's** arm, not the new one: the slide it is nulling is the patch's. Measuring it on the
   `.phy`'s arm makes the bound about four times looser, and loose is not safe. Written down in the
   function's doc.
4. `Ride::frame` now writes the rider's rate-limited clutch into `mv::Ride::clutch_in` every frame.
   Without it the only writer in the whole crate was the idle rule, which sets it to 1 and never
   clears it: a bike that had ever stood still with the throttle shut had its clutch pulled in for
   ever and could not launch again at any throttle. That is a second reason the bike "reads as not
   moving at all", and it is why the new test could not be written until it was fixed.

Also: `openmx --trace` learned `--trace-gas` (hold the throttle, the way `--trace-brake` holds the
brakes) and now prints each tire's force arm in its header, because that is the number this task is
about and nothing printed it.

Before, on the disc's bike (`openmx --trace 300 --trace-gas`): pitch 0.0097 → 1.3111 rad over frames
8–18, front load 248 → 0 at frame 9, wreck reason 11 at frame 18. After: no wreck in 300 frames,
front load never reaches zero, pitch peaks at about 0.07 rad.

## What the original does

**The tire's force on the chassis** — `./kit fn 0x0025c530`, and the arithmetic at
`./kit dis 0x0025c5f0 --len 200`:

- the force is rotated into the world (`0x0025c5f0`: the three-row `vmulax/vmadday/vmaddz` matmul
  against the chassis' rows at `bike+0x8d0`) and left at `sp+0xf0`;
- airborne (`wheel+0x3c` or `wheel+0x38` clear) it goes down a different arm entirely — `FUN_0025cf40`;
- on the ground it is added to the chassis' force accumulator: `lw $3, 484($16)` is the chassis,
  `addiu $3, $3, 304` is `chassis+0x130`, `vadd.xyz`;
- then, **unless `wheel+0xc0` is set**, the couple: `addiu $3, $16, 176` loads `wheel+0xb0` into
  `vf1`, `vopmula.xyz`/`vopmsub.xyz` against the force at `sp+0xc0` makes `arm × force`, and it is
  added to `chassis+0x140` (`addiu $3, $4, 320`, `vadd.xyz`).

There is no contact point in that sequence and no centre of gravity subtracted from the arm. The
vector at `wheel+0xb0` is `BodyTireForcePosition`, written there by the loader — `./kit dis
0x00253d3c` — and `crates/game-sim/tests/phy.rs`'s loader proof already checks `wheel+0xb0` against
the parsed key in live EE RAM. On the disc's 125sx it reads `(0, -0.25, 0.75)` front and
`(0, -0.25, -0.75)` rear: a tuning arm the file's author chose, not a geometric one.

**The surface drag** — `./kit fn 0x0025c390`. It writes the chassis' force accumulator
(`chassis+0x130`) and never its torque accumulator; everything else it touches is on the wheel. The
three per-surface floats it reads, which answers an open question P4's journal left:

- `surface+0x778` is `MaxBrakingTorque`, multiplied by the brake pedal and applied to the spin;
- `surface+0x7b8` is `WheelFriction`, a constant rolling-drag torque on the spin;
- `surface+0x7f8` is `WheelDrag`, quadratic in the contact's forward speed in the chassis' frame, and
  a pure force — it is not multiplied by the tire's load.

## How it was proved

**Against the original's code.** The two functions above, read instruction by instruction. The arm is
not an inference: it is one `addiu` and a cross product.

**Against the human's capture.** `context/capture/acceptance/acceptance-freeride.csv`, 1,811 ticks of
the acceptance run at 30 Hz with all sixteen `ctlN.held` columns in it. The new test finds the
original's own launch rather than hard-coding a row: the first window where the bike is under
1 ft/s, some button is held for twelve straight ticks, and the speed is five feet a second higher by
the end of it. That lands on row 594, 0.056 → 8.651 ft/s — a standstill launch with the throttle
pinned. Its peak frame-to-frame acceleration is 28.7 ft/s² and its mean over the window 21.5 ft/s².
The port, on the test bike, makes 47.0 and 36.8. The test asserts a factor of two either way and
prints both numbers; see the next section for why it cannot assert more than that.

**Against the port's own statement**, in `crates/game-sim/tests/ride.rs` section (7): a second of
pinned throttle from a settled standstill on level ground, at both 60 Hz and 30 Hz, keeps both
wheels carrying more than a tenth of the bike's weight, keeps the pitch under the wheelie arm, does
not wreck, and reaches 27 ft/s. Put the force back at the contact patch and the front wheel's load
goes to zero on the first two frames, which is what the load floor catches.

`./kit check`: all tests pass.

## Not verified / not 1:1

- **The forward acceleration is not matched by value, only to a factor of two.** The test bike is
  `crates/game-sim/tests/freeride-bike.phy.txt`, which is hand-written and whose header says no
  number in it is the game's; the capture is the disc's bike on the disc's start line, and its
  `bike_type` is 4, which is not the 125sx the `openmx` trace runs. Two different bikes cannot be
  compared by value. The port's launch came out 1.6–1.7× the original's on both the peak and the
  mean. Whether that gap is the two bikes or a second defect in the engine or gearbox is **not
  settled** — it is the obvious next thing to look at and is in "Next".
- **`BodyTireForcePosition` is doing two jobs in this checkout.** Besides the arm above, it is what
  `stock.rs`, `openmx` (when the model has no `TireFront`/`TireRear` node) and six test files hand
  `Ride::new` as the wheel *mounts*. The test fixture's value was written for that second job (axles
  a foot up, a four-foot wheelbase), and read as an arm it is a foot *above* the body origin — a bike
  whose drive force shoves its own nose down. Section (7)'s launch therefore runs on a copy of the
  fixture with that one key rewritten to a tuning arm of the right shape, the way P4's
  `stiffened()` rewrites `SideForce1`, and keeps the mounts from the unmodified file. The two jobs
  are not untangled; doing that touches eight files across three crates and is not P8's.
- **`wheel+0xc0` is not ported.** It is the flag that suppresses the couple when it is non-zero. This
  port behaves as if it is always zero, which is what it is on a wheel the loader has just built, and
  nothing found so far writes it.
- **`ride::surface_drag`'s magnitude is still the port's own stand-in**, as its doc already said; only
  the couple was removed. The three per-surface floats read out of `0x0025c390` above are a reading
  of the disassembly and are **not ported** — the port still multiplies its drag by the tire's load
  and uses the roll speed clamped to one unit, which the original does not.
- **The tire force model around it is still a stand-in.** `FUN_0025c920` and `FUN_0025ca60` are not
  ported, so `along`/`across` are this module's arithmetic bounded by the `.phy`'s coefficients, not
  the original's curves.
- **The brief's weight arithmetic was wrong and the finding does not depend on it.** It gives the
  bike's weight about the rear patch as "mass 12.42 × gravity 400 × 2.59 ≈ 12870 ft·lbf". Gravity in
  this game is a **force** of 400 lbf, not an acceleration: the two spring loads sum to 400 at rest,
  and the trace's `vel_y` gains 1.0725 ft/s per 1/30 tick in free fall, which is 32.2 ft/s² = 400 /
  12.42. So the restoring torque is about 400 × 2.59 ≈ 1036 ft·lbf, not 12870. The couple the brief
  identified is still an order of magnitude larger than it, which is why the diagnosis held.
- **The capture comparison is skipped in a checkout with no `context/`**, like `the_port_idles_no_
  livelier_than_the_original` beside it, and prints a line saying so. `context/capture/acceptance/`
  is git-ignored game data.
- **One test outside P8 had to move.** `motion.rs`'s `a_ride_reaches_the_four_clip_blend_and_the_
  pair` rolled a bike down a hill for 600 frames with the throttle commanded, and its doc said the
  port "cannot drive this bike yet" so the hill's 12.4 mph terminal speed kept it under the grid's
  last threshold. It can drive it now, so the ride reached that threshold and sat astride it,
  crossing back and forth once a frame. The ride now stops at the threshold and the doc says why.
  Nothing about which blend entries it proves changed.
- **The pinned fixtures moved**, as they do whenever the ride changes: `HUNDRED_TICKS` in
  `src/lib.rs`, `tests/hash.rs` and `tests/platforms.rs`, and `platforms.rs`' `HASHES`,
  `TRACE_DIGEST`, `POS_Y`, `VEL_Y`, `FINAL_DUMP_HEAD`, `FINAL_DUMP_DIGEST` and
  `FIXTURE_TEXT_DIGEST`, regenerated with that file's own `print_the_vectors`.
- **No emulator was run.** Everything here is the disassembly, the committed tests and the two
  captures already in `context/`.

## Next

Measure the drive force itself. The couple is right now, but the port's launch is still 1.6–1.7×
the original's and the `openmx` trace reaches 48 ft/s in 1.3 s on the 125sx, which is about 2.4 g —
far more than a 125 makes. The arithmetic to look at is `mv::engine_update`'s torque path into
`spin_integrate`'s `drive`: the curve's torque, `torque_scale`, the overall ratio and
`clutch_torque_scale`'s launch multiplier, against `MaxDriveForce × load`, which on the disc's bike
caps at 1775 lbf and is not what is binding. The capture's row 594 launch is the number to match.

Second, `mv::step` has the same missing `clutch_in` write that `Ride::frame` had (`move.rs`, just
above the idle rule). It is the call-list reference path rather than the live one, so nothing fails
today, but the two spines should say the same thing.
