# 1 — thirty hz tapes (FINAL)

Task S222. 2026-11-02.

## What I did

- S184 found that V70's tape script scheduled 30 Hz ticks but stamped every line 1/60 s
  (`0x3c888889`). The shell ticks on a played line's own length (`World::simulate`'s `played_dt`), so
  it rode the schedule in half the game time. Two more scripts did the same:
  - `research/v40/tape.py`;
  - `research/journal/2026-10-25-p72-reverse-l3/capture/port-tape.py`.

  Both now stamp `0x3d08850a` (0.03333, the 30 Hz cap), and their docstrings say why.
- I rode the old and the new tape of each in the release shell on HEAD 51ea62e4, headless, with the
  commands their journals give. To follow the ride I read the `end:` line at several `--frames` counts.
  Each journal now ends with an S222 section that has the end points.
  - **V40** (`--track free09`, defaults gas 75 and steer 200, 195 frames):
    - old tape: ends at 4319, 142, 3455, and at the shot frame 190 is still going at 39 mph, in the air
      off the slope;
    - new tape: ends at 4324, 138, 3480, stopped (2 mph).
  - **P72** (`--trace-at 4620,2320,0`, 240 frames):
    - old tape: ends at 4620, 135, 2301;
    - new tape: ends at 4620, 135, 2284.

    Both back up at about 14 ft/s. The new one only gets twice as far, because its clock runs at the
    right speed.
- Shots, old and new, are in `context/shots/s222/` (not committed): `v40-{old,new}.png`, the new tape at
  frames 45/90/120, and `p72-{old,new}.png`.

## What the original does

Nothing new about the original. The point is the tapes: a level ticks at the 30 Hz cap, so a line that
stands for one ride tick must say 0.03333.

## How it was proved

- Looked at the shell's shots, old tape against new, at the frames the two journals used.
- Read the end points at 1 to 7 (V40) and 1 to 11 (P72) `--frames` counts per tape.
- No code changed, so no test was needed. The tapes are by-eye aids, and nothing in the gate rides them.

## Not verified / not 1:1

- **V40's "faint and thin, beside the original" now has no shell shot behind it.** On the fixed tape the
  bike stops, and no mark shows behind it at frame 190, nor at frames 45, 90 or 120. The old shot's mark
  was behind a bike still going at 39 mph. I did not look into whether marks are laid on this ride and
  are just out of frame (the follow camera looks ahead), or not laid at all. `tests/tire_tracks.rs`
  still proves the rules on a replayed ride.
- **P72's shot frames differ in phase.** At frame 230 the new tape is 3.8 s into the back-up and the old
  one 1.9 s. Both show the same pose with both legs down, but they are different moments.
- The original was not re-shot. Its side of both comparisons is the old shots in `context/shots/v40`
  and `context/shots/p72` (other slots).
- The ride has changed since V40 and P72 (the clutch ramp, the riding ragdoll since S90), so these
  re-rides are today's shell and not those tasks' shell.

## Next

Done. V40's look beside the original, on a tape that really stops, is filed as V120.
