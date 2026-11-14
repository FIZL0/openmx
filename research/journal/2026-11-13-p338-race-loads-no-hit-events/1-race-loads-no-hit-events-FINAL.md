# 1 — race loads no hit events (FINAL)

Task P338. 2026-11-13.

## What I did

- The user saw a free-ride challenge marker on a Race on sx01 (v0.13.0). Every Supercross and National
  track's archive carries a `hit.dat` (one activity start zone each, and the AI seeker databases), and
  the ride's build read it whatever the mode: the zone's portal in the world, its radar icon, the
  freestyle event and (since S316) the free-ride AI seeker world.
- `free_ride_hit_dat` (`crates/openmx/src/main.rs`) reads the track's `hit.dat` once, and not at all
  when `args.race` is set (the menu's Race sets it, as does `--race`). The radar zones, the world's zone
  portals, the stunt event and the seeker world are all built from that one text, so a Race gets none of
  them. Free ride is unchanged.
- Test `free_ride_hit_dat_reads::a_race_loads_no_hit_events`: a race reads nothing, a free ride reads the
  file, and the empty text builds no zones, radar icons or stunt zones.
- Nothing taken from opentennis: it has no per-mode track events.

## What the original does

Taken from the task's brief (the user's report that a race shows none): a Race loads none of free ride's
hit events. Not re-read in the decompilation here.

## How it was proved

Release build, headless (`--headless --frames 2`), every race track (sx01–sx20, nat01–nat20) twice: with
`--race` and free-ridden. Free ride printed `ai seek: N targets, 1 zones, …` on all 40 (the file is
there and read); the race printed no `ai seek` and no `freestyle event` line on any of them, and its
`race:` gate line as before. The portals and radar icons come from the same empty text, which the unit
test shows builds nothing.

## Not verified / not 1:1

- The original's race load was not read in the ELF to confirm it skips `hit.dat` entirely: the brief
  states it as the original's behaviour and the fix follows it.
- No picture of a race was looked at: the portal and radar counts are not printed, so their absence is
  shown by the shared empty input and the unit test, not by a shot.
- Free ride on a race track still shows its one zone; whether the original's free ride on sx/nat tracks
  shows it was not checked (out of this task).
- An online guest's race: a guest builds its ride from the same `args.race` the lobby hands it, which
  was not run here.

## Next

Nothing.
