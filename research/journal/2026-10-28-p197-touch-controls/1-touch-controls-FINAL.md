# 1 — touch controls (FINAL)

Task P197. 2026-10-28.

## What I did

- `web/touch.js` (new): an on-screen pad over the canvas, a virtual standard-mapping gamepad (index 16, named
  "Touch controls") appended to `navigator.getGamepads()`. index.html already hands every
  gamepad to the worker each frame, so the shell reads it through its own pad layer (`web_host.rs` → `pad.rs`) with
  no Rust change. Laid out for the default (new) layout of `pad.rs`'s `Scheme`: left thumb stick (axes 0/1, centred
  where the thumb lands in the bottom-left 42 % × 78 %), GAS = R2 and BRAKE = L2 under the right thumb, the face
  diamond above them (menus' OK/back, ○ △ tricks, ✕ □ the trick pack's extras), CLUTCH = L1 and R1 at the top
  corners, CAMERA = Select and PAUSE = Start at the top middle. Safe-area insets on every edge.
  - Shown only once the game has started (`body[data-state="started"]`), on `(pointer: coarse)` or after any touch,
    while no real gamepad is connected; a hardware key hides it, the next touch brings it back. `?touch` forces it.
  - A short tap (< 350 ms, < ¼ of the stick's radius) on the stick's area is handed to the canvas as a pointerdown,
    which index.html turns into the shell's click, so the stick area hides no menu item.
  - Upright on a touch screen the game is covered by a "Turn your phone sideways" hint (plus "Share, then Add to
    Home Screen" on iOS Safari outside the installed app); the pad hides.
  - While riding: `user-scalable=no, maximum-scale=1` added to the viewport (restored on the site), overscroll,
    selection, callout, tap highlight, double-tap/pinch (`gesturestart`), `selectstart`, `contextmenu` all off;
    touchstart/move on the canvas and pad `preventDefault`ed; edge-swipe touches swallowed and a spare history entry
    so a back gesture does not leave the ride.
  - Audio: `AudioContext` wrapped (and `webkitAudioContext` used where it is the only one) so every context the page
    makes is resumed on each touchend/pointerup/keydown, with a one-sample silent buffer the first time (old iOS's
    unlock). iOS suspends contexts after calls/other apps; each tap resumes them.
  - Fullscreen on the first tap while riding, where the browser has it (`requestFullscreen`/`webkitRequest…`), then
    `screen.orientation.lock('landscape')` (Android); skipped in the installed app.
- `web/index.html`: `apple-mobile-web-app-capable`, `mobile-web-app-capable`, `black-translucent` status bar, the
  app title, an `apple-touch-icon` (`web/icon-180.png`, rendered from `icon.svg` with rsvg-convert: iOS takes no SVG
  there), and `touch.js` loaded as a classic script in `<head>`, before the module, so the AudioContext wrapper is in
  place. `web/manifest.webmanifest` already had `display: fullscreen` + `orientation: landscape` (P196's), left as is.
  `web/sw.js` caches `touch.js` and the icon.
- Test: `crates/openmx/tests/web.rs` `the_touch_pad_rides_on_a_phone` + `tests/web/touch.mjs`: headless chromium,
  `Emulation.setTouchEmulationEnabled`, an 844×390 mobile viewport; no disc and no module needed.

From opentennis (`web/touch.js`): the whole frame of it — the virtual gamepad in `getGamepads()`, per-finger touch
events by `identifier` (its B274), a press held until a poll has seen it a frame (B165), the finger-gone sweep on
`e.touches`, the selection/zoom/edge-swipe guards and the history guard, the AudioContext resume. Changed: one pad
(no seats, chat, vibration, labels from the game, menu mode); bike controls (pedals for R2/L2) instead of tennis's;
the stick floats in all screens and short taps become canvas clicks instead of the game telling the page it is in a
menu; a portrait hint instead of a portrait layout (the task asked for landscape); hiding on a hardware keyboard;
dropped opentennis's WebAudio start-time rescheduling (this page plays through one ScriptProcessor, not queued
buffers). The whole file is wrapped in a block: a classic script's top-level `const top` collided with `window.top`.

## What the original does

Nothing here: the page and its controls are the port's own front end (AGENT.md: menus and front-end are ours).

## How it was proved

- `the_touch_pad_rides_on_a_phone` (headless chromium, touch emulation, 844×390): no pad on the site; pad shown
  once started; all 10 controls inside the screen, none overlapping, none under 33 px; stick pushed right/down reads
  axes ≈ (0.97, 0.24) with no button; GAS → button 7, GAS+BRAKE with the stick still held → [6, 7] (three fingers);
  all released → axes 0, no buttons; a quick tap on the stick area → exactly one canvas pointerdown; a drag leaves
  scroll at 0 and the page no wider/taller than the screen; viewport says `user-scalable=no`; portrait → pad
  `display:none`, hint `flex`; a hardware key → hidden; a touch → back. Run through `./kit check` (its shots were
  written under `OPENMX_WEB_SHOTS`, so it did not skip).
- End to end with the real module and disc (a scratch copy of `chromium.mjs` with touch emulation, not committed:
  it copies the 2.2 GB disc like the ignored `the_page_starts_in_a_browser`): the shell listed "Touch controls
  (gamepad/16)" on its splash; PAUSE (Start) left the splash for the main menu, ✕ opened Free Ride's track list, ✕
  again started a ride on Dallas; GAS held 4 s took the bike to 44 on the speedometer, and the stick to the right
  leaned and turned it (it wrecked from the full lean). Looked at the screenshots.

## Not verified / not 1:1

- **Nothing was run on an iPhone or iPad, or in WebKit at all**: this desktop has only chromium. Every iOS point
  below is from the code and WebKit's documented support, not from a run.
  - OPFS in the worker: `worker.js` uses `createSyncAccessHandle` (Safari 15.2+) and WebKit's `move(dir, name)`;
    untested on Safari. Not changed by this task.
  - A 4 GB ISO: the worker never holds the disc in memory (the import copies 8 MB slices, play reads sector runs
    through the sync handle), so the disc's size should not matter to memory; Safari's OPFS quota for a ~2.2–4 GB
    file, the wasm heap limit iOS puts on a tab, and iOS killing a backgrounded tab mid-copy are all unmeasured.
  - WebAudio unlock: resume + a silent one-sample buffer on every touchend/pointerup; untested on iOS. The ringer's
    silent switch mutes WebAudio on iOS by default; `navigator.audioSession.type` is left alone (a game that
    respects the switch), unverified which a user wants.
  - Fullscreen: iPhone Safari has no element fullscreen, so there only the Home Screen app is fullscreen
    (`apple-mobile-web-app-capable` + the manifest); iPad's `webkitRequestFullscreen` path untested.
    `screen.orientation.lock` only works on Android in fullscreen; not tested on Android either.
  - Safe-area insets are in the CSS (`env(safe-area-inset-*)`), but chromium's emulation has no notch, so the insets
    were 0 in every run: the layout around a real notch/home indicator is unseen.
- Touch emulation in chromium is not a real phone: multi-finger timing, iOS's own edge-swipe and text-selection
  gestures (whether the `preventDefault`s really stop them), and the history guard were not exercised by a person.
- The pad's GAS and BRAKE are full on or off (value 1.0): a real trigger's part throttle/part brake has no touch
  equivalent here.
- Laid out for the default (new) controls: a rider who picked the original's layout in Settings has gas on ✕ and
  brake on □, which are on the pad but not labelled GAS/BRAKE. The page cannot read which layout the shell uses.
- The ride HUD (speedometer, minimap) and the menus' hint band sit under the pedals and the stick's rest spot;
  filed as P206.

## Next

Nothing for this task. P206 for the HUD overlap; a run on a real iPhone (the user) for the iOS points above.
