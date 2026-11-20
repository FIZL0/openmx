# 1 — machine race brain (FINAL)

Task C41. 2026-11-20.

## What I did

- **Found the image instead of making one.** Every free-ride image already holds a MachineRace activity:
  the activity manager's +0x58 array, slot +0x20 (the factory's ninth). Its +0xb8 is a loaded race file
  in c34-free06-boot, recording2, s133-free10, s196-free11, s342-baja-free07 and first_recording.
- The event decides whether a machine has a racer. first_recording and recording2 (both free09) have
  event +0x11c = 4 and +0x120 = 1. That is the "Career Machine Race" branch of machine placement
  (`./kit fn 0x0020f6e0`). With +0x120 set, it walks `Machine1..NumberOfMachines` (1 on every freestyle
  map) for the section with `MachineRacer = TRUE`, then reads `HasPlayer` and `HasAI`. On free09 that is
  `[Machine1]`, a Monster at 4594, 135, 3838. Every map's Machine1 is its racer: free06 Helicopter,
  free07 Baja, free09 Monster, free10 Biplane, free11 Buggy.
- **The racer's brain in first_recording** is the truck's +0xb30, at 0x01061340:
  - +0xd0 mode = **4**.
  - +0x354 seeker state = **0**.
  - +0x34c target jump = **0**.
  - +0xdc net = **0x0183a360** = MachineRace +0xb8. That is `Tracks\free09\free09MR1.tdf`, one path of
    118 nodes that links to itself (path flags 0x41).
  - +0xd8 = MachineRace +0x50 (race word +0x5c = 0x15000). +0xd4 = its card 0x01c83b40.
  - The card's flags are 0x80808. The taking-part bit 0x2 is clear, so the gate driver lets go
    (`let_go`): the race had not started. The truck sits at its spawn, its spot not placed (+0x220 = 0).
- **Ran S339's proof on that brain and net.** New test
  `c41_the_ticks_path_branch_rides_the_machine_racers_own_net` in `crates/game-sim/tests/ai/path_branch.rs`.
  It does S339's loop on the image's own brain and mr1 net. There are nine runs: eight with a target jump
  poked into free RAM (0x01f80000, checked zero), its setup leg on a node at 0, 1/8 … 7/8 of the way along
  the path, and one with no jump. Each run goes up to 300 branches, the truck moved along its heading at
  its target speed between them. Each branch compares the follower's words, the goal's spot, the path's
  scratch words, the pools and the generator. Result: 2,602 branches, all bit-exact. The spot was placed
  on the first branch in all 9 runs, the goal's spot in all 8, there were 122 node moves, and one run
  handed over to the seeker.
- **Did not save a running-race image.** I looked for what starts a machine race and did not find it in
  the time I gave it:
  - Starting is MachineRace's vt(+0x44)+0x40 (`./kit fn 0x003a3320`: shows the gates, the first one
    bright). The activity manager's switch (`./kit fn 0x0021c1b0`, its 6th argument) calls it.
  - The switch's callers are a menu/restart handler (`./kit fn 0x0028be30`), two message-box callbacks
    (`./kit fn 0x003601b0`, which also handles the machine switch for a +0x6c0 == 1 player, and
    `./kit fn 0x003626a0`), MachineRace's own gate listener (`./kit fn 0x003a4140`) and others not read
    (0x00338780, 0x0033e10c, 0x003deb70).
  - Which in-world action raises it was not traced.

## What the original does

- **The racer's brain** is made by machine placement through `ai_mx_spawn` (`./kit fn 0x00384cc0`) with
  mode **4** as its argument. Only an argument of −1 picks the mode from the event (2 race, 5 free ride).
  So a machine racer's mode is 4 from the start, whatever the session.
- MachineRace's add-player (`./kit fn 0x003a24f0`) then gives it +0xd8/+0xd4/+0xdc: the race, the card
  (category 0x80008), and the race file. This is S339's oracle note.
- `ai_update` (`./kit fn 0x00383000`) runs the seeker only on mode 5. Other modes go to the gate and race
  drivers. Mode 4 is `ai::drive::mode_four`: with a net, it places the spot on the net and follows it
  (`route_follow`) toward the card's next gate. It never calls the seek, so +0x354 and +0x34c stay 0. The
  free-ride path branch (`Seek::Path`, S339) is reached only by a mode-5 brain with a net. The free-ride
  riders of every image have +0xdc = 0 (S339), and the one free-ride brain with a net is mode 4.
- **So the Sim needs no machine-race lane net for the path branch.** A machine race in the Sim means the
  mode-4 driver on the race file and its gate chain. That is S423 (filed, `later`). I added these findings
  to it as a note.

## How it was proved

- Save-state RAM image: first_recording's (PCSX2, free09, the human's own session). Read with a scratch
  script: the brain's words, the MachineRace's +0xb8/+0x50, the card's flags, and the net's size.
- Function oracle: `ai_seek_path_branch` → `ai_seek_path_end` against `Racer::path_branch`, 2,602 branches
  on the image's brain and net, bit-exact (`./kit check -- -p game-sim --test ai c41_`).
- Mode 4 for a machine racer: read from the spawn's code (the constant argument). The image agrees.

## Not verified / not 1:1

- **No image of a running machine race.** The race's start trigger was not found (see above). The brain
  was read waiting, with the card not taking part. Mode 4 is fixed at spawn and the gate driver never
  calls the seek, so the answer does not depend on the race running. But a running brain's words were not
  seen.
- The test pokes the goal jumps and moves the truck by a stand-in ride, as S339's does. The original
  never runs this branch on this brain.
- `ai_update`'s mode dispatch (mode 5 → seeker, else the drivers) is taken from S281/S12e's notes and the
  port's `ai::update`, not re-read here.
- Other freestyle maps' racers (free06 helicopter, free07 baja, free10 biplane, free11 buggy) were not
  imaged in a machine-race session. Their machine.txt files name a Machine1 racer, and the spawn gives
  every one mode 4. The flying ones (helicopter, biplane) may drive through another interface type;
  not read.

## Next

Nothing for C41. S423 owns running a machine race in the Sim. It needs a running-race state, which needs
the start trigger found first.
