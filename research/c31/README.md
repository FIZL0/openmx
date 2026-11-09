# C31 — a live sx01 single race, every score card per tick

Journal: research/journal/2026-11-09-c31-sx01-laps-capture. The captures are in context/capture/c31 (git-ignored).

- `boot.py`: S133's scripted boot, set to sx01, SingleRace, NumberOfRacers 4, AllAI on, 3 laps. It saves a
  state the moment `in_game` is 1, which is before the countdown and before any card exists. Poke from the
  emulator's first second: a copy that booted a while before the poke reaches the front end and never loads.
- `capture.sh SLOT NAME`: runs the boot and then records `spec.py`'s list-ordered spec. That run gave **sx01-ai4.csv**.
- `tape.sh PID STATE.p2s NAME PAD`: plays a from-savestate tape (research/c15/tape.py) over AT-SPI from the
  start state and records the fixed-address spec. With `gas.pad` that run gave **sx01-gas.csv**.
- `spec.py [CARDS [BASE STRIDE]]`: writes the watch spec. It records the score manager (named words + raw
  0x110 bytes) and, per card, the words the lap line and score_race_tick use + the raw first 0x1d0 bytes.
  Without BASE the cards are read in list order, `cardK_0` = K-th in the list. With BASE they are read at
  fixed addresses, `cardN_0` = rider id N.
- `by_rider.py IN OUT`: rewrites a list-ordered capture into `riderN_0` columns, keyed by card id.
- `summary.py CSV`: laps per card (the tick, the lap time, the best), off-track ticks, flags, places.
- `peek.py`: a one-off look at the boot state over PINE.

Files in context:

| file | what |
|---|---|
| `context/states/c31-sx01-race-start.p2s` | the start state (copy 2 slot 27); the tape plays from it |
| `context/states/c31-sx01-ai4.ee.bin` | its RAM image (no cards yet; the event, the manager's +0x60/+0x64/+0x94) |
| `context/capture/c31/sx01-ai4.csv` | boot run, no input: the player idles at the gate, 3 AI riders ride 3 laps. **List order** (use by_rider.py) |
| `context/capture/c31/sx01-gas.csv` | tape run, gas held from the start: the player goes off the track 37 times. **By rider id** |
| `context/recordings/c31/sx01-gas.p2m2` | the tape (+ `.ticks.csv`, the pad held each tick) |

Both captures run 12,000 rows from frame_counter 1. Cards are at 0x8fc760 + id × 0x1e0 in this session.
Card ids: 0 is the human, 1–3 are AI.
