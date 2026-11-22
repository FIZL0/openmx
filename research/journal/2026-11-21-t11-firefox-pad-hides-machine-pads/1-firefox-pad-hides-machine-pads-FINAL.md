# 1 — firefox pad hides machine pads (FINAL)

Task T11. 2026-11-21.

## What I did

- Found what `-p openmx --test web` raced in P324's merge gate: `a_firefox_pad_rides` failed on
  `listed: true`. Its page log had `pads: 2P: 1209-5002-ps2kit pad 2 (gamepad/0)`: another slot's
  `./kit pad serve` uinput pad. Firefox lists every evdev pad on the machine, and since /dev/uinput opened
  up to the user on 11-21, other slots' pads exist while the gate runs. One that is listed or pressed
  shows before the test's own press, or can take a seat before the test pad does.
- `tests/web/firefox_gamepad.mjs` now starts Firefox under bubblewrap with an empty tmpfs over /dev/input
  (and over /run/udev, the udev database), the rest of the machine bound as it is. Firefox then sees only
  the pad from its own gamepad test service. Without bwrap it runs as before and says so on stderr.
- No timeout widened.

## What the original does

Not about the original: a test harness fix.

## How it was proved

- Reproduced the race: served this slot's pad (`./kit pad serve`, copy 2, `ps2kit pad 2` in
  /sys/class/input) and pressed its Cross every 2 s.
  - The script as it was on main, run by node under those presses: `pads: 1P: 1209-5002-ps2kit pad 2 …
    the menus and the first seat are its`, the test pad 2P, `"listed":true,"first":false,"seated":false`:
    the gate's failure and worse.
  - With the fix, `./kit check --rerun "-p openmx --test web -- a_firefox_pad_rides"` under the same
    presses: passed (gas 0.14, brake 0, webgl2).

## Not verified / not 1:1

- Machines without bwrap (or with unprivileged user namespaces off) still see their real pads; the script
  prints `no bwrap: …` there. This gate machine has bubblewrap 0.13.1.

## Next

None.
