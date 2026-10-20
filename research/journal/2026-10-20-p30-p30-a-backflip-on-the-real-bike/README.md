# P30 — p30 a backflip on the real bike

2026-10-20. Task: `./kit plan P30`.

You could not backflip in the shell. The original's backflip and preload read the left stick squared
(x·|x|) and not rate-limited, and their "how long it was held" is the speed of the last full flick,
taken from a 32-sample timed ring of pad reads; the port read the rate-limited stick and a held time.
The port reads the original's now, with the lean pair rate-limited at the `.phy`'s axis or d-pad rates,
and the shell's arrow keys are the d-pad. The flip is ridden on the disc's `race125p.phy` in both
crates' tests (on the stick and on the keys) and in the shell's own trace off free06's first jump. S31's
four missed backflip ticks reproduce. The flick ring runs on the simulation's clock, not the
original's wall clock (see 1-…, Not verified).
