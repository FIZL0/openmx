# 1 — hyprland gtt screen on (FINAL)

Task R18. 2026-11-16.

## Answer

Hyprland's GTT does **not** climb with the screen on. Over 3 days of monitors_on=1 on one boot it moved
between 138 and 235 MiB and ended lower than it started. The old leak was ~400 MiB/h. Moving PCSX2 to a
private display is not warranted.

## What I did

- Read `~/.local/state/gtt-watch.log` for boot 0 (up since 2026-11-01 16:55, still up): 15 days, one row
  per 10 min. Monitors off 11-01 → 11-13 11:33, on from 11-13 11:43 to now (11-16 19:00).
- One gap: 11-15 22:28 → 11-16 08:01, a `systemctl suspend` (system journal). Not a timer fault.

## Findings

Screen off (11-01 → 11-13, ~11.5 days):

- hyprland_mib 20 → 36 in the first two days, then flat.
- One step on 11-05 at 05:06: 36 → 79 MiB, xwayland 16 → 29 MiB, as chromium went 0 → 37 processes.
  Kept, not climbing after. Flat 66-74 MiB from then to 11-13. One-off; the 10-min rows can't say which
  client it was (the log has counts, not pids).

Screen on (11-13 11:43 → 11-16 19:00, ~3 days less the 9.5 h suspend):

- The switch itself added ~+95-140 MiB within 20 min (74 → 168 → 212). That is the monitors' scanout
  buffers, as expected.
- Daily min/max: 11-13 160-214, 11-14 162-235, 11-15 148-200, 11-16 138-173. No upward trend; the last
  two days sit lower.
- Swings of 40-50 MiB both ways follow hyprlock: rows with hyprlock=1 sit ~150-165, unlocked rows ~190-215.
  Lock and unlock give the memory back.
- Running then: chromium 19-111 processes (39 most of the time; a browser plus headless test pages),
  weston 3-19 (gate windows), pcsx2 0-5 copies, openmx 0-2. None of their changes lines up with a kept rise.
- xwayland_mib 28 → 36 once (11-14 01:55, with chromium 44 → 29); flat after.

## How it was proved

Read from the log only (awk over boot-0 rows; daily min/max with monitors_on=1; every ≥4 MiB step listed
with what ran). No experiment.

## Not verified / not 1:1

- Which client caused the 11-05 +43 MiB step (screen off): the log has process counts, not per-pid GTT
  of clients, so it can't be pinned. It stayed flat after, so not chased.
- Only 3 screen-on days, not a full week of them: the task asked for a week on one boot including screen-on
  days, which this is (15 days), but the screen-on stretch itself is 3 days. A slow leak under ~1 MiB/h
  would hide inside the lock/unlock swings.
- The screen-on days had light emulator use (pcsx2 ≤5 copies); a heavy PCSX2 day with the screen on was
  not in the window.

## Next

none
