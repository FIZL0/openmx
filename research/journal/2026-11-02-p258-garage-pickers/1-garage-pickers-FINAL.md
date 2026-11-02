# 1 — garage pickers (FINAL)

## What was slow

Measured in the test profile (opt-level 2), headless GPU, disc catalogue (54 bikes):

| | before | after |
|---|---|---|
| menu frame's `Stage::dress` on a pick change | ~42 ms dress (+ ~14 ms mips) | median ~20 µs, slowest 66–112 µs of 30 frames |
| GPU upload on the frame the new pick shows | 16–21 ms | 2–9 ms (mips precomputed, `put_mips`) |

So a pick step used to cost one ~60–80 ms frame; now no menu frame waits on the disc.

## Changes

- `garage_view.rs`: `Stage` owns `Tools` (disc + kit) and hands them to a `crate::thread` job per pick.
  The shown pick changes only to the currently asked one; done jobs for other picks go to `kept` (6).
  `ahead()` prefetches the ±1 neighbours of the row under the cursor (`Menu::garage_neighbours`); off on
  web, where a job runs inline. `settle()` waits for shots. `shown_of` folds Tier/preset so they don't
  re-dress.
- Repeats: the P165 `Reader` stays as is (its tests want trusted-timed holds to repeat in slow frames).
  Instead `window::Arrivals` restamps receipt-stamped moments after a poll that came > 100 ms late: each
  is put early, 40 ms apart, except a down that is its key's last (still held → stamped now). Used by
  gpuwin keys, web keys, and the web/iOS/Windows snapshot pads (a pad going back to rest is stamped
  early). X11 and evdev already carry real times.
- `roster.rs`: `Catalogue::model_name` / `paint_name`; Model/Livery steps keep the engine.

## Checked

- tests/garage.rs `paint_steps_keep_the_bike_and_bike_steps_land_where_asked` (every bike, every paint;
  Bike/Class steps land where asked), roster unit test, garage_view
  `ten_quick_paint_presses_end_on_the_tenth_without_a_long_frame`, tests/menu.rs
  `one_push_and_release_across_a_600_ms_read_is_one_press`. tests/menu.rs 226 pass, garage 11 pass.
- Builds: desktop, wasm (`tools/web.sh --check`), Windows gnu check, iOS (`tools/ios-build.sh check`).

## Not verified / not 1:1

- Not watched on a real window or a real pad; times are test-profile and headless, not release.
- Web and iOS runtime not run; on web the dress still runs on the main thread (after a 150 ms settle), so
  a web step still has one long frame, just not one per press.
- The restamp spacing (40 ms) is a guess at a fast tapper, not measured from the original.
