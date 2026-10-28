# 1 — wreck particle contacts (FINAL)

Task P153. 2026-10-27.

## What I did

- Wrote a capture spec for the wreck. `capture/make_spec.py` writes `capture/p153.toml`, with 115 fields:
  - frame_counter, dt, rider_type, wreck_reason, the wreck word s868, the bike matrix;
  - per manager entry 0..4: the flag byte, the switch byte, the carry, `at`/`from`, the zone byte and
    the view-cull scale;
  - the crash body's ground word, object word, velocity, contact count and 12 contacts (point, zone);
  - the ragdoll's sphere count and 9 spheres (ground, object, zone, point).
- `capture/play.sh` is P74's play.sh with that spec. I played S109's `animandpump.p2m2` from 3700 for
  230 rows: `SAVE=3790:1 ./kit pcsx2 with -- bash …/capture/play.sh PID context/recordings/animandpump.p2m2
  3700 230 context/capture/p153/wreck.csv`. The capture covers the whole wreck (s868 set ~3726..3917,
  respawn on 3918) with 0 torn rows. The csv and a state at 3790 (`wreck-3790.p2s`) are in s1's
  `context/capture/p153/` only.
- particles.rs:
  - `Emt` has three new fields: `distance_ratio` (ParticlesToDistanceRatio), `speed_ratio`
    (ParticlesToSpeedRatio) and `min_movement_speed` (MinimumMovementSpeed);
  - `Source::emit_over` now takes the original's rate. If the emitter moved faster than
    MinimumMovementSpeed and either ratio is set, the share is `PtD·distance + PtS·speed`, clamped to
    `rate·dt`. Otherwise the share is random in `rate·dt`. Particles go out only when the carry is
    **greater than** 1, and at least MinParticlesPerFrame go out;
  - CollisionModel (Vehicle Dust/Chunks) now uses **one** source. It is switched by `Disable & flags`
    and run once per contact, ground contacts first and then object contacts. Each contact sets
    `from = point − vel·dt`, `at = point`. Before this, the port kept one source per contact, each
    with its own carry;
  - CollisionModel and Constraint (Rider Dust/Chunks) follow the switch-on rule: the first point after
    switching on only takes the position.
- New test file `crates/openmx/tests/wreck_particles.rs`:
  - `the_wreck_entries_read_as_the_original` checks the five entries' kinds, rates, ratios, minimum
    speeds and zone tables against the values in the original's RAM;
  - `the_wreck_emits_as_the_original` feeds the capture row by row and compares the switch, carry and
    emission of each entry.

