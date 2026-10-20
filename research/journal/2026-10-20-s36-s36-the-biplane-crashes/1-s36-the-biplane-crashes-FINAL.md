# 1 — s36 the biplane crashes (FINAL)

Task S36. 2026-10-20.

## What I did

- C7's capture was not on main, so I made my own. I played back the human's `biplane.p2m2` (a copy,
  using C6's AT-SPI recipe) with a per-tick watch spec on the plane's crash words: `crash.toml` and
  `play_crash.sh` in this folder. The output is 1719 rows, frame_counter 10971..12689, with no gaps.
  It lives at `context/capture/s36/biplane-crash.csv`, in a subfolder so the acceptance readers do not
  glob it. The human's recording kept its md5 (8a0fd1f5…).
- Ported the crash as a new module `crates/game-sim/src/biplane.rs`: `Wrecks` (the `[Wrecks]` reader),
  `Wreck` (reasons 1 and 13), `Plane::{wreck_check, wreck, unwreck, reset, reset_chord, frame,
  arm_respawn, prop}`, `map_extent`, `near_edge` and a `State` walk. I did not touch `ride.rs`. The plane
  is not in `ride.rs` yet, and its chord decodes to the same two commands as the bike's
  `Controls::respawn`, so `Plane::reset_chord` takes that bool.
- Tests in `crates/game-sim/tests/biplane.rs`: `the_biplane_crashes_and_respawns` (against the
  capture) and `the_biplane_crash_rules_the_capture_does_not_reach` (unit checks of the rules the
  capture never reaches).
- I added S36's findings to the end of `research/oracle.txt`.

## What the original does

The plane's vtable slot for the crash test is `./kit fn 353180`. It runs from the measure slot, after
the surfaces and **before** airspeed (+0x598) and up.y (+0x5e4) are recomputed, so it reads the
previous tick's values.

- **Reasons.** There are only two. 1 is contact: the body's own contact word ((+0x170)+0x40, not a
  tire) is set, and either the previous airspeed is above `MinWreckSpeed` × 1.46666 (50 mph = 73.333 ft/s)
  on a surface without the `VEHICLE` flag, or the previous up.y is below 0. 13 is the map edge: within
  150 ft of the terrain's edge (`./kit fn 1a6e50`), the plane is thrown toward the map's middle on an arc
  with a random tumble. Ground, walls and buildings all go through the contact rule. Water has no reason
  of its own and there is no separate wall test.
- **The wreck** (`./kit fn 3535d0`; the MX override `354aa0` adds the rumble) uses the plane's own
  `[Wrecks]` block, not `Wreckbike.phy`. The spin goes ×4. Both body dampings become `WreckDamping`
  (0.999). The inertia becomes `WreckInertia` × the saved inertia (×0.5). The gravity becomes
  32.2·(−1.5·m). The throttle and the engine power are zeroed and the engine sound stops. The plane
  stays the same rigid body and keeps flying under those numbers. Its controls keep running during the
  wreck: the capture's throttle climbs again from the tick after entry.
- **The broken prop**: while the wreck word is set, the model shows `propconebent` (texture
  `biplanepropbroken`) instead of `propconeblur`. That is presentation, so the port only exposes
  `Plane::prop()`.
- **Biplanewreckdust** is an effect in `biplaneparticles.dat`, keyed on the wreck word. It is
  presentation only and I did not port it (see below).
- **The unwreck** (`./kit fn 3536e0`): reason 1 ends when the clock passes `WreckTime` (5 s). Reason 13
  first waits until the plane has been wrecked for over 2.5 s and is on its wheels or its body is
  touching; it then sets the "down" latch, zeroes the clock, waits `WreckTime`, and keeps 0.3 of the
  velocity. The unwreck puts the dampings, inertia and gravity back (and doubles +0x194 back for reason
  13), restarts the engine sound, and calls the reset.
- **The reset** (`./kit fn 355b00`, the freeride path): the base reset (`351df0`) puts the body on the
  actor's transform. That transform is the spawn transform at +0xe10, which is what `machine.txt` wrote
  (free06: 2304, 135 → 138.614 after SetGroundHeight, facing 0,0,−1). The reset also zeroes the stored
  velocity, the throttle and the four surface words, and the respawn clock. The plane respawns **on the
  ground, at its spawn, facing the spawn's way, at rest**, and is grounded on the next tick.
- **The reset chord** (`./kit fn 355df0`): L1 pressed while R1 is held, or R1 while L1 is held, only on
  the wheels or wrecked, and once per frame (a latch that the frame slot `354b20` clears).
- **The respawn clock** (+0xe50, `./kit fn 355c50` through the racer interface) is armed at 10 s, or
  resets at once in mode 1. The freeride capture never arms it.

## How it was proved

