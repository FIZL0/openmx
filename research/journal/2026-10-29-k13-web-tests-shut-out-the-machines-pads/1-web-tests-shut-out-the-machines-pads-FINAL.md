# 1 — web tests shut out the machine's pads (FINAL)

Task K13. 2026-10-29.

## What I did

- Found what `-p openmx --test web` races: **the machine's own gamepads**. Headless chromium exposes every pad
  in `/dev/input` to the page through `navigator.getGamepads()` once that pad presses a button. Other slots'
  `./kit pad` (a uinput "ps2kit pad N" per PCSX2 copy, pressing buttons whenever an agent drives its
  emulator), or a pad in the user's hand, therefore reach every page a test opens. `web/touch.js` hides the
  touch pad the frame a real pad is connected (`want = riding && touched && !anyReal()`), so
  `the_touch_pad_rides_on_a_phone` read `"shown":false` — the failure recorded on 10-28 (S215, "a started game
  on a touch screen shows the pad") and the one in P212a's merge gate. Alone, nobody else's pad was pressing.
- Reproduced it: a python-evdev uinput pad (vendor 0x1209, like `ps2kit/pad.py`'s) pressing BTN_A every
  100 ms beside `node tests/web/touch.mjs` gives `"shown":false,"clear":false,"small":10,"steer":null`.
  An idle pad (no presses) does not — the Gamepad API hides a pad until it is used — which is why it came
  and went with the other slots' activity rather than with load as such.
- Fixed in every chromium driver under `crates/openmx/tests/web/` (touch, shots, gpu, store, update, chromium,
  page — the last also used by `netplay_browser.rs`): right after `Page.enable`, before the first navigation,
  `Page.addScriptToEvaluateOnNewDocument` replaces `navigator.getGamepads` with `() => []`. `touch.js` binds
  its `realPads` to that stub at load, so neither the touch pad's visibility nor `index.html`'s pad loop (which
  sends presses on to the worker as the rider's input) sees the machine's pads. The other drivers did not fail
  on it yet, but a stray Cross from another slot's pad could move their menus (store's card, update's
  Square) the same way. No timeout was touched; no product code changed.
- opentennis: nothing taken — its page tests do not drive chromium with a touch pad.

## What the original does

Not applicable: test tooling, no game behaviour.

## How it was proved

- `touch.mjs` with the pressing uinput pad alive, before the fix: `"shown":false`; after: `"shown":true` and
  every other field as the test wants.
- `./kit check -- -p openmx --test web` with the pressing pad alive for the whole run: `TESTS PASSED: 7 tests`.

## Not verified / not 1:1

- P212a's gate log was overwritten before I looked (merge/context/notes/check.log holds a later run), so which
  of the binary's tests failed there is inferred from S215's identical recorded failure and the reproduction,
  not read from that log. If the web binary reds again under load with something other than the touch pad's
  fields, it is a second cause.
- `the_browser_rides_on_the_gpu` still asserts a median of ≥30 fps over wall clock; under a heavily loaded
  machine that is a genuine timing assertion, not a shared-resource race, and I left it alone (it did not fail
  here). The drivers' 20 s "chromium gave no DevTools address" waits are likewise untouched.
- The disc-needing tests (gpu, store, update) had the disc here (this slot's kit.toml iso links to the main
  checkout's), so they should have run, but a green `./kit check` prints no log and I did not see their output.

## Next

Nothing; done.
