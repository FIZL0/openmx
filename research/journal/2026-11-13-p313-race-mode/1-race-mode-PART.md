# 1 — race mode (PART)

Task P313. 2026-11-13.

## What I did

- **The race's lap line in the Sim.** `game_sim::race::Laps` (in race.rs, "a race's laps in a ride")
  holds what the race handler runs on: the score manager's rules (`supercross_rules`, the live sx01
  race's words), the course (the line, its spot, the lap length), the line lists, one card per rider
  (`Entry`: the player and its `RaceCard`), and the track's paths.
  - `Race` carries it as `laps: Option<Box<Laps>>`. `Race::tick` drops it with the gate (`Laps::start`:
    the manager's started bit, TAKING_PART on every card that takes a line test). From the tick after
    the drop, `Race::tick` runs `Laps::tick`: the opening look for a waited-for card, then each card in
    list order (its owner tells it where its rider is, then `card_on_lines`), then `LapRules::close`.
    `close` is now generic over the list's element, so `Laps` keeps the player beside each card.
  - The hash (`hash.rs`, the `laps` group) and the snapshot (`snapshot.rs`, wire lines for every type
    under `Laps`, plus `Arc<T>`) both carry it. With no race, or a race without laps, both are
    unchanged, so every Free Ride replay hash stays.
- **The shell's race.** `race_start::laps` loads the track's race file (`tracks\<stem>\<stem>.tdf`
  off the second archive). It snaps the nodes to the terrain and loads the paths. It moves the line
  onto the scene's finish-gate prop as the supercross loader does: the centre on the prop, the line
  spot found again, `path_ahead`, then `place_finish_line`. Then it makes the cards for the riders.
  It prints `race: laps on … — lap 4153.40, line at … on the finish gate`.
  - The HUD's `LAP n/N POS p/R` reads the human's card (`race_start::shown`).
  - `race_start::Watch` diffs the lap line every simulated frame, because the Sim keeps no frame.
    It prints `race: player N on lap L`, `… in place P`, `… finished in place P at T s`, and
    `race: over — places …`.
  - A finish at the line goes to `Supercross::race_over`, through the stadium's flashes
    (`Stadium::card_finishes`) where the track has them. Its fireworks answer calls `TrackFx::finish`
    (S192).
  - At the Over the shell builds `race_start::hand_over` (the cards in list order, `race_end`, the
    finishes' answers) and gives it to `Menu::race_ended`.
  - Seven seconds on (the activity's outro wait) the ride leaves to the menu, whose `race_left` opens
    the results screen with the race's own cards (P330's `Results::handed`).
- **P188 (the closed start gate)** is S13b's, already in: `Sim::race_frame` runs the gate watch and
  the push, and puts the penalty on the card. `race_menu.rs` checks that the rider stays behind the
  closed gate until the drop.
- **S192 (the finish fireworks)**: the finish fires `TrackFx::finish`. A restart (`Leave::Restart`)
  rebuilds the whole ride, track effects included, so it needs no separate reset.

## What the original does

- The race kind's handler (`./kit fn score_race_tick`) runs only once the manager's started bit is
  set. It looks for a waited-for card. Then for each card in list order, the owner's vt+0x20 slot 8
  tells the card its position, and the card steps on the lines. Then it runs the finish wait, the
  over-told draws, the rank and the gaps (ported earlier as `LapRules::close`).
- The supercross loader puts the line on the finish prop, finds its spot again, takes the path ahead
  and places the line across it. The lap length stays the track's, measured at load.
- The live sx01 race's manager words (score manager inline at act+0x50 in s282c-sx01-race):
  - flags 0x5e00, then 0x5e01 after the drop; kind 1, mode 2, laps 3;
  - record limit 10, finish wait 30, time limit 0, below limit −1, off limit 5, aim reach 1,
    ghost runs 1, checkpoint reach 20.
  - The player's card flags are 0x1880c06 and an AI's 0x1980c0a. Before the drop the cards hold
    TAKING_PART off, which is what `HUMAN_CARD` / `COMPUTER_CARD` hold.
  - The track's reach is 125 ahead and 75 behind.

- **The drop gate's motion (for P313b).** The track's `.toy` has a `[DropGate]` section, read by
  `./kit fn 0x002af2e0` (the same reader takes `[Funnel_Up]`, `[Funnel_Down]`, `[LowerGate]` and
  `[RaiseGate]`). It keeps its words in an object at the loaded world's (app+0x3cc) +0xb4:
  - GateName at +0, 0x40 bytes; LookVector +0x40; UpVector +0x50; RotationAxisLocal +0x60;
  - RotationAngleDegrees +0x70 (default 45.0); RotationRateInSeconds +0x74 (default 1.0);
  - MoveGateDown +0x78 (int); MoveTime +0x7c (default 5.0);
  - NumberSoultreePieces +0x80 and NumberCollisionPieces +0x84 (ints).
  - `InitialPosition` is in sx01's file but this reader does not read it.
  - sx01's values: axis 1,0,0, angle −60, rate 3.0.
  - What plays the rotation at the drop was not found. `race_drop`'s calls (see oracle.txt) do not
    read +0x70 or +0x74 through that pointer, so some object must keep a copy of it.
- **sx01's finish gate is a toy** (`[animatedtoy2] Name = FinishGate`, THQ_finishgatenarrow.mcf). The
  scene has only its collision objects, so the loader's toy fallback is the one that finds it.

## How it was proved

- `game-sim/tests/race.rs` `riders_walked_round_sx01_run_their_laps_to_the_over` walks two riders
  round sx01's paths from the grid gate's path (the human at 4 ft a tick, the computer's at 3). It
  checks the laps (3 each), the finish order (human first), the places (1, 2) and the Over. A Sim
  racing with laps round-trips the snapshot with the same hash.
- `openmx` `race_start::tests::the_watch_tells_laps_places_the_finish_and_the_over` checks the
  watch's lines, the outro's wait and the hand-over.
- `openmx/tests/race_menu.rs` rides sx01 headless with `--race`. It checks the lap line's load (lap
  4153.x, on the finish gate) and the first place after the drop. The free-ride test still prints no
  `race:` line.
- Free Ride's replay hashes: the gate's existing replay tests, unchanged.

## Not verified / not 1:1

- **No AI riders on the grid (the user's "no bots").** The cards take whatever riders the Sim has,
  which is the human alone until S12e spawns AI riders in the Sim. Split as P313a (after S12e).
- **No gate drop on screen.** The gate drops in the sim (it stops holding the riders), but the
  `THQsxdropgate` model stays drawn standing. What animates it in the original (its .psg, a toy, or
  the activity) is untraced. Split as P313b.
- **C23 (the crowd cheer on places)** is not done; C23 stays open with a note on where to hang it: PlayerCheer and the panned AI Cheer
  on a place gained after 5 s, and the boo. No track sets CrowdPresent, so nothing would be heard.
- **Not checked against a live race capture.** The lap line's step and close were each proved
  against the original by earlier tasks (S13c, S13d, S302 …), but this task's wiring of them, and
  each card's first words, are not:
  - A card's first spot, to-line and lap length come from its rider's grid spot found on the paths.
    The image shows the player at the gate with to_line_kept = lap_length = 4051.8, which the port's
    first words should give, but that was not compared.
  - The close's rand draws come from the race's own rng, not the global rand.
- **The S302 helpers** (card flags 0x800 / 0x1000 / 0x80000, the computer aim) are not run.
- **The finish prop's frame and bound.** When the finish gate is a toy (sx01), the prop is the toy's
  spec placement, and its bound is the box round its model's triangles at rest. The original reads
  the toy's model node and that node's own bound. A toy found this way has no LookVector, so its look
  is the placement's default.
- **No nodes are raised onto props** (S296a). sx01's file has none.
- **The results' trackers are zero.** Laps led, passes, wrecks, jumps, preloads and clutches read 0,
  because the riders' statistics trackers are not kept. The human's seat reports no preloads or
  clutches to `race_end`. The Hall of Fame is the default one, and AI riders are named "Rider N".
- **The watch's lap line** is the port's own: it diffs the cards' laps, places and OUT flag rather
  than hearing the lap line's notices, which the Sim does not hand on.
- **Not run in a release build here.** See Next.

## Next

- P313a, once S12e lands: give each AI rider a `COMPUTER_CARD` entry in `race_start::laps`
  and its own grid lane.
- P313b: trace what moves THQsxdropgate on the drop.
