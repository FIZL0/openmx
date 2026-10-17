# V15 — the pose it fades from

2026-10-17. Task: `./kit plan V15`.

V12 could not compare the original's pose on the 248 ticks where the cross-fade ramp was still
running, because what it fades *from* — the pose snapshot behind the motion object's `+0x248` and
`+0x24c` — was not in the capture. V15 put it there (two more `[[array]]` blocks in
`watch/rider-joints.toml`, a re-record) and fed it to the renderer as the pose under the blend, so
those ticks are posed and compared like the rest: 1322 of 1499 ticks now drawn against the original
instead of 1117, of which 104 are a genuine fade out of its own snapshot, worst lane 1.4e-6. The
re-record also landed on a stretch of the lap with two wrecks in it, which turned up three
behaviours nothing had seen before (the frame a transition ends still poses the transition; the
frame a wreck is declared does not step the animation; the pose grid's duplicated back column
reaches three bands, not one) and one piece of good news — the free-running advance does not fade at
all, so the port's omission there was right. Details, numbers and the eight things this did not
prove: `1-the-pose-it-fades-from-FINAL.md`.
