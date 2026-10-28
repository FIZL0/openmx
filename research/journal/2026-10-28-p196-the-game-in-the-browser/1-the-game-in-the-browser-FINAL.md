# 1 — the game in the browser (FINAL)

Task P196. 2026-10-28.

## What I did

The page no longer draws one still: its Ride (or a freshly picked disc) runs the desktop's whole
shell in the worker — the press-start splash, the main menu with its attract camera, Free Ride's
track and bike pick, the ride (bike, rider, chase camera, HUD, radar, pause menu), and the menu's
Online / Garage / Mods / Settings / Quit, all the desktop's own code.

- `web.rs` mounts `main.rs` (and so every shell module) as `shell` on wasm32 only and exports
  `openmx_run`. The native rlib the tests link is as it was.
- The shell's frame loops (`run`, `front_end`, `ride`, `load_tick`, `loading_frame`) are `async`;
  each frame ends in `pace::shown().await`. On the desktop that is a ready future and `main` blocks
  on `run` with pollster, so nothing moves; in the page it is the worker's next animation frame
  (`web_host::next_frame`), which hands the browser the turn — net tasks (spawn_local) run there.
  Same 30 Hz sim, same fixed-step pacing and catch-up cap.
- What the page has instead of the desktop's: `web_host.rs` (wasm-bindgen host calls: canvas size,
  keys as X keycodes, pointer presses, gamepads as Linux js events through pad.rs's own mapping,
  sound, files, frame clock), `clock.rs` (web-time), `thread.rs` (jobs run inline), `files.rs`
  (the shell's file writes go to OPFS `files/` through the worker; read back from a Map loaded at
  start), `web_gpu.rs` (no GPU window yet: P48).
- Sound: `audio::Output::start` in the page registers its mixer with `web_host`; each frame
  renders up to 0.1 s ahead of what the page has queued and posts f32 stereo to the page's
  ScriptProcessor (48 kHz AudioContext opened on the Ride click, so autoplay allows it).
- Canvas: CSS fills the window (100vw × 100dvh, no letterbox); the shell is told the window's
  shape and draws a frame of that shape capped at 640×448 pixels (`web_host::PIXELS`), stretched.
  Resizes and orientation changes are re-sent.
- The page's HTML menu (Ride / Join / Host / Files / Full screen) is gone; the stage has one Ride
  and the site parts (pick disc, forget it, downloads). `?join=` / `?host=` URL mode stays for
  `tests/netplay_browser.rs`.
- Tests: `tests/web.rs` knows the 14 host calls and the `openmx_run` export;
  `tests/web/host.mjs` supplies the new calls (inert) for parity. Tests that `#[path]`-mount shell
  modules now mount `clock.rs` / `thread.rs` / `files.rs` too (std re-exports natively).

## What the original does

Not a port of original behaviour; host plumbing only.

## How it was proved

Headless chromium over CDP (scratch driver, worker attached), the user's disc picked through the
file input: splash with attract camera → Enter → main menu → track list → Dallas ride; holding J
(gas) the speedo read 2 → 19 → 29 → 38 over 6 s and 60 on the final build; HUD, radar and pause
menu drawn. Screenshots looked at by eye. `./kit check --all` green (2842).

## Not verified / not 1:1

- Frame rate: the ride draws ~2 fps in headless chromium at 640×448 (menus ~15 fps). The
  software ground's splat paint is ~70% of a frame on one thread. The sim stays real-time by the
  catch-up cap, so it plays like a slideshow. Needs P48 (GPU on the canvas) or a parallel software
  ground. wasm simd128 gave nothing measurable and was dropped.
- The frame is capped at 640×448 pixels and stretched to the window, so it is soft on big screens.
- Online Join/Host from the game's menu in the page: not tried (the lobby's net tasks should run
  between frames, but no test drives it).
- Gamepads: no gamepad in headless chromium; the mapping is the standard-mapping table to Linux js
  events, untested on a real pad.
- Sound: the samples flow (page queue advances) but nothing was listened to.
- Settings persisting in OPFS across a reload: not checked. In the headless run OPFS was the
  no-sync-handle path.
- World mods, texture packs, music folders: the page has no mods folder. World-mod streaming
  (worlds.rs, world_dress.rs) and track preview stills still `std::thread::spawn` and would panic
  in the page if reached.
- Loads build inline (no threads), so the loading screen does not animate while a track builds.
- Only Chromium headless; Firefox and Safari not tried.
- Keyboard mapping is KeyboardEvent.code → evdev → X keycode for the common keys only.

## Next

Follow-ups filed: browser frame rate, mods in the page's storage, online from the menu in the page.
