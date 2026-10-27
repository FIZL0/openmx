# S156 — 2p wreck thresholds

2026-10-27. Task: `./kit plan S156`.

This task measured how sensitive wrecks are against the original. The original's wreck test has 20
clauses, and each one's threshold is tabled with its [Wrecks] key and bike offset.

tests/wreck_thresholds.rs proves three things:

- Both 2p bikes hold the limits the port reads from Race500P.phy, bit for bit.
- Every clause's quantity set on its limit rides out and one float step past wrecks: 57 cases, the
  original's bike_wreck_test vs the port, on a real 2p bike. Every comparison is strict.
- C10's 2p ride sits on the right side of every limit. The human rode out 79.68° nose down, 84.48° nose
  up and 84.85° of roll in the air, then wrecked a tick later at 80.53°, 85.33° and 85.03°. A landing at
  85.95° of roll rides out, because there is no roll clause on the ground.

No gap was found, so there is no simulation change. For S144: the wreck test has no pitch-rate or
speed clause, so too-generous tail landings would have to come from the ride's loads, not the
thresholds. Details in 1-2p-wreck-thresholds-FINAL.md.
