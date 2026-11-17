# R20 — sim thread on a window

2026-11-17. Task: `./kit plan R20`.

R20 asked for N23a's `--sim-thread auto` to be timed on the Steam Deck. The standing rule is that runs never
use the Deck (user, 2026-11-02: the Deck profile was a one-time test), so I timed it in a real window on this
desktop instead. Auto engaged in every windowed run. But the picture alone took 36–124 ms against a 16.7 ms
refresh, and the machine's load (11–50) moved the frame counts more than the thread could. So no
frames-saved number came out. All 16 windowed runs, off and auto, gave the same ride hash. Details in
1-sim-thread-on-a-window-FINAL.md.
