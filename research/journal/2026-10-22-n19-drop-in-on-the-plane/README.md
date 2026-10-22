# N19 — drop in on the plane

2026-10-22. Task: `./kit plan N19`.

A player whose setup picked the biplane can now drop in on a ride opened online. The host reads the
machine off the joiner's pick and puts the plane, standing in the joiner's bay, into the state every
machine is sent (`rides::spawn_flying`). The shell no longer refuses the drop-in. Proved over real LAN
networks (both machines seat the joiner flying from its bay and stay in step) and on `Game::seat`
(the lane equals N18's start).
