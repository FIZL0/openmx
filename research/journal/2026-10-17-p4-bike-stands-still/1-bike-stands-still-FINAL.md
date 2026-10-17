# 1 — bike stands still (FINAL)

Task P4. 2026-10-17.

The port's bike would not stand still: dropped on level ground with nobody touching the pad it built
up a yaw rate of two hundred degrees a second inside a second and wandered off, and on a real track
it carved a circle at the bottom of the hill. The brief asked for the missing upright rather than a
damper turned down by eye, and there is one — but it was not where the brief guessed.

## What I did

**The two hypotheses the brief offers are both wrong, and worth writing down so nobody re-checks
them.**

1. *"The rider controller is wired but not called, or called with the wrong gains."* It is called.
   `ride::Ride::frame` drives all four of S21's controllers — `mv::stand_upright`, `mv::balance`,
   `mv::roll_pitch`, `mv::no_hander` — with the `.phy`'s own gains, in `move::step`'s order, and
   `tests/ride.rs`'s callee-order test already proves that order against `move::step` itself. With no
   input `held.stick` centres, so the steer command really is zero.
2. *"The real bike is held at rest by the brake or the clutch."* It is not held at all. S8's idle
   capture (sixty seconds of the acceptance recording's start, every one of the sixteen `ctlN.held`
   columns zero for all 1,811 rows) shows the **original** rolling off the start line too: in its
   first ten seconds it creeps from 0.52 to 1.72 ft/s and travels 12.3 ft. So there is no hold to
   port. What the original does that the port did not is creep *straight*: over the same ten seconds
   it yaws at most 0.39 deg/s, slides its tires sideways at most 0.0052 ft/s, and holds its roll to
   0.0004 degrees. The port, on *level* ground with the same empty pad, yawed at 201 deg/s and slid
   at 0.4 ft/s. The defect is the spin, not the roll-off.

**The defect is in `ride::spin_integrate`**, the port-authored stand-in for the tire's own force
model, and it is two bugs in one line.

*The per-surface limits are coefficients, not pounds.* `wheel_set_surface_drive` (boot `0x0025ada0`)
takes `SideForce`, `MaxDriveForce` and `MaxSideForce` through one setter, and `wheel_set_surface_drag`
(boot `0x0025ae50`) takes `WheelFriction` through another. On the disc's own dirt they read 0.165,
2.0, 5.0 and 10.0, while a wheel of a 400 lbf bike carries about 200 lbf — so read as pounds the
whole grip circle is five pounds against four hundred, which is a bike with no sideways grip at all.
That is why the retail bike carved: `./target/release/openmx --trace` prints the numbers it loaded
(`front surface0: friction 10 drive 2 side 5 drag 0`) and the stand-in was clamping the lateral force
to five pounds. Every one of the four is now multiplied by the load the suspension is putting through
the patch.

*The lateral force was a velocity damper past the integrator's stability limit.* It was
`-slip_speed * wheel_friction * load`, saturated — a damper with a gain stiff enough to hold a bike
up. Measured on the flat with no input, `ang_accel.y / ang_vel.y` came to 56.9 /s, so with the
substep at 1/60 the eigenvalue times the step is 0.95, which is exactly where `chassis::advance`'s
two-step Adams-Bashforth goes unstable; the saturation then turned the blow-up into a bang-bang limit
cycle alternating +1.80 and -1.62 rad/s of yaw every frame. (That alternation is also why the
quaternion barely moved while `ang_vel.y` read 1.8 — the net turn per frame is the difference, not
the rate. Worth knowing before chasing `QuatDeriv`.)

So the lateral force is now a tire and not a damper, and it is bounded by what friction can do rather
than by a constant:

* the cornering stiffness is the surface's own `SideForce` per unit load against the patch's slide —
  which makes `SideForce%d` a key the port reads, where before it was dead;
