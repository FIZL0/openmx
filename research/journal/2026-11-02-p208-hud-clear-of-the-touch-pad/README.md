# P208 — the ride HUD and the menus' hints clear of the touch pad

2026-11-02. Task: `./kit plan P208`.

With web/touch.js's pad up on a phone, the speedometer sat under GAS, the radar under the stick's resting
base and the front end's hints under BRAKE/GAS. The canvas's poll now tells the shell when the page lists
the pad (`Touch controls`) and over what window, and those three, plus the band's line, move clear of the
pad's controls. Checked in tests that lay the controls out from touch.js's own numbers, and on the real
page in headless chromium at 844x390. Details are in the numbered file.
