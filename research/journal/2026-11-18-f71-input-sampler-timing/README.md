# F71 — input sampler timing

2026-11-18. Task: `./kit plan F71`.

Traced when SLUS-21104 samples the pad: five times a frame on a whole-millisecond clock, the pad's new
reading landing between the first and second. The port's stand-in sampler is replaced by that frame model
(`pad::Sampler`). Flip-1's hold on rows 66..71 is now 1000.55 against the original's 992.97 (was 60.006):
the same 2 ms pair, one millisecond earlier on the port's grid. Details in 1-input-sampler-timing-FINAL.md.
