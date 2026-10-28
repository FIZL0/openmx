# C9 — freestyle event capture

2026-10-28. Task: `./kit plan C9`.

C9 checks S92's freestyle event (`Competition`) against a live one. Free09's stunt competition was
ridden twice in PCSX2 and its activity, manager and card were captured every frame. In
`crates/game-sim/tests/event.rs`, every row of both runs equals the port after some tick, or is
read mid-tick between two port ticks:
- 3703 event ticks each;
- the countdown, the 120 s clock, the card's out, the late *all done* and the end;
- the join's saved total (1234) and the total put back (1364.0, bit for bit);
- the verdict (Lose).

`event.rs` was not changed. The warning, Win/Tie and two riders were not captured; see
1-freestyle-event-capture-FINAL.md.
