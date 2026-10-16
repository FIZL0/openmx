# S18 — s18 phy load

2026-10-16. Task: `./kit plan S18`.

The bike's `.phy` file as the port reads it: the game's own plain-text reader and the whole of
`bike_phy_load` on top of it, in `crates/game-sim/src/phy.rs`, so every physics parameter — the
ground step's and the air step's together, both wheels' tire tables, the engine, the collision
record, and the `MaxCrossupAngle` S6 could not prove — comes off the disc. Proved four ways against
the original in `crates/game-sim/tests/phy.rs`: 1987 keys read six ways each, 960 words of the bike
filled from a zeroed start, all 128 setter calls' arguments, and 190 words of the bike in the human's
own recording. What is hooked rather than run, and the handful of values the format allows that no
retail file holds, are in `1-s18-phy-load-FINAL.md` under **Not verified / not 1:1**.
