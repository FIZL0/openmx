# 1 — plane ceiling (FINAL)

Task P136. 2026-10-23.

## What I did

- **What caps the plane's height, looked for one by one:**
  - *The engine's altitude fade* (`biplane::Engine::step`): the thrust's share is
    `remap(y, fade_altitude, max_altitude, 1, 0)`. `fade_altitude` is the constructor's 0 (no `.phy` key
    sets it; `tests/flight.rs` reads the RAM word as 0), `MaxAltitude` is **600 ft** in the disc's
    `biplane.phy`. So the fade is a straight line from y 0 to 600, and the stock plane at full throttle,
    held at 90 ft/s, levels off at **445.7 ft** (inside a minute). This is the only cap in the sim.
  - *A hard ceiling in the flight step*: none. Nothing in `flight.rs` / `biplane.rs` compares the body's
    `y` with a bound; the only `y` reads are the fade's, the tire probes' ground drops (a heightfield
    sample under the wheel, any height) and MD14's "below y 0 with no ground under it counts as
    landed", which is a floor.
  - *The world's / track's bound*: none in height. The edge throw (`Crash::step_in`, `Area::outside`)
    and the tiles' `off_world` / `keeps` test `x` and `z` only.
  - *The edge throw*: horizontal, as above; its lift is a fixed upward kick, no height test.
  - *The camera*: **yes**. The race setup caps the eye at `RACE_CEILING` (200 ft world `y`) on a
    scene whose fog's `MaximumFogDistance` is past 1500 ft (`Race::ceiling_for_fog`, S103), and the
    plane's camera is built by the same setup (`main.rs`, `plane_camera`). Free09's 1000 ft fog gets
    none. On a long-fog scene a plane past 200 ft is watched from an eye held at 200 ft.
  - *The far clip / sky / fog*: no bound on the plane. The eye keeps its preset distance behind the
    plane at any height, so the plane is always drawn; the ground fogs out past `fog_far` and is
    clipped past the 4000 ft far plane, which is a look, not a cap. The sky is a cube map drawn by
    direction alone (`gpu.rs` `sky_faces`), so it is the same at any height.
- **`Knob::PlaneCeiling`** ("Biplane max height", `plane_ceiling`), per seat, after `PlaneTurn` in
  `ALL` and `MENU`. At k: `[Engine] MaxAltitude × k` (the fade's start, 0, is 0 at any k), so the
  thrust's share at k times the height is the file's: the level-off height is k times. 100% is skipped
  outright like every knob, so the file is the original's to the bit.
- **Uncapped**: one step past `MOST`, `modifiers::UNCAPPED` = 510, `Knob::most()` (the only knob with
  it; `set` clamps and `parse` refuses by `most()` now). It sets `MaxAltitude` to `F::MAX`: the share is
  `1 - y / F::MAX`, which rounds to exactly 1 at any `y` below ~1e31 (and clamps to 1 below 0), so the
  fade is off by the original's own code on another number — no branch in the step. `Mods::uncapped`,
  and `Mods::plane_eye_ceiling(seat, fog)` lifts the camera's 200 ft cap (to `F::MAX`) for an uncapped
  seat; the shell sets it on each rig's plane camera when it is built, in `remodify` (P98's
  apply-on-close; it now takes the scene's fog ceiling) and on the host's online change.
- **Held through put_back and retune**: `Flight::retune` and `Flight::put_back` carry
  `engine.fade_altitude` and `engine.max_altitude` beside P116's numbers.
- **Menu**: the row shows `Uncapped` at the top step; the slider steps off the settings' value now
  rather than parsing the row's `%` text (which `Uncapped` is not), and its meter runs to `most()`.
- opentennis has no aircraft; nothing to take.

## What the original does

The fade is S41/P31's port of the engine step (oracle-tested in `tests/biplane.rs` over altitudes
-100..900 with the RAM's own fade words). The camera ceiling is S103's. Nothing new read this task.

## How it was proved

- `crates/game-sim/tests/plane_ceiling.rs`:
  - `at_one_hundred_percent_the_ceiling_is_the_files`: a set taken to 100% leaves the disc's
    `biplane.phy` equal to the bit (Debug form); the Uncapped step reads back through the text form,
    511 and `plane_turn=510` are refused; per seat; the eye ceiling is the fog's unless uncapped.
  - `the_climb_levels_off_k_times_as_high_and_uncapped_does_not`: the full `Flight::frame` at full
    throttle, a hand holding the wings level and 90 ft/s: 100% levels at 445.7 ft, 200% at 891.4
    (×2.000), Uncapped climbs a steady 53.7 ft/s for ten minutes to 32,084 ft (5× past the old
    MaxAltitude) as fast in its last minute as its first.
  - `the_ceiling_survives_a_put_back_and_moves_in_flight`: replane to 200% in flight doubles the top
    where the plane is; a put-back to a marker kept at 100% keeps the 200% (and Uncapped's) ceiling; on
    the original's numbers again the engine is the marker's to the bit.
- `crates/openmx/tests/menu.rs` `the_biplane_max_height_tops_out_uncapped`: 490% → 500% → Uncapped →
  stays → 500%, Cross back to 100%, 1P only.
- Replay/acceptance tests unchanged (`./kit check --all` in `done`): the knob is skipped at 100%.

## Not verified / not 1:1

- **The original's full-throttle climb was not flown on the emulator.** The 100% claim rests on the
  file being unchanged to the bit and on the engine step's existing oracle, not on a capture of the
  original levelling off at ~446 ft.
- **The level-off heights are under this test's hand** (90 ft/s held by a port-side stick controller,
  not the player's controls through the shell's New scheme); another airspeed levels at another height.
  The ×2 holds because the fade is linear from 0, at any airspeed.
- **Uncapped keeps no height bound.** Kept: none in the sim. The float range is not one in practice:
  at 53.7 ft/s the plane takes ~87 hours to reach 2^24 ft, where a position's step would be a foot;
  at an hour (~190,000 ft) it is 1/64 ft. Not run past ten minutes (32,000 ft).
- **The camera's lift was not looked at on screen**: no shell run on a long-fog scene with the plane
  above 200 ft; the code path is a single assignment per rig, and free09 (the states we have) has no
  cap to lift.
- **A wreck high up was not tried**: a plane that crashes or is thrown at 30,000 ft falls a long way
  before its wreck recovery; whether the recovery's timers suit that was not checked.
- The online peers (`net/rides.rs`) retune the sim's plane on a change as before; the session has no
  plane camera, so only the shell's rigs take the eye ceiling.

## Next

Nothing.
