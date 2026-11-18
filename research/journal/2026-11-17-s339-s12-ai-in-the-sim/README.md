# S339 — s12 ai in the sim

2026-11-17. Task: `./kit plan S339`.

The AI tick's free-ride path branch is wired: on `Seek::Path` `Racer::tick` runs the ported path
follower on the brain's net, with the target jump's setup leg as its goal, kept on the jump. The free-ride
case is a machine race (MachineRace hands its AI racer the `mr1.tdf` race file as net). It was proved bit
for bit on sx01's race image with poked goals (3,189 branches, 14 hand-overs). No machine-race image
exists yet (C41). Details in 1-…-FINAL.md.
