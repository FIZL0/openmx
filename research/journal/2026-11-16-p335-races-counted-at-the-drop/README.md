# P335 — races counted at the drop

2026-11-16. Task: `./kit plan P335`.

The riders' records now count races, so the win percentage is wins over races, not 100 or the FPU's
largest float. The original counts a race at the gate's drop (`race_drop`, the only stat-9 call), not
at the finish; that loop is ported as `Records::count_race`, oracled over 20,000 fronts, and the shell
calls it when seat 0's gate opens. Not ridden end to end in a menu race. Details in 1-…-FINAL.md.
