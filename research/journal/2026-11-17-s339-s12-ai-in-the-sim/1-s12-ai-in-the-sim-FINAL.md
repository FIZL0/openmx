# 1 — s12 ai in the sim (FINAL)

Task S339. 2026-11-17.

## What I did

- `Racer::tick`'s route (crates/game-sim/src/ai.rs) now runs the path branch on `seek::Seek::Path`
  instead of telling `"path"` missing:
  - New `Racer::path_branch(brain, net, world, race_word, rng, dt)`. It puts the brain's shared words
    into the follower (`Join::follower`), runs S309's `follow::path`, and takes them back
    (`Join::follower_back`).
  - The net is `Around::racing`'s `net` (the brain's one `+0xdc`, the same word the race driver
    reads). The side step's race word is `Racing::race_word`.
  - The goal is the seeker's target jump's setup leg (`seek::World::jumps[seeker.jump].setup`), or
    none without a jump.
  - After the branch the route goes on to the tail, as after a steered seek. The original's
    `ai_seek_path_end` (0x0038496c) is one instruction before `ai_seek_tail` (0x00384970).
  - With no net in `Around::racing`, `"path"` is still told missing.
- `seek::Leg` gains `spot: race::Spot`, the leg's `+0x50`: the setup leg's `+0x110` on the jump. The
  goal's spot is found once and kept on the jump, which every rider shares, as the original keeps it.
  The hit-zone reader and the four `Leg` literals in tests/ai/seek.rs set it to the default.
- Updated the `follow` module doc. It still said the path follower was not ported.
- research/oracle.txt: S339 note under the S282c path-net note (MachineRace's add-player).
- Filed **C41** (low): an image of a free-ride machine race, to run this proof on its own brain.
- Nothing taken from opentennis: it has no AI riders or path nets.

## What the original does

Which free-ride brain has a net:
- A brain reaches the path branch only with seeker state 0 and `+0xdc` set.
- The free-ride images (C33, recording2, S305's) all have `+0xdc` 0. The freestyle archives have no
  `<track>AI.tdf` (S299's disc scan).
- `./kit fn 0x003a24f0` is the add-player of the **MachineRace** activity:
  - vtable 0x0045d2b0; ctor `./kit fn 0x003a21b0` names it "MachineRace"; `0x0021b880` makes it.
  - For an AI interface (`+0x34 == 2`) with `+0x6c0 == 1`, it adds a card. Then it writes the
    `MXVehicleAIInterface`'s brain: `+0xd8` = act+0x50, `+0xd4` = the card, `+0xdc` = act+0xb8.
  - act+0xb8 is the race file. A freestyle archive's only race file is `<track>mr1.tdf`.
- So a free-ride brain meets a net in a machine race.
- An ELF scan for `+0xdc` stores just after an act+0xb8/+0xbc load finds only this one. The race
  load's act+0xbc → ai+0xdc copy goes some other way and was not found.

The branch itself is S309's (its journal has the disassembly). The goal is `+0x34c`'s setup leg:
point `+0xe0`, spot `+0x110`.

## How it was proved

`s339_the_ticks_path_branch_rides_to_a_target_jump_as_the_race_image` (crates/game-sim/tests/ai/path_branch.rs):
- It runs on S282c's sx01 race image: three AI riders, the image's real 444-node AI net.
- A target jump is poked into free RAM (0x01f80000, checked zero) as the brain's `+0x34c`. Its setup
  leg stands on one of 8 node points per rider: 4 on other paths, 4 a few nodes on along the
  rider's own path. That makes 24 goals.
- The original runs from `ai_seek_path_branch` to `ai_seek_path_end`. The port runs
  `Racer::path_branch`, with the image's words put in their homes (flag word, seeker state, aim,
  heading, speed, frame, own speed, reach, controller angles).
- Then both ride on along the heading at the target speed, until the seeker takes over or 300
  branches pass.
- Each branch compares, bit for bit: the follower's words read back out of the homes, the goal's spot
  at jump `+0x110` against the leg's, the net's scratch words and pools, and the generator.
- 3,189 branches match. All 24 goals were placed, 14 handed over to the seeker, and 32 path changes
  redrew.
- `--test ai` with filter s339 passes.

## Not verified / not 1:1

- **No free-ride image.** The proof runs on the sx01 race's brains (mode 2) and net, with a poked
  goal. No image of a free-ride machine race exists. Its racer's mode, seeker state, target jump and
  net were not read. Filed as C41.
- **The Sim never reaches the branch.** The Sim has no machine race, and its lane net is only a
  race's `<track>AI.tdf` (`Sim::give_ai_net`). So `Pilot::think` hands no `Racing` (and so no net)
  to a free-ride brain. This task wired `Racer::tick`, as asked. Feeding a machine race's
  `mr1.tdf` is for C41 to decide.
- **The whole tick with the branch was not oracled.** The seek before it (Seek::Path: the seeker
  oracle, path_net in 1 of 7 cases) and the tail after it (from `ai_seek_tail`) are oracled
  separately. The join is the one-instruction gap, read in the disassembly, not run.
- **The race load's `+0xdc` writer** (act+0xbc → ai+0xdc) was not found. It does not change the port.
- S309's notes carry over: the unset stack route-list head with a goal (the port starts empty, the
  same as a 0), and S311's cross-path spot. The new test did not hit the cross-path case; it would
  panic with a message if it did.

## Next

Landed. C41 for a machine-race image.
