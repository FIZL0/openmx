# R35 — a backflip stopped short, in the original

Journal: research/journal/2026-11-06-r35-partial-backflip-score. Proof:
`a_backflip_stopped_short_scores_nothing_as_the_original` in crates/game-sim/tests/score.rs, which reads
`context/capture/r35/{stage0,stage1,stage2,round}.csv` and skips without them.

1. Copy recording2 and its state from the main checkout into `context/recordings/r35/` (two commands:
   the big-file guard refuses a `.p2s` path in a compound one), `chmod u+w` them.
2. `python3 -I research/r35/make_tape.py recording2.p2m2 fNNN.p2m2 --from NNN` in that folder, for NNN
   272 (stage0), 300 (stage1), 330 (stage2), 360 (round), and link
   `fNNN.p2m2_SaveState.p2s` to recording2's state.
3. Start the copy with `WAYLAND_DISPLAY=wayland-1 XDG_SESSION_TYPE=wayland QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
   QT_ACCESSIBILITY=1 ./kit pcsx2 start`; under load set the copy's ini `NominalScalar = 0.12` first
   (the test refuses a capture with a gap). Then per tape
   `STALL=30 ./kit pcsx2 with -- bash research/r35/play.sh PID context/recordings/r35/fNNN.p2m2 2374 330 context/capture/r35/NAME.csv`
   (PID: `./kit pcsx2`'s). `VIDEO=<abs path>.mp4` films it (two video frames a tick).
4. `python3 -I research/r35/summary.py context/capture/r35/NAME.csv` prints the watch, wreck and card
   changes.
