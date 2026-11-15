# S319 — rider statistics tracker

2026-11-14. Task: `./kit plan S319`.

The results menu's per-card statistics (laps led, times in the lead, passes, times passed, wrecks,
preloads, clutches) come from a tracker inside each motocross bike. Every writer of the seven counts and
their two latches was found, ported as `game_sim::race::Tracker` on each race card, fed from the lap and
place notices and from each bike's frame, and checked against the original's five writers with the
function oracle (tests/race.rs items 45–50). The longest jump and longest air time were already ported
(S302's `Jumps`); the results hand-over now fills both from the cards and the trackers. Done; see
`1-rider-statistics-tracker-FINAL.md`.
