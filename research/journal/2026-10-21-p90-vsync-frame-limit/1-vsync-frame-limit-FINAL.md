# 1 — vsync frame limit (FINAL)

Task P90. 2026-10-21.

## What I did

- `menu.rs`: two Settings rows under the speedometer's, above Game Modifiers (which stays the last
  row, so the wrap-up tests still reach it): **VSync** (On by default, Cross/left/right toggle it) and
  **Frame limit** (`FrameLimit`: Off, 30, 60, 120, 144, 240 fps, Refresh rate). `settings.txt` gets
  `vsync = on|off` and `frame_limit = off|N|refresh`. A file from before P90 reads as VSync on, no
  limit. The file and `--fps-limit` take any number from 1 to 1000. The row steps a number it does not
  offer to the next one it does.
- `pace.rs`: `frame_floor(vsync, limit, refresh, waits)` is the least time a picture takes.
  `Limiter` sleeps each frame to that floor, keeping deadlines rather than "now + floor", so the
  sleep's lateness does not pile up and a 60 limit averages 60.
  - VSync on, with a present that waits (wgpu Fifo): the loop keeps only a limit below the refresh.
  - VSync on, with the X11 software window, whose `PutImage` never waits: the loop paces to the
    refresh, or to a lower limit.
  - VSync off: the limit alone, or uncapped.
- `gpuwin.rs`: VSync on is `PresentMode::Fifo`. Off is the old choice: Mailbox, then Immediate, then
  Fifo. `set_vsync` configures the surface again only when the mode changes, and is called every
  frame, so a change in the pause's Settings applies live. `screen.rs` passes the call through, plus
  `waits()` and `refresh()`. The X window does nothing on `set_vsync`, waits for nothing, and gives
  no refresh.
- `main.rs`: `--vsync on|off` and `--fps-limit off|N|refresh` win over the settings
  (`Args::pacing`). `--draw-fps` (the tests' fixed rate) and a headless ride's 60 still win over
  both.
  - The ride loop's sleep is now `limiter.wait(draw_frame().or(picture_floor(..)))`. It reads the
    front end's settings as they are that frame; a ride with no front end and a window reads
    `settings.txt` once.
  - The menu loop used to sleep to a 60 Hz `FRAME`. Its key repeat and slideshow are counted in
    frames, so `menu_floor` keeps it at 60 and only a lower limit slows it. With a waiting present on
    a 60 Hz display, Fifo is that pace already, so it does not sleep on top (the old gpuwin.rs
    comment's double wait).
- The simulation is untouched: `Pacer` still owes ticks by the wall clock, and `tick_clock` still
  depends only on the tick's index.

From opentennis: `hst-base/src/graphics.rs` has the same pair of settings (`vsync: bool`, `fps`, 0 is
none) in Bevy's `PresentMode`. I took the shape and nothing else: here the loop is our own, so the limit
is a sleep in `pace.rs`, not Bevy's frame pacing.

## What the original does

Nothing to port: the PS2 always presents on vblank at 30 fps. This is a presentation setting (AGENT.md:
the picture is ours; the ticks are the original's).

## How it was proved

- `pace.rs` `a_60_fps_limit_sleeps_the_frame_and_leaves_the_ticks_alone`: 30 real frames of 2 ms of
  work under a 60 limit average 16.6–17.5 ms, and none ends under 16 ms. A ride over three seconds of
  simulated wall clock is drawn uncapped (1 ms frames) and at 30/60/120/144/240. It gives the same
  ticks at every rate, and the engine's `dt` is the same bits for every tick.
- `pace.rs` `the_settings_make_the_pictures_floor`: the floor for each combination of VSync, waiting
  present and limit.
- `tests/menu.rs` `the_vsync_and_frame_limit_rows_change_by_pad_and_are_saved`:
  - A fresh menu, no file and a file from before P90 all have VSync on and no limit.
  - Pad taps change both rows and leave the speedometer's alone; the next row down is still Game
    Modifiers.
  - Saving and loading gives the same values, every `FrameLimit` reads back from the file, and words
    the file cannot read leave the defaults.
- `every_pill_fits_its_label_and_nothing_covers_the_logo` walks twelve Settings rows, so it covers both
  new labels at four window shapes.
- Live, in a real window on :0, riding free06 at 320x224 (`--seconds 6 --fps`):
  - GPU, defaults: `Fifo`, 60.0 fps, present 15.7 ms (waiting on the 60 Hz display).
  - GPU, `--vsync off`: 3804 fps.
  - GPU, `--vsync off --fps-limit 30`: 30.1 fps.
  - GPU, `--fps-limit 45` with VSync on: 45.1 fps.
  - Every run rode 180 ticks and ended at the same position (2884.24, 335.27, 2763.80).
  - Software renderer: 59.7 fps with VSync on, 88 fps off. Both runs rode 150 ticks in 5 s and ended
    at the same position.

## Not verified / not 1:1

- **The live switch mid-run was not driven.** No keystrokes reach a window on this desktop, and the
  pad on js0 is another slot's idle one. The rows changing the settings is proved, and the loop calls
  `set_vsync` and `frame_floor` with the current settings every frame, but nobody changed VSync in
  the pause and watched the present mode change.
- **No tearing check.** Fifo is vsynced by the wgpu/Vulkan contract. Nobody looked at the screen
  (grim grabs the lock screen here).
- **The software window does not know the refresh.** The X11 `Window` has no monitor query, so its
  VSync paces to 60 Hz. That is right for this 60 Hz display but wrong on a 144 Hz one. Its pacing is
  also a sleep, not a real vblank wait, so the software path can still tear. It is the fallback
  renderer, so I left it there.
- **Refresh rates 120 Hz and up were not tried.** This display is 60 Hz, so the `limit < refresh`
  branch with Fifo (for example 120 on a 144 Hz display) was only tested at 45 on 60. The GPU
  window's `refresh` is winit's monitor query and falls back to 60 when the monitor does not say.
- **The ride's pause menu is still stepped once per drawn picture**, so with VSync off its key repeat
  runs faster. This was already true before P90 (the GPU path was uncapped already); I did not change
  it.
- **The web build was not checked.** `web.rs` and `web/worker.js` have their own frame loop (a
  `setInterval`), which this task does not touch; the browser gets the default settings fields only.

## Next

Nothing.
