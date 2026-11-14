# 1 — per card helpers (FINAL)

Task S302. 2026-11-14.

## What I did

- Ported the race handler's last per-card helpers into `crates/game-sim/src/race.rs`:
  - `RaceRules::jumps`, the jump watch, on card flag `WATCHES_JUMPS` (0x80000);
  - `RaceRules::off_warning`, on manager flag `OFF_WARNINGS` (0x800);
  - `RaceRules::wrong_way`, on manager flag `WRONG_WAY_WARNINGS` (0x1000);
  - `RaceRules::computer_aim`, on card flag `COMPUTER_AIM` (0x800000). It replaces the kept-spot aim and
    reuses `ai::follow::look_ahead`.
- New card words: `Jumps` and `Warnings` on `RaceCard`. New manager words: `WatchRules` on `RaceRules`,
  with sx01's values in `supercross_rules`. New notices on `RaceNotice`. All of them are added to the
  snapshot wire.
- `card_on_lines` now runs these in the original's order:
  - the jump watch after the odometer;
  - the computer aim where the kept-spot aim was;
  - the two warnings after the way back and before the time limit.
- Oracle names `score_card_jumps`, `score_card_off_warning`, `score_card_wrong_way` and
  `score_race_aim` are in research/oracle.txt, with the decoded behaviour.

## What the original does

See research/oracle.txt under "S302 — score_race_tick's per-card helpers". Recipes:
`./kit fn 0x00267d90`, `./kit fn 0x00268350`, `./kit fn 0x00268660`, and `./kit dis 0x00263628` up to
0x002638f0 for the computer aim inside `score_race_tick`.

Points worth knowing:
- The jump watch clears the **manager's** 0x100000 bit, not the card's. So a landing that waits to be
  judged stays set on the card, and its bests are tested every tick from then on.
- The computer aim gives `path_look_ahead` a null draws pointer. On a fork that picks by draw, the
  original reads the word at address 0, which is 0x3c1a8001 in C31's image. The port uses that constant
  (`NULL_DRAW`).
- Notice order: takeoff and landing go to the owner and chain first, then the manager's listeners. The
  bests and the warnings go to the listeners first, then the owner and chain.

## How it was proved

- Function oracles in tests/race.rs. Each runs 20,000 generated cards and managers with the listener
  hooks in place, and compares the card words, card flags, manager flags and every listener call
  (including the jump's 0/1 argument). One test per function:
  - `the_jump_watch_matches_the_score_cards` (0x00267d90);
  - `the_off_track_warning_matches_the_score_cards` (0x00268350);
  - `the_wrong_way_warning_matches_the_score_cards` (0x00268660).
- `the_race_handler_matches_the_score_managers_per_card` runs the whole `score_race_tick` with the
  computer aim, the helper flags, the manager words and node `+4` bit 0 all randomised. It asserts more
  than 1000 computer aims, more than 300 that walk past a marked node, and more than 300 where the
  look-ahead found a place.
- `the_race_handler_holds_c31s_live_sx01_laps`: both of C31's live sx01 races, every card every tick
  (25,058 steps on the gas capture), now also compared on the aim spot, the jump words and the warning
  words. The rider's own inputs are taken from the captured card: the ground bit 0x400, the height
  `+0x104` and the look `+0x120`. `C31_UNPORTED_FLAGS` shrank from 0x183ce0 to 0xe0, which leaves only
  the listeners' bits.
  - The card's owner turns the jump watch (0x80000) off and on, and clears 0x100000 while the watch is
    off. That happens on the human's card before the handler and on a computer's card after it (42
    turns on the gas capture). On a tick where either row has the watch off, the test runs the card
    both ways, keeps the run whose jump words match, and takes those two bits from the capture.
- The on_path long-way exemption could not be removed: one tick is left on the gas capture (frame 293,
  the human). With the exemption turned off, that is the only word that differs.

- The gate went red in `openmx --test machine_packs` (six tests), which this task never touched: the
  slot's installed `mods/machines/mxvatv_cars` pack lists `cars`, which only main's reader knows about
  (MD39c, bbc5589c9). I took main's `crates/openmx/src/machine_packs.rs` unchanged
  (`git checkout bbc5589c9 -- …`; HEAD's copy was the fix's parent). This is that file only, not a merge of main.

## Not verified / not 1:1

- **The live race does not feed the rider's words.** `race::Laps::tick` never writes the card's look,
  ground bit or height. So in a live ride:
  - the jump watch sees the rider standing throughout;
  - the wrong-way warning sees a zero look (facing 0 <= 0), so its clock runs and the warning shows
    after a second.
  These are card state only: nothing outside game-sim reads the notices. Follow-up S302a. The C31 test
  takes these words from the capture.
- **Some names are inferred**, from how the code uses each word and not from any symbol: `GROUNDED` /
  `WAS_GROUNDED` (0x400 / 0x800), `Jumps::rise` (+0x104, read as height above the ground), and the
  warnings' meaning (off-track / wrong way).
- **The owner's handling of 0x80000 / 0x100000 is not ported.** It belongs to the card's owner (the rider
  object) and is outside the handler. The C31 test treats those two bits as inputs on the 42 ticks where
  the watch turns, and on ticks with the watch off.
- **The null draws read is a constant.** `NULL_DRAW` (0x3c1a8001) is what C31's image holds at address 0.
  Another boot's kernel could hold something else there. It only matters on a fork with path flag 0x20.
- **One branch of the computer aim was never reached.** After the look-ahead finds a place,
  `spot_point` failing on it would leave the original reading a stale stack quad. The port starts from
  (0,0,0,1) instead. This can't happen, because the look-ahead never answers with a path's last node,
  but no test reaches it.
- **The listener list past its second entry** (manager listener +4 chain) is exercised only up to two
  entries in the function oracles. The whole-handler test has one.
- **Why the long-way exemption is still needed (frame 293) is a guess.** It is put down to path scratch
  left by the AI riders' own path searches, which are unported. Not proven.
- **The C31 check on the second capture** (sx01-ai4-by-rider.csv) passes, but I didn't read its own
  counts (jump watches turned, long-way ticks), because a passing check prints no log.

## Next

Nothing for this task. S302a feeds the rider's words in the live race.
