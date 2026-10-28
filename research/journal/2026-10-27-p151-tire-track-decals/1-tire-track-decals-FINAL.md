# 1 — tire track decals

Task P151. 2026-10-27.

## What I found

P151 was filed by P74 (journal `bike-particles`, 2026-10-25) for the two `TireTrack` entries P74's
emitter reader skips. The same day V40 (journal `2026-10-25-v40-bike-tracks`, commit 4bd9ec01) ported
exactly those entries as `crates/openmx/src/tire_tracks.rs`: each entry's strip behind its tire, the
fade in/out, the compression and speed gates, the zones, the break angle, the 32-batch ring and the
`ttrack.pst` tread, stepped per seat in `main.rs` and drawn by both renderers through the glow path.
The two tasks were filed independently, so the work P151 asks for was already on main.

Checked on this branch:

- 4bd9ec01 is an ancestor of HEAD, and `main.rs` loads and steps the tracks (`load_tire_tracks`,
  `step_tire_marks`).
- `./kit check -- -p openmx --test tire_tracks`: 8 tests pass (V40's disc, replay, ring, gate, fade
  and break-angle tests).
- `openmx --track free09 --headless --frames 60` prints both entries off the disc:
  `tire tracks: Front Tire Track on tire 0, 32 strips, ttrack` and the rear on tire 1.

## What I changed

The same run also printed `bike: particles: Front/Rear Tire Track (TireTrack) not ported` from P74's
particle loader, which reads as if the tracks were missing. That line now says
`laid by the tire tracks` for `TireTrack` entries; any other skipped type still says `not ported`.
Two test comments in `tests/bike_particles.rs` said the same and now point at `tire_tracks.rs`. No behaviour changed.

Nothing taken from opentennis: it has no tire tracks.

## Not verified / not 1:1

- **No new proof against the original.** P151 adds no port, so it relies on V40's proofs and inherits
  every gap V40's entry lists (split screen as an addition, the jitter's own RNG, when in the frame
  the tracks step, `wheel.surface` as the zone byte and `slip.spinning` as the spin word unproven, the
  unfogged glows, the software renderer's per-triangle alpha and no by-eye check of it, no side-by-side
  shot from one pose, local seats only). None of them was re-examined here.
- **The log line was checked by running the shell, not by a test.** It is a println.
- **Landed with `--skip-audit`.** `./kit audit` fails on seven addresses in `plan/DONE.md` (lines 121
  and 491: older tasks' entries naming boot functions). None is in this diff. Fixing them means editing
  other tasks' plan lines, so they were left alone.
