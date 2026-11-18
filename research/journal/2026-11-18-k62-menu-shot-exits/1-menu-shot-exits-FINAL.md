# 1 — menu shot exits (FINAL)

Task K62. 2026-11-18.

## What I did

- `crates/openmx/src/main.rs`, the menu loop: once a menu `--shot` png is written, the loop breaks and the
  shell exits. Before, `--frames` counted only ride frames, so a menu shot ran on to `--seconds` (120 s in
  menu.rs's `screen_shot` and `garage_shot`).
- A shot of the pause (`--screen pause|tricks|pause-garage`) is still the ride's and unchanged: rides keep
  going after a shot and end on `--frames` as before.
- Nothing in opentennis to take: its menu has no `--screen` shot path.
- Filed K77: `every_pill_fits_its_label_and_nothing_covers_the_logo` never starts the shell (it draws every
  screen in-process), so this fix can't speed it up. It takes 128 s alone.

## What the original does

Nothing to port: this is about the shell's command line.

## How it was proved

- Per-test times, copied `menu` test binary, `--report-time --test-threads 8`:
  - `the_race_pages_are_shot_from_the_menu` 0.87 s (was 481 s)
  - `the_garage_shows_the_picked_bike_and_rider_in_its_paint` 0.68 s (was 240 s)
  - `every_pill_fits_its_label_and_nothing_covers_the_logo` 128 s (unchanged; K77)
- By hand: `openmx --headless --menu --screen race --size 640x360 --frames 3 --seconds 120 --shot …` wrote
  the png and exited 0 in 0.11 s. The png shows the Race track pick.
- `./kit check -- -p openmx --test menu` on the three tests: passed.

## Not verified / not 1:1

- The pill test is still slow (128 s). It doesn't use the shell, so K62's fix can't reach it; filed as K77.
- A windowed (non-headless) `--menu --shot` run now quits after its shot too. No test covers that, and I
  didn't run one: there's no display in this slot.

## Next

none
