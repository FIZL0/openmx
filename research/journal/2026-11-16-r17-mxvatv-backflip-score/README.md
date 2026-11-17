# R17 — what MX vs. ATV scores its backflip on

2026-11-16. Task: `./kit plan R17`.

MX vs. ATV scores the backflip in the stunt start and end themselves. S223 read those score calls as
animation calls. The start pushes a node on the bike's score card worth the stunt table's PointValue
(2000), growing HoldPointValue (10 000) a second until the flip's end. The combo banks 0.2 s after
landing, at its sum times its node count, and each banked stunt's repeat scale drops ×0.8. This differs
from MX Unleashed's flat 10 000 watch, which the port still uses. Proved on state04 in
`tests/mxvatv_stunt_score.rs`. The port is filed as S340. Details in 1-mxvatv-backflip-score-FINAL.md.
