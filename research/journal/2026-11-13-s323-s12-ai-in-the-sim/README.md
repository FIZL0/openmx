# S323 — s12 ai in the sim

2026-11-13. Task: `./kit plan S323`.

The Sim gained an AI lane: `ai::Pilot` (S12e's spawn, read and take, plus the levers, recovery,
card and seat), `ai::Lane` on `Sim`, and `Sim::think` in the rider loop. Each pilot thinks against
the Sim's own world: the port's ground answers the probes, the port's broad phase is the scene walk,
and `sweep_object` against the port's props does the sweeps. Its controls go to the bike. AI riders
go through `gesture_gate`, and `plan_trick` runs inside the tick. It was proved on recording2's three
free09 riders over 60 ticks each, against the original's own `ai_update`. The ground and every brain
and handoff word matched bit for bit. Near those riders the static world is empty, so a second test
walks the original's interface on each of free09's 275 props: the fixed walk matched in every one.
The original's walk also hands out every moving object (bikes, ragdoll parts), which the lane's
scene lacks. That gap, and the Sim-stepped bodies, are unproved (file 1).
