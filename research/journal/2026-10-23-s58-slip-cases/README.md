# S58 — slip cases

2026-10-23. Task: `./kit plan S58`.

The second half of S58, after S64 gave the replay the original's whole wheel state. All four of the
brief's cases were recorded from first_recording's save state, and each was replayed in the port from
the same state. Fixing the first tick each one parted at found four things the port did differently:

1. the levers' rate limit, which in the original only holds them back on the way up;
2. the airborne rear borrowing the front's load;
3. the bike's speed, which is the touching wheel's view of the chassis, a step behind;
4. the fork's rake, which the replay never seeded.

With them the turn holds to 1e-4 ft/s through four seconds of full lock. The launch, the brake and
the traverse agree until their landing off the jump face at tick 124. What parts them after that is
filed: the landing on S67, the turn's quaternion flip as S112, a held steep traverse as S113. Part one
(the spin step, proved by oracle) is in `../2026-10-21-s58-tyre-slip/`. Details in
`1-slip-cases-FINAL.md`.
