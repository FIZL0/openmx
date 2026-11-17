# 1 — s12 ai in the sim (FINAL)

Task S364. 2026-11-17.

## What I did

- **`ai::keeper_mode`** names the score keeper's three stored modes (1 machine race, 2 supercross race /
  fast lap, 3 free ride and stunt competition; S92's store scan, S292).
- **`ai::keeper_mode_of(Option<&Race>)`**: a Sim ride with a race (`Sim::race`) is mode 2, any other is
  mode 3. The Sim runs no machine race, so mode 1 never comes out.
- **`Sim::think`** (`crates/game-sim/src/tick.rs`) puts it in `Habits::keeper_state` before every think.
  Before, the word stayed 0, so the cross-up's "mode 3: no cross-up" gate never held: AI riders on a free
  ride could cross up, which the original's never do. In a race nothing changes (0 and 2 both pass).
- New test file `crates/game-sim/tests/ai/keeper.rs`.
- `../opentennis` is not on this machine; nothing taken from it.

## What the original does

- `ai_habits` (`./kit fn 0x00280070`, its cross-up) reads the rider's score
  keeper (rider `+0xfa0`) at `+0x64` and skips the cross-up when it is 3.
- That word is the keeper's mode, the one `score_manager_tick` (`./kit fn 0x25fcd0`) switches its
  handler on. It is not the race kind at `+0x60` (which `Seat::race_phase` already reads off the laps).
- The keeper is one per session and is always there during a ride, so the word is always read.

## How it was proved

`crates/game-sim/tests/ai/keeper.rs`:

- **`s364_the_keeper_mode_is_the_images_on_a_race_and_a_free_ride`**: on sx01's race image (S282c,
  `context/states/s282c-sx01-race.ee.bin`) each of the three AI riders' keeper `+0x64` (rider = brain
  `+0x40` − `0x10d0`) equals `keeper_mode_of(Some(race))` = 2. On free09's four-rider free ride (C33,
  `context/capture/c33/free09-ai4.ee.bin`) each of the three AI riders' equals `keeper_mode_of(None)` = 3.
- **`s364_the_sim_puts_the_keeper_mode_in_the_habits`**: a stock Sim with one AI rider; the habits' word
  set to −1 before each `Sim::think`, then read: 3 with no race, 2 with a race, 3 again without.

## Not verified / not 1:1

- **Mode 1 (machine race) is never fed.** The Sim has no machine race (S292a: its handler has no caller
  in the Sim), so no image of one was checked. When one is added, `keeper_mode_of` must learn it.
- **A Sim race's mode is taken as 2 for every race.** Only sx01's race was checked against an image.
  Other tracks' races (national tracks, a fast-lap race) were not captured; S292 says fast lap is also
  mode 2, from the store scan only.
- **The stunt competition is taken as mode 3** from S92's scan; no image of one running was read here.
- **The race start's keeper state** (`Racer::race_start`) still has no caller in the Sim; this task only
  feeds the habits' word, as asked.
- **No live ride shows the change.** No capture compares an AI free ride's cross-ups tick by tick; C33's
  replay (race4.rs) already read the word off its image (3), so it was 3 there before this task too.

## Next

Nothing.
