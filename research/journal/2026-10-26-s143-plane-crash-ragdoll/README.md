# S143 — plane crash ragdoll

2026-10-26. Task: `./kit plan S143`.

The user reported that the biplane crash's ragdoll does not act as the original's. I captured four
biplane crashes in the original (nose, wing tip, belly, and a nose crash with the stick moved through the
wreck), with the pilot's 22 joints and the plane's body every tick.

The original never throws its pilot. The body stays in the seat, posed by the plane's clips, with the lean
following the stick, until the put-back 151 ticks later, and there is no poof. The port does the same.

What tumbles is the plane. `crates/game-sim/tests/plane_crash_pilot.rs` proves the port's wreck tumble
bit-exact a tick at a time in all three full crashes. It also proves the wreck tick, the 151-tick put-back,
and that the lean's pad follows the stick through the wreck.

The one difference found is where the plane is put back: 2.5 ft along and 0.11 ft below the original's
spot. It is filed as S161. Details in `1-plane-crash-ragdoll-FINAL.md`.
