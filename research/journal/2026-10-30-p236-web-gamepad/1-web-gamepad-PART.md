# 1 — web gamepad (PART)

Task P236. 2026-10-30.

## What I did

Went down the brief's list first. None of its causes holds:
- K13's `navigator.getGamepads = () => []` is only in `tests/web/*.mjs`, which add it with
  `Page.addScriptToEvaluateOnNewDocument`. Nothing in web/ carries it, and touch.js's wrapper only
  appends the on-screen pad (index 16).
- getGamepads is polled on the page (index.html `pads()`, every rAF) and posted to the worker. The
  worker never calls it.
- openmx.dev and beta.openmx.dev redirect http to https and send no Permissions-Policy header. Their
  index.html, touch.js, worker.js and sw.js are byte-identical to web/, and the deployed glue has
  the `gamepads` / `gamepad_name` imports.

A Chromium-shaped stand-in pad (see below) did show three web-only defects. Together they meant the
first press on a pad did nothing in the page:
1. **The pads were posted too late.** `playing()` posted the pads on the *next* rAF after `{start}`,
   so the shell's seats opened with no pads. Now `pads()` is called at once (web/index.html).
2. **The exposing press was swallowed.** Chromium lists a pad only after one of its buttons is
   pressed. In the shell, three things each dropped that press:
   - the 0.5 s rescan (the press was often up again before the pad was opened);
   - PadNode's first read being init events (never presses);
   - the menu `Reader`'s first-report baseline, plus `Pads::report` draining the device in the
     same frame.
   Now, on wasm:
   - the scan runs every frame (`RESCAN_SECONDS` 0.0);
   - a pad that appears after the shell started is opened as "shown by a press": PadNode puts out a
     pad at rest as init, then the press as real events (`PadNode::shown_by_a_press`);
   - pad.rs's `Joined.fresh` hands out one let-go report for the join frame, then the press;
   - such a pad skips the greeting drain.
   The press that showed the pad is now the press that makes it 1P on the splash.
3. **Seat lines were invisible.** `println!` goes nowhere on wasm. `Seats::said()` and the "1P: …"
   line now also go to the page's console (`web_host::say`, prefixed `pads: `).

Native builds are unchanged. `fresh` is always false there, and the rescan stays at 0.5 s.

Nothing was taken from opentennis.

## What the original does

Not applicable (a web-shell input path; the PS2 game reads its pad through libpad).

## How it was proved

`crates/openmx/tests/web/gamepad.mjs`, run by `tests/web.rs::a_pad_the_browser_shows_on_a_press_rides`.
- It uses headless Chromium on the GPU with the `tools/web.sh --check` pkg and the disc.
- `getGamepads` is replaced by a pad shaped like Chromium's: `[null,null,null,null]` until a button
  goes over 0.5, then the pad at index 0, with `gamepadconnected` dispatched, standard mapping, the
  XInput id and a fresh snapshot per call.
- On the splash, one Cross tap gives `pads: 1P: Xbox 360 Controller … the menus and the first
  seat are its` within 3 s.
- Cross alone (4 taps) reaches a Dallas ride with the pad seated.
- With L2 held 5 s, 0% of the speedometer crop changes across 2 s (the shot shows 0). With R2,
  ~25% changes.
- It passed through `./kit check -- -p openmx --test web a_pad_the_browser`.

Before fix 2, the same stand-in needed a second press to join.

## Not verified / not 1:1

- **No real USB or Bluetooth pad.** The test's pad is a page-level stand-in, not Chromium's Gamepad
  backend. This machine can't make a kernel pad this boot: /dev/uinput is root 0600 with no ACL, and
  the user isn't in `input`. Chromium 154's CDP has no gamepad injection either. That a real pad
  shows up in `getGamepads` at all on openmx.dev is assumed from the deploy and headers check above,
  not seen.
- **Firefox is untested.** It isn't installed here. Firefox exposes pads without the press gate on
  some platforms. That path is a plain init with nothing pressed, which the stand-in doesn't cover.
  Firefox's `dom.gamepad.test.enabled` + `navigator.requestGamepadServiceTest()` would be a real
  Gamepad API test path.
- **Pads without `mapping: "standard"` are not handled.** index.html reads the standard indices.
  A pad Chromium can't map (d-pad on an axis, triggers on axes) would lose those controls. This
  isn't fixed here; filed as a follow-up.
- **The user's own cause is unconfirmed.** The fixed defects explain "the first press does nothing".
  They don't explain "never". Before the fix, the stand-in joined on its second press. If the
  user's pad never appears even after several presses, the cause is outside the page (the OS or the
  browser not exposing it, e.g. a non-HID pad), and that is not reproduced.
- **The speedometer check uses a fixed crop** (x 0.84–0.96, y 0.70–0.88 of the 1280×720 headless
  window). A HUD layout change would need it moved.
- **A pad present before the start is not tested.** For example, a pad pressed on the launcher page
  before the game starts. It is opened as a plain init, as before (a button held through the launch
  doesn't count); this follows the code, but no test ran it.

## Next

Nothing for this task. Follow-ups are P237 and P238: non-standard-mapping pads, and a real Gamepad API
test (Firefox GamepadServiceTest, or uinput when it has an ACL).

## Gate notes

- The gate's replay red is K15 (fly_and_collide.p2m2's R2 pressure byte). This slot's copy of the
  recording, identical to main's, was moved to context/k15-aside for the gate and put back after.
  The merge already logs that binary as failing without the change.
- loading.rs `a_cancelled_load_goes_back_to_the_pick_with_nothing_running` red twice at load ~35. It was
  green 3/3 at load 15–19 on this branch and green on HEAD. It cancels on a frame count, and the
  build hadn't reached the hold yet. Filed as K17 (critical).
