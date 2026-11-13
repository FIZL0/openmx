# C33 — free09 with four racers: the AI riders' bodies and rand_state per tick

Journal: research/journal/2026-11-13-c33-free09-ai4-capture. For S12e's replay. The captures are in
context/capture/c33 (git-ignored).

- `boot.py SLOT`: C31's scripted boot set to free09 Freestyle with NumberOfRacers 4 (recording2's event
  words, but bike class 2 and AllAI 1 — see its docstring for why). It saves the state on the first frame
  past frame_counter 1, when the four bikes still stand on the spawn line.
- `keep.py SLOT STEM`: copies that slot's `.p2s` to `STEM.p2s` and its RAM to `STEM.ee.bin`.
- `riders.py IMAGE [--json]`: the riders in actor-list order, with each one's body (rider +0x7a4), interface
  and brain (0 = the human's).
- `spec.py RIDERS.json FRAMES`: the watch spec. frame_counter, dt, rand_state, in_game, then per rider its
  body's centre, quat, velocity, world spin, body-frame spin, forward speed, the rider's on-ground word, and
  the body's raw first 0x2c0 bytes, at the fixed addresses the state holds. Columns: `aiK_` (K-th AI rider in
  actor order), `human_`.
- `capture.sh SLOT NAME [FRAMES]`: boot + keep + riders + spec + record from the slot. On a freshly started copy.
- `agree.py OUT IN IN [IN...]`: merge recordings of the same ride. PINE reads land mid-tick on some rows,
  and which rows moves with the speed. A row two recordings hold byte for byte is taken as the game's own.
- `record.sh STEM SLOT NAME [FRAMES]`: record again from a kept state (`place.py` puts it back in the slot,
  `wait_boot.py` waits for the game to boot first). Run the copy at NominalScalar 0.1 so the recorder's
  settle after the load costs no ticks.

Files in context/capture/c33:

| file | what |
|---|---|
| `free09-ai4.p2s` / `.ee.bin` | the start state (frame_counter 2) and its RAM image |
| `free09-ai4.riders.json` | its riders: rider, body, interface, brain |
| `free09-ai4-agreed.csv` | **the one to replay**: per frame the row most of the three recordings hold, + `agree`/`of` columns. Skip rows with agree < 2 (26 of 1803) |
| `free09-ai4.csv` (+ `.json`, `.toml`) | the boot run's record at speed 0.3, frames 6..1805 (the 300 ms settle cost ticks 3–5) |
| `free09-ai4-slow.csv` (+ ...) | `record.sh` at speed 0.1, frames 3..1802 |
| `free09-ai4-mid.csv` (+ ...) | `record.sh` at speed 0.2, frames 3..1802 |

Riders in the state (actor order): AI 0x005ff210 (body 0x00899860, brain 0x008a8800), AI 0x008c7b10
(body 0x008cf9b0, brain 0x00888360), human 0x00888770 (body 0x0088cfc0), AI 0x0149c910 (body 0x009014b0,
brain 0x009453b0). Spawn line x = 4318, 4322, 4310 (human), 4314; z ≈ 3314.5.
