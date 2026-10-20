# 1 — p a brake like the original (FINAL)

Task P23. 2026-10-20.

## What I did

- **Recorded the original braking to a stop.** I did it twice, on emulator copy 1 (slot s1). `./kit pad` cannot be used here
  because `/dev/uinput` is not writable for this user. So the input came from hand-made `.p2m2` movies, played
  over AT-SPI with C6's playback scripts. Each movie is one of the human's recordings, cut on a fast, flat,
  straight stretch (`tools/flat.py` finds those). After the cut it holds 400 fields of square alone: buttons `ff 7f`,
  stick `7f 7f 7f 7f`, square pressure `ff`, everything else released (`tools/mkmovie.py`). The save state
  is copied beside it (`tools/cpstate.py`).
  - `brake1` = `first_recording.p2m2` cut at field 2072. Captured as `context/capture/p23/brake1.csv`, 210 rows
    from frame counter 3794.
  - `brake2` = `recording2.p2m2` cut at field 2595. Captured as `context/capture/p23/brake2.csv`, 150 rows
    from frame counter 3659.
  - Both captures were made with `./kit pcsx2 with -- bash
    research/journal/2026-10-20-c6-c6-recording2-capture/playback/play_capture.sh <pid> <movie> <first fc>
    <rows> <out.csv>`. During the run `[emu] vsync_addr` was pointed at `pad0_stamp_a` (0x47a9d8), so rows trigger on the
    pad stamp and not on the frame counter (see the memory note on torn rows). It was put back to 0x479d04
    afterwards. Neither capture has a gap in its frame counter, and no position repeats.
- **Read the original's brake levers live** over the brake1 playback (`tools/play_read.sh` + `tools/levers.py`,
  bike 0x00dad810, words +0x630..+0x65c). From fc 3811, square sets +0x640 = 1 and drops the gas (+0x63c) and the
  throttle (+0x658) to 0. **Both** wheel brake words, +0x650 (front) and +0x654 (rear), ramp 0.333 → 0.667 → 1.0
  over fc 3812–3814, together. So square brakes both wheels in the original. That is what `Held` already does
  with `Controls::brake`, and S16's open question ("one lever into two rate limiters, or two levers") is
  answered for square: one lever, two ramps.
- **Read each wheel's tire force live** over the same playback (`tools/wheelread.py`). The front wheel is at 0x00da7d10 and the rear at
  0x00da26d0. Each was found as the word pointing at bike+0x650 / +0x654, which is the wheel's +0x30. Read per tick:
  contact +0xdc, the spinning/locked flags +0xe0/+0xe4, load +0xf8, side force +0x850, forward force +0x858,
  spin +0x1d4, the brake lever through +0x30, and the chassis body's velocity at 0x00da79d0+0x260.
- **Found that the brief's "twice as hard" is a unit mismatch and not a brake defect.** See below.
- **Added `race125p_brakes_like_the_original`** to `crates/game-sim/tests/ride.rs`. It reads race125p.phy and the
  `125_faisst` model's tire nodes off the disc, and skips with a line when the disc is absent. It rides the bike on the level at
  the 30 Hz clamp up to 76.95 ft/s, holds `Controls { brake: 1 }` with the stick centred, and asserts that the
  60→12 ft/s decel and the time to under 1 ft/s are within 15% of brake2's. It passes: **46.7 ft/s² against 48.8 (−4%), and 1.70 s
  against 1.63 (+4%).**
- Named the original's tire-force helper `wheel_tire_drive_force` (0x0025ca60) in `research/oracle.txt`.
- **No change to `ride.rs`.** On the same footing the port's brake is within 4% of the original's. The one
  difference in the brake path is the `WheelFriction` torque, and that cannot be wired without recalibrating P10's fixture
  bike (below).

## What the original does

