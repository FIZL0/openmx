# 2 — the merge, redone (FINAL)

2026-10-23. Entry 1's work (wip/P113) did not survive its merge: the tests' build broke. Main had
moved under it, so this session took entry 1's commit onto current main and fixed what the move
broke. Nothing in the design changed.

## What I did

- **The ride's load runs on its own thread now (P127)** and gives back one big tuple. The fix
  session's merge had kept P113's `plane_starts` (where each seat's plane starts when no copy is
  parked) inside that thread but left it out of the tuple, while the seat loop outside still used
  it, so the build failed. It is in the tuple now.
- **`markers::Spot` gained `motion` (P132).** The test's teleports set `motion: None`, which sets the
  bike down stopped: these are placements, not rides.
- **Menu taps need a rest since P109.** The test's menu and pause taps now hold the pad up for three
  idle frames after each release, as menu.rs's own `tap` does. Before this the second Cross was
  dropped as a bounce, and the menu never reached the ride.
- The conflicts with main were in main.rs and track_mods.rs. For the main.rs particle load I kept
  P113's `plane_kit` argument and main's V53 preload-meter step. In track_mods.rs both sides' tests
  are kept.

## How it was proved

`./kit check -- -p openmx --test machines_everywhere`: all six P113 tests ran on the disc with no
skips (checked with `--nocapture`). `--test track_mods` passed, including the mod-map test. The full
gate runs in `./kit done`.

## Not verified / not 1:1

Everything in entry 1's list still stands. Nothing new this session: the fixes are glue, and the
tests that cover them ran.
