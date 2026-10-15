# C5 — c5 pad

2026-10-15. Task: `./kit plan C5`.

The virtual pad reaches the game and the three places a press lives in RAM are now named in `kit.toml`
and written up in `docs/PCSX2.md`: libpad's double-buffered port buffer (active-low button word), the
game's 16 per-control records in `PSXControl` (held + strength + four ms timers, no bitmask anywhere),
and the one-frame event queue in the input manager that the front end reacts to. The answer to the
task's open question is that the game keeps **no** "pressed this frame" bitmask — a press is an event
consumed inside the same tick, and afterwards only the held timer reading 0 remembers it. Proved with a
300-frame capture (`watch/pad.toml`) in which seven scripted presses move the Training dialog's
selection on the press frame itself, 25/25 self-checks passing, plus a before/after screenshot pair.
Done; see `1-c5-pad-FINAL.md`, whose "Not verified / not 1:1" list is what I1 should read first.
