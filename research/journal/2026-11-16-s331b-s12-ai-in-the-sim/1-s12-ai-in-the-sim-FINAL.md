# 1 — s12 ai in the sim (FINAL)

Task S331b. 2026-11-16.

## What I did

- `ai::Pilot::read_laps(laps, net)` (crates/game-sim/src/ai.rs) copies the race's words onto the pilot
  each tick:
  - the card's flags and progress from the player's lap card (settle and bike word stay the pilot's);
  - the race phase from the laps' race kind;
  - the path net from whether the lane has one;
  - live from "has a card" and the manager's started bit (`ai::RACE_STARTED`, set by `Laps::start`).
  A player with no lap card gets no card and is not live.
- `ai::racing_of(laps, player, net)` builds `Around::racing` for the race driver (mode 2) and the gate
  driver (mode 4): the race kind, the race track's reach and line spot, the lap card's flags,
  distances, on-track word, seen points and kept spot, the race track's paths, the line gate's centre
  and normal, and the manager's flag word.
- `ai::Lane::net` holds the track's AI net, and `ai::race_net(file, terrain)` builds it: the file
  stood on the terrain, `TrackPaths::load`, and the reach set to `RACE_REACH` (125/75).
  `Sim::give_ai_net` hands it to the lane.
- `Sim::think` (crates/game-sim/src/tick.rs) calls `read_laps` then passes `racing_of` to
  `Pilot::think`, which takes a new `racing` argument. With no race or no laps nothing changes: the
  Entry words stay, and `racing` is `None`, as before. The race's tick runs before the riders', so the
  brains read this tick's laps.
- The shell (crates/openmx/src/main.rs, race_start.rs) reads `tracks\<stem>\<stem>AI.tdf` after
  spawning the AI riders and hands the net over (`race_start::give_ai_net`). It prints
  `race: AI net …, N paths`.
- Nothing taken from opentennis: it has no AI riders on a path net.

## What the original does

- The brain holds pointers, not copies: `+0xd4` its card, `+0xd8` the score manager, `+0xdc` its AI
  net. Every tick reads them live.
- The seeker's hold (`./kit fn 0x00383580`, its first two tests) reads the manager's `+0x60`, the race
  kind (4 = fast lap), and the card's `+8` flags (bits 0x100 and 2). So "race phase" is the race kind.
- The hand-off (`ai_hand_off` in research/oracle.txt) lets go of the levers when the rider's
  `+0xf9c` (its card) is null or its `+0xfa0`'s `+0x5c` bit 0 is clear. The image shows `+0xf9c` and
  `+0xfa0` are the brain's own card and manager, so live = card and the manager's started bit.
- The race load reads `<track>AI.tdf` into the act's `+0xbc` and sets its reach to 125/75, as it does
  for the race file (S282c's journal). The image shows its lap length is measured too.

## How it was proved

- **Save-state RAM image** s282c-sx01-race.ee.bin (sx01, frame 604, a live race with three AI
  riders): `s331b_the_lane_reads_its_race_where_the_brains_do` (crates/game-sim/tests/ai/path_nets.rs).
  - `ai::race_net` on sx01AI.tdf off the disc against the act's `+0xbc`: tag word, reach ahead and
    behind, fork pick and lap length bit for bit, and every path's flags, length, speed word, links and
    node count. (The first try used `TrackPaths::new` and failed on the lap length: 0 against the
    image's measured one. Now it uses `load`.) Node points were already proved by S310's test.
  - For each of the three brains: rider `+0xf9c`/`+0xfa0` equal the brain's card and manager; the
    brain's net is the act's `+0xbc`; the manager's kind is 1 (race) and its started bit is up; the
    card takes part.
- **Unit test** `s331b_a_pilot_reads_its_race_off_the_laps` (crates/game-sim/tests/ai/lane.rs): not
  live and not taking part behind the gate; live and taking part after `Laps::start`; no card for a
  player without one; `racing_of` carries the same card and race words.
- **Shell ride** `a_race_of_four_has_three_ai_riders_behind_the_gate` (crates/openmx/tests/race_menu.rs)
  now also checks the AI net loads (38 paths) and that an AI rider takes first place after the drop.
  Checked it ran (no skip). By hand, a 12 s `--racers 4` ride on sx01: places change all through the
  race after the drop.

## Not verified / not 1:1

- **The AI riders' race is not compared against a capture.** No PCSX2 recording of a supercross race
  with AI riders was compared tick by tick with the Sim's. The words they read are proved (above), and
  the drivers themselves were proved by S314b on the same image, but the whole ride is not.
- **The card is a copy, not a pointer.** The original's brain writes into its card (the habits' air
  time and racing bit, `0x8_0000`). The port keeps those on the pilot (`Pilot::think` merges them) and
  never writes them back to the lap card, so the laps don't see them. Nothing in the laps reads them.
- **The card's next gate is `None`.** The laps hold no chain of gates, so a gate race's mode-4 driver
  would have no gate. Supercross races (mode 2) don't read it.
- **The net's scratch pools** (route, parent and candidate counts) were not compared with the image:
  the image's are after 604 frames of queries.
- **The nodes the props would raise** stay on the terrain (`raise_on_props` not asked), as the shell's
  race file does. sx01's AI file has none (S310); other tracks were not checked.
- **One net for all three brains.** The image shows all three share the act's `+0xbc`. The drop-in
  branch that points `+0xdc` at the race file instead (S282c's note) is not ported.
- **Without laps the words stay static.** A race with no laps (a start alone) or a free ride keeps the
  spawn's `Entry`, as before. What the original's free-ride manager holds was not read again (C33 shows
  live).
- **Edge, node and spot are still not fed** (`Around::edge`, `node`, `spot`): that is S331c.

## Next

Landed. S331c feeds the avoidance's edge and the habits' node and spot from the nets.
