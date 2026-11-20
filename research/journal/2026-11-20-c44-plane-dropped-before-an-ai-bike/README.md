# C44 — player 1's plane dropped before a riding AI bike

2026-11-20. Task: `./kit plan C44`.

A capture of the original with player 1's biplane put in front of a riding AI bike, and a replay of
it through one-player `Sim::step`'s plane lane.

## Result

- The replay matches bit for bit until the AI bike backs up.
  - a1 (the struck bike) matches on every compared chassis word (centre, velocity, rates, quat) through 11134.
  - a0 and a3 stay within 0.001 ft for the whole run.
  - The plane is where the original's is.
- It parts at 11135, the first tick a1's brain backs up (reverse lever 1, gas 0, brake 0).
  - The original pushes the bike back from 77 ft/s.
  - The port's bike coasts on.
- Why: the original's AI levers go straight into the rider's lever words (`+0x628`…`+0x648`).
  - The pad's reverse rule (P72) never runs on them.
  - The port's ride runs that rule on the AI's controls too. With gas and brake both 0, it drops the reverse.
  - Proof in the test: on the four ticks a1 is still within 3 ft, the original's extra slowing over ours
    is 1.23, 0.97, 0.92 and 0.86 reverse pushes.
  - One push is ReverseGain 30 × (ReverseSpeed −14.67 − speed) / mass 12.42 × dt.
- Because of that, ours reaches the plane 12 ft further on, a different way. The lists and the plane's
  push can't be compared until the reverse is fixed. Filed as a follow-up.
- No port change. The brief's files allowed only tests, research and watch.

## Deviations from the brief

- **free06, not free09.** biplane.p2m2's state is free06; its three AI riders ride there. The brief's
  "free09" was taken to mean the plane state's map.
- **Dropped, not parked.** Parked in a rider's way (try1), the AI steered round the plane: a1 passed
  16 ft off it. The plane was instead put 0.5 s ahead of a1's bike, 3 ft up, by its put-back.
- **The list is captured but not compared.** The port's per-bike collision list is not exposed to the
  Sim, and our a1 never meets the plane as the original's does (above).

## The capture

On PCSX2 copy 2 (`KIT_PCSX2=2`), with the copy's NominalScalar at 0.3 so the scripts keep up:

1. `python3 -B research/c44/slots.py put biplane 8`: biplane.p2m2's state into slot 8.
2. `research/c44/drop.py PID 8 0x019a44b0 11125 0.5 31`, under `./kit pcsx2 with --`:
   - loads slot 8 and waits for frame 11125;
   - puts the plane 0.5 s ahead of a1 (at 78.45 ft/s: the plane at (2953.72, 273.13, 3019.75));
   - lets the put-back run (3 ticks) and saves slot 31.
3. `slots.py keep 31 drop`, then `slots.py ram drop`: context/c44/drop.p2s and
   context/states/c44-drop.eeMemory.bin (frame counter 11127; its bodies are row 11128's).
4. `./kit record watch/c44-plane-bike.toml --state 31 -n 200 -o context/capture/c44/drop-a.csv`.
   - The spec is from `research/c44/spec.py`.
   - drop-b and try2 are two more recordings from the same slot.
   - `research/c44/agree.py` finds them equal on every column on all 149 common rows.

What the capture shows (`research/c44/lists.py`, `paths.py`):

- a1 brakes, then backs up from 11134/11135, with the bar flipping ±1 (its swerve).
- a1's record lists the plane's record (0x01e73b00) from 11145: the plane's kind-1 mirrors first,
  then a1's own kind-0 entries.
- The plane is pushed from 11146.
- a1 wrecks (reason 7) at 11183.

## The replay

`crates/game-sim/tests/ai/c44.rs` `c44_a_plane_dropped_before_an_ai_bike_meets_it_as_the_original`.

How the Sim is seeded from the state:

- **The plane is player 1.** Its body, tires, engine, noise and controls come from RAM, as S252 seeds it.
- **The human's parked bike is player 1's bike.** It uses `race4.rs`'s new `c44_human_bike`, because
  the human's body band isn't C33's.
- **The three AI bikes** are players 2–4 by their seats, seeded with C33's `c33_bike` (class: the 250s on the A file).
- **Their brains** come from RAM, on a lane over free06's seeker world.
- **The world** is free06's terrain, props and plants.

How it steps:

- Each step into row t rides row t−1's levers. The steps into the first rows ride their own row's,
  because no earlier row exists.
- The previous row is the right phase: with it a1 is exact through 11134. With the same row's levers it
  parts at 11132, where the gas lever drops for one tick.

Run: `cargo test -p game-sim --release --test ai c44 -- --nocapture`. Set `C44_SHOW=1` to print both
velocities and the row's levers.

## Not verified / not 1:1

- No riding ragdoll, notes, dust or tracks in the replay (C33 has them; not needed for the chassis).
- a1's collision list and the plane's push are not compared (see above).
- Only one drop, one bike, one map.
