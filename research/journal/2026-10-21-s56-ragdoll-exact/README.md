# S56 — ragdoll exact

2026-10-21. Task: `./kit plan S56`.

S56 recorded four wrecks of the original (over the bars, a side crash, a loop-out, a tip-over) and
replayed each through the original's own ragdoll tick in the oracle.

- **Fixed:** the first divergence was VU0's `vsqrt`, which PCSX2 chops and ps2-float rounded to
  nearest. With that fixed, and the rest of the game's random draws found tick by tick, the oracle
  reproduces every bone bit-exactly from each wreck's state until the bike first touches the rider.
  In the earlier captures, with no bike contact, it ran through respawn.
- **Not done:** the port is not compared. It lacks the per-tick mode machine (S60) and contact with
  the moving bike (S61). The task is blocked on those two.

Details: 1-ragdoll-exact-PART.md.
