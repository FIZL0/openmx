# 1 — firefox gamepad (FINAL)

Task P245. 2026-11-02.

## What I did

- Of the brief's two ways, uinput is still closed: `/dev/uinput` is `crw------- root root`, no ACL, and the
  user is not in `input`. (A uinput pad would also reach every other slot's browser, K13.) Firefox isn't
  installed, so `tools/firefox.sh` fetches Mozilla's 159.0 linux-x86_64 tarball once, checks its SHA-256,
  unpacks it under `~/.cache/openmx/firefox-159.0` (`$OPENMX_FIREFOX_DIR` overrides it) and prints the
  binary's path. Its own build, not a distro's, so the test pref is there and the version is fixed.
- `navigator.requestGamepadServiceTest()` needs more than `dom.gamepad.test.enabled`. Firefox 159 throws
  NS_ERROR_UNEXPECTED unless `xpc::IsInAutomation()`: the
  `security.turn_off_all_security_so_that_viruses_can_take_over_this_computer` pref plus
  `MOZ_DISABLE_NONLOCAL_CONNECTIONS=1`. Under that variable any connection off the machine aborts the browser
  (seen: aus5.mozilla.org for updates, update.googleapis.com for GMP). So the profile's user.js turns off
  updates, telemetry, normandy, safebrowsing, GMP/EME, captive portal and connectivity checks, and points
  remote settings and a proxy (with `network.proxy.failover_direct` false) at 127.0.0.1:9.
- The service only answers once the page is a gamepad listener (P236's poll is). `addGamepad(id,
  standardMapping, noHand, 17, 4, 0, 0, 0)` gives index 0; buttons go through `newButtonValueEvent`. Firefox
  then lists the pad, fires `gamepadconnected` and snapshots it itself.
- `firefox_gamepad.mjs`: the same page server and flow as P236's `gamepad.mjs`, but over BiDi
  (`script.evaluate`, `input.setFiles`, `browsingContext.captureScreenshot`; `log.entryAdded` carries the
  worker's console too, so the `pads:` lines are heard). The pad is added on the splash after the song,
  `listed` is read a second later, and then the first Cross is pressed.
- One difference from gamepad.mjs: the gas reading compares the braked picture (speed 0) with one taken
  3.5 s into R2, not two pictures on the gas. In software the ride is slow, and two such pictures 2 s apart
  read 0.055 once, just over the 0.05 floor (one speed showing in both). The new way read 0.107–0.108.
- Two page bugs found, not fixed here (they belong to their own tasks):
  - **B13**: with a display (the gate's private X, or :0), headless Firefox offers WebGL2. `web_gpu.rs`
    `webgl()` takes the opaque context, then wgpu's `create_surface` asks again and Firefox throws
    `InvalidStateError: OffscreenCanvas.getContext: Cannot update created context to different thread`.
    The software fallback's `getContext('2d')` then throws the same on the taken canvas, and the worker
    dies with no ride. The test sets `webgl.disabled` so it draws in software either way. Drop that pref
    once B13 is fixed.
  - **B14**: `picked()` awaits `navigator.storage.persist()`. Firefox makes that a permission prompt, so
    headless it never resolves and the game never starts. The profile grants it with
    `dom.storageManager.prompt.testing{,.allow}`.

## What the original does

Not applicable (a browser input path; the PS2 game reads its pad through libpad).

## How it was proved

- `./kit check -- -p openmx --test web -- a_firefox_pad_rides a_pad_the_browser_shows`, run three times
  inside the gate (its private display exported). The first run was red on B13, before `webgl.disabled`;
  the next two were green: 2 tests passed each time, P236's chromium test included.
- By hand, 4 runs of `firefox_gamepad.mjs`. One used the old gas reading (green, gas 0.055). One with
  `DISPLAY=:0` and no `webgl.disabled` was red on B13. Two were green with the final driver, one with
  `DISPLAY=:0` and one without: `listed: false`, `first: true`, 3 taps to a seat, `braked: 0`, gas
  0.107–0.108.
  About 20 s a run once the module is built. Looked at the R2 picture: Dallas, the HUD's speed reads 39.

## Not verified / not 1:1

- **Still not a kernel pad.** The test service puts its pad into Firefox's parent-process gamepad service,
  in place of a platform backend (evdev on Linux). Firefox's evdev reading of a real pad, and its mapping
  of one, are not run. uinput would need root or an ACL here.
- **Chromium has no such service.** Chromium's real Gamepad path is still covered only by P236's stand-in.
- **Software drawing only** (B13). Firefox on Linux has no WebGPU, and its WebGL2 path is broken in the page.
- **Firefox's "plain init" path is not reached.** P236 thought Firefox might list a pad without a press.
  This headless 159 on Linux did not: the pad stayed out of `getGamepads()` until the first press, and the
  test now asserts that (`listed`). Other platforms and builds are not tested.
- The speedometer crop is gamepad.mjs's fractions. The Firefox picture is 1280×634 (`--height` is the
  outer size); the digits still fall inside the crop.
- The first run of the Rust test downloads about 80 MB from ftp.mozilla.org. Offline with nothing cached,
  the test skips and says so.

## Next

Nothing for this task. B13 and B14 are filed.
