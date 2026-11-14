# 1 — s13 laps (FINAL)

Task S301. 2026-11-13.

## What I did

- `race::RaceRules::join` / `RaceRules::tick` (game-sim/src/race.rs) are the score manager's card
  join and whole per-tick handler. `race::Laps` holds the rules, track paths, course, line lists and
  one `RiderCard` per rider, and `Race::tick` steps it once the countdown has dropped. It lives on
  `Race.laps`, which is on the wire and in the hash. What a tick told is gathered in `Told`, which is
  off the wire. Two fields were added to it:
  - `finished`: each card the line told `Finished`, as it stood after its step;
  - `over_list`: each rider and its place in list order when the close told `Over`.
- Lap length: `TrackPaths::load` routes the line spot to itself, as the track load does. `near_line`
  reads `track.lap_length`, not `Course::lap_length`.
- The shell's part (crates/openmx/src/race_laps.rs is new):
  - It reads the race file (`race_file_names(track)[0]`), stands it on the ground, and builds the
    course with C31's score-manager words. Every human rider gets a player card.
  - Each frame it takes `Told` and calls `Supercross::race_over` for each finished card, using the
    stadium's `Stadium::podium` flashes. Fireworks go to `TrackFx::finish`.
  - On `Over` it calls `Supercross::race_end` once, then hands the results to the menu
    (`race_ended`) with `ResultCard`s taken from the real cards.
- The HUD's lap and place now come from seat 0's card.
- `--race-win` and `Stadium::player_wins` are gone: the real finishes replace them (V127).

## What the original does

- Card join: `./kit fn 0x25f370`.
  - It masks the card's flags with `0x1a8801c` and sets `0x800`.
  - With no line flag, kind 3 or track word `+0x34` bit 4 gives `0x10000`, and bit 8 gives `0x20000`.
  - It zeroes the lap, timing, jump and penalty words, and resets the best lap unless the card or the
    manager keeps it.
  - It copies the manager's gate list and count.
  - It hands the card's records back to the free list.
  - In mode 2 it finds the card's spot on the path and measures to the line (`path_distance`, or the
    track's lap length with no line).
  - It tells the listener and owner in this order: owner 8, listener 0x6c, owner 0x5c, owner 0x70
    (only if the card has flag 8), listener 0x60, owner 0x50.
- Whole tick: `./kit fn 0x262aa0`.
  - It returns 0 with no cards head or no track.
  - Per card: owner vt+8, then the card step. Card flag `0x80000` goes to the air-time helper.
  - The tail picks one of four ends:
    - kind 3 without the finished flag → `./kit fn 0x2629d0`;
    - ranked → `./kit fn 0x261b80`;
    - finished → return 1;
    - otherwise → `./kit fn 0x2628f0`.
- The close's order: countdown, OVER (all cards OUT), Over told per card, then ranking and gaps. The
  close changes no place before it tells Over, so the places taken before it are the places at Over.
- The join's on-track word: `./kit fn 0x26c010` is called with out null and reach -1/-1. It then
  takes the first candidate and skips both distance walks (`if (found == 0 || out != 0)`), so the
  paths' scratch distances stay as they were. With a dummy out, the port's `find_spot` measured and
  wrote them. `TrackPaths::holds_any` is that null-out answer.
- C31's live sx01 single race:
  - manager flags `0x5e00`, kind 1, finish wait 30, record limit 10, ghost runs 1;
  - below limit -1, off limit 5, aim reach 1;
  - track reach 125 ahead and 75 behind; line-list reach 20;
  - human card flags PLAYER | `0x100_0000`.

## How it was proved

- `the_card_join_matches_the_score_managers_join` (crates/game-sim/tests/race.rs): an oracle of the
  join over 20,000 generated cards. It compares the card words, gate list, records, paths and the
  listener and owner call order.
- `the_race_handler_matches_the_score_managers_whole_tick`: an oracle of the whole tick over 10,000
  generated races of 1–5 cards. It compares list order, card words, flags, countdown, records, rand
  state, paths and every listener call, including the finish's.
- Shell unit tests: race_laps (the answers to a finish and to Over) and camera_flashes (a real
  race_over lights the burst).

## Not verified / not 1:1

- No PINE capture of a whole race was compared: the oracles cover the join and one tick, not a
  ridden race.
- Only human riders have cards. AI riders are not in `Race::tick`'s riders, so a solo race always
  places 1st (S328).
- The finish line is the race file's. The original's loader re-places it on the finish-gate prop,
  and the line lists are not raised on props (S329).
- `race_over` uses the Hall of Fame's default tables and track -1. `race_end`'s preload and clutch
  counts are 0 (S330). The event class is 0.
- The score manager's words are C31's, less flags `0x1000` and `0x800`, whose helpers are not ported.
  The `0x80000` air-time helper is not ported either.
- COMPUTER_AIM's aim (S302) is not ported, nor are the owner look's other words.
- The join's mode 1 and kind 4 branches are not ported, and its listener calls are only recorded,
  not acted on.
- These are not ported: score_grow, the leading card, the gate penalty word `+0x1c0`, and ghosts.
- The card handed to `race_over` is the card after its step, not as the lap line held it mid-step.
  Its race time is the same, because OUT cards do not tick.
- A rollback re-sim refills `Told`, so a re-simulated tick could tell a finish twice. The shell's
  answer block is not gated on `simulated`. Neither has been tried online.

## Next

S328 (AI riders' cards) first: until then a race's places and gaps mean nothing past 1st. Then S329
(the finish line on the gate prop) and S330 (Hall of Fame tables, preload and clutch counts).
