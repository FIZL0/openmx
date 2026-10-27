# 1 — Foot Dust while riding (PART)

Task P152 (P74 follow-up). 2026-10-27.

## What I did

- `crates/openmx/src/main.rs`:
  - `bike_fx_frame` now feeds the bike's ParticleCollisionConstraint entries from the riding body
    (`rider.riding`, S109/S110's riding ragdoll) while the rider is up, and from the wreck's ragdoll
    while down, as before.
  - The mapping is now one function, `constraint_frames`: contacts in the kit's sphere order (the
    entries' mask bits), each bone's velocity, and the terrain's surface under a touching point.
  - `object` is now the bone collision's scene flag on its own. P74 had `scene && !ground`, but the
    manager sets the ground (2) and object (4) bits independently (`./kit fn 0x003d7a30`: +0x18 → 2,
    +0x1c → 4, neither → 1).
- Nothing in game-sim changed. The riding body already keeps the nine bone collisions' contacts (it
  clears them each tick and sets them in its step). The drawn rider is unchanged: the riding ragdoll
  still runs whether or not it is drawn, and `OPENMX_RIDING_RAGDOLL` still gates only the drawing.
- Took nothing from opentennis; it has no equivalent.

## What the original does

- **The entry** (`./kit fn 0x003d7a30`) walks the actor's bone-collision list (count +0x10, array
  +0x14; the oracle's actor +0x620/+0x624).
- For each collision whose bit is in `AttachToParticleCollisionConstraints`:
  - flags = NoContact 1 when neither ground (+0x18) nor scene (+0x1c), ground 2, scene 4;
  - it skips when Disable & flags != 0, or when the entry's zone byte for the collision's surface
    (byte +0x3c) is 0xff;
  - otherwise it sets the emitter's position to the point (+0x40) and its "from" to point −
    bone velocity × dt, then emits.
- Foot Dust is mask 0x3 (collisions 0 and 1, the feet). Its Disable is NoContact|ObjectContact|Wrecking,
  so it runs only while riding.
- The emitter's own minimum speed (P74's `emit_over`) means a still foot gets no particles. The original
  shows the same: from 1940 to 2005, at a standstill, the PCC rewrites `e4_at` every frame but the
  carry stays exactly 0.

## How it was proved

- **The feet against the original's feeds**: `crates/game-sim/tests/ragdoll_riding.rs`
  `the_riding_feet_raise_the_originals_foot_dust`.
  - It steps the port's riding actor from each of S109's run1 rows (1203 ticks; the same harness as
    `the_riding_tick_is_the_originals_from_each_row`, which already holds the contacts' flags
    bit-exact).
  - It reads each foot collision's surface byte (+0x3c) off the original's tick.
  - It compares against P74's capture (`context/capture/p74/run.csv`, the same animandpump playback).
    A row a single frame on whose `e4_at` moved is the PCC feeding Foot Dust.
  - Result: the port's feet raise dust on 93 ticks and the original fed it on 84 rows, the same
    stretches: the standstill 1940–2007, the landings 2204–2207 and 2304–2311, and the slow rolls
    3011–3016 and 3108–3116. Nothing disagrees within the label lag below.
  - The feet also touch on the road (2787–2827, 2889). That is surface 0, `none` for Dust: 41 touches
    zoned off, and the original fed nothing there either.
  - **The two captures' labels lag.** A port touch at tick f is the original's feed at f+1 or f+2,
    1 at 2204/3108 and 2 at 2304/3011. Each side is held to the other within 1–2 labels. The two
    captures were not caught at the same point of the frame (P74's rows also have two-frame and torn
    rows), so this is not a timing difference I could pin down.
  - P74's csv was copied from s2's context (`context/capture/p74/`) into this slot. The test skips
    where it is missing.
- **The entry's gating off the disc**: `crates/openmx/tests/bike_particles.rs`
  `the_riding_feet_raise_foot_dust`. A moving foot on dirt raises Foot Dust (left or right foot).
  None is raised on the road, when still, when also on the scene, by collision 2, or in a wreck.
- **In the shell**: the human's `animandpump.p2m2` replayed headless on free09
  (`--trace-at 4310,3317,90 --play animandpump.p2m2 --seconds 40`). The once-a-second log line shows
  "Foot Dust 12..33" on 7 seconds. Before this change Foot Dust could only be fed from the wreck
  ragdoll, and it is Disabled while wrecking, so it was never on.

## Not verified / not 1:1

- **The emitted particles are not compared with the original's, only the feeds.** The carry, count
  and placement of Foot Dust's particles were not compared row by row. The test checks when the feet
  feed the entry; the emitter itself is P74's port, compared on the bike's other entries.
- **The 1–2 tick lag between the two captures is tolerated, not explained.** I did not establish
  whether the manager reads this frame's or the last frame's contacts. The port reads the contacts
  the riding body's step of the same frame left.
- **The surface under a foot is the shell's own ground query**, as for the wreck (P74). The original
  reads the collision's surface byte (+0x3c) that its ground query stored. The test uses the
  original's byte, so the shell's own query was not compared.
- **Online there is no Foot Dust.** The riding body runs only where the shell hands it the grips
  (`ride_holds`, offline only, S110), so online seats have no riding contacts.
- **The object-flag change also affects the wreck's Rider Dust and Chunks.** A sphere touching both
  the ground and the scene is now an object contact, which those entries' Disable turns off. This
  follows from the decompilation; the wreck's emission is still uncompared (P153).
- **The port's own ride is not the original's.** The proof steps the port's body from the original's
  rows. In a free ride, the feet touch where the port's riding body puts them, and that depends on
  the port's ride and request matching (S109–S111).
- **No look by eye against the original's video.**

## Next

Nothing for P152.
