# S291 — s13 live gate

2026-11-16. Task: `./kit plan S291`.

A live PCSX2 capture of sx01 starts with riders teleported through the closed gate, first the human alone
and then all four riders with the card list relinked. Replayed bit for bit against `Race::tick`. It
settled two questions:
- the race manager's tick runs after every bike's update, so `Sim::race_frame` moved after the riders;
- several riders caught in one tick draw in the card list's order, so the watch now walks the laps' list.
Details in 1-s13-live-gate-FINAL.md.
