# 1 — openmx draw off ps2 float (FINAL)

Task V124. 2026-11-17.

## What I did

I read every `ps2_float` use in the eight files, as C26 did for game-render, and sorted each into a
per-frame draw path that nothing reads back, or a seam into the sim or a proven port.

**Moved to host f32: the wreck camera (`chase::WreckCamera`).** It was already host f32 apart from
its seven lags (`game_sim::camera::Lag`, three eye, three look-at, one field of view) and its aim's two
turns (`game_sim::camera::rotate` through the engine's sine table). It is the shell's own camera. The
sim never reads it, and no replay, capture or oracle test compares it bit for bit (`tests/wreck_camera.rs`
and `tests/camera_view.rs` check its distances and angles within tolerances). Now:

- `chase::WreckLag` is the same filter in f32: `value += min(dt, tau) / tau * (target − value)`.
  Its taus are never 0 (0.2 to 0.77), so the PS2's 0/0 answer never mattered.
- `chase::rotate` is Rodrigues' turn with the host's `sin_cos`, the same handedness as the original's.
- `WreckCamera::update` lost its `curve` argument (one caller in `main.rs`, and the test).

**Left on ps2_float, and why:**

- car_draw.rs (`bay_at`) and tire_tracks.rs (`Bike::car`): read `Vf` fields of sim structs into host
  vectors. Type seams, no arithmetic.
- pace.rs: `F` only in its tests (sim `DeltaTime`).
- garage_view.rs and trick_view.rs: `F(…)` around `Blend::One` and `apply_vehicle`, the sim's motion
  API. Type seams.
- world_dress.rs: `moving` shifts placements into the sim's prop world (`game_sim::prop_world`), and
  the rest is in tests (`Ground::at`, `Reach`). Sim seams.
- particles.rs: `Emt::size_and_life`, `Gate` and `touch_among` are in the PS2's arithmetic on purpose
  and proven by `tests/particle_size_life.rs`, `tests/emit_gate.rs` and the C18 captures. `touch_among`'s
  triggers fire actions. `PlaneFrame::touch` takes a `Vf` from the contact search: a seam.
- chase.rs, the rest: the `Target` builders and `mod_follow` feed `game_sim::camera`, the original's
  race camera, which is ported to the bit and compared against captures (`tests/bike_camera.rs`,
  `car_camera.rs`, …). `carry`, `ride_out`, `reframe`, `FovEase`, `framed` and `keep_in_view` already do
  host arithmetic (C27) and only write the follow camera's `F` fields back. `OtherCamera` hands the
  ported thrill camera its `F` near clip.

Opentennis has nothing like this to take from.

## How it was proved

- `tests/wreck_camera.rs::the_wreck_aim_turns_as_the_originals_rotation`: `chase::rotate` against
  `game_sim::camera::rotate` with `CurveTable::build()`, over 10,000 generated angles (−π to π), unit
  axes and vectors (±50 ft). They agree within 1e-5 of the vector's size.
- `the_wreck_lags_step_as_the_originals`: `WreckLag` against `Lag` over 100 generated lags of 100
  frames each (tau 0.05 to 0.85 s, dt 0 to 0.1 s, values ±500 ft). They agree within 1e-5 of the 500 ft
  range. A first tolerance relative to the value failed at 3e-5 absolute where a value crossed near 0,
  which is the PS2's round-toward-zero add at magnitude 500.
- The four existing wreck-camera tests and `camera_view`, `bike_camera` pass unchanged.
- The sim is untouched: no file under game-sim changed, and the wreck camera's output only goes to the
  drawn view.

## Not verified / not 1:1

- **The wreck camera's picture is no longer the PS2's bits.** Its lags are within a few ULPs a frame,
  and its turn within 1e-5. This is a rendering difference, which AGENT.md allows. I did not watch a
  wreck on screen before and after.
- **I did not compare whole wreck-camera runs, old against new.** The proof is per part (the lag and
  the turn). The rest of `update` was host f32 already.
- **No ride hash was taken before and after.** Nothing in the sim or its inputs changed, so I skipped
  it.
- **No frame-time gain was measured.** Seven lag steps and two turns a frame, only while a rider is
  down, are well under a microsecond.
- **world_dress.rs was named in the brief as "missing — this task creates it".** It exists; the brief's
  file list was mangled. I audited it as one of the eight.

## Next

Nothing for V124.
