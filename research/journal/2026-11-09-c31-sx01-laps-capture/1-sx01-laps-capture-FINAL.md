# 1 — sx01 laps capture (FINAL)

Task C31. 2026-11-09.

## What I did

- **Two live sx01 single races, 12,000 ticks each, every score card every tick.** Everything is in
  research/c31 (README.md lists the files). The captures are in context/capture/c31.
  - `sx01-ai4.csv`: a scripted boot with no input. The player idles at the gate. The three AI riders each
    ride 3 laps and finish.
  - `sx01-gas.csv`: a generated `.p2m2` tape from the race's start state, gas held throughout. The player
    leaves the track 37 times and counts one 193 s lap. The AI riders again ride 3 laps and finish.
- **No pad was needed for the laps.** /dev/uinput is still root-only. Rather than steer a tape round
  sx01, the boot (S133's `boot.py`) asks for NumberOfRacers = 4 instead of 1. The AI riders' cards go
  through the same race handler and lap line as the player's, so they give real laps with no input.
  The tape covers the human's card: off the track, the off-track clock, put back on the track.
  - The boot also sets the event's lap count to 3 so that the finish happens inside the capture.
  - I tried AllAI = 1 to put the player's bike under the AI. It did nothing (see below).
- **Spec** (`spec.py`), per card:
  - flags (u64), to-line kept/now, path_to (+0x18), progress, distance travelled (+0x20), the lap
    lengths, place, laps, runs, on-track word, lap time, lap start, best lap and its number;
  - now/last (f32x4), the path spot and the kept spot, race clock, lap clock, +0xbc, off-track clock
    (+0xc0), +0xc4, the line-words pointer, +0x108, the records list;
  - plus the card's raw first 0x1d0 bytes, so a word not named here can still be read later.
  - The manager gets its flags, +0x60, +0x64 (the handler switch), laps, +0xb8, the countdown and its
    raw 0x110 bytes, plus frame_counter, dt and rand_state.
- **The card list is re-sorted by place every few ticks**, and the first capture tore a row on 6 of those
  re-sorts. The cards themselves never move: they sit contiguous at 0x8fc760 + id × 0x1e0. So the tape
  run reads them at fixed addresses, one column group per rider id. `by_rider.py` rewrote the first
  capture the same way (`sx01-ai4-by-rider.csv`).
- `research/oracle.txt`: a C31 block, with the +0x60 finding below, the card layout and re-sort, and AllAI.
- Plan notes on S294 and S13c (+0x60 is not the activity type; a single race does finish at the lap
  line) and on S291 (a gas-held start does not reach the closed gate).
- Nothing taken from opentennis: the recipe is C28's tape and record flow and S133's boot.

## What the original does

Read off the captures and the start state's RAM, not off new decompilation:

- **The score manager's +0x60 is not the event's activity type.**
  - The SingleRace boot (event type 2) has +0x60 = 1. S196's sx01 image is event type 1 (Practice)
    with 5 laps, and it has +0x60 = 2.
  - `score_manager_tick` switches on +0x64 (`./kit fn 0x0025fcd0`), which is 2 in both, so both run
    score_race_tick.
  - With +0x60 = 1, the lap line's "kind not 2/3 and laps ≥ mgr+0x94" branch fires. Each AI card's
    third crossing sets card flag 1 and manager flag 0x10, live.
  - S13c's journal said "a single race never finishes at the line". That was a practice session.
    race.rs's `race_kind` names are inferred from that, and they are wrong.
- **Timing within a tick, at a counted lap:**
  - The row's lap time = the race clock at the line − the previous lap start, so the line runs before
    the clocks advance.
  - Lap start = that race clock. The race clock in the row is lap start + dt. The lap clock in the row
    is dt (it was zeroed at the line, then += dt).
  - A card that has just finished keeps its clocks stopped (race clock = lap start, lap clock 0).
  - Every lap time and best-lap update in both captures follows `best = min` per rider.
- **Off the track:**
  - The human's off-track clock (+0xc0) runs up while the on-track word is 0, and stops at
    4.9995 s (150 ticks).
  - The rider is then put back on the track: card `now` jumps and the clock goes to 0.
  - Shorter episodes end at 3.1–4.4 s when the rider gets back on by itself.
- **The AllAI knob is not read.** Poked to 1 before the world loads, card 0's rider stayed at the gate
  for all 12,000 ticks. The oracle had already said nothing reads it.
- **The gate holds the bike.** With gas held from before the countdown, card 0 does not move until the
  drop (S13b's height-only pin), so no gate catch happens.

## How it was proved

What this task proves is the captures themselves (`summary.py`, plus the checks above):

- **sx01-gas.csv:** 12,000 rows, frames 1..12,006.
  - 6 frames missing: 3 torn reads, 3 the emulator ran past (2400, 3001, 6354, 9233, 11112, 11850).
  - Every row has card ids 0, 1, 2, 3 at the four fixed addresses.
  - Laps per rider: 0 → 1 (tick 6080); 1, 2, 3 → 3 laps each, finished.
  - Rider 0 is off the track for 5,189 ticks in 37 episodes.
- **sx01-ai4.csv:** 12,000 rows, frames 1..12,006, with 6 rows lost to list re-sorts (2100, 2242, 2244,
  2524, 3894, 3902). AI riders 1–3 ride 3 laps each and finish. Rider 0 never moves.
- At every lap tick of both captures, the lap time is the race clock at the line minus the old lap
  start, bit for bit in f32. The best lap is the minimum so far.
- Provenance: each `.csv.json` sidecar. The tape is `context/recordings/c31/sx01-gas.p2m2` (+ ticks),
  and the start state is `context/states/c31-sx01-race-start.p2s`.

## Not verified / not 1:1

- **Nothing in the port is held against these captures yet.** C31 is the capture; holding
  `race::LapRules::line` and the race handler against it is S13c/S293/S294's work.
- **The laps are the AI riders'.** No human card counted a lap with the rider on the track. The tape
  holds gas only, because steering an open-loop tape round sx01 was not attempted. Rider 0's one lap
  (193 s) comes from riding off-track and being put back. The race handler treats AI and human cards
  through the same code (card flags differ: 0x8 AI vs 0x4 human, inferred from the values, not traced).
- **The capture's row phase is not fixed against the handler.** About half the rows have card
  `now == last` (the handler copies now into last at the end of its card step), and half do not. Where
  each row falls in the tick (before or after the riders move) is not measured. Use the lap-tick
  arithmetic above, which is consistent, and see [capture rows read at three phases] in the kit notes.
- **6 rows are missing from each capture.** sx01-gas's 6 are torn or skipped frames. sx01-ai4's 6 fall
  on list re-sorts, which is why the second capture reads fixed addresses.
- **The two runs differ.** AI lap times differ between the idle and the gas run (66.72 vs 67.22 s for
  rider 1's first lap). The human's presence changes the race. Neither run is a repeat of the other,
  and no run was taken twice to check that it repeats.
- **The meaning of manager +0x60 = 1 vs 2** is read off two sessions (SingleRace → 1, Practice → 2).
  The code that writes +0x60 was not read, and the other event types were not booted.
- **Card fields named by guess:** `path_to` (+0x18), `travelled` (+0x20), `behind` (+0xbc) and `w_c4`
  (+0xc4) are named from the handler's use of them, not identified. The raw bytes carry them anyway.
- **The checkpoint lists** (mgr+0x70, flag 0x40; mgr+0x78, flag 0x200) are only in the raw manager
  bytes and in the card's line-words pointer. The per-line words themselves (at *(card+0xe4)) are not
  captured. S294 will need them, and so will a capture of that array.
- **No gate catch**: the gas held from before the drop never reached the closed gate (S291 still needs
  a teleport).

## Next

S293 (the handler per card) and S294 (lists and finish) can be held against `sx01-gas.csv`: seed a card
from a row's raw bytes, step it, and compare with the next row. S294 should first fix `race_kind`'s
meaning (+0x60 = 1 finishes at the line).