- Updated `tests/biplane_particles.rs` (P58/P61/P63). The biplane's Vehicle Dust/Chunks are the
  same CollisionModel entries, and five of its synthetic tests held the old rules:
  - "one emitter per body contact" (P58's journal) is wrong: it is one emitter run once per contact;
  - "the first puff is on the contact's tick" (P63) was never compared to the original frame by frame.
    The tick the entry switches on, its first run only takes the point, so the puff comes a tick later;
  - a 30 a second rate over a 30th carries exactly 1.0, and the original only lets go of a carry
    **over** 1.0, so the counts are 0, 0, 2 rather than 1, 1, 1. Two contacts over 30 ticks give 58,
    not 60.
  Each changed assertion now says why (P153).

## What the original does

- The manager's five wreck/rider entries come from `mxparticles.dat`:
  - 0 Vehicle Dust and 1 Vehicle Chunks (CollisionModel);
  - 2 Rider Dust, 3 Rider Chunks and 4 Foot Dust (ParticleCollisionConstraint, masks 0x1fc/0x1fc/0x3).
  - Disable is 0x15 for entries 0–3 and 0xd for entry 4.
- CollisionModel update, `./kit fn 0x003d75d0`:
  - The body is `*(entry+0x28)`. It holds the ground word at +0x48, the object word at +0x4c, the contact
    count at +0x100, the contact list at +0x108 (stride 0x80: point +0x40, zone int +0x70) and the
    velocity at +0x1e0.
  - The flags are NoContact/Ground/Object plus Wrecking. `Disable & flags` switches the emitter off
    (vtable +0x9c); otherwise it is switched on (+0x98).
  - For each contact whose zone in the table at `entry+0x18` is not 0xff, it sets emitter +0x60 =
    point − vel·dt and +0x40 = point, then calls emitter +0x8c(dt). It is one emitter and one carry
    for all contacts.
- PCC update, `./kit fn 0x003d7a30`:
  - It switches on or off by the wreck flags.
  - It walks the spheres at `R = *(*(entry)+0x30)`: count at R+0x10, list at R+0x14. Each sphere holds
    ground +0x18, object +0x1c, zone byte +0x3c (table at entry+0x1c) and point +0x40.
  - A sphere is used when it is in the mask, passes Disable, and its zone is not 0xff.
- The emitter's switch byte is at +0x240. Bit 4 means on, bit 2 means "just switched on" and bit 8 means
  ran.
  - Switch-on is `./kit dis 0x14e490`. It sets 4|2 and zeroes +0x1d8.
  - Switch-off is `./kit dis 0x14e4f0`. It clears 4 and 8 and sets prev = cur, but **leaves bit 2 set**:
    an emitter that is off reads e.g. 0xe2.
  - With bit 2 set, the first +0x8c call sets prev = cur and returns without emitting (`./kit dis 0x14daa0`).
- Emission, `./kit dis 0x150540` with the rate at `./kit dis 0x151d80`:
  - `speed = |at − from|/dt`. The emitter emits only when MinimumEmissionSpeed (+0x3c4) ≤ speed.
  - If MinimumMovementSpeed (+0x3c8) < speed and PtD (+0x3bc) or PtS (+0x3c0) is above 0, then
    `share = clamp(PtD·dist + PtS·speed, min·dt, max·dt)`. Otherwise the share is random in min·dt..max·dt.
  - The share is multiplied by the view-cull scale +0x254 and added to the carry (+0x3b0). Only when
    the carry is > 1.0 does it emit `trunc` particles (and subtract them), and never fewer than
    MinParticlesPerFrame.
- +0x254 is the view cull (`./kit dis 0x14d8c0`): a fade by camera distance against
  FarCullDistance/DirectionalCullDistance. It stays 0 until the emitter has been drawn once after its
  first switch-on.
- The values in RAM (S109's start state):
  - Vehicle and Rider Dust: rate 10..20, PtD 1, PtS 10, MinimumEmissionSpeed 3, MinimumMovementSpeed 0;
  - Vehicle Chunks: rate 100..150, MinimumEmissionSpeed 15, no ratios;
  - Rider Chunks: rate 10..20, MinimumEmissionSpeed 15;
  - Foot Dust: rate 100..150, MinimumEmissionSpeed 30;
  - zone tables: index 0 is 0xff and 1..15 are the group (4 for dust, 2 for chunks).
- When the rows are read, relative to the manager:
  - the crash body is the frame's own row (b);
  - the ragdoll spheres are the row before (a), because the ragdoll steps after the manager;
  - the wreck flag is row a's s868.
- On the respawn frame (3918) the manager still runs on the wreck: Rider Dust's carry moves exactly as
  the port's does. Then the respawn switches entries 0–3 off before the row is read.

## How it was proved

- I recorded a PINE capture on the original: `context/capture/p153/wreck.csv`, 230 frames
  (3700..3929), every frame present.
- `tests/wreck_particles.rs` compares 228 of its rows, step by step. After each row it puts the port's
  switch, carry and last point back to the capture's values, so each row is checked on its own:
  - The switch byte (on and "just on") matches on every compared row, for all five entries.
  - Vehicle Dust and Rider Dust: the port's carry equals the original's within 2e-5 on every row. That
    is exact to the capture's float print, and covers the 43 and 125 rows on which they emitted.
  - Vehicle and Rider Chunks have a random rate. On every row the original's carry move has to fit
    `k·[min·dt, max·dt]` plus whole particles, where k is the port's call count. This held on 27 and 6
    rows.
  - Foot Dust: off for the whole wreck in both.
- The bit-2 return, read in `./kit dis 0x14daa0` (0x14db74..0x14dbc4): with bit 2 set, the run
  clears it, copies +0x40 to +0x60 and +0x50, and returns 1 before any emission.
- `biplane_particles.rs` passes with the corrected counts.
- `bike_particles.rs` (P74) still passes with the new rate and the single CollisionModel source.
  `./kit check --all` is green.

## Not verified / not 1:1

- **The view-cull scale (+0x254) is not ported.** It depends on the camera (distance fade), so it is
  presentation and not simulation. The test does not compare rows where the original's scale is not 1.
  In this capture there are none with a carry move; the switch-on rows with the stale 0 scale emit
  nothing in both.
- **The chunks' random share** is only checked as consistent with the port's call count (the RNGs
  differ). The draw for each call is not compared.
- **The zone-off path was not exercised.** Every contact and sphere in the capture is in zone 13, so a
  0xff zone skipping a contact is only seen in the decompile.
- **One ragdoll row (3850) is not compared.** There, Rider Dust ran no sphere, although row a has four
  touching. Its spheres are not the ones the manager read, a capture-phase effect. The test resyncs
  after it.
- **The respawn row (3918): only its switches are skipped.** The original switches the wreck's entries
  off after the manager has run, so the row shows them off. The port's `wrecked` is a per-frame input,
  and it switches them off one frame later. That is one frame of difference in when the dust stops,
  and no particle is lost.
- **InitialSizeToSpeedRatio (0.1) and LifeToSpeedRatio (5)** on the dust emitters are not ported. They
  change a particle's size and life by speed, not how many particles go out. Filed as a follow-up (see Next).
- **main.rs's feed for a live wreck was not compared in a ride.** The test builds `PlaneFrame` from the
  capture. In the shell, `bike_fx_frame` differs from the original in three ways:
  - it takes the velocity from `crash.bike.body.velocity` (the original uses body +0x1e0; the same
    quantity by name, not checked against each other);
  - the sphere zones come from the P61 ground query and not from the sphere's own +0x3c byte;
  - a contact surface of −1 is clamped to 0, so a contact with no surface reads zone 0 (off).
  Filed as a follow-up (see Next).
- **The capture lives in s1 only** (`context/capture/p153/`). In other slots the test skips and still
  reads as passing.
- This playback wrecks at ~3726. P74's run of the same tape wrecked at 3719 (the tapes are not
  frame-stable across runs). Each capture is self-consistent, but they are not the same wreck.

## Next

Nothing for P153. Follow-ups: P185 (size and life by speed), P186 (the shell's live wreck feed).
