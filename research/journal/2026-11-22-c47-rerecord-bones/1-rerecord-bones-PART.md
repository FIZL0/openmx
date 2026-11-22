# 1 — rerecord bones (PART)

Task C47. 2026-11-22.

## What I did

- **The states.** S74's `--state 6` ("free06") is `context/recordings/first_recording.p2m2_SaveState.p2s`
  (frame 2773): its RAM matches `context/s66/free06.eeMemory.bin` byte for byte. "free09" is
  `recording2.p2m2_SaveState.p2s` (frame 2373). Both are on free09 terrain.
- **Why tapes.** On PCSX2 copy 2, PINE `load` did nothing for the high slots S74 used. The copy also runs at
  NominalScalar 0.3 (about 9 ticks a second, set on purpose in `openmx-slots/pcsx2/s2/PCSX2`), which breaks
  wall-clock pad scripts. So I used S201's route: a generated `.p2m2` tape per script, played over AT-SPI.
- **New scripts** in `capture/`:
  - `bones_tape.py` writes S74's three pad scripts as tapes, each step 75 ticks (S74's 2.5 s) after the load.
  - `play_bones.sh` plays a tape and logs it with S74's watcher.
  - `bones_sum.py` prints a capture's frames, torn/gaps, put-backs and wreck entries.
- **The watcher.** S74's `bones_watch.py` gained `--attach/--after/--frames`. With them it loads nothing and
  sends no pad, and logs 540 ticks from the load + 75. Without them it runs as before.
- **What came out.** All captures have torn 0 and gaps 0.

| capture | rows | put-backs | wreck entry |
|---|---|---|---|
| s74/free06-ride | 2848..3387 (540) | 3050, 3179 | — |
| s74/free06-down | 2848..3387 (540) | 3062, 3116 | 2982 |
| s74/free06-downchord | 2848..3387 (540) | 3020 | 2982 |
| s74/free09-ride | 2448..2987 (540) | 2650, 2780 | — |
| s74/free09-down | 2448..2987 (540) | 2668, 2716 | 2588 |
| s201/free09-ride3, free09-ride2 | 2373..2893 (521) | 2575, 2705 | — |

  In all: 9 put-backs and 3 wreck entries across the s74 files, which is what rider_anim.rs asks for. The
  s201 pair came from S201's own `entry_watch.py` and `play_entry.sh`, unchanged.

## The check fails, and not because of a regression

`./kit check -- -p game-sim --test riding_spring` no longer prints 'no context/s74', but it fails three tests
(full log: `context/notes/check-s2.log`):

- **the_riding_entry_starts_from_the_last_held_tick.**
  - The entry starts are still bit for bit ("starts 0 ft, 0 ft/s").
  - The entries miss ENTRY_AT 0.06 by a hair: 0.0601 ft at free09-ride3 2707, node 7. S201 had 0.010.
- **the_riding_spring_follows_the_original_after_each_put_back.**
  - Entries: worst 0.134 ft at free06-down 3121, node 16.
  - Settled stretches of the *ride* captures: up to 1.4–2.65 ft, all at part 2 (node 4). Examples: free06-ride
    3039/3112, free09-ride 2635/2758. The down and downchord settled stretches are mostly within bounds.
- **the_riding_body_gives_as_the_originals.**
  - 306 gives are off. Example: s74/free06-ride 3050 part 0 peaks at 1.86 ft against the original's 1.42.

**Bisect.** I built S201's own commit (aeec49464) in a spare worktree and ran its riding_spring on the new
captures. It fails the same way, with the same numbers: 0.0601 entries, the 2.4/2.65 ft openings, 0.134 ft at
free06-down 3121. So the riding code since 11-02 did not cause this. The new ride captures differ from the
ones S74/S201 measured against, and the riding model does not follow them.

The big misses sit on the opening stretch, before any put-back. That points at the ride itself, for example
a landing or a bump the old live-pad rides never hit, rather than at the put-back entry.

**Captures parked.** All slots share `context/`, so leaving these files in `context/s74` and `context/s201`
would turn riding_spring red at every gate. I moved them to `context/c47/s74/` and `context/c47/s201/`; the
tapes are in `context/c47/`. Moving the two folders back restores them.

## Not verified / not 1:1

- **Tapes, not S74's live pad.** The steps are S74's sleeps rounded to ticks, so the put-backs fall at
  different frames from S74's (S201's ride3 tape reproduces its own put-backs within 2 ticks).
- **No clock put-back.** S74's set had 7 chord + 2 clock put-backs. `bones_sum.py` calls all 9 of these
  chord. free06-down 3062 and free09-down 2668 come 80 ticks after a wreck and are likely the clock's, but
  the ghost clock never read exactly 3.0, so this is unconfirmed.
- **ride2 is a superset.** free09-ride2 was recorded with today's `entry_watch.py`, so it holds the ride3
  regions as well as S201's ride2 ones.
- **Emulator speed 0.3.** The tapes play by the tick, so this should not matter, but I did not prove it.
- **Other tests not run.** rider_anim and motion were not run on the new captures. They skip now that the
  files are parked.
- **Why the rides differ is unknown.** I did not find which bump or landing the riding model misses.

## Next

Follow-up task S (filed): find why the riding model misses the new ride captures' openings and settled
stretches (part 2 / node 4, up to 2.65 ft), with `context/c47/s74` as the input. When it is fixed, or the
bounds are re-measured, move `context/c47/s74` and `context/c47/s201` back to `context/` and run riding_spring,
rider_anim and motion.
