# 1 — stock engine braking (PART)

Task S351. 2026-11-17.

## What I did

- `stock.rs` and its twin `tests/freeride-bike.phy.txt`: `EngineBraking` 0.3 → 0.00000005.
- The value is the one all four disc 250 bikes write (`pro250p/a`, `race250p/a` .phy). The machine .phy files (buggy 13, baja 10, monster truck 5) are not bikes and were not used.
- With honest braking three `tests/ride.rs` tests failed: the launch (5.1 ft/s at half a second, needs 9.35), the held turn (2.7 ft/s, needs 8) and the lean. The stand-in had been launching on the bug: the braking pinned the engine at MinRPM, the clutch never locked, and its kinetic friction drove the rear. Locked honestly, the stand-in's low end (6 HP at 2000 rpm) is too weak to move it.
- So the stand-in's first curve point went from `RPM1 = 2000, HP1 = 6` to `RPM1 = 1000, HP1 = 11` (disc bikes start at 1000 rpm, 15 HP). Found by trying values: 15 and 12 left the bike leaning 0.042 / 0.0355 rad a second after the stick centred (limit 0.0349); 10 dropped the held turn to 7.7 ft/s (limit 8). 11 passes all 67 ride tests.
- Re-pinned: `HUNDRED_TICKS` (lib.rs, tests/hash.rs, tests/platforms.rs), and in platforms.rs `HASHES`, `TRACE_DIGEST`, `FINAL_DUMP_DIGEST`, `FIXTURE_TEXT_DIGEST`, `RIDERS_HASHES`, `RIDERS_DIGEST`, `RIDERS_WIRE_DIGEST`, `RIDERS_WIRE_LEN` (spliced from `print_the_vectors` / `print_the_riders_vectors`).

## What the original does

No code change: the stock bike is a hand-written stand-in, and this only brings one of its values to the disc bikes' order (R16 found the 6M× gap).

## How it was proved

Value read off the disc .phy files in context/res. The test gate (`./kit check --all`) passes with it (via `./kit done`).

## Not verified / not 1:1

Everything this task left unproven or inexact goes here, each with its reason — or the single word `none`.
A reader must be able to tell from this section alone what they may not trust yet.

- The stock bike as a whole is not a disc bike; its other values stay hand-written. Only EngineBraking was brought in line.
- HP1 = 11 is a tuned stand-in value, not the original's: the window between the lean test and the held-turn test is narrow (10 fails one, 12 the other), so a later change to the ride may push it out again.
- The new platform vectors were made on x86-64 only; no other architecture was run (no upstream cross toolchain here).
- Not re-ridden on screen: the coasting creep and the hanging-rear throw are expected gone from R16's analysis, not watched.

## Next

none
