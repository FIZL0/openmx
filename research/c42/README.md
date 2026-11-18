# C42 — a race trick's bank, and the results' score commit, live

Journal: research/journal/2026-11-17-c42-race-trick-bank. The capture is in context/capture/c42 (git-ignored).

- `make_pad.py`: the tape's script. Gas (Cross) from the C31 race start; a no-hander (Triangle + stick up)
  for 8 ticks every 45 from tick 300 to 3300; then Cross lifted 3 ticks every 30 to the end, which the
  results screen takes as Restart. Triangle stops early: on the results screen it is Quit.
- `c42.toml`: the watch. Card 0's flags, laps, combo node count, combo seconds, bank (+0xdc), combo points;
  the AI cards' banks; the manager's flags and laps; player 0's profile points (0x56fea0 +0xc0, fixed in
  this session).
- `poke.py`: `laps N` sets the manager's lap count (one lap, so the player finishes in ~6,500 ticks);
  `read` prints points, frame and banks.
- `run.sh PID NAME [ROWS]`: C31's tape.sh flow, with the lap poke and a points read between the load and
  the recorder.
- `summary.py CSV`: the rows where a watched word changes.

| file | what |
|---|---|
| `context/states/c31-sx01-race-start.p2s` | C31's start state; the tape plays from it |
| `context/capture/c42/trick1.csv` | 16,000 rows from frame 1: banks, the finish, the Restart, the second race |
| `context/recordings/c42/trick1.p2m2` | the tape |

The copy ran at NominalScalar 1 (its ini had 0.05, put back afterwards).
