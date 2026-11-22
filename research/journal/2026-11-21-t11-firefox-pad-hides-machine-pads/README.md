# T11 — firefox pad hides machine pads

2026-11-21. Task: `./kit plan T11`.

`-p openmx --test web` failed under load in P324's merge gate: `a_firefox_pad_rides` saw another slot's
`./kit pad` uinput pad (`ps2kit pad 2`) in Firefox's `getGamepads()`, which can show before the test's press
and take 1P from the test pad. Firefox now runs under bubblewrap with an empty /dev/input and /run/udev, so
it sees only its gamepad test service's pad. Reproduced with this slot's pad pressed every 2 s: the old
script failed, the new one passed. Details in 1-…-FINAL.md.
