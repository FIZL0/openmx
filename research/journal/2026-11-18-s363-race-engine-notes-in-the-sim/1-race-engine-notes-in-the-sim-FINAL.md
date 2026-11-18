# 1 — race engine notes in the sim (FINAL)

Task S363. 2026-11-18.

## What I did

- `engine_note.rs`:
  - `build_order`: the order the loader builds the bikes in. It sorts the players by bike model file
    name, case folded, with a port of the game's C library qsort (BSD).
  - `RiderNote::build`: a note plus its start delay, one unit draw.
  - `Sim::spawn_ai_noted`: walks the build order. Each bike draws its delay; an AI rider's brain spawns
    right after its own delay. Other seats' humans draw a delay that is dropped.
  - `Sim::load_notes`: the same for a race with no AI riders.
  - `Voice::detune`: the Sim now keeps the pitch variance it drew, for the shell to play.
- `audio.rs`:
  - `BikeSounds::note_sounds` and `audio::note_sounds(disc, bike)`: the Sim's `NoteSounds` off the .aud.
  - The `[Params]` are now read as the game reads floats (truncated, `phy::number`). LowTorqueSpeed's
    ×1.46666 is a PS2 float multiply.
  - `Engine::follow`: plays the Sim's voices. It starts a sample that is new or whose clock went back,
    with the Sim's drawn variance. It stops a sample the Sim dropped. It pauses the rev and plays
    `[Panic]`'s loop while the Sim holds.
  - `SeatFrame::note`: the Sim's note for the seat. `SeatFrame` lost `Copy` (two tests now clone).
- `race_start.rs`: `spawn_ai` takes the notes and calls `spawn_ai_noted`.
- `main.rs`:
  - At a race's load, read the bike's `NoteSounds` (sound on or off).
  - With AI riders, spawn them noted. With none, `load_notes`.
  - `seat_sound` hands the seat's note to the audio unless the rider is wrecked. The wreck keeps the
    shell's own dice and `Hands::wrecked`, as presentation only.
- `research/oracle.txt`: the S363 rows (`vehicle_model_order`, `vehicle_model_names`, `libc_qsort`).

## What the original does

- `./kit fn 0x00208c50` (vehicle_build_load):
  - Fills one slot per player (list order: humans, then AI), with each bike's model file name at
    `0x004e0880 + i*0x104`.
  - Then `qsort`s the slot indices with `0x00208bd0`, a case-folded strcmp (`0x00410c50`) of the names.
  - Calls the spawner `0x0020e0b0` in that order.
- The spawner builds the bike; the builder's `0x00280fb0` draws the delay. Only after that, for an AI
  rider, come the brain spawn (10 dice) and the reader (2 draws).
- `./kit fn 0x0040f338` is the BSD qsort: insertion sort under 7, or after a pass with no swap; med3,
  ninther past 40. Below 7 equal names keep list order. From 7 up they may not.
- C33:
  - Models: human 250SX_THQ_01, players 1–3 RM250, 250SX, 250SX.
  - Built order: players 2, 3, 0, 1 (ai0, ai1, human, ai2).
  - Delays found 658, 645, 632, 631 draws back from the image. Each is one dt above rider `+0x14a4`.

## How it was proved

- `tests/ai/race4.rs` `s363_c33s_note_delays_draw_between_the_brains`:
  - Walks C33's generator back about 400k draws and finds each brain's 10 dice.
  - Checks the build order and that each delay sits right before its brain.
  - Runs `spawn_ai_noted` from the generator before the first delay. The delays, every AI rider's dice
    and the generator left all match bit for bit.
- `tests/ai/race4.rs` `s363_the_build_order_is_the_loaders_sort`: the game's own qsort with its
  comparator, run in the interpreter on C33's image over 600 name lists (1–8 players, equal names,
  case, prefixes). `build_order` gives the same order every time.
- `tests/audio.rs` `the_sims_note_sounds_are_the_original_notes`: `note_sounds` of the 125 against
  first_recording's note. Params and far are bit for bit; counts and looping are exact; lengths are
  within 2 ms.
- `tests/audio.rs` `the_seats_engine_plays_the_sims_voices`: hand-made Sim voices over 7 frames. The
  mixer sounds exactly the Sim's samples, starts a restarted one as a new voice, and adds the limiter
  loop.

## Not verified / not 1:1

- Every racer is given the first bike's model name in the shell, because every AI rider rides that
  bike here. With other bikes (split screen, or AI bikes that differ) the original would sort by each
  real model.
- From 7 racers up, the qsort can reorder equal names. The port does the same, but no capture has 7+
  racers. Proof is the interpreter oracle only.
- Split-screen: the other seats' humans are Sims of their own. Their delays are drawn and dropped in
  seat 0's Sim, and their engines still pick on the shell's own dice.
- The original draws a delay only when the .aud opened and the note was allocated. The port always
  draws, with a default (empty) `NoteSounds` if the disc read fails.
- Sample lengths come from the decoded clip (frames / rate), within 2 ms of the original's words, not
  bit for bit. A pick near a sample's end may part.
- Listeners are the human bikes, not the cameras (as S328b).
- AI riders are still silent in the shell. Only the seat's own bike follows the Sim.
- Dust is not seeded in the shell, so the live generator can still part at dust draws (S328b's
  finding stands).
- Notes are hashed but not on the netplay wire. Both peers build them the same way at the load; not
  run online here.
- `Engine::follow` sees the Sim once per render frame. A sample restarted and run past its old clock
  between two render frames is not seen as restarted. Pitch is the mixer's own plus the Sim's
  variance, not the sound chip's pitch word.
- The wreck path uses the shell's own dice, not the Sim's (by the task's ruling).

## Next

- None.
