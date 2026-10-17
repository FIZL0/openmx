# 1 — the brake that never stops (FINAL)

Task P13. 2026-10-17.

## What I did

Made the brake brake. `it_brakes_to_a_stop` is no longer ignored in either
`crates/game-sim/tests/ride.rs` or `crates/openmx/tests/ride.rs`, and ten seconds of pinned brake now
takes the minute's bike from 47.7 ft/s to 0.09 ft/s in two seconds and then holds it at exactly
0.000 for the other eight, and for the eight more the rider's hands are off the pad.

**The brief's diagnosis was wrong, and the reason it was wrong is the finding.** Both tests' doc
comments blamed a *locked* clutch: the bike settled at the road speed of the engine's idle through
first gear, so the story was that `mv::engine_update`'s MinRPM clamp was driving the rear wheel
against the brake and that `ride.rs`'s idle rule (`speed < IDLE_SPEED && gas_rate_limited == 0`,
7.33 ft/s) could never fire to let the clutch out. I measured it instead of believing it:

- the clutch is **slipping** the whole way down the brake leg, not locked — `clutch_locked` is false
  and `clutch_in` is the rider's rate-limited lever, which is zero;
- the hold is a **servo**, not an idle. A slipping clutch passes `ClutchKineticFriction` in whichever
  direction the engine and the road disagree. On the minute's bike that is 70 ft·lb at the crank
  through first gear's 2.1 × 3.6 = 7.56, so **±529 ft·lb at the rear wheel**, and the sign flips the
  instant the road's revs fall under the MinRPM-clamped engine revs. The speed that balance sits at
  is 1800 rpm / 7.56 × 1.12 ft = **27.9 ft/s**, which is where my probe held, to the tenth;
- so the engine was pushing back with 529 ft·lb against a rear brake of 400 — and winning by more
  than that difference, because **the brake was not reaching the chassis at all**.

`ride::surface_drag`'s stand-in took `MaxBrakingTorque * brake` off the rear wheel's `spin`. For a
wheel in contact, `spin_integrate` *overwrites* the spin from the road in the same substep
(`spin = roll * inv_radius + slip`), so the subtraction was thrown away every tick and the pad never
became a force anywhere. The only thing slowing the bike was whatever the engine's own braking
happened to drag back through the clutch, which is why the deceleration came out plausible-looking
at speed and why it vanished the moment the clutch's servo took over. The original does not put the
brake on the spin: it puts it on the **driven-wheel torque accumulator**, the same one the engine
adds to, where `spin_integrate` turns it into a force at the contact patch capped by the grip the
load allows. That is the change — one `if` and one accumulator, in `surface_drag`:

```rust
let fade = if wh.w.contact { clamp_unit(F(0.3) * pt.roll) } else { clamp_unit(wh.mv.spin) };
if brake > F::ZERO {
    wh.mv.drive_torque = wh.mv.drive_torque - (brake * surf.max_braking_torque) * fade;
}
```

With it, nothing in `mv::engine_update` or in the idle rule had to change, and **neither did**: both
are faithful (see below), the clutch's servo is the original's behaviour, and it simply loses to a
brake that exists. `crates/game-sim/src/move.rs` is untouched by this task.

Nothing else in the crate changed shape. `surface_drag` lost its `dt` argument, because the
original's own version does not read the `dt` it is handed either.

## What the original does

`./kit fn 0x0025c390` — `wheel_surface_drag`, already in `research/oracle.txt` as "the three
per-surface tables at +0x778, +0x7b8 and +0x7f8 ... into a force on the chassis and a torque at
+0x210". Read instruction by instruction, it is four steps and one number:

1. **One signed fade, clamped to ±1**, and both of its torques are that fade times a table entry:
   airborne (`wheel+0xdc == 0`) it is the wheel's spin at `+0x1d4`; in contact it is
   `0.3 * wheel+0x198`, the contact velocity's forward lane in the owner's frame. So the table's
   number arrives whole above 3⅓ ft/s of slide and fades to nothing at a standstill — which is what
   stops any of this from shoving a stopped wheel backwards.
2. **In contact only, one force**: `-sign(v) * WheelDrag(+0x7f8) * v²` down the owner frame's forward
   lane, rotated into the world and added to the chassis' force accumulator (`chassis+0x130`). It
   never touches the chassis' torque accumulator — P8 established that and it still holds.
3. **Always**: `wheel+0x210 -= fade * WheelFriction(+0x7b8)`. `wheel+0x210` is the drive-torque
   accumulator (`./kit fn 0x0025c380`, `wheel_add_drive_torque`, which is how the engine reaches the
   same field).
4. **If the brake pointer at `wheel+0x30` is non-null and the lever is positive**:
   `wheel+0x210 += MaxBrakingTorque(+0x778) * fade * -brake`. `wheel+0x30` is the per-wheel
   rate-limited brake the bike's own setup points at `bike+0x650/+0x654` (oracle, S15's
   `engine_ctor` line), and the oracle's own note on `+0x210` already says what it is: "an
   accumulator: the engine, the brake and the tires all add here".

The table index is the contact record's surface byte at `wheel+0x110` in all three cases.

Step 4 is what P13 ported, with step 1's fade. Step 3 is deliberately still not ported — see below.

## How it was proved

- **Against the original's code**: the disassembly above, and `./kit fn 0x00246fd0`
  (`engine_update`) and `./kit fn 0x00257a00` (the ground step) read to settle that the clutch and
  the idle rule are not the defect. `engine_update` is bit-exact against the original by S15's
  function oracle, the MinRPM clamp on the locked path is the original's own, and the only two
  writers of `bike+0x65c` (the rate-limited clutch) are the rider's lever and the idle rule, both of
  which the port has. Leaving them alone was a decision, not an omission.