- `./kit fn 0x25c390` (wheel_surface_drag): this is as P13 read it. A signed fade is built first (`0.3 * roll` on the
  ground, the spin in the air, each clamped to ±1). Then `-fade * WheelFriction[s]` goes onto +0x210 unconditionally, and
  `-fade * MaxBrakingTorque[s] * lever` when the lever (`*(wheel+0x30)`) is above 0. There is also a `WheelDrag * v²` force,
  which is zero on race125p's surfaces except the rear's index 2.
- `./kit fn 0x25c530` (wheel_spin_integrate) calls `0x25c920` and then `0x25ca60`. Its force vector
  (+0x850 side, +0x858 forward) is rotated through the wheel frame at +0x8d0..+0x8f0 and added to the chassis' force
  accumulator. Then the spin is set: locked (+0xe4) → spin 0; not spinning and on contact → spin = contact speed (+0x848)
  × inv radius; otherwise it integrates `dt * (+0x210 - forward * radius) * inv_inertia`.
- `./kit fn 0x25ca60` (wheel_tire_drive_force): `f = +0x210 * +0x838`, `cap = MaxDriveForce[s] * load`. If `f > cap`
  (or +0x44 > 0 and f > 0), `f = SkidDriveForce[s] * load` and the spinning flag is set. If `f < -cap` (and cap > 0),
  `f = -SkidDriveForce[s] * load` and the locked flag is set. Otherwise f is left alone. The rest of the function is the side force.
- Live numbers on brake1, during the steady brake on the level (fc 3828–3858):
  - The front puts down a constant **−279.2 lb**. That is exactly `-(325 + 10) / 1.2`, i.e. (MaxBrakingTorque +
    WheelFriction) / TireRadius on race125p's numbers.
  - The rear puts down −291 to −308 lb: the same plus the engine's braking through the clutch.
  - Loads are about 220–270 front and 140–200 rear, so neither cap binds and no flag is set.
  - On brake1's bump (fc 3818–3831) the loads drop to 50–140. The caps bind there, both wheels lock (+0xe4 = 1, spin 0)
    and the force becomes `-SkidDriveForce * load`.
- The port's equivalent on race125p (the same test, one-off debug print, since removed): front −270.8, which is
  325/1.2 with no `WheelFriction` torque; rear −283 to −317; loads about 222–241 / 142–178; no cap binds.

### The 1.25 in the captures (P10's 1.254, settled)

The captures' `bike_vel` columns (actor +0x130) are a steady **0.8** of the chassis' real speed. Three independent reads agree
on that:
1. The chassis rigid body's own velocity (0x00da79d0+0x260), read live: 70.45 ft/s at fc 3810, where the capture's
   `bike_vel` says 56.89.
2. The wheels' spin times their radius: 59.14 × 1.2 = 70.97 at the same tick.
3. The capture's own positions over its `dt`: 71.07. The ratio is 1.249–1.253 on every row of both captures.

The port's `Ride::chassis.vel` is the rigid body's, so the original's speeds must be taken off the positions.
Where the 0.8 comes from (the actor copy's own scaling) was not chased.

### The captured brake, in the body's speed (`tools/bodyspeed.py`)

| capture | where | lever bites at | 60 → 12 ft/s | to under 1 ft/s |
|---|---|---|---|---|
| brake2 | Free09 near (3916, 122, 1729), heading ≈ −160°, y within 0.2 | 76.95 ft/s, row 13 | rows 24→53, **48.8 ft/s²** | row 62, **1.633 s** |
| brake1 | near (4372, 196, 3170), heading ≈ 137°, slight downhill, a bump at fc 3818–3831 | 71.07 ft/s, row 16 | 42.2 ft/s² | 1.667 s |

In `bike_vel` units, brake2 is 39.2 ft/s² from 61.55. Measured that way the port's 46 looked "1.2–1.4× too hard", and P15's
54 ft/s² from 121 ft/s looked about twice as hard. P15's figure is higher than this test's because it starts higher: at
121 ft/s the engine's braking through the clutch adds to the brake (the rear wheel's extra force grows with speed in both the
port and the original).

## How it was proved

