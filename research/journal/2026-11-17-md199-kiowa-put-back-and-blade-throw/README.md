# MD199 — kiowa put back and blade throw

2026-11-17. Task: `./kit plan MD199`.

MX vs. ATV's Kiowa put-back placement and blade throw, oracled against the port over 10,000 cases each.
Both differed only in the body's centre: SLUS-21104 adds the turned centre of gravity, MX Unleashed does
not. Ported behind a new `Wreckage::mxvatv` flag, now bit-exact. The put-back's ground snap is not
ported, so the flag stays off in the game for now (MD214). Details in `1-…-FINAL.md`.
