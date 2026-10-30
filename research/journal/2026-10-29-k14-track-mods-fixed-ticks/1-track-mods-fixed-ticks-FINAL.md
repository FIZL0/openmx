# 1 — track mods fixed ticks (FINAL)

Task K14. 2026-10-29.

## What I did

- Why it reddened: the shell's `--seconds N` is a wall-clock limit. The front end hands the ride
  `N − started.elapsed()` (main.rs, the `a.seconds = …` lines), and the ride loop breaks once its clock
  passes that limit. At load ~33 loading took the whole 20 s, so the ride got a limit of 0 and broke on
  its first pass: "ridden for 0.0s over 0 frames", with the end still at the spawn [512, 5, 300].
- `tests/track_mods.rs`: every taped ride (`--play`) now runs on `--frames N --steady-clock` and no
  longer on `--seconds`. The tape still ends the ride ("the tape ran out after 150 frames"), and
  `--frames` is just a tick cap: 600 for the 20 s rides, 270 for the 9 s biplane-switch ride, so the
  same limit, counted in 30 Hz ticks. The steady clock (deb7bebc) means a late picture drops no ticks.
  This covers the named test (`a_one_level_terrain_is_256_samples_a_side`) and the four other
  taped rides in the file that had the same race (`a_track_mod_loads_and_rides…` and its ride-hash
  rerun, `a_disc_ride_is_the_same_with_or_without_mods`, `a_mod_maps_spawn_is_its_start` and its
  "gone" ride, and the biplane switch tape).
- I left `a_mod_map_has_the_biplane_with_no_machine_txt`'s first, tapeless `--seconds 2` start as it
  was. It only reads lines printed at the start (vehicle, plane hash, the dirt under the plane) and
  counts no ticks.

## What the original does

Not applicable: this is test-gate work.

## How it was proved

`./kit check -- -p openmx --test track_mods`: 17 passed. Running the K14 test alone with --nocapture printed
"the tape ran out after 150 frames … ridden for 5.0s over 150 frames, ending at 512.70, -0.28, 594.88".

## Not verified / not 1:1

- I did not reproduce the failure under load ~33. I found the cause by reading the code: the logged
  0.0 s / 0 frames matches a `--seconds` limit that loading had already used up. With `--seconds` gone
  from these rides, no wall-clock limit is left that could end them early.

## Next

None.
