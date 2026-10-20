# 1 — the biplanes wheels (FINAL)

Task S39. 2026-10-20.

## What I did

- Ported the three tire listeners into `crates/game-sim/src/biplane.rs` under
  `#![deny(clippy::float_arithmetic)]`:
  - `Tire::clear` is the preforce listener (`./kit fn 34ef80`).
  - `Tire::probe` is the first force listener (`./kit fn 34ef90`). It asks the port's `World` for the
    ground.
  - `Tire::force` is the second force listener (`./kit fn 34f150`): `Tire::strut` (`./kit fn 34f190`),
    then `Tire::roll` (`./kit fn 34f7e0` with its frame helper `./kit fn 34f9d0`).
  - The tires push on the body through a new `Body` trait. It has the body's velocity, force,
    force-at-a-point and local-moment entries, plus the three fields the wheels read back. What the
    plane hands them each substep is a `Mount`: its drawn frame, its rates, airspeed, brake and steer.
- Ported the plane's collision force (`./kit fn 15af40`) as `collision_force`. It applies gravity
  through the body's push entry (the one that reaches `./kit fn 15afc0`), then calls the collision
  object's contact resolution, which is handed in as a closure, and reports whether it ran (the
  caller then clears its pending word).
- Ported the crash test (`./kit fn 353180`) as `Crash::step`:
  - `Area` is the area query (`./kit fn 1a6e50`).
  - `Contact` is what the test reads off the collision object and the measure.
  - The return value is the wreck/recover it asks for.
  - It sets the reason and the landed flag itself, the way the wreck does, and leaves the rest of the
    wreck to S36.
- Proofs: five new oracle tests in `crates/game-sim/tests/biplane.rs`, 10,000 cases each.
- Named everything in `research/oracle.txt` (block "S39").
- Rewrote S39's own brief in `plan/TODO.md` with those names instead of raw addresses, because once
  the addresses were named the audit flagged the brief.
- **Split**: S42 is the collision object's contact resolution (`./kit fn 270dc0` and its callees). It
  is a large routine of its own, and it is where the crash test's contact and surface come from.

## What the original does

**Preforce.** This listener clears "ground at the tire" (+0x80) and the held-contact flag (+0x84).

**Probe.** The probe works from the plane's drawn frame (plane+0x600, written once a frame by the
measure).
- It places the hub, `BodyTireCenter`, into the world.
- It takes the last ground normal (+0x140) into the plane's frame, drops its sideways (`x`) part and
  normalises it. One radius along it (negated) gives the hub-to-bottom offset +0x50. That makes the tire
  a disc, not a ball.
- The bottom of the tire (+0xa0 local, +0x100 world) goes to `ground_query` (`./kit fn 1dc960`), which
  moves a copy (+0xc0) onto the ground and writes the normal (+0xd0). The normal is kept as the next
  probe's lean.
- "Ground at the tire" is set unless the ground is below the bottom (`!(ground.y < bottom.y)`, so a NaN
  sets it).
- Nothing runs if the held flag is set or the tire has no world.

**Strut.** Only when the probe found ground, or the held flag is set:
- It recomputes the hub and bottom, then intersects the hub-to-bottom line with the ground plane. The
  compression is the distance from the bottom to the crossing (+0xe0).
- The compression is multiplied by a fade from the slope: the ground normal is rotated into the body's
  frame by its quaternion, its `z` is dropped, it is normalised, and its `y` is remapped from 0.5..0.64
  onto 0.5..1. The 0.5 floor is a RAM global, only ever read.
- Past `StrutCompression`, the strut is meant to push the body out by 0.9 of the excess. But the
  original clamps the compression first and takes the excess afterwards, so the position moves by
  `n·0`.
- In that same case it does cancel the body's velocity into the ground, through the velocity entry
  (`./kit fn 1f7ec0`). It also cancels the world force gathered so far, through the push entry.
- The contact velocity is `v + ω × r`, with ω the plane's rates (plane+0x5b0).
- The damping is the opening one (+0x4c) when the strut is not closing (`dv <= 0`), otherwise the
  closing one (+0x48).
- The spring force is `max(0, k·c − d·dv)`, applied along the normal at the contact through the
  force-at-a-point entry (`./kit fn 1f7a70`), with mask (1,1,1,1).
- If nothing touched, the spring force and contact velocity are zeroed.
- In every case, decompression is limited to `MaxDecompressionRate·dt`, and the last compression is
  `min(c, max)`.

**Friction.** When touching:
- The tire frame is `mat4_basis(origin, plane forward row, normal, orthogonalise, keep up)`. The contact
  velocity is taken into it (the slip).
- `s = clamp(0.1·airspeed, −1, 1)`.
- The friction is `(SideForce·slip.x, 0, BrakeFriction·s·max(brake, MaxBrakingForce))`. The brake is
  read only by the two front wheels (their +0x30 points at plane+0x350).
- The tail wheel (its +0x34 points at plane+0x364) adds a body moment `(0, SteeringMoment·steer·s, 0)`.

When not touching, slip and friction are zeroed. Either way, the friction goes through the tire frame
and is applied at the contact. Off the ground that means last substep's frame, last substep's contact
and a zero force. Then spin = slip.z / radius, and the angle advances and is wrapped to ±π. The wrap is
the same one the engine's prop uses.

**Collision force** (argument: plane+0x50). If the body's first word is nonzero, it applies gravity
(plane+0x140) through the body's push entry, which tail-calls the add-force entry (body+0x90 += f, then
its listeners). Then, if there is a collision object (plane+0x170), it calls the object's vtable +0x80
with 1, which is the contact resolution. Then it clears the component's +0x1b0.