- **A save-state capture of the original**: `brake2`, 150 rows, square held to a stop on the level. Compared
  against the port by `race125p_brakes_like_the_original` (`crates/game-sim/tests/ride.rs`), which rides the disc's race125p
  in the port. Result: 46.7 vs 48.8 ft/s², 1.70 vs 1.63 s. `brake1` was used as a cross-check: 42.2 ft/s² over a bump and a
  downhill, and the port's 46.7 is within 11% of it as well.
- **Live PINE reads of the original over the brake1 playback**: the brake levers (both wheels, ramped over three ticks),
  each wheel's forward force, load and lock flags, and the body's velocity. These are not frame-exact samples (the reader
  polls the frame counter), but every value was read on its own tick, and the brake force is constant over 30 ticks.
- `./kit check`: "TESTS PASSED" over the whole workspace, with the disc and both captures present.

## Not verified / not 1:1

- **The captures are not on the x=4620 lane the brief asked for.** Teleporting the bike over PINE (actor transform,
  the body's centre of gravity and quaternion, the vehicle's copies) was undone on the next tick every time. The likely
  cause is the integrator's position history at body +0x2b8. So the human's movies were cut at flat, straight, fast
  stretches instead. brake2 is level to 0.2 ft. brake1 has a slight downhill and a bump that locks both wheels for 13 ticks,
  which is why it is only the cross-check.
- **"Full speed" is 77 ft/s (body), not race125p's top speed.** The human's recordings never hold the throttle long
  enough on a flat stretch to reach top speed, and a longer gas hold in a crafted movie would have run off the flat. So the
  brake from 121 ft/s that P15 traced in the port has no captured counterpart. The test compares 77 → 0.
- **The port leaves out the `WheelFriction` torque**, which is about 8 lb a wheel on race125p and the whole of the 4% the
  test shows. It is one line in `surface_drag` (P13 left it out on purpose). Wiring it means the fixture bike in
  `freeride-bike.phy.txt` spends `WheelFriction` twice (once as grip in `spin_integrate`, once as torque). P13 measured
  7 ft/s of top speed lost and the minute's jump moved: a recalibration of P10's fixture, not P23's.
- **The port's longitudinal force is still P4's stand-in, not 0x25ca60.** The port clamps the force to
  `MaxDriveForce * load` and then to `WheelFriction * load`. The original instead switches to `±SkidDriveForce * load` past the cap, sets
  the spinning/locked flags, and zeroes a locked wheel's spin. On a level brake no cap binds in either, so the test cannot
  see the difference. It shows on a bump (brake1 fc 3818–3831) and in wheelspin. Porting it as it stands would leave the
  fixture bike, which has no `SkidDriveForce` keys, with zero drive past its cap. That is also a fixture recalibration.
- **The 0.8 between `bike_vel` and the body's speed is measured, not explained.** Its source in the actor's update was not
  read. Every earlier test that calibrated against `bike_vel` (P10's `CAPTURED_TOP_SPEED`, `CAPTURED_SLOWING`,
  P15's launch) is comparing the port's body speed against a column that reads 0.8 of it. Those were not touched (not
  this task's), but they deserve a look.
- **The lever reads and the wheel reads are polled**, not captured with the recorder: the reader samples whenever the
  frame counter moves. A row could be read mid-tick, as `frame_counter-triggered captures tear` warns. The values used here
  are constant over many ticks, so a torn row cannot have changed them.
- **The test's 15% is the brief's.** Both numbers are inside 5%, and the test does not hold them tighter.
- The test reads only surface 0 for the port's flat floor. The original's wheels were on surface 13 in Free09. race125p's
  tables are identical across surfaces for every key the brake reads (dumped from the state at the wheel offsets), so this
  makes no difference here.

## Next

- If the `WheelFriction` torque and the 0x25ca60 force model are to be ported, do it together with a recalibration of
  `freeride-bike.phy.txt` (add `SkidDriveForce` keys, and treat `WheelFriction` as the torque the original treats it as).
  That is one task, and this test will tighten to about 1%.
- Re-read the speed-calibrated tests against positions instead of `bike_vel` (the 1.25).