- **Against the disc's own data**: `Bike\250sx.res` → `bike\race250p.phy` out of `MX.PSR`, read with
  `game_sim::archive`. Its MinRPM is 4500 with a first-gear overall of 18.97 — an idle road speed
  even further above `IDLE_SPEED` than the fixture's — so the hold the brief blamed on bad gearing is
  what the original's own numbers do too, and `IDLE_SPEED` being unreachable under brakes is not an
  anomaly to fix. The disc's `EngineBraking` is 5e-8 where the test fixture's hand-written value is
  0.3; that is the number the fixture's off-throttle deceleration actually comes from.
- **Against the port's own statement**: `./kit check` — 545 tests, all pass, with
  `it_brakes_to_a_stop` live in both crates for the first time. The game-sim version asserts the
  first two seconds take more than half the speed off, that it does so no harder than the human's
  recording ever managed (`CAPTURED_SLOWING`, 70.9 ft/s²; the port's is 23.8), that the leg ends
  under 1 ft/s, and that the eight-second hands-off leg never exceeds 2 ft/s. Measured: 47.687 →
  35.056 → 24.959 (0.5 s) → 10.450 (1.5 s) → 0.093 (2.0 s) → 0.000 from 2.75 s to the end, and
  0.0000 for all 480 ticks of the hands-off leg. No wrecks in either leg.
- **No emulator was run.** Nothing in this task needed a frame of video or a PINE session: the defect
  was a thrown-away write, and the two numbers that pin the diagnosis (27.9 ft/s and 529 ft·lb) come
  out of the fixture's own keys.

## Not verified / not 1:1

- **`WheelFriction`'s torque (step 3 above) is not ported.** It is one line and I had it working, and
  I took it back out. The reason: this crate already spends `WheelFriction` as the tire's *grip*
  limit in `spin_integrate` (P4's stand-in), and the test bike's `WheelFriction1` of 8/9 was written
  for that reading, so applying it as a rolling torque as well spends one number twice at a scale
  nobody chose. Measured cost of adding it: the minute's flat-out leg ends at 65.5 ft/s instead of
  74.9, which moves the jump late enough that
  `it_takes_the_jump_and_lands_on_its_wheels` runs out of leg to settle in. Putting it in therefore
  means re-deriving P10's fixture `Drag` patch and re-pinning every speed in that file, which is a
  calibration task and not this one. **Whoever does the tire's grip model next owns this.**
- **The `WheelDrag` resistance force is still the port's own shape**, as its doc already said before
  P13 and still says: the original is `WheelDrag * v²` and is *not* scaled by the tire's load, where
  this port uses `WheelDrag * load * clamp_unit(v)`. Only the torques were brought over. Changing the
  force's shape moves every speed in the ride tests, same reason as above.
- **The fade reads `pt.roll` where the original reads the contact velocity's forward lane in the
  owner's frame** (`wheel+0x198`). `pt.roll` is `dot3(contact_velocity, forward)`, which is the same
  quantity whenever the patch's forward and the owner frame's forward agree — true for this port,
  because it has no separate owner frame — but it is not the same load, and nothing proves they stay
  equal for a steered front wheel at a lean. Not tested either way.
- **`0.3` is the original's literal and nothing else.** It is `BRAKE_FADE_RATE` in `ride.rs`. Which
  `.phy` key or constant it corresponds to, if any, is not known; it is immediate in the
  instruction stream.
- **No capture of a braking stop was compared.** `CAPTURED_SLOWING` (70.9 ft/s²) and
  `CAPTURED_TOP_SPEED` (75.78) come from the human recording that is already in the test file; the
  port's 23.8 ft/s² is well inside it but is **not matched by value against the original**, and it
  cannot be on this bike — the fixture is hand-written and its header says no number in it is the
  game's. A PINE recording of a full-brake stop on a known surface is the proof this deserves and
  does not have.
- **The brake torque is one lever for both wheels.** `wheel_integrate` is handed a single `brake`,
  and the original's per-wheel pointers at `bike+0x650/+0x654` are filled from the front and
  rear levers separately. The two tests pin `brake`, `front_brake` and `rear_brake`
  together, so nothing here distinguishes them.
- **Two files outside P13 had to move, and both are pinned-constant churn**:
  `crates/game-sim/tests/platforms.rs`' generated block (`HASHES`, `TRACE_DIGEST`, `POS_Y`, `VEL_Y`,
  `FINAL_DUMP_HEAD`, `FINAL_DUMP_DIGEST`, `FIXTURE_TEXT_DIGEST`), regenerated with that file's own
  `print_the_vectors` because the canonical run holds the brake; and
  `crates/game-sim/tests/motion.rs`' hill ride, which now drops bands it only touched for a single
  frame. That ride holds the brake at the gate for sixty frames, and the brake now holds it there, so
  the whole climb shifted and its speed crossed the second threshold back for one frame on the way
  up — a hundredth of a foot a second of suspension ripple. The bands it rode are unchanged.
  `HUNDRED_TICKS` did **not** move: the hundred-tick run never touches the brake, which is a small
  piece of evidence that nothing else in the ride changed.

## Next

Nothing is pending for the brake itself. The two things this task found and left:

1. **Port step 3 and own the calibration.** `WheelFriction`'s rolling torque plus the `WheelDrag`
   force's real `v²` shape go in together, with `spin_integrate`'s grip limit reading some other key,
   and then the minute's speeds and P10's `Drag` patch get re-derived against the human recording.
   That is a task, not an edit.
2. **`mv::step` still has the missing `clutch_in` write** P8's journal flagged (`move.rs`, just above
   the idle rule). Nothing fails today because it is the call-list reference path, but the two spines
   disagree, and this task's measurements were all taken on the live one.
