# C14 — a captured Kiowa take-off, hover and landing

2026-11-02. Task: `./kit plan C14`.

**The capture.** The original flies the Kiowa on free09 from a generated tape: climb, hover, forward,
rudder, bank, descend, land. 854 ticks, a row each, taken twice (`1-kiowa-capture-PART.md`).

**The replay.** Once S220a and S220b landed, the flight replays bit-exact through `Kiowa::tick`
(`crates/game-sim/tests/kiowa_capture.rs`). It runs 577 ticks, from the take-off's first airborne row
to the row before the touchdown. Each tick it compares the pad words, commands, controller loop, rotor
and aero states, the measure's figures and every body word.

**The finding.** The world step flies the pad words the previous row's input step left, so a replay
runs the input step after the world step (`2-kiowa-replay-FINAL.md`).

**Not done.** The ground contact at both ends needs the collision record's response on the terrain.
That is C14a.
