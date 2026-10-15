# C4 — c4 first capture

2026-10-15. Task: `./kit plan C4`.

The project's first live recording: 600 consecutive engine frames of the main menu to
`context/captures/timing-menu-600.csv`, one row per frame, no gaps, every row self-checked against R3's
model of the engine tick. The watch list is R3's timing state rather than R6's actor table, because R6 is
not done. The capture that costs the most time was found on the way and is written up in full: clocked off
the counter `./kit probe vsync` picks — which counts NTSC *fields*, while this game's engine ticks at 30 fps
— the recorder produced 600 clean-looking rows spanning 1199 ticks, every other frame missing, exit 0. A
stalled capture (570 of 600 rows) also exits 0 with `"suspect": false`; only a killed emulator makes the
tool shout `CAPTURE FAILED` and exit 1, which it does. Six kit defects are reported in the entry.

Details in `1-c4-first-capture-FINAL.md`.
