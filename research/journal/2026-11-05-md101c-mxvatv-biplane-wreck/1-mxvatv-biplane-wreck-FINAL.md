# 1 — mxvatv biplane wreck (FINAL)

Task MD101c. 2026-11-05.

## What I did

- `biplane::Crash::step_mxvatv` zeroes the clock on every wreck entry: the edge throw and the crash test's
  own wrecks. This is MXVehicle's wreck entry. Before, a plane that had already crashed and was then thrown
  from the edge kept its running clock, so its 2.5 s wait counted from the crash, not the throw. On
  reason 0 the old code matched by accident, because the clock was already 0 there. Nothing else in the
  machine changed: `flight.rs` already calls `step_mxvatv` with `wreck_time` = MaxWreckTime (the plane's
  `[Wrecks]`), and that is what the original runs.
- `crates/game-sim/tests/mxvatv_biplane_wreck.rs` holds two tests (see "How it was proved").
- `research/md80a/biplane3crash.pad` is the crash flight's pad script. The capture:
  `KIT_PCSX2=3 research/md80a/fly.sh`-style, from biplane3's state, wrote
  `context/capture/md80a/biplane3crash.csv` (770 rows, no gaps), with its tape at
  `context/recordings/md80a/biplane3crash.p2m2` (+ `.ticks.csv`). The flight goes off the runway with
  throttle and rudder, climbs for 45 ticks, then holds the stick into a dive for 150 ticks until the plane
  strikes the desert. My first script started the dive higher (y ≈ 320); that only made an outside loop
  with no wreck. The emulator copy's ini has `NominalScalar = 0.3`, so the recorder keeps up.
- research/oracle.txt `# MD101c`: names for the crash test, the plane's wreck/unwreck, MXVehicle's
  wreck/unwreck and the car's crash test, plus the machine in words.
- Follow-ups MD111 (the crash mask against the port's VEHICLE category) and MD112 (the wreck events,
  rumble and the +0xbbc damping gate).
- Nothing was taken from opentennis.

## What the original does

Described in words; `./kit fn -p atv <addr>` reads each one (the addresses are in research/oracle.txt
`# MD101c`).

- **The crash test** (biplane vt+0x49c) runs once a frame, called by the plane's update after the tire
  step.
  - If the plane is not already thrown (reason ≠ 13) and MXVehicle's wreck check says so (the edge test,
    gated on +0x308), the plane wrecks with reason 13.
  - Flying (reason 0), it wrecks with reason 1 when either holds:
    - the collision record is touching, the struck object's class does not share a bit with the plane's
      mask (+0x30c), and !(speed ≤ MinWreckSpeed);
    - or the record is touching and up.y < 0.
  - Wrecked, the clock (+0xc84) += dt, then:
    - reason 13, latch unset, !(clock ≤ 2.5) and (grounded or touching): set the latch (+0xc8c), clock 0,
      post event (8, 12);
    - reason 13, latch set, !(clock ≤ MaxWreckTime): velocity.xyz × 0.3, then unwreck;
    - reason 1 and !(clock ≤ MaxWreckTime): unwreck.
  - A wreck entered this frame gets the same frame's clock step, so the clock reads dt after a wreck
    frame. The port does the same.
- **The wreck entry** (vt+0x368) is MXVehicle's base, then the plane's own part.
  - Base: reason, clock 0, latch 0, rumble if it is on, the body's damping/inertia calls unless +0xbbc is
    0 or 1, post (8, 11).
  - Then the plane's part: rates.xyz × 4, wreck gravity, +0x318 0, engine +100 0, and a sub-object's
    vt+0x94.
- **The unwreck** (vt+0x36c): gravity back from the scale, the sub-object's vt+0x90 and vt+0xe8, then
  MXVehicle's recovery. That recovery divides the scale by the throw's on a throw, sets reason 0 and
  clock 0, and keeps the latch.
