# S13c — s13 laps

2026-11-09. Task: `./kit plan S13c`.

Lap and checkpoint counting for a race. Part 1 ported and oracled the score card's lap line, then split the
rest into S292 (the other crossing tests), S293 (the race handler per card), S294/S294a (the line lists,
the finish and ranking) and C31 (two captured sx01 single races). Part 2 held the assembled per-card handler
against both C31 captures, every rider every tick, bit for bit on 23 card words: 16 laps, 9 best laps, 6
finishes, and the human's off-track clock with its 5 s limit. It needed no port change. Two exceptions,
both caused by unported helpers (S302): flag bits 0x18_3ce0, and the long-way search distance of a rider just
behind its kept spot. The HUD's white and checkered flags are presentation and went to P328.
