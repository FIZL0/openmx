# 1 — drive on loop (FINAL)

Task P145. 2026-10-27.

## What I did

- `game_sim::ride::Ride::rear_material`: each substep, the struck prop's material (the low byte of its
  mesh's surface word) for the rear wheel's last contact, or 0. `wheel_hits` now carries each hit's
  material, and `contacts` picks the rear wheel's by the same `select` rule the distributor uses.
  Nothing in the sim reads it, so the state hash and the netplay snapshot leave it out. The snapshot
  skips it through a new `skip { … }` arm of `wire!`, so the riders' resync bytes pinned in
  `platforms.rs` stay as they were.
- `crates/openmx/src/audio.rs`: `DriveOnTable` / `DriveOnMaterial` (read from the track archive's
  `col\template.cmt` in `Sound::read`), the pure rules `drive_on_mph`, `drive_on_slope`,
  `drive_on_pitch`, `drive_on_level`, `own_pitch`, and `DriveOn`, the one-wheel emitter. It is a
  `SeatSound` field, ticked after the engine; it stops in the plane, when the seat leaves and when the
  ride goes quiet, and the seat's pause holds it with the rest.
- `main.rs seat_sound`: hands over `DriveOnFrame { material, at: rear contact point, vel: chassis vel }`.
- Tests: `crates/game-sim/tests/drive_on.rs` (a plank prop of surface word 0x10002 gives 2, the ground
  and a version-1 mesh give 0). `crates/openmx/tests/drive_on.rs` checks the speed rules against
  hand-worked values, the emitter's start / update / stop paths, and the disc's free09 table.

## What the original does

The disassembly is in `./kit dis 0x1b8d30 0x1b9a90`. The names are in research/oracle.txt under
"P145".

- **The bike's emitter** (`./kit fn 0x280fb0`, the bike builder) is made at bike+0x14a0 by the generic
  maker 0x104250 (MinDistance 20, MaxDistance 100), with **one wheel**.
  - 0x1b9a20 wires it: position = rear wheel +0x140, contact object = bike +0x7d4, touched = rear
    wheel +0xd4.
  - The terrain-char and terrain-gate slots (+0x208 / +0x210 of the per-wheel arrays) are never set,
    and calloc zeroes them.
- **What the terrain gives.** In the frame 0x1b9690, the material is `obj+0x89` when the touched word
  and the contact object are both set. Otherwise it is the terrain path, and that reads the NULL
  slot → 0 → stop.
  - So **the bike is silent on the ground**.
  - Even if the slot were wired, the terrain ids would be TerrainMask (200) + surface, and no
    template.cmt has an id above 22.
- **bike +0x7d4** is cleared at the top of `bike_wheel_contacts` (each substep). It is then set to the
  struck object of the last contact whose segment start z ≤ 0, which is the rear wheel.
- **The material byte +0x89** is set by `collision_file_load` from the .col surface word's low byte
  when the file version is ≥ 2; version-1 files leave it 0. Trees are kind-1 cylinders and give 0.
- **The level share** (0x1b98b0) is 100 / wheels when the copied velocity's length is not 0, else 0.
  Share 0 or material 0 → stop (0x1b9570: stops every material's sound, k = -1, cur = 0).
- **Play** (0x1b8da0):
  - If material == cur, or cur's sound is not playing, then a non-zero cur goes to **update**
    (0x1b9060).
    - So a stale cur whose loop has gone quiet keeps a new material from starting until the wheel
      leaves props or the bike stops.
  - Otherwise stop_old (0x1b94a0).
  - Then it looks the material up. If that material's sound is idle: cur = material, set position,
    work out (pitch, vol), set volume = vol·share/100 and the pitch, then sound_3d. It starts only
    when that is non-zero, which it is not beyond MaxDistance.
  - **Update** works the values out again and sets them, then sound_3d. A zero from it stops the
    sound but keeps cur. It never restarts.
- **The speed rule** (0x1b91f0), checked instruction by instruction:
  - mph = (int)(0.68182 · sqrt(x²+y²+z²)). The length is a mula/madda/madd chain.
  - k = (int)(0.5 + FreqSlope · (own / (float)SampleSpeed)), cached until a stop. own is the sound's
    pitch (vt +0x48).
  - pitch = (int)(own + (float)((mph − SS)·k − (int)((k+1) · 2⁻³¹·rand()))).
  - The level is two lines with integer quotients, clamped 0..100.
  - The pitch setter (0x166a20) ignores a pitch ≥ 0x4000 unsigned.
- **template.cmt** is identical in all 113 track archives:
  - 22 materials, every one `Impact\drv_drt1.vag` (22,050 Hz mono, ~2 s).
  - Speeds 50 / 0 / 1 / 99 / 50, slope 0.75.
  - Distances 30/70 only for 1 Solid Wood, 2 Hollow Wood, 4 Solid Metal, 5 Hollow Metal, 9 Stone and
    22 Tree Hit. Every other material is 0/0, so it is never heard.
- **free09's props:**
  - The ramps are surface 2 (Hollow Wood): riding a ramp plays the loop.
  - The tractor is 6, which is silent.
  - The power line and radio tower are 5, and the green boxes and tracking facility are 4.

## How it was proved

- The rules: by reading the disassembly (the unknown words decoded with `./kit dis`), then hand-worked
  values in `crates/openmx/tests/drive_on.rs` (`the_speed_rule_is_the_originals`). Those values are
  not from a function oracle.
- The emitter's branches: `the_loop_starts_moves_and_stops_as_the_emitter_does` drives each path once.
- The sim side: `crates/game-sim/tests/drive_on.rs`, a bike thrown along a plank with no disc.
- The disc's table: `the_discs_table_is_free09s` read all 22 materials. I made it fail once on
  purpose to confirm that it really ran and did not skip.

## Not verified / not 1:1

- **No live capture and no listening test.** The rules were read from the disassembly and tested
  against hand values, not run against the original (no ee oracle run of 0x1b91f0, no PINE recording
  of a ramp ride). Nobody has heard the port's loop beside the original's.
- **The pitch wobble's draw** comes from the loop's own `Dice`, not the shared libc `rand()` the
  original calls. This is on purpose: sound must not touch the sim's RNG.
- **`own`** is taken as rate·4096/48000 (1881 for drv_drt1), the way the engine port takes it. It is
  not read from the bank slot's pitch field.
- **The position** is the rear wheel's contact point, not rear wheel +0x140 (I did not identify which
  point that is).
- **An unknown material id** (not in the table) gives no sound, and cur stays 0. The original
  dereferences whatever lookup returns.
- **During a wreck**, whether the emitter keeps running is not checked. The port ticks it whenever the
  seat is on the bike and not in the plane.
- **Distance gating** uses the mixer's nearest listener against MaxDistance (`Mixer::within`). The
  attenuation is the mixer's linear near/far, and the original's sound_3d curve callback (+0x1ec) was
  not read. Its Doppler is left out.
- **A seat's pause** holds the voice where it is. Whether the original's pause holds it the same way
  was not checked.
- **Hierarchy props** (free09's shack) are not in the port's world, so they give no material.
- **World-of-tiles maps** use the home track archive's table (every archive's is the same file, so
  this costs nothing on the disc).
- **AI bikes**: I did not check whether they make this emitter. The port only sounds local seats
  anyway.
- **The state hash and the snapshot** leave `rear_material` out (it is sound only and recomputed each
  substep). A peer that has just restored a snapshot reads 0 until its first substep after the
  restore.

## Next

Land it. A follow-up could record a free09 ramp ride in PCSX2 with Video Capture
(recording-the-originals-audio) and compare the loop's pitch and level by ear.