- **MaxWreckTime − 2.0** appears in the car's crash test, not the plane's: a flipped car (wreck 14) starts
  its clock at MaxWreckTime − 2.0, and that test also has a 0.06 velocity scale and a speed < 10 check.
  Another function, unrelated to the plane's recovery, reads MaxWreckTime − clock.

## How it was proved

- **Function oracle, 10,000 generated cases** (`the_mxvatv_crash_test_matches_the_original`).
  - It runs the original crash test on biplane3's image (`context/mxvatv-ps2/md80a/biplane3.ee.bin`, plane
    0x01b13790) with the real wreck check, terrain edge test and wreck entry.
  - `rand` hands out the port's generator's numbers, rumble is off, and the event post and the unwreck are
    recorded.
  - The generator varies:
    - reason (0, 1, 13, others) and the clock, around 2.5 and around the max (on it and one step either
      side);
    - MaxWreckTime, MinWreckSpeed, speed, dt (0 included) and up.y (±0 included);
    - touching, grounded, the latch, the struck object, its class and the mask;
    - the point inside, on or past the 150 ft margin, plus velocity, rates and the gravity scale.
  - It compares, bit for bit:
    - reason, clock and latch;
    - velocity, rates (with the wreck's × 4) and gravity scale;
    - the rng's state;
    - recovery ⇔ the unwreck ran, a wreck ⇔ event 11, a new latch ⇔ event 12.
  - The cases hit 2595 wrecks (1923 thrown), 362 latches and 1350 recoveries.
  - The old code (no clock zero) fails on case 0: a crash, then a throw.
- **The crash capture** (`the_mxvatv_wreck_clock_matches_the_crash_capture`).
  - The interpreter flies biplane3crash from biplane3's image with the tape fed as
    tests/mxvatv_biplane_replay.rs does.
  - All 770 rows agree word for word on the body, engine, aero, the three tires and the wreck words
    (+0xc80..+0xc8c), with no late rows.
  - The crash test is hooked: the original runs nested, and a port `Crash` seeded once from the first frame
    runs beside it on the plane's words. The port's reason, clock and latch equal the original's on every
    frame.
  - The port wrecks once (reason 1, frame 298 = row 297) and recovers once, at frame 449: 151 frames, the
    first clock past 5.0.

## Not verified / not 1:1

- **The thrown plane's path (reason 13: the latch, the 0.3 velocity, the throw's recovery) is proved only by
  the oracle.** The crash capture never reaches the edge.
- **The crash mask.** The original spares a struck object whose class word shares a bit with the plane's
  mask (0x4 in biplane3). The port's `Flight::post` reads the struck category & `VEHICLE` (0x2000). The
  oracle feeds `ignored` as the original computes it, so the port's mapping from its categories is
  unproved. The capture strikes only the ground (no struck object). MD111.
- **Not ported:** the events (8, 11) and (8, 12), the rumble, and the +0xbbc gate on the wreck entry's
  body damping/inertia (the port always damps; biplane3 has +0xbbc = 2, which damps). MD112.
- **Not compared by the new tests:**
  - the wreck entry's damping, inertia, gravity, throttle and engine words in the port's `Flight::wreck`;
  - the unwreck's put-back in `Flight::unwreck`.

  In the oracle, the original's unwreck is replaced by a recorder. The capture's frames after the recovery
  match because the interpreter runs the original, not the port's flight.
- **The latch on recovery.** The original keeps it; the port's `Flight::unwreck` clears it. Nothing can see
  this, because every wreck entry clears the latch and only reason 13 reads it. The capture test compares
  the latch only while the plane is thrown.
- **The contact words.** Hull contact on the terrain (touching) is the original's here. The port's flight
  does not make it yet (MD110), so a whole-port crash ride would not wreck on the same frame.
- NaNs and denormals are left out of the generator. The PS2 FPU makes neither, and the port's `F` flushes
  them, so a word holding one reads differently in the two.

## Next

Done. MD111 and MD112 are the follow-ups.
