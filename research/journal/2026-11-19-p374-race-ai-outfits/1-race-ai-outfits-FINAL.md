# 1 — race ai outfits (FINAL)

Task P374. 2026-11-19.

## What I did

- `roster::AiGarage` (crates/openmx/src/roster.rs) reads three disc files from `appstart.res`:
  - `rider\gear.dat`'s `[Uniforms]` items, each with its pro flag;
  - the race class's `bike=` list from `bike\bikes.dat`;
  - every rider of `rider\riders.dat` (number, first uniform, class bike).
- `AiGarage::draw` ports the original's gear and bike draws on the race generator. `AiGarage::pick` turns one draw into a garage `Pick`:
  - the drawn gear set and bike;
  - on the jersey and plates, the rider's second name cut to 12, and their number.
- `race_start::draw_roster` does the roster plus these draws. `spawn_ai` now calls it, so the draws sit between the roster and the bikes' engine-note and brain draws, where the original makes them.
- main.rs, at the race load:
  - draws the same thing once more on a copy of the race's generator, before the rigs are built;
  - dresses each AI rider with `dress_tagged` (`dress_peer`'s body, given a tag and a font source), under `@ai<n>` names;
  - `AiRig` keeps its own bike, parts and body models, and `ai_actors` draws them;
  - if the draw the brains' spawn makes differs from the one the picture used, the load prints a line. It never did.
- S363's engine-note build order now gets each AI rider's real bike archive, not the human's bike for every racer.
- Took nothing from opentennis: its outfits are picked by index from the command line, with no draw on a game generator.

## What the original does

Full detail, with addresses, is in research/oracle.txt under "P374". In words:

- Right after it spawns the racers, the vehicle loader makes extra draws on the shared rand(), one set per AI rider:
  - **Gear:** a uniform index over the 80 `[Uniforms]` items. It draws again while the item is a pro rider's set (`ProRiderUniform=T`, items 34–79), and draws the whole step again while an earlier AI rider already has it.
  - **Bikes:** after all the gear, a bike index over the class's list. It draws again on a repeat only in the 125 and 250 classes.
- These draws happen on every load, but the names are only used on a front-end boot that is not a career round.
  - In a career round the rider's own riders.dat uniform and bike are used.
  - A direct boot by level type (the c31/c40 images) takes fixed tables instead.
- The AI rider's nameplate:
  - name: second word of the rider's name, cut to 12;
  - number: riders.dat `Number=`;
  - fonts: from the rider's **own** riders.dat uniform and bike, not the drawn ones. The port does the same.

## How it was proved

- `./kit check -- -p openmx --test garage`: 19 passed. That includes three new unit tests in roster.rs:
  - list parsing;
  - the draw over 40 seeds against a plain replay of the generator, including where it leaves the generator;
  - the pick's name and number.
- Also in that run, the new disc test `the_ai_garage_is_the_originals_tables`: every list read off the disc equals the original's RAM tables in the c31 image. It ran (checked with `--nocapture`; no skip line).
- `./kit check -- -p openmx --test race_menu`: 5 passed. `--test menu`: 331 passed.
- Two races (headless shots, kept in context/shots/p374, not committed):
  - sx01, 250 class, 4 racers (`a.png`, `front.png`, `back.png`): Howard #73 in Azonic1 on a YZ450F; Becker #90 in Skin1 on a KX250; Henein #38 in Rainbow1 on a YZ450F ProCircuit.
  - sx17, 125 class, 6 racers (`b.png`): five different gear sets and bikes.
  - `back.png` shows "HOWARD 73" on the jersey and 73 on the plates.
- Beside PCSX2: I loaded c31's race-start state live in kit copy 2 and saved slot 9 a few seconds on (`pcsx2-live2.png`, the original's own frame). Both grids show each AI rider in different gear on a different bike, with a number on the plates. I checked this by eye.
- `--race-bot` race loads and rides 200 frames with the draws in.

## Not verified / not 1:1

- **The look comparison is by eye only.** The c31/c40 images are direct boots, which use fixed tables. So no capture shows a front-end race's drawn outfits to compare one for one.
- **The draw's place among the load's draws is the port's own order.** The draws are right after the roster, as in the original. But the port's lay-out draws before the roster are still S333's guess. No capture checks the generator after the draws.
- **The seed is the port's** (RACE_SEED 0, as before). The original's comes off the clock. So every race with the same track, class and racer count dresses its bots the same way. Different setups give different looks (sx01 vs sx17 above).
- **The full-detail gear is drawn.** The original swaps in the `_AI` low-detail rider model.
- **Career rounds' own gear is not ported.** The shell has no career races.
- **All racers share one engine-note sound set**, the human's bike's `.aud`, and one AI character file. The original may read them per bike. Only the build order now follows each bike.
- **A draw of rand()'s top value** gives index == count, which the original reads past the end of the list. The port keeps it on the last item.
- **A disc with too few open sets or class bikes** would make the original loop forever. The port skips the draws, and the bots wear the human's look.
- **GPU path only**, as V128: the software path draws no AI riders.
- **The bot's jersey name is printed as the file writes it** ("Howard"). The font draws it in capitals, as the original's does in its frames.

## Next

none