**Crash test.**
- Unless the reason is already 13, the area query runs: the terrain extent is
  `(256·a)·c × (256·b)·d` from the terrain's four floats at +0x530. If it reports the body within 150
  of an edge (or NaN):
  - The plane is aimed at the centre, at its own height. The speed is `min(2073.6, W/2) / k` with
    `k = 2·sqrt(14.907)`, plus `16.1·k + 240/k` upward.
  - The gravity scale +0x194 is halved.
  - The body rates are set to 0.5, 0.4 and 0.7 times `rand()·2⁻³¹`, with w = 1.
  - It calls wreck(13).
- If the reason, read again, is 0 and the collision object has a contact: wreck(1) when the contact's
  surface is not masked out (surface +0x84 & plane+0x580) and the speed is above `MinWreckSpeed`.
  Otherwise wreck(1) when the up row's `y` is negative.
- If wrecked, the clock runs.
  - At reason 13 the plane counts as landed once more than 2.5 s have passed and it is grounded or in
    contact; that also resets the clock. Once landed, when the clock passes `WreckTime` the velocity
    is scaled by 0.3 and it recovers.
  - At reason 1 it recovers when the clock passes `WreckTime`.
- The wreck sets the reason and clears "landed"; it does not touch the clock.

## How it was proved

Function oracle on `context/states/biplane.p2m2_SaveState` (the live plane at 0x00c44a40, tires at
+0x408/+0x40c/+0x410), 10,000 generated cases per test. Every case writes its inputs into the
original's own objects, runs the original, and compares every field it writes, bit for bit:

- `the_tire_probe_matches_the_original`: the probe (with the preforce first in a quarter of the
  cases). The frame is generated from random rotations, and the tuning around the shipped values.
  `ground_query` is hooked on both sides to the same sloped plane through the case's bottom of the
  tire. Every tire field is compared. 7,057 cases found ground.
- `the_tire_forces_match_the_original`: the strut and friction listener. Every tire field is
  compared, plus every push the body is given (velocity, force, force-at-a-point, moment), in order,
  and the body's position, velocity and force afterwards. 2,136 cases touched, 1,339 bottomed out and
  703 steered.
- `the_collision_force_matches_the_original`: enabled and disabled bodies, with and without a
  collision object. Compared: the push, the force accumulator, the call to the contact resolution and
  the pending word.
- `the_crash_test_matches_the_original`: positions near the edges and at the exact margins, all three
  reasons and an unhandled one, every contact combination, and the RNG state written into
  `rand_state`. Compared: the wreck and recover calls (hooked plane vtable +0x1c0/+0x1c4), the
  reason, landed, clock, gravity scale, body velocity, body rates and the generator's state after.
  4,784 cases threw, 886 crashed and 1,077 recovered.

Mutation check: moving `FADE_TO`, `ROLL_SCALE` or `THROW_UP` by an ulp, or disabling the probe's
ground flag, fails the probe, forces and crash tests. Restored afterwards.
`./kit check -- -p game-sim --test biplane` gives TESTS PASSED (8 tests).

## Not verified / not 1:1

- **Water contact is not ported, because I found none.** The brief asks for it. `ground_query`
  (`./kit fn 1dc960`) is the terrain query (bilinear cell, nearest-corner surface into the record) and
  has no water branch. The tire code reads nothing water-related. If the plane touches water, it is
  through something I did not read. The likeliest place is the collision object's resolution (S42).
- **The held contact (tire +0x84) has no setter that I could find.** It is only cleared (the preforce
  and the init), and no store to +0x84 in the tire code reaches it. The port keeps it as `Tire::held`,
  and the proofs exercise it set (1 case in 12). Whether the game ever sets it is unknown.
- **The collision object's contact resolution (`./kit fn 270dc0`) is not ported.** It is now S42.
  `collision_force` takes it as a closure, and the proof hooks it. So the crash test's `Contact`
  (contact pointer, surface flags) is an input here, not something the port computes.
- **The ground is a stand-in in the proofs.** Both sides use the same sloped plane; the original's
  `ground_query` never runs. The port goes through `World::ground`, so it is exactly as good as the
  port's world. `World::surface` is asked for the surface byte, but the original's record (+0xb0) is
  not compared: the hook does not fill it, and the port's `Tire::surface` is unproved.
- **The body's entries are hooked, not run.** The velocity and push entries are stood in by adding into
  the fields the way the original's do. Their listeners and the force-at-a-point entry's
  moment/accumulation are S38's.
- **The wreck and recover are hooked.** `Crash::step` sets only the reason and "landed", which is what
  the wreck does to the fields this test reads. The body inertia/damping, rumble and recover are S36's.
- **The "no world" path of the probe is not ported.** The port's probe takes a `World` always. The
  original returns early when the tire's world pointer is null; the save state's tires all have one.
- **`MinWreckSpeed`, `WreckTime` and the tire tuning come from the save state's RAM, not from
  `biplane.phy`.** The `.phy` key names follow R8/S33. As in S34, the key-to-field mapping is not
  checked against a parsed file.
- **The collision component's +0x1b0 (plane+0x200) is cleared but not understood.** The port reports
  "ran" and leaves the word to the caller.
- **The generated ranges are bounded.** Rotations, positions, speeds and tuning are plausible, not the
  whole of f32. NaN and inf inputs are not exercised.
- **`./kit audit` still fails on plan/TODO.md:81** (S38's brief names biplane_measure by address).
  That is S38's line and was failing before this task; I left it alone.

## Next

S42 (the contact resolution), S38 (the body) and then S40 (the wiring). S40 should call, per substep
and in this order: `collision_force`, then the S34 four, then each tire's `probe`, then each tire's
`force`. That is the plane's force listener order: `./kit dis 352850 352960`. `Tire::clear` goes in the
preforce. `Crash::step` runs after the measure.
