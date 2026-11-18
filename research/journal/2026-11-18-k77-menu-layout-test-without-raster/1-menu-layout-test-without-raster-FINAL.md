# 1 — menu layout test without raster (FINAL)

Task K77. 2026-11-18.

## What I did

- Sampled `every_pill_fits_its_label_and_nothing_covers_the_logo` alone (eu-stack sampler, per-test-times
  recipe): 2058 of 2082 stacks were in `Art::parts`, 2057 of them in `ink::raster` (filling the frame's
  pixels). The layout itself showed in ~2 samples; the menu stepping (`tap`) in ~6.
- Added `Art::parts_at(menu, size)` in `crates/openmx/src/menu.rs`: the same `layout` call `parts` makes,
  with the sprites thrown away and no raster.
- The test now calls `parts_at`. Same 4 window sizes, every Screen, 12 steps, same asserts.
- New test `parts_without_drawing_are_the_drawn_parts`: `parts_at` equals `parts` for Splash, Main and
  Keyboard at the 4 sizes (1.5 s, mostly the 12 raster draws it makes on purpose).
- Timed with copied binaries on a loaded machine (load ~20–40): before 192.1 s, after 0.41 s.
- Nothing taken from ../opentennis: the fix is local to this test and `Art`.

## What the original does

Not applicable: test tooling, menus are ours.

## How it was proved

- `./kit check -- -p openmx --test menu`: 323 tests passed, the pill count floor (≥ 528) still met.
- `parts_without_drawing_are_the_drawn_parts` pins that the undrawn parts equal the drawn ones.

## Not verified / not 1:1

- The pill/logo test no longer runs the raster, so it no longer catches a panic inside `ink::raster` for
  some screen at some step; other menu tests and the shell's `--shot` tests still draw. The equality test
  draws only three screens.

## Next

none