A save-state capture: the biplane recording played back on the original, one row per tick, 1719 rows
(`context/capture/s36/biplane-crash.csv`, made by `play_crash.sh` + `crash.toml`). On each tick
`the_biplane_crashes_and_respawns` gives the port that tick's body contact and the previous row's
airspeed, up.y, grounded and throttle. It asserts that the port's wreck word, wreck clock and gravity
force are **bit-equal** to the capture on all 1718 stepped ticks. The entries come out at **11664 and
12498** (both reason 1, prev airspeed 101.38 and 137.23), with no other ticks. The second wreck is waited
out and unwrecks on **12649, 151 ticks** after entry. That tick's body position, velocity and spin are
bit-equal to the capture's (spawn, 0, 0), with the spawn's facing. The first wreck ended at **11747, 83
ticks** in, with a clock of 2.733: that is the reset chord, and the recording holds L1+R1 from field 1550.

The gravity bits also settled the plane's mass. The RAM holds 0x423a55dc = 1500 × 0.0310559 chopped.
`Weight / 32.2` through `div.s` gives 0x423a55dd and fails the flying gravity by 1 ulp. The instruction
sequence (neg.s, mul.s, mul.s with 0x4200cccd) was read from RAM.

`./kit check`: TESTS PASSED. `./kit audit`: ok.

## Not verified / not 1:1

- **The chord's tick is taken from the capture, not derived from the pad.** In the test, the chord
  fires on the tick where the capture's wreck word drops before the clock could have dropped it. I did
  not pin which p2m2 field becomes which tick (the hold starts at field 1550 and the reset lands on tick
  11747). It is edge-triggered and fires once (the next tick is not reset again), which matches the
  port, but the latency is unmeasured.
- **The chord respawn is integrated on its own tick.** The capture's 11747 row shows a body already one
  step past the spawn, consistent with the chord running in the input slot before the step. That row's
  velocity is not asserted: the port has no integrator.
- **Map edge (reason 13) never happens in the capture.** The throw's arithmetic, the rand order (x, y,
  z), the 2.5 s settle, the 0.3 velocity keep and the +0x194 halving are ported from the decompilation
  and checked only for internal consistency. As ported, the reset zeroes the velocity immediately after
  the ×0.3, so the keep has no effect. That holds only if the base reset zeroes the body's velocity the
  way the waited-out capture shows (it does for reason 1). The terrain extent inputs (+0x530..+0x53c)
  are not read from any track by the port: `map_extent` takes them as arguments.
- **The `VEHICLE` exception and the upside-down rule** are unexercised. Neither occurs in the capture
  (the only contact on a non-wrecked tick is 12649, the respawn tick itself). The flag value 0x2000 and
  where the surface comes from ((+0x170)+0x1fc, flags at +0x84) are from the decompilation.
- **Water** is assumed to go through the contact rule like everything else. No water crash was captured.
- **The plane's mass rule** (Weight × 0.0310559, chopped) is inferred from the RAM value and the gravity
  bits. I did not read the plane loader's own instruction.
- **The body dampings' roles.** The wreck writes `WreckDamping` through two body setters (vt +0x1c,
  +0x54), and the unwreck writes (1,1,1,1) to both. Which one is linear and which angular is not
  established, and nothing here uses them.
- **Inertia.** The test's inertia is made up. The RAM's is (2833.59, 5880.47, 3846.80), and the
  ×`WreckInertia` scaling is checked only on made-up values.
- **The spin ×4 on wreck** is ported but not asserted against the capture. The capture's spin is the
  post-step value and the port has no step.
- **Not ported (presentation or out of scope):** Biplanewreckdust (I did not extract or read
  `biplaneparticles.dat`); the rumble; engine sound stop/start; the camera snap on reset (`200a70`);
  the particle-zone bookkeeping (`354e00`); the previous-transform copies at +0x600..+0x630; the
  odometer (+0xe08).
- **The reset's racer branch** (+0xd64 set: keep the transform, take the velocity from +0xe40) is not
  ported. Freeride never reaches it.
- **The respawn clock** has a hook (`Plane::arm_respawn`) but nothing calls it. What arms it in a race
  is the racer interface's job and was never captured.
- **The chord's owner-pad check** (the reset only takes the plane's controller's pad) is the caller's
  job and is not modelled.

## What S34 has to wire in

- Grow `Plane` rather than replace it. `body` becomes the real rigid body: `at/forward/up/vel/spin`,
  `inertia`, the two dampings, mass = Weight × 0.0310559.
- Feed `wreck_check` from the measure slot, before airspeed and up.y are recomputed, with the body's
  contact word, the touched surface's `VEHICLE` flag, the terrain's extent (`map_extent` from terrain
  +0x530..+0x53c) and the game's `Rng`.
- Call `reset_chord(controls.respawn)` from the input slot, before the forces, so a chord respawn is
  integrated on the same tick. Call `frame(dt)` from the frame slot.
- The flight's throttle, elevator, elevator command, aileron and rudder are `Plane`'s fields, which
  the wreck and reset zero. The engine power (+0x400 engine's +0x70) is `engine_power`.
- Draw `prop()`'s model, and spawn Biplanewreckdust while `wreck` is set (V21's particles).
- A machine swap into the plane (oracle `machine_swap`) unwrecks it and zeroes the respawn clock:
  `unwreck()` then `respawn_clock = 0`.

## Next

Nothing for S36. The flight is S34's (above).
