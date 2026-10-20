# 1 — p a turn that keeps its speed (FINAL)

Task P24. 2026-10-20.

## What I did

P15 measured a held full-lock turn with the gas pinned: the port went from 76 to 35 ft/s in 1.33 s,
while the original went from 61.4 to 52.4 in 1.1 s. The cause was **P4's stand-in tire**. Its side force
was a straight line in the slide, capped at the smallest of `MaxSideForce`, `WheelFriction` (read as a
grip limit) and an impulse bound. In a held turn it pinned at that cap every substep and scrubbed speed
away at the full cap. The second cause was that **the contact frame steered about the chassis' own `y`**,
where the original steers about the fork's raked hinge.

What changed:

- **`crates/game-sim/src/tire.rs` (new).** Ports four functions:
  - the original's tire force, `tire::force` (`./kit fn 0x0025ca60`);
  - the surface drag, `tire::surface_drag` (`wheel_surface_drag` in `research/oracle.txt`);
  - the wheel's spin inverse inertia, `tire::spin_inv_inertia` (`wheel_set_tire`'s disc);
  - the third row of the axis-angle matrix, `tire::steered_forward` (`./kit fn 0x00134290`), which is
    the only row the contact frame reads.

  The file carries `#![deny(clippy::float_arithmetic)]`.
- **`ride.rs`:**
  - `spin_integrate` now runs the original's order:
    - it calls the tire force when this wheel *or its partner* touches;
    - three flags (spinning, locked, sliding) persist on the wheel;
    - the force goes in on `BodyTireForcePosition`;
    - a locked wheel's spin is 0, a rolling grounded wheel's is roll/r, and otherwise the torque is
      integrated less drive·r.
  - `surface_drag` is the original's:
    - the `WheelFriction` torque, which P13 had left out because the stand-in read that key as grip;
    - a drag of `WheelDrag·v|v|` along the chassis' z, not scaled by the load.
  - `patch` builds the forward the way `./kit fn 0x0025c920` does: for a grounded steered wheel, the
    steered row turned into the world; otherwise the chassis' z. Then side = n×f, f' = side×n,
    normalised.
- **`Wheel::hinge` (+0x8c0).** New field, default (0,1,0). The shell (`crates/openmx/src/main.rs`) sets
  it from the model's Shocks→Handlebars line through `ride::hinge_axis`, which is what
  `./kit fn 0x00253760` does. On race125p this gives (0, 0.88078, −0.47353), the same as the live value.
- **The stand-in bike** (`stock.rs` = `tests/freeride-bike.phy.txt`, kept identical from `[Mass` on).
  Every change is commented in the file:
  - added the `Skid…`/`Slipping…` keys. A missing key loads as 0, and the original's linear family
    gives `Skid·load` past the cap, so without them a saturated tire had zero grip;
  - `SideForce` went from 0.2/0.25 to 0.1/0.12. The original magnifies the slide up to ×5 below
    10 ft/s, and on the stand-in's ±2.1 arm the old stiffness made the bike creep and spin at idle.
    Skid keys alone at 3.0/3.5 gave a 16 ft limit cycle, and at 1.5/1.8 it still wandered 11 ft;
  - `WheelDrag` went from 0.02 to 0. The game's own bikes carry 0 on dirt (pro125a/p, race125a/p:
    `WheelDrag 0`, `WheelFriction 10`). In the original's v² meaning, 0.02 held the minute's bike
    under 38 ft/s flat out.
- **`tests/trick-bike.phy.txt`** got the freeride bike's per-surface keys. It still had the pre-P4
  pound units (`SideForce 60`, `MaxSideForce 400`, no skid). Under the original's tire that stood the
  bike on its side at the motion test's start line.
- **Hash vectors moved:**
  - `HUNDRED_TICKS` is now `0x2eb0_821a_075d_6c4c` in `lib.rs`, `tests/hash.rs` and `platforms.rs`;
  - `platforms.rs` was regenerated, with only the changed consts spliced;
  - the walk has 484 leaves (12 new: locked, sliding and the hinge, ×2 wheels);
  - `Ride` is 1832 bytes and `Sim` 6680.
- **The minute.** Since the held turns now scrub speed as the original's do, three scripts and checks
  moved. Each one's doc says why:
  - `RAMP` moved from 250 ft to 200 ft;
  - the jump's airtime is now > 50, and its landing speed is now relative to the takeoff speed;
  - the flip's forward stab moved from 295 to 326, in `the_flip` and in `ride-minute.pad`.
- **Two stand-in tests retired.**
  - `no_cornering_stiffness_can_make_the_bike_spin` is removed, and a note is left where it was. It
    tested the stand-in's impulse bound, and the original's tire has no such bound.
  - `a_tire_kills_a_slide_and_never_reverses_it` keeps its slide checks, but its 100× leg is gone. Its
    yaw check is now the net turn (< π/4; it measures 0.31 rad) plus a peak rate < 3.0. The peak is
    2.48 rad/s at 60 Hz and 1.98 at 30.
- **New test:** `a_full_lock_turn_keeps_its_speed` in `crates/game-sim/tests/ride.rs`. It takes the
  disc's race125p and the 125_faisst model's tire and hinge nodes. It holds the gas and the stick hard
  over for 1 s from 55 ft/s and from 76 ft/s, and asserts ≥ 80% is kept. It measures 1.038 and 0.852.
  On HEAD the same turns keep 0.884 and 0.709, so the 76 leg is the one that bites. Without
  `context/iso/MX.PSR` it prints why and returns.

The brief says `mv.rs`; the file is `move.rs`.

## What the original does

