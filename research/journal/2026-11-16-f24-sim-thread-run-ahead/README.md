# F24 — sim thread run ahead

2026-11-16. Task: `./kit plan F24`.

A split-screen ride with the sim thread on (N23a) showed its motion in 30 Hz steps. That's because
the picture drawn beside a tick held the last tick's end. Now a tick that ran on the thread waits
until the next one is due. Every picture until then blends the two ticks before it, from
before-copies kept a tick longer. The motion is as smooth as on one thread, one tick (about 33 ms)
behind it. I decided that latency is worth it where the thread engages, and made it the default.
`--run-ahead off` keeps the held picture. Over the 1800-tick 2P free06 gas tape, the per-tick hashes
of both seats are identical with the thread off, held and run ahead. Every run-ahead picture is the
off-thread picture one tick later.
