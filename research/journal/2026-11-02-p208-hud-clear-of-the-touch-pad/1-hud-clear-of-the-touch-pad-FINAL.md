# 1 — HUD and hints clear of the touch pad (FINAL)

Task P208. 2026-11-02.

## What I did

- New `crates/openmx/src/touch_room.rs`, mounted in `menu.rs` as `menu::touch_room`. It goes there, not at
  the crate root, so every test that mounts `menu.rs` gets it with no new `#[path]` lines. It holds:
  - the window the pad is shown over (`set`/`window`). This is a thread-local: the page's shell is one
    thread (`thread.rs`), and a test thread that sets it races no other test.
  - `Room { window, frame }`. It maps touch.js's `u` (`min(7vw, 12.5vh)`) from the window's corners onto
    the frame, which is drawn centred and scaled to fit, so a barred Picture Shape still lines up. It
    gives three clearances: `over_the_pedals` (above MODIFIER, left of □ and BRAKE), `left_of_the_pedals`
    (left of MODIFIER, up to GAS's top) and `right_of_the_stick` (past the resting base). Each control
    gets touch.js's 0.15 `u` hit slop plus 0.2 `u`.
- `web_host.rs` `Canvas::poll`: `touch_room::set` with the canvas's size while a pad named
  `Touch controls` is listed (`pad::is_touch`). touch.js lists the pad only while it is shown. I used this
  pad-name check, not a new host call, so the page needs no change.
- `ride_hud.rs`: new `sprites_clear_of(…, pad: Option<Room>, …)`. `sprites` calls it with `None`, so its
  output is unchanged. It draws the radar and the speedometer once aside, measures their sprites' box,
  and then draws them in a view moved just far enough. The radar moves right of the stick and keeps its
  height. The speedometer moves up over MODIFIER and left of the faces. Both styles move, the dial and
  the readout, with the preload meter. A view whose corner is not the frame's (a split) is not moved.
  `main.rs`'s GPU HUD calls it with `Room::of(frame size)`.
- `menu_screens.rs`: the hint tab's hints end left of the pedals (the tab itself still runs to the
  frame's edge, under the translucent buttons), and the band's line starts right of the stick. New
  `menu::Part::Hints` and `Part::Line` say where those went. `tests/menu.rs`'s one exhaustive `Part`
  match gained the arm.

## Taken from opentennis

Nothing: its touch.js has no HUD in the shell to move (its HUD is the page's).

## How it was proved

- `tests/ride_hud.rs` `the_speedometer_and_the_radar_stand_clear_of_the_touch_pad`: 844x390, 932x430,
  1180x820 and 1024x768, digital and analog. It lays the pedals, faces and stick out from touch.js's own
  numbers (not `touch_room.rs`'s). It checks that no HUD sprite crosses one with the pad up, and that the
  same HUD with no room made does cross one. With no pad (`None`) the output is identical to `sprites`.
  A half-height top view is not moved.
- `tests/menu.rs` `the_hints_and_the_band_stand_clear_of_the_touch_pad` (needs the disc; ran with it):
  every `Screen::ALL` at 844x390, 932x430 and 1180x820. The hints are clear of the right-hand controls,
  and the band's line is clear of the stick and inside the frame. The same screens with the pad down did
  overlap.
- `touch_room` unit tests: the places at 844x390, a scaled frame and a 4:3-barred one.
- `cargo test -p openmx --test ride_hud` 55 passed; `--test menu …stand_clear` passed;
  `cargo check -p openmx --tests`; `cargo build -p openmx`; `tools/web.sh --check` built.
- On the real page: `tests/web/gpu.mjs` with `OPENMX_GPU_PHONE=844,390,1` and the check build's pkg,
  WebGPU. On the splash (`OPENMX_GPU_ARGS=`), the hints "Launcher / Start" end left of MODIFIER. Riding
  (`--ride`), the radar is right of the stick and the readout and meter are over MODIFIER, left of the
  faces. The log said `pads: 1P: Touch controls (gamepad/16)`.

## Not verified / not 1:1

- The page's `env(safe-area-inset-*)` is not known to the shell, so a control is taken to stand from the
  window's edge. On a phone with a notch side inset, touch.js puts the pad further in than this allows
  for, by the inset (up to ~47 CSS px on an iPhone in landscape outside fullscreen).
- `vh` in touch.js is the layout viewport. The canvas is `100dvh`. On a mobile browser whose toolbar
  shows, the two differ a little, and `u` here is computed from the canvas.
- The software renderer's HUD (`ride_hud::draw`, the web's last fallback) is not moved. In a split its
  target is the view's own frame, so the frame's corners are not known there.
- The big screen title over the band's left end (e.g. "Settings") still sits partly under the stick's
  resting base: filed as F10. The iPhone app's own pad (touch_pad.rs) does not set the room: filed as F9.
- The CLUTCH/CAMERA/PAUSE pills along the top are not checked against the HUD (the task named only the
  bottom). The trick pop-up and the machine-switch prompt, low in the middle, were not moved, and I did
  not check whether a long pop-up line reaches MODIFIER.
- The original is not involved: this is presentation only, with no simulation or RNG touched.
