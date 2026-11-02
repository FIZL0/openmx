# V114 (2) — the flown Kiowa's sound and particles, hooked and matched

2026-11-02. Reopened after P216 (the Kiowa is a machine in the shell: parked, switched into, flown).
Part 1 (1-kiowa-sound-PART.md) built the readers. This part hooks them up and checks them against PCSX2.

## What changed

- `audio.rs`: `HeliFrame::of(kiowa, ride, touching)` carries the filtered collective, the body's position
  and velocity, and a `PlaneCrash` (wreck word, skids touching, shock-peak impulse). `SeatFrame.heli`
  carries it. `Sound` loads `[Helicopter]` (otherengine.aud) and `[helicopterWreck]` (machine.aud).
  `Sound::frame` runs the Kiowa's loop on the collective while not wrecked, silences it while wrecked,
  and runs the wreck clank from the crash. While the Kiowa is flown it silences the bike engine, the edge
  rumble, the drive-on and the plane. On leaving the Kiowa it silences the loop.
- Wreck stop and unwreck start come from the ELF. The helicopter's wreck slot calls the engine-loop
  object's "stop" slot (it clears the playing flag). The unwreck slot calls its "start" slot (it sets
  the flag). This is the same pair as the plane's (P187). Addresses are in this entry's notes only:
  wreck 0x0038eeb0 calls loop vtable 0x00450480 +0x8c (0x1b6bf0); unwreck 0x0038efc0 calls +0x88
  (0x1b6b80); the loop object is at heli+0x544.
- `main.rs`: `load_heli_fx` loads `particles::KIOWA_PARTICLES` with the track's fx sheet and binds the
  Kiowa model's nodes. `heli_fx_frame` fills a `PlaneFrame` each tick. The nodes are bind·body matrix.
  `wrecked` is the crash word. Velocity and airspeed come from the body. The ground contacts are the skids
  point (`KiowaRide::skids`, the body position less its rotated centre of gravity) while the ride reports
  touching. The particles are drawn on the GPU and the software paths like the plane's. The end-of-ride
  report prints the particle counts.
- Test `tests/audio.rs::a_flown_kiowas_loop_rides_the_collective_and_stops_on_the_wreck` covers:
  - pitch 1 while holding, (own+400)/own while climbing, (own−400)/own while descending;
  - no voice while wrecked, one voice again when put back;
  - the loop stops on switching to the bike.

## Checks

- `./kit check -- -p openmx --test audio -- kiowa helicopter`: 3 passed. The kiowa_particles,
  bike_particles and wreck_particles tests: 11 passed (the kiowa disc test ran, not skipped).
- Headless free09 rides (generated tapes, `switch N helicopter`):
  - idle on the skids: 290 wash particles (`Dust`), and no body dust at rest;
  - climbed to 42 ft: the wash is gone (fade 15→40 ft);
  - climbed to 188 ft: no particles;
  - `--trace-at 2848,3999,180` (where the original's Kiowa rests in C14's capture): 295 wash particles,
    a brown ring round the Kiowa.
- PCSX2 copy 2: C14's flight tape played under Video Capture (`capture/run.sh`, NominalScalar 1 for
  the run, then put back).
  - **Eye.** While low (≈21–28 s) the original shows a faint broad haze of wash. Higher up it shows
    none, and none on the grass at the start. The emitter's steady count (~300 at 150–250/s, 1–2 s
    life) matches the shell's. The shell's ring reads denser and browner: that is render-side, noted
    and not chased.
  - **Ear.** `capture/pitch.py` matches the capture against the loop (22050 Hz) resampled at 0.96–1.04.
    Windows are 1 s at 0.5 s steps. The "rule" column scores the port's three rates.

    | capture time | tape | best pitch | rule scores −400 / 0 / +400 |
    |---|---|---|---|
    | 7.5–16.5 s | hold, forward, rudder, left | 1.000 (0.29–0.50) | 0 wins every window |
    | 4.5–6.5 s | climb (1–4 s, filter lag ~1 s, video offset) | +309…+617 Hz | +400 wins (0.14–0.19 vs 0.12–0.16) |
    | 17.5–19 s, 26–27.5 s | descend (Square) | −375…−816 Hz | −400 wins (0.11–0.15 vs 0.08–0.11) |
    | 19.5–25.5 s | descend | mixed, scores ≈0.1 | no clear winner |

    Holding plays the sample's own rate, climbing plays above it and descending below it, with steps
    about the size of ±400 Hz. That fits the ported rule.

## Not verified / not 1:1

- The skids contact is a stand-in. It is one point under the centre of gravity while the ride reports
  touching, not the original's collision record per model (P216a).
- The blades' collision models have no producer, so the four blade-dust/chunk entries never emit.
- The wash looks denser and browner than the original's haze. This is a render difference, matched by
  eye, and not chased.
- The near/far distances use the file's values, but the curve, the Doppler (bike `doppler()` with factor
  1.3), RolloffFactor/AttenuationCurve and the 20.0/100.0 init floats are the shell's, not ported.
- Whether a parked Kiowa's loop sounds in the original is unchecked. The shell plays it only while flown.
- The descent's pitch reads are noisy (music and wind under it). Only the hold and some windows of
  the climb and descent are clear. The size of the steps is "about 400 Hz", not measured exactly.
- The rate rule is not oracle-tested against the original's function, only hand-ported and heard.
