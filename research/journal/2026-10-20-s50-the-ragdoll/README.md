# S50 — the ragdoll

2026-10-20. Task: `./kit plan S50`.

The rider and the bike have no physics after a wreck. The shell draws both frozen at the ride's pose,
and `Crash` stands in for the rider with one falling point. The rider's ragdoll was too big for one
session, so it is split:

- **S50 (this task)** is the first part, `crates/game-sim/src/ragdoll.rs`: the eighteen bones'
  records read from `mrider.phy`, the riding entry, the wreck entry for every reason, and the damping
  switch. Each is proved bit-exact on the live rider body actor over 10 000 cases.
- **S51** is the step.
- **S52** is contact, plus the settle test on the real bones. That test is a clearance, not a rise.
- **S53** puts it in the ride and on screen, and holds it against the captured wrecks.

Details are in 1-the-ragdoll-FINAL.md.