The wheel step runs the surface drag, then the spin integration. The spin integration (`./kit fn
0x0025ca60`'s caller) does four things in order:

1. It builds the contact frame (`./kit fn 0x0025c920`). For a wheel that is touching and steered, the
   forward is row 2 of the axis-angle matrix. That matrix is built from the steer angle (+0x8ac) and
   the hinge (+0x8c0), and turned into the world by the transform's rotation. Otherwise the forward is
   the chassis' z row. The frame builder (`./kit fn 0x0013e330`) flattens it onto the normal.
2. It calls the tire force with the patch velocity in that frame.
3. It pushes the force at `BodyTireForcePosition`.
4. It integrates the spin.

The tire force works per unit load:

- **Drive** is torque/r, capped at `MaxDriveForce·load`. Past the cap it becomes `±SkidDriveForce·load`
  and sets spinning or locked.
- **Side force** comes from one of three families, picked by brake/spinning/locked:
  - linear: past `Max…` it gives `Skid…·load` and sets sliding;
  - parametric: `slide·k·(−scale·load) / (1 + |slide·roundoff·k|^dropoff)`.
- **At standstill:** below 10 ft/s of roll the slide is magnified, up to ×5.

The game's bikes use the parametric curve, which falls away past its peak instead of pinning. That is
why a held turn keeps most of its speed.

The hinge is set when the model is attached (`./kit fn 0x00253760`): the Handlebars node minus the
Shocks node, normalised. The wheel mounts are the model's TireFront/TireRear nodes.
`BodyTireForcePosition` is only the force's arm. On race125p that arm is (0, −0.25, ±0.75), and using
it as the mounts flips the bike, which is what invalidated this task's first probe numbers.

## How it was proved

- **Function oracle**, `crates/game-sim/tests/tire.rs`, 4 tests, bit-exact:
  - tire force, 12,000 inputs;
  - surface drag, 12,000;
  - spin inverse inertia, 10,000;
  - steered forward, 12,000, against the game's own curve table built by `curve_table_build`. A
    mutation (reordering one term) fails it.
- **The turn**, measured on race125p with the real model mounts, 1 s at full lock with the gas held:

  | Start speed | HEAD | Tire ported, flat hinge | Tire + raked hinge |
  |---|---|---|---|
  | 55 ft/s | 0.884 | 0.971 | 1.06 (the test's own setup: 1.038) |
  | 76 ft/s | 0.709 | — | 0.852 |

- **`./kit check`:** passed, 593 tests. **`./kit audit`:** ok.

## Not verified / not 1:1

- **The frame builder's arithmetic.** `patch` uses the port's own `cross`/`normalize`, not
  `./kit fn 0x0013e330`'s operations bit for bit. Only the steered row is proved. The frame helper's
  write of the frame × velocity (+0x840) is the port's own.
- **`ride::hinge_axis`'s normalisation.** It is the port's `normalize`, not VU0's sequence. The
  direction matches the live race125p value to the printed digits, but it is not proved to the bit.
- **The partner contact copy (+0xc4)** is not ported. The tire runs when the partner touches, but the
  original's copy of the partner's contact data is not reproduced.
- **The hinge-force path** the original takes for a *balancing* steered wheel is not ported. Here that
  wheel's force goes into the chassis like the other's.
- **The gyroscopic term and the wheel's rolling angle** are not ported. Nothing ported reads them.
- **`chassis::rotate` stands in for `transform_rotate`** (`./kit fn 0x001b2ff0`), both in the drag
  and in the steered forward's turn into the world. They are believed equal but were not proved here.
- **The drag's forward speed** is the z of `unrotate(chassis_velocity)`, standing in for the
  original's +0x190 written by the contact-velocity helper. Not proved to be the same word.
- **`spin_inv_inertia` returns 0 for a radius ≤ 0.** The original leaves the field unwritten.
- **The turn is still not the original's.**
  - The port turns at about 2.5 rad/s at full lock near 55 ft/s, close to the kinematic rate. P15's
    original turns at 0.6–1.3 rad/s at 54–57 ft/s. What limits the original's rate (steer command
    scaling, the hinge's dynamics, or the parametric tire on the original's loads) is not found.
  - Coasting, the port keeps only ~0.5 of 55 ft/s over the same second. The original's coasting turn
    has never been measured, so that number has nothing to compare with.
- **The 55 ft/s leg passes on HEAD too** (0.884). Only the 76 ft/s leg distinguishes the fix.
- **The stand-in bike's new numbers are tuning, not data.** That is `SideForce` 0.1/0.12, the skid and
  slipping keys, and `WheelDrag` 0, chosen so the stand-in stands still and the minute rides. The same
  applies to the trick bike's copy of them.
- **The minute's rescripted numbers come from the port, not the original.** That is `RAMP` at 200 ft,
  the stab at 326 and the jump's landing floor.
- **The tests built on the stand-in bike** (`tests/replay.rs`'s acceptance fixture, `stock.rs`'s
  bike) leave the hinge at the chassis' y, because the stand-in has no model. Only the shell and the
  P24 test set a raked hinge.
- **Two stand-in properties were dropped, not re-proved:**
  - the 100× stiffness standstill test was removed (see above);
  - the shove test now allows a 2.48 rad/s peak yaw.

  Nothing measured shows what the original does with either input.

## Next

Find what holds the original's full-lock turn rate to 0.6–1.3 rad/s at 55 ft/s, where the port turns
at ~2.5 rad/s. Start by comparing `steer_angle` (+0x8ac) tick by tick against a PINE recording of a held
turn. Then measure a coasting turn on the original.
