# 1 — engine note on the sims generator (FINAL)

Task S328b. 2026-11-17.

## What I did

- Moved the engine note's machine (Note, Sample, Hands, Brain, Bed, Draw, pick, jitter) out of
  `crates/openmx/src/audio.rs` into a new `crates/game-sim/src/engine_note.rs`. It is now generic over a
  `Sounds` trait. `audio.rs` re-exports it, and `BikeSounds` implements `Sounds`, so the shell and its
  oracle test (`tests/audio.rs`, `the_engine_note_matches_the_original`) still run the same code.
- Added `Draw for Rng`, so the machine can draw on the game's generator.
- Added `Sim::notes: Option<Box<Notes>>` (None means as before: no draws, same hash and wire). With
  notes, each frame after the bodies' look clocks and before the race manager, every player's note runs
  in walk order (last player first):
  - the start-delay countdown and switch-on;
  - `Brain::frame` on the AI lane's generator, else the race's;
  - the queued idle started;
  - the emitter's place kept for the next frame's hearing check.
- `Voices` stands in for the sound chip: each started sample is a clock that runs with the frames.
- `race4.rs` seeds the four notes from C33's image (lists, lengths, params, delay, last emitter place).
- Result: the Sim's generator now agrees with C33 through tick 16, up from 14
  (`C33_AGREES_THROUGH[4]` = 16). From 17 the dust (S328c) and render draws (S328d) are missing.
- `draws.rs` (original, whole frame in Ee) and `race4.rs` (Sim) both hold `C33_NOTES_ON` (ai2 on at 9,
  ai0 at 14) and `C33_NOTE_PICKS` (both pick low-accel sample 1 at 15).
- Gate fixes outside the task:
  - `tests/hash.rs`: the Sim size pin moves to 12296 for the new field.
  - `tests/untamed_race_quad.rs`: its three md147 replays were red from row 1 on the base, with the
    quad thrown about 5 ft up a tick. This is MD144a's map-edge throw: the gate is armed by default
    and Rocksbury's spawn is inside the port's band. Main has no fix, so the test now seeds the gate
    from the image's word (off in Free Ride), as noted for MD168b. All four pass.
- Follow-up S363: the shell's live races should give their Sim the notes and play the Sim's picks.

## What the original does

- The loader that makes the bike's note (`./kit fn 0x00280fb0`) sets rider +0x14a4 to one unit draw:
  a start delay in seconds.
- The player update (`./kit fn 0x0027c890`) counts it down by dt. When it runs out, it calls the note's
  vt+0x88 (`./kit fn 0x001b6b80` → `./kit fn 0x00132b20`), which sets +0x29 bits 0 and 1.
- The child walk (`./kit fn 0x00133500`) runs the note's frame only once bit 0 is set. On the switch-on
  frame the emitter gate +0x1f0 gains bit 1 (0x64 → 0x66) and the queued idle starts.
- The positional mix (`./kit fn 0x00104480`) hears nothing at distance ≥ max (+0x8c, 200 in C33) from
  the nearest listener. It measures from +0xb0, the place written last frame. Unheard: nothing starts
  and no pitch is drawn.
- C33: at 15 ai0 then ai2 (actor order) open the throttle. Each picks low accel (one draw → 1) and draws
  the pitch. ai1 picks at 30. Power chains at 48, 33 ticks after 15.

## How it was proved

- A save-state capture: C33's image and agreed csv.
  - The original side: `s328_c33s_other_draws_are_looks_notes_and_dust` runs 60 whole frames in Ee and
    reads each note's switch bit, state and last pick.
  - The Sim side: `s12e_free09s_four_racers_ride_as_the_capture` replays the 1800 rows; the generator
    is bit for bit through 16.
- The moved machine is still held by the existing function oracle `the_engine_note_matches_the_original`
  (`crates/openmx/tests/audio.rs`), unchanged, passing.
- The full gate `./kit check --all`.

## Not verified / not 1:1

- **The listeners are the human players' bikes**, not the cameras. The Sim has no camera. In C33 every
  note is well inside 200 ft of the human, so the gap does not show.
- **The hearing test compares squared distance** with far², not the original's own distance arithmetic.
  This could differ only right at the edge.
- **The voice clock is the sum of the frames' dt**, starting a frame after the key-on. It is fitted to
  one fact (power chains 33 ticks after 15) rather than ported from the sound chip. Pause and pitch do
  not change it. Only tick 48 and later depend on it, past where the generator agrees now.
- **The countdown and the note run in the notes pass**, after the bodies and before the race manager,
  not inside the player update and the child walk. Through 16 the only draws are the notes', so the
  order against other draws in the same frame is not tested.
- **The emitter gate bit is taken as set once the note is on.** Seen 0x64 → 0x66 at switch-on in C33,
  but what clears it is not ported.
- **The +0x29 bits 2/3 and +0x20 bit 1 walk gates** are not modelled.
- **The Sim's hands have no wreck override.** The shell's `Hands::wrecked` is a user-asked presentation
  change. The Sim reads the bike's throttle as is.
- **No Sim load makes the notes.** Only `race4.rs` seeds them, from the image. The loader's delay draw
  is not in the Sim's spawn. The shell still runs its own copy on its own Dice. Both are S363.
- **The Sim's notes are not on the wire**, like the AI lane. They are in the hash only when present.

## Next

Land. Then S328c (dust draws) moves the generator mark past 16.
