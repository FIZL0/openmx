# P9 — p9 analog throttle

2026-10-17. Task: `./kit plan P9`.

P9 made the throttle a pressure instead of a switch, in all three places that needed it: the pad module
now fills the DualShock 2 report's per-button pressure bytes from whichever joystick axes a device
publishes them on (`--pad-pressure ds2@8`), the shell can ride gas on a trigger's travel
(`--throttle r2`) or on a keyboard key that ramps open over 0.3 s instead of stepping, and the human's
recording was counted tick by tick to see what the original was actually given. That count is the
surprise: across the 1,810 ticks the game reads, every pressure byte is 0 or 255 — the human's minute is
a switch, 18 edges in nine stabs, and the only part throttle the bike ever saw is the 54 ticks (3%) that
`Bike\Race125P.phy`'s `GasRateLimit = 10` spends catching up after each edge. A PCSX2 recording can only
carry what the host pad reported, so that bounds the recording, not the original — `CONTROL_MAP` marks
Cross analogue, and the new pressure path is what lets a real pad use it. Details, numbers and the
unproven parts (chiefly: no pressure-reporting device exists on this machine, so the mapping is tested
only in unit tests) in `1-p9-analog-throttle-FINAL.md`.
