# S222 — thirty hz tapes

2026-11-02. Task: `./kit plan S222`.

V40's and P72's shell tape scripts stamped their 30 Hz ticks 1/60 s, so the shell rode them at half
the game time, as S184 found for V70's. They now stamp `0x3d08850a`, and I re-rode them.

- **V40's by-eye look changes.** Its "stopped bike" was still going at 39 mph down the slope at the
  shot frame. On the fixed tape the bike stops at 4324, 138, 3480, and no mark shows behind it.
- **P72's look stands.** The rider backs up with both legs down at about 14 ft/s on either tape, next to
  the original's 13.85. Only the distances double: the end moves from z 2301 to 2284.

Details in `1-thirty-hz-tapes-FINAL.md`.
