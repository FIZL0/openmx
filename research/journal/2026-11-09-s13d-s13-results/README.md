# S13d — s13 results

2026-11-09. Task: `./kit plan S13d`.

The race's results. Places, finish times, best lap and the tie draw were already ported (S294/S13c).
This task ported the supercross activity's two answers to them: `race_over` answers each card's finish
at the line (fireworks, podium flashes, a player's win, Hall of Fame entries), and `race_end` answers
the race's Over notice (it hands the results menu the score manager, sets the tip popup's words, picks
the outro scene). Part 1 had the fireworks firing at the race's end; it is the line finish. Proved by
three 20,000-case oracles with their mutants caught, and by a live sx01 finish (an AI win at the line,
then the human's card poked done) held bit for bit at the line and over the over tick's made-up
finish, tie draw, ranking and outro scene. Not live: a player's own finish (no ridden lap), the
second AI finish (lost in a recorder gap), and the presentation (P329, V127). Neither answer is in the
Sim yet (S301). Done: details in `2-s13-results-FINAL.md`.
