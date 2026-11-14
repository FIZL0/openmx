# 3 — race mode (FINAL)

Task P313a. 2026-11-14.

## What I did

- S331 has landed (`Sim::spawn_ai`, `ai::Lane::spawn`), so the shell's race now fills its grid:
  - `race_start::lay_out` keeps every card's grid spot and facing (`Start::grid`, card 0 the human's).
  - main.rs: in a one-player race (`riders == 1`), lanes 1..racers each get an AI rider. The rider is
    built with `new_rider` on the human's bike and `.phy`, stood on the ground under its grid spot and
    facing the grid's way. It goes into `Start::ai` and is added to seat 0's Sim as players 1.. in card
    order. The race is then built over every rider (`Start::race_of`), so the gate watch, the start
    timeline and the drop hold and release them all.
  - The lap line gets a `race::COMPUTER_CARD` card for each AI rider at its grid spot, after the human's
    `HUMAN_CARD`. The results already list non-zero players as `Who::Ai`.
  - After `load_seek` (which now counts the AI players' slots and owners), `race_start::spawn_ai` draws
    the roster (`ai::spawn_roster`, one human, difficulty 90, 99 rider-database entries). It then calls
    `Sim::spawn_ai` with the bike's `AIChrctr.aip`, a race event kind (0, the race driver's mode 2) and
    `race_start::ai_entry()` (live, a computer's card, no race phase or path net).
  - Lines: `race: AI rider N in lane L place P; grid x,y,z`, `race: 3 AI riders, skills …`,
    `race: 4 cards, 3 computer`, and `race: AI rider N at x,y,z at the drop`.
- Test `a_race_of_four_has_three_ai_riders_behind_the_gate` (crates/openmx/tests/race_menu.rs):
  `--race --racers 4` on sx01 gives three AI riders, every rider in a lane of its own behind the gate, AI
  brains spawned, four cards with three computer ones, and all three still behind the gate at the drop.
- Took nothing from opentennis: it has no AI riders on a grid.
- Filed V128 (draw the AI riders) and P350 (AI with more than one local seat). Noted on S331b what makes
  them race.

## What the original does

- The rider database's entry count (`app_pointer` +0x3d8 → +0x14 → its list's first word, which the
  roster spawn reads) is 99 in C33's image and in all six S138 images.
- The brain's mode comes from the event's kind: 0 or 1 gives 2, the race driver (`./kit fn 0x00384cc0`).

## How it was proved

- The spawn itself is S331's proof (C33). This task's part is wiring, so it is checked by riding the
  shell:
  - The test above, run headless on the disc's sx01 (`./kit check -- -p openmx --test race_menu`: 3
    tests passed).
  - The same ride by hand, reading the lines. AI riders sat in lanes 1, 3 and 0 (the human in lane 2) at
    z 446.58 against the gate's 454.58, and the drop at tick 97 found them all at z 447.13. The lap line
    ranked all four from the drop.
- Release build (`cargo build --release -p openmx`, then the same `--racers 4` ride for 8 s): the same
  lines, bit for bit, as the debug build's.

## Not verified / not 1:1

- **The AI riders are not drawn.** The shell draws each seat's own rider through its `Rig`, and AI
  players have none. They are in the Sim, hold their lanes, collide, and are ranked and listed, but they
  aren't on screen. Filed as V128. So the user's "no bots" is only half fixed.
- **They don't race after the drop.** In mode 2 the race driver needs `Around::racing` (race, card, path
  net, track paths, gate, line), and `Pilot::think` passes `None`. Their load-time `Entry` is never
  refreshed from `Sim::race` either. That is S331b and S331c (noted on S331b). Until then they leave the
  grid on whatever the brain's levers hold, which isn't the original's race line.
- **The race words at the spawn are the port's choice.** `ai_entry()` gives live, no race phase, no path
  net and a `COMPUTER_CARD` card. No race capture shows what the original's seats hold at a supercross
  load (C33 is a free ride).
- **The event kind is taken as 0.** Kinds 0 and 1 both give mode 2. Which one a supercross event has was
  not read.
- **The AI riders ride the human's bike and `.phy`.** Which bike the original gives each AI rider was not
  read.
- **The generator is the port's own.** The roster and spawns draw from `Rng::from_seed(0)`
  (`race_start::AI_SEED`), apart from the race's. The original's comes off the wall clock and is shared
  with everything, and the order of the roster against the race reset's lane draws was not traced.
- **The difficulty is 90**, the event's default. The shell has no difficulty setting.
- **The rider database count 99** was read from RAM images, not from the disc's own file. Identities
  don't reach the lane yet (S331's note).
- **Only one-player races get AI riders.** With split-screen seats each seat is its own Sim (P350).
- **The seeker world's slots and open zones** are still the shell's own, players+1 and place 0 for
  everyone (S331a).
- The AI riders get no `mods.ride` seat mods. They are not a player's seat.

## Next

Landed. V128 makes them visible, and S331b/S331c make them race.
