# MD92 — untamed ride capture

2026-11-03. Task: `./kit plan MD92`.

The task: capture an MX vs. ATV Untamed (SLUS-21701) MX ride with a wreck in my own PCSX2 copy, then
replay it bit-exact on `--handling mxvatv --handling-pack untamed_tricks`.

The capture is done. From copy 9's save state 8 (Rocksbury Free Ride, a Race250P at rest at the gate), a
generated tape: gas, three airs, a hard-landing wreck at row 340 (reason 1), the put-back at row 489,
then steer both ways, brake, gas and coast. That is 665 rows, one per 30 Hz tick, captured three times in
copy 1. The bike's body and every decoded column are identical in all three runs.

The replay is not done. The brief puts two things first, and neither has landed:

- a bit-exact SLUS-21104 bike ride replay. No task covered it, and C15's captures are lost, so I filed
  it as F31;
- MD93's split-off ports MD93a–d.

The task is blocked until those land. Details in 1-untamed-ride-capture-PART.md.
