# 1 — auto photos (PART)

Task P390. 2026-11-21.

## What I did

- Settings, Display tab: an **Auto Photos** heading with three switches, Wrecks, Landed Tricks and
  Race Results, all off by default. Saved in settings.txt as `auto_photos = wrecks,tricks,results`
  (no line when all are off, so an older file is written as it was). `--auto-photos LIST|off` sets
  them from the command line, as `--show-fps` does.
- `photo.rs`: `Auto`, the ride's waiting auto-shot. A landed trick is shot on its landing tick; a
  wreck 12 ticks (0.4 s) after it starts, so the rider is off the bike. One waits at a time, and
  nothing is shot within 60 ticks (2 s) of the last picture, so a run of tricks is one picture, not
  a burst. Ticks going back (a restart) clear it.
- `main.rs`, the ride: a local player's `said.wreck` / `said.landed` feed `Auto`; when one is due,
  it sets P385's `photo_shot`, so the picture is written by photo mode's own path (offscreen
  read-back on the GPU, the frame itself in software, `photos/photo-NNNN.png`, "Photo saved" line
  and note). The HUD and the FPS counter are left off that frame, as photo mode leaves them off.
- `main.rs`, the front end: a race left with Race Results on asks for a picture; it is taken on the
  results screen's first frame (drawn on the CPU, as `--shot` draws a menu), into the same folder,
  via `photo::save`.
- opentennis has no photo mode or auto-shots: nothing taken.
- MXA (the reference the task names) is not on this machine (context/mxa is missing, see MD259), so
  which moments and what delay it uses were chosen here, not copied.

## What the original does

Nothing: MX Unleashed has no photo mode. This is an addition, off by default; off, the ride is
untouched (the auto-shot only reads the ride's tick count and its said moments).

## How it was proved

- `crates/openmx/tests/auto_photos.rs`: the shell, headless, rides free06 on a generated tape (gas
  held, No Hander tapped every 40 ticks). With `--auto-photos tricks`: one picture for the first
  landing, none for the second inside the gap, a 640x448 PNG at `photos/photo-0001.png`, taken on
  the tick it was due. Without the flag: no picture, no folder.
- `photo.rs` unit tests: trick at once, wreck 12 ticks later, the gap, a restart.
- `tests/menu.rs`: the three rows change by pad, are saved and read back; all off writes no line.
- By eye: a wreck (md16-cross-up.tape, shot at tick 512 for a wreck at 500) and a No Hander landing
  on free06 (rider with hands up, no HUD).
- Results, live: a front-end race driven by a fifo pad (P271's `pads.py`, Start on Race Setup,
  `--race-bot`, `--auto-photos results`) ran 3 laps of sx01 and printed `auto photo: results —
  Photo saved: …photo-0008.png (640x448)`; the PNG is the Race Results screen with all four rows.
  Not a test: the race takes minutes.

## Not verified / not 1:1

- MXA's player shots: not compared. Its reference (context/mxa) is not on this machine, so the
  moments, the 0.4 s wreck delay and the 2 s gap are this task's choices.
- On the GPU window, the frame a ride auto-shot is taken on draws without its HUD in the window
  too (the shot is the same scene drawn again offscreen): a one-frame HUD blink. Not looked at on a
  real display (no display here).
- A wreck or trick on a later split-screen seat shoots the whole frame (every view), not that
  player's view alone.
- The plane's crash (biplane, MX vs. ATV machines) is not a wreck moment: only the rider's wreck
  entry (`said.wreck`) is.

## Next

-
