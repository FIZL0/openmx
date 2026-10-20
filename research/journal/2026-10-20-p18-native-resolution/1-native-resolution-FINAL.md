# 1 — native resolution (FINAL)

Task P18. 2026-10-20.

## What I did

- `window.rs`: the setup parse keeps the first screen's two extents (`width-in-pixels`,
  `height-in-pixels`, at +20/+22 of the screen block, after root, colormap, the two pixels and the
  input masks) as `Window::screen`. `Window::open` takes `Option<(w, h)>` now: a size opens a window of
  that size times `scale` as before; none opens it at the screen's size.
- The event mask already had `StructureNotify` (bit 17) for the destroy/unmap it read, so
  `ConfigureNotify` (22) was already arriving and being dropped. It is read now: a size different from
  the window's own updates `width`/`height` and raises a flag that `Window::resized()` hands out once.
  Moves (same size) are not resizes.
- `present` draws only the picture's rectangle (`frame * scale`), centred in the window with
  `PutImage`'s destination x/y. The bars are the window's black background pixel, painted by the
  server; with the default bit gravity (`Forget`) a resize clears the window, so an older, larger
  picture is not left behind in them.
- `fit_frame(window, scale)`: the largest 4:3 frame in `window / scale`. Without `--size` the shell
  draws at that and re-makes the `Frame` when `resized()` says so. 4:3 because that is what the
  original draws for (`vertical_fov`'s doc: 85° across gives the save state's 68.997° down at 4:3), so
  `vertical_fov(aspect(&frame))` and `short_viewport` read 1.333 on any window shape. Nothing is
  stretched, and a 16:9 window pillarboxes until V3.
- `main.rs`: `Args { width, height }` → `size: Option<(usize, usize)>`, default `None`. `--size` keeps
  the old fixed-frame path (`--size 640x448` is the PS2's), `--scale` still duplicates pixels in both
  paths (without `--size` it means "draw at 1/N of the window"). `--moment` has no window, so it draws
  at `--size` or `PS2_FRAME` (640x448), as before. A line at start says
  `window WxH (screen WxH), drawing WxH at scale N`, and another on each resize.
- Tests (window.rs): `the_setup_says_how_big_the_screen_is` (a synthetic setup body with a padded
  vendor name and two formats), `a_resize_is_read_and_said_once` (ConfigureNotify over a socket pair,
  including a move with the same size and a sent event's top bit),
  `the_frame_is_the_largest_four_by_three_inside_the_window`, and
  `a_frame_goes_in_the_middle_of_a_wider_window` (the `PutImage` read back off the socket: width,
  destination x/y, pixel duplication). `./kit check`: passed.

## What the original does

Nothing to port: this is the shell's window. The only original-derived fact used is the 4:3 aspect the
camera's field of view was proved at (V2, `vertical_fov`).

## How it was proved

- Unit tests above, through `./kit check`.
- Live on `:0`: `openmx --seconds 10` (no `--size`) printed `window 1920x1080 (screen 1920x1080),
  drawing 1440x1080`, then the tiling WM resized it at once: `window 456x500, drawing 456x342`. The
  `--shot` it wrote (frame 120, context/p18/native.png) is a correct 456x342 picture of the ride. So
  the screen parse and the ConfigureNotify path both work against the real Xwayland.
- fps, the shell's own line, steady state (seconds 7–11 of a 12 s ride, `--pad /dev/null`, machine
  load average 15–19 on 32 threads from the other slots). The window was tiled smaller than the
  larger frames, so the server clipped what was shown, but the full frame was rasterised and sent:

  | frame                       | fps         | fits 60 Hz |
  |-----------------------------|-------------|------------|
  | 640x448 (`--size`, PS2)     | 59.6–59.9   | yes        |
  | 1440x1080 (4:3 in 1920x1080, the new default on this screen) | 20.5–27.0 | **no** |
  | 1920x1080                   | 17.3–25.6   | **no**     |

  A temporary timer (reverted, not committed) split the frame: at 640x448 the draw (simulation + raster
  + HUD) took 8–13 ms and `present` 0.6–1.1 ms; at 1440x1080, 26–43 ms and 3.6–4.5 ms; at 1920x1080,
  37–55 ms and 4.2–6.4 ms. So the software rasteriser is the cost, not the socket. **The native frame
  does not fit 60 Hz on this machine.** The frame clamp was not touched: it belongs to the simulation.
  A slow frame is a longer step, not a slower game.

## Not verified / not 1:1

- **The pillarbox bars on screen.** The tiling WM never left the window 16:9 here, and a screenshot of
  the window can't be taken from a session (grim grabs the lock screen). The centring is proved only
  by the `PutImage` header test. That the server paints the bars black, and clears them on a resize,
  is X11's background/bit-gravity behaviour, not observed.
- **A resize by hand** (dragging an edge) was not done: no input reaches the desktop from a session.
  The WM's own tile-resize at startup is the only live resize seen.
- **Screen size on more than one monitor:** the setup's screen is Xwayland's root, which spans every
  output, so an untiled window could open wider than one monitor. Not tested (one monitor here). The
  WM's ConfigureNotify corrects it as soon as it places the window.
- **fps under load:** measured with the other slots running (load 15–19). An idle machine would be
  faster, so the numbers are a floor, not the figure on an idle machine. 1440x1080 would need about
  2.4x to reach 60 Hz, which an idle machine is unlikely to give.
- `--scale N` without `--size` now draws at 1/N of the window. That is my reading of "`--scale` stays",
  not something the brief spells out.

## Next

The human: `play.sh` (main checkout, git-excluded, not touched) still passes `--scale 2`. With this
change that is a half-resolution frame doubled, the same blow-up. Drop `--scale 2` to draw at the
window's pixels, or keep it (or use `--size 640x448`) while the native frame is under 60 Hz.
A rasteriser speed-up (threads or bands, the frame is embarrassingly parallel) is what would make the
native frame fit. That would be its own task.
