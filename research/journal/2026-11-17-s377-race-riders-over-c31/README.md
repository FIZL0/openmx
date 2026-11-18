# S377 — race riders over c31

2026-11-17. Task: `./kit plan S377`.

S377 runs the Sim's race laps and AI hand-off over C31's whole live sx01 race. The resync run (it restarts
from the capture at each lost tick) matches every card word bit for bit on 9872 held ticks. The free run
(seeded once) matches order, places, laps, clocks and flags on 10,785 held ticks. `Pilot::read_laps` and
`Racing::of` agree on every one. `Sim::load_ai_net` now measures the AI net's lap length, as the image
holds it. Gap left: the Sim's card owner doesn't drop the jump watch during a wreck (follow-up S429).