* saturated at `MaxSideForce * load`, less the donut command, and at `WheelFriction * load`;
* and held to `m * |slide| / dt`, where `m` is the mass the chassis actually presents to a push at
  that contact point along that direction. A friction force may null a slide inside one substep; it
  may not reverse it. That is unconditionally stable for any surface in the game's data, and it is
  the same argument `surface_drag` already makes for the brake ("the brake cannot turn the wheel
  backwards inside one substep").

The effective mass is the standard one for a constraint — `1 / (d.M-1.d + n.I-1.n)` with
`n = arm x d` — in the body's own frame, with the tensor going through `chassis::rotate`, which is
the three instructions the chassis' own integrator already puts a torque through its inverse inertia
with. New in `ride.rs`: `struct Body` and `fn effective_mass`.

**What it reads now.** Level ground, nobody touching the pad, ten seconds from the drop: the bike
ends 0.027 ft from where it landed, with a yaw rate, a sideways slide and a roll swing that are all
exactly zero in single precision. On the real track (`--trace 1800`, free06/125sx) it rolls down the
fall line the way the original does, peak roll 0.148 rad over sixty seconds, peak lateral tire force
3.2 lbf — the tires are no longer fighting each other.

**Scope.** `wheel_spin_integrate` (boot `0x0025c530`, 1,004 bytes, plus the two unported helpers it
calls at `0x0025c920` and `0x0025ca60`) is the original's own version of this, and porting it is a
task of its own that belongs in `wheel.rs` beside `wheel::Call::SpinIntegrate` — not in P4, which is
a bug in a stand-in that lives in `ride.rs`. Named under **Next**.

## What the original does

`wheel_integrate` (boot `0x0025bbf0`) runs three things per substep in this order: the surface's drag,
the wheel's spin, then the steering hinge. The port has the third (`wheel::integrate`) and stands in
for the first two.

* `wheel_surface_drag` (boot `0x0025c390`) — the rolling resistance of the surface under the wheel,
  out of the three per-surface tables at wheel `+0x778`, `+0x7b8` and `+0x7f8` indexed by the contact
  record's surface byte at `+0x110`, into a force on the chassis and a torque at `+0x210`.
  `0x0025c4c8` is where it reads its own word as a brake and turns it into a torque against the spin.
* `wheel_spin_integrate` (boot `0x0025c530`) — the wheel's own spin and the steering hinge's
  contribution to the chassis' angular velocity, reading the hinge axis at `+0x8c0` and its frame at
  `+0x8d0..+0x8f0`. Neither the frame nor the two helpers it calls are ported.

The dimensional reading above is from the setters, not from the arithmetic inside those two:
`wheel_set_surface_drive` groups `SideForce` with `MaxSideForce` (a stiffness and its cap belong
together), and `wheel_set_tire_derived` (boot `0x0025b160`) is where the wheel works out the numbers
it derives from its tire and surfaces once they are in — which is the natural home for a load
multiplication and is not read here. See **Not verified**.

## How it was proved

Four tests in `crates/game-sim/tests/ride.rs`, section (5). Three of them were run against the code
as it was and fail there with the number in the message, which is the point of writing them this way;
the fourth (`no_cornering_stiffness_can_make_the_bike_spin`) cannot be, because its hundredfold
stiffness is a rewrite of the re-scaled `.phy`'s keys and the old file does not have them — it refuses
to run rather than quietly testing the unmodified bike.

* `a_dropped_bike_stands_still` — ten seconds of an empty pad on `world::Flat`, at both 1/60 (the
  engine's clamped step) and 1/30 (the rate the recording was made at, so one frame is two substeps;
  an instability that only shows at one of the two is still an instability). Moved < 0.5 ft, roll
  < 0.1 rad, yaw rate < 0.02 rad/s, slide < 0.02 ft/s; and after two seconds of settling, moved
  < 0.05 ft. *Before:* 2.97 ft and 3.5 rad/s.
* `no_cornering_stiffness_can_make_the_bike_spin` — the same, with a hundred times the `.phy`'s
  cornering stiffness, which no surface on the disc has and every explicit damper blows up on. The
  bound is on the impulse, so the bike stands just as still.
* `a_tire_kills_a_slide_and_never_reverses_it` — shoved sideways at 20 ft/s and left alone: the slide
  decays monotonically (`|v_side|` never grows, frame by frame, to within a thousandth of a foot a
  second) to under 0.01 ft/s in two seconds, at both rates and at both stiffnesses, and the shove
  never spins the bike past 2 rad/s. *Before:* the slide grew back on frame 13.
* `the_port_idles_no_livelier_than_the_original` — reads S8's idle capture, asserts that all sixteen
  `ctlN.held` columns are zero across the window (so the bounds are not somebody's thumb), measures
  the original's own yaw rate, slide, roll swing and travel over its first 300 rows exactly as the
  test measures the port off its transform, and requires the port to exceed none of them. It also
  checks the capture is of a bike that *creeps* rather than one wedged against something, so the
  bounds are real measurements. Skipped with a printed line where `context/` has no capture (it is
  game data: `AGENT.md`), which is why the three tests above carry the same statement in absolute
  numbers.

`./kit check`: 469 tests passed.

Also re-run by hand: `./target/release/openmx --trace 300` and `--trace 1800` on free06/125sx, i.e.
the disc's own parameters rather than the test `.phy` — the numbers are in **What I did**.

`crates/game-sim/tests/freeride-bike.phy.txt` is re-scaled to match: its five per-surface grip keys
were hundreds and are now coefficients. It is still hand-written and still not the disc's — the
numbers are plausible for a 250 cc four-stroke and deliberately not the ones `--trace` prints.

## Not verified / not 1:1

- **The tire's force model is still a stand-in**, not `wheel_spin_integrate` (boot `0x0025c530`) and
  not `wheel_surface_drag` (boot `0x0025c390`). Nothing in this task was run against either. What
  changed is the stand-in's dimensions and its stability; its *shape* is still this port's.
- **"The limits are per unit load" is read off the setters and the magnitudes, not off the
  arithmetic.** `wheel_set_surface_drive` groups `SideForce` with `MaxSideForce`, and five pounds of
  lateral force on a four-hundred-pound bike cannot be what the disc means — but which function does
  the multiplication (`wheel_set_tire_derived` at boot `0x0025b160` is the likely place) is not read
  from the disassembly here, and the factor could be the normal load, the static load, or a load
  scaled by something else. The *shape* of the fix does not depend on which; the exact forces do.
- **`WheelFriction` never binds on the disc's own surfaces.** It is 10.0 where `MaxSideForce` is 5.0
  and `MaxDriveForce` 2.0, so as the outer grip circle it is slack and its real role is unproved.
  `research/oracle.txt` puts it in the drag setter with `MaxBrakingTorque` and `WheelDrag`, which
  suggests it belongs to `wheel_surface_drag` instead; it is kept here because dropping it would make
  a disc key dead.
- **Eleven per-surface keys are still unread by the port**: `SlippingSideForce`,
  `MaxSlippingSideForce`, `SkidDriveForce`, `SkidSideForce`, `SkidSlippingSideForce`, the
  `SideForceRoundOff/DropOff/Scale/SpeedScale` family and its `Slipping` and `Braking` twins, and
  `DonutsSideForceSpeedScale`. The slipping and skidding force curves are what a real slide looks
  like in the original and none of them are in this model.
- **The brake still does nothing to the chassis.** `surface_drag` takes the brake out of the wheel's
  spin and then `spin_integrate` overwrites that spin with the patch's own speed over the radius, so
  a braked wheel in contact loses nothing. Not touched here: nobody is touching the pad in this
  task's acceptance, so it is not what makes a bike spin out. It is the next thing a rider will
  notice.
- **The capture test compares bounds, not state.** Tick-exact comparison against S8's capture is
  impossible from this side: the original is standing on a real track and the test's bike is on a
  flat plane, and the track's own start-line geometry is game data. What is compared is
  restlessness — turn rate, sideways slide, roll swing, distance travelled — and the port must not
  exceed the original on any of them. The capture's decimal is read with Rust's own float parser
  rather than `replay`'s bit-exact one, because nothing in it is compared for equality.
- **The roll bound the brief asks for does not bite.** `mv::pose` pins roll while the bike is on the
  ground (`ch.flags = (1, 1, 0)`), so on level ground roll cannot move and "under a tenth of a
  radian" is satisfied by a bike that is spinning on its axis — the broken bike held roll to 0.0023
  rad while yawing at 3.5 rad/s. The assertion is kept because the brief asks for it; the ones that
  catch the defect are the yaw rate and the slide. The same goes for "within a few feet": the broken
  bike wandered 2.97 ft, which is a few feet, so the test asks for half a foot.
- **Cornering is not proved.** Nothing here measures whether a *ridden* bike turns the way the
  original turns; the new stiffness is the disc's `SideForce`, but no capture of real cornering was
  compared. `./kit check`'s existing ride, trick and wreck tests pass, which only says the change
  broke nothing that was already proved.
- **`contact_velocity` is unchanged and still unproved** against the original (`wheel_contact_velocity`
  at boot `0x0025b710`): the effective-mass bound takes its arm from the centre of mass, and if the
  original takes it from the transform's origin the bound is slightly loose. Loose is safe — it binds
  later, never earlier — so it cannot reintroduce the instability.
- **`context/capture/s8/` was copied into this worktree from the main checkout** for the capture test
  to read. It is git-ignored and is not in the commit. The subdirectory matters: every CSV directly in
  `context/capture/` is parsed by `tests/replay.rs` as an acceptance candidate.

## Next

1. **Port `wheel_spin_integrate`** (boot `0x0025c530`, 1,004 bytes) and its two helpers
   (`0x0025c920`, `0x0025ca60`) into `wheel.rs`, behind `wheel::Call::SpinIntegrate`, and
   `wheel_surface_drag` (boot `0x0025c390`) behind `wheel::Call::SurfaceDrag`. That retires both
   stand-ins in `ride.rs`, answers where the load multiplication lives, and brings in the eleven
   unread slipping/skidding keys. It is the largest unported thing left in the ground step and it
   wants its own task.
2. **Make the brake reach the chassis.** See **Not verified**; a brake that holds a bike on a hill is
   also what `mv::stand_upright`'s along-slope branch is waiting for.
3. A capture of real cornering would turn `SideForce` from "the right key, plausibly scaled" into a
   proved number.
