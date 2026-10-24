# S56 — ragdoll exact

2026-10-21. Task: `./kit plan S56`.

S56 recorded four wrecks of the original (over the bars, a side crash, a loop-out, a tip-over) and
replayed each through the original's own ragdoll tick in the oracle.

- **Fixed:** the first divergence was VU0's `vsqrt`, which PCSX2 chops and ps2-float rounded to
  nearest. With that fixed, and the rest of the game's random draws found tick by tick, the oracle
  reproduces every bone bit-exactly from each wreck's state until the bike first touches the rider.
  In the earlier captures, with no bike contact, it ran through respawn.
- **Then (part 2, 2026-10-23):** S61 and S62 have since put the port through every captured wreck to
  the rider at rest, bit-exact against the original. A slow tip-over cannot be ridden: the ridden
  bike's roll freedom is off while a wheel is down. The slowest wreck the original has, an airborne
  roll-out at 2.3 ft/s (reason 8, made by writing a hop and a roll into RAM), is captured. Both the
  original's update and the port hold it on all 699 ticks to rest. Front's part at 2628 is filed as
  S112.

Details: 1-ragdoll-exact-PART.md, 2-slow-roll-out-FINAL.md.
