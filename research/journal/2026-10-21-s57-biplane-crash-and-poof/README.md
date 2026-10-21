# S57 — biplane crash and poof

2026-10-21. Task: `./kit plan S57`.

The user asked for the original's biplane crash, its poof and fade, and the same on L1/R1. Captured
live, the original's plane has no poof or fade. It crashes with a dust puff, tumbles 151 ticks, and
cuts hard back to its spawn. L1+R1 (on the wheels or wrecked) is the same cut, on the press frame.
The poof the user remembers is the bike's hand reset: the bike and the rider are drawn see-through,
then fade back in. This task ports the plane's chord (`Flight::reset_chord`, wired into `openmx`
under both pad schemes) with tests at both levels. The crash contact itself waits on S48. Follow-ups
are filed for the replay after S48, the bike's ghost fade and the particles. Details are in
1-biplane-crash-and-poof-FINAL.md.
