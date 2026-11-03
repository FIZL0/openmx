# S253 — the drawn rider's lean in hard turns

2026-11-03. Task: `./kit plan S253`.

**The finding.** The hard-turn lean ratio of 0.80 had two causes. The riding ragdoll's are fixed here;
the chassis' are split off.

- **The lean.** `Riding::ride` hung the target skeleton under `holds.root × before`: the drawn bike's
  root node, then the ride. The body's own root joint already carries its clip's lean (−1.6° of roll
  at rest, and the crossup in a whip). So the target counted the root's roll twice. It now hangs
  under `before`, the bike's transform. The request frame stays `holds.root × m` (S111).
- **The root.** The chassis is posed by the body's sixty-state machine with its lags
  (`vehicle.rs`'s V13 note). The original's bike is posed by the motion driver
  (`game_sim::motion`, the rider's `animation`). Posed that way through `anim::Rig`, the root
  rolls 1.02 of the original's and keeps the original's 1.49° rest roll (Mid@0.5's root up is
  (0.026, 1.000, −0.008), S8's). This is follow-up work: moving the chassis also moves what poses
  the body.

The details and the figures are in `1-the-lean-FINAL.md`.
