# 1 — openmx host floats (PART)

Task C27. 2026-11-04.

## What I did

I sorted the 86 files under `crates/openmx` that name `ps2_float` (22 in `src/`, 64 in `tests/`).
Nearly all of them hold `F`/`Vf` only because they are the types game_sim's API takes and returns:
spawns and bays, `Controls`, camera presets, tiles and `Reach`, `Stunt`, `Blend`, `Area`, the
targets in `chase.rs`. `F(x)` and `.0` are a newtype wrap and cost nothing, so those stay. A file
"moves to host f32" only where openmx does PS2 arithmetic of its own (`ps2_float::fpu`, or `F`
operators). In `src/`, that happens in:

| place | what | verdict |
|---|---|---|
| `audio.rs` (`unit`, `pick`, `jitter`, drive-on pitch/slope/mph, the note's timers, tire speeds) | the original's sound engine | **stays**: `tests/audio.rs` runs the original's code in `ps2_oracle::Ee` and compares |
| `particles.rs` (`Emt::size_and_life`, `Gate`, `touch_among`'s roll) | the original's emitters and triggers | **stays**: C18/P185 compare them to PINE captures. They also draw the game's `rand`, and the trigger roll decides whether actions fire |
| `chase.rs` (`flown`, `driven`, `heli`, `bike`, the wreck camera's `Lag`/`rotate`) | the camera's target block, the original's camera | **stays**: `car_camera.rs` and `camera_view.rs` compare them bit for bit. The wreck camera's own maths was already host f32, and it only calls game_sim's `Lag` and `rotate` |
| `chase::ride_out` | ours (P112): rolls the lags back at a teleport so the camera rides out with the bike | **host f32 now**. Nothing reads the camera back, and `markers.rs`' test is a 0.01 ft tolerance |
| `overlay::PreloadMeter::strength` | the HUD's preload level | **host f32 now**. It is a HUD readout that feeds nothing back. `overlay.rs`' captured cases still give the same levels |
| `worlds.rs` `regions()`' wrap period | `game_render::terrain::Regions`, render only | **host f32 now**, the same as the line above it |
| `vehicle.rs` (Kiowa build, the plane's autopilot dead zone, `F(1.0 / mass)`) | goes into the sim's state | stays |
| `main.rs`, `worlds.rs`, `menu_tricks.rs`, `garage_view.rs`, `trick_view.rs`, `same_ride.rs`, `net/rides.rs`, `online.rs`, `car_draw.rs`, `tire_tracks.rs`, `ground.rs`, `ios.rs`, `web.rs` | API types, `NATIVE` checks, comparisons, doc links | stays: no PS2 arithmetic of their own |
| `pace.rs`, `pad.rs`, `world_dress.rs`, `trick_packs.rs` | `F` only in `#[cfg(test)]` | stays: tests of sim values |

The 64 test files use `F`/`Vf` to build game_sim's inputs, or to compare its outputs bit for bit.
None was changed.

opentennis: nothing to take. Its shell has no split like this one.

## What the original does

Nothing new. Only `PreloadMeter::strength` is a reading of the original: the speedometer's
`0.5 + 20·force/max` truncated. It is now done with the host's IEEE operations instead of the EE
FPU's.

## How it was proved

- `./kit check -- -p openmx --test overlay --test markers --test tiled_world --test wrapped_world
  --test wrapped_world_draw`: 254 passed.
- The sim, the tapes and the replays are untouched (no game_sim file changed, no openmx sim path
  changed). The gate's replay tests show this (`./kit done`).
- Frame time: release builds of HEAD and this change, run back to back with
  `openmx --track free09 --headless --frames 600 --timings` and `OPENMX_THREADS=1`. The sum of the
  session's stages without pace was before 2.97 and 2.97 ms, after 3.00 and 2.55 ms. The camera
  stage was 0.01 ms in all four runs. The difference is the shared machine's noise. None of the
  converted code runs on an ordinary frame.

## Not verified / not 1:1

- `PreloadMeter::strength` in host f32 can differ from the original by one level, but only where
  `0.5 + 20·force/max` falls within an ulp of a whole number. I fed it no generated inputs (only
  the test's 7 cases). The task rules it a HUD readout and not 1:1.
- `chase::ride_out` in host f32 differs from the old PS2-arithmetic result in the last bits of the
  eye and look-at, on the one teleport frame. It is ours, not the original's, so there is nothing
  to match. The only check is `markers.rs`' 0.01 ft test.
- Frame time was measured on one track (free09, headless, software present) on a loaded machine.
  It was not measured on the web or the Deck.
- The task's "86 files" stayed nearly all `ps2_float`, by its own rule ("anything a replay or a
  test compares bit for bit stays"). No openmx file dropped `ps2_float` entirely.

## Next

The work is done, but `./kit done` cannot tick C27 while main is red. On 2026-11-04 the full gate
failed in four tests:
- `web.rs` `the_browser_installs_from_the_store` has failed at any load since P293. It is filed as
  P294, critical, and is still open on main at 36a51c7f.
- `disc_worlds` `world_mxu…` took 593 s against a 480 s limit, `world_ultra_load`'s first frame
  came at 90.8 s against 20 s, and `web.rs` `a_pad_the_browser_shows_on_a_press_rides` failed once.
  All three ran at a load average near 60. Run again alone at load 10, `world_ultra_load` and
  `disc_worlds` passed, and of `web` only the store test failed. None of these tests touches
  `chase::ride_out`, the preload meter or the render regions.

The commit is on `task/C27`, pushed by hand. Once P294 lands, run `./kit done C27 "<msg>"` again.
