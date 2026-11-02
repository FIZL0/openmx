# S223 — backflip rider side

2026-11-02. Task: `./kit plan S223`.

MX vs. ATV's backflip, rider side. The stunt start answers yes on every path, so the clip always starts as
the port had it. What it changes is the rider's stunt id, now kept as `Flip::stunt`: set at the clip's start
unless a trick holds the word, and cleared as the flip ends. The air control reads that id, so it now
reads the stunt up to the landing, not just while the clip runs. The mid-clip wreck's reason test
(`flip_wreck_steep`, also the wreck test's reason 15) is ported and proved against SLUS-21104's code.
In the port it answers 0xb, because the port's world has no struck class. Scoring stays MX Unleashed's
watch: no MX vs. ATV score call was found. Details in 1-backflip-rider-side-FINAL.md.
