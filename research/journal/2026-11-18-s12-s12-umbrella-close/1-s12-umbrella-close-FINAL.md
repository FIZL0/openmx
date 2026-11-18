# 1 — s12 umbrella close (FINAL)

Task S12. 2026-11-18.

S12 is the umbrella for the opponent AI and the race activities. Its pieces were split into their own
tasks and have landed. This session checked the brief against what is on main, filed the one piece no
task held, and closes the umbrella.

## What I did

- Walked the brief's parts against the plan and the code:
  - AI riders: roster draws (S12a, S12f), route and path nets (S12b, S282*, S308, S309, S310), avoidance
    (S12c), tricks (S12d), the brain's tick and spawn (S281, S283, S306, S313, S305), the Sim's AI lane
    and race driving (S12e, S322, S323, S326, S331*, S339 and the rest). All ticked in plan/DONE.md.
  - The start gate and its drop (S13b, S13e, P313b), laps and checkpoints (S13c), results (S13d), the
    Supercross/Nationals activity class (S13a, S13f). All ticked.
  - The Ghost activity class: no task held it. Filed as **S12g** (`later`).
- Read the activity factory (`./kit fn 0x0021b880`) and the ELF strings it compares, to name the
  Ghost class for the follow-up. The names, in order: Supercross, Nationals, Ghost, HitMaker,
  HitFreeride, HitProgression, HitFastLap, Run (Run1/Run2), MachineRace, StuntCompetition.
- Named the Ghost activity's constructor and loader in research/oracle.txt (`activity_ghost`,
  `activity_ghost_load`).
- Open AI follow-ups already filed by others stay as they are: S347 (C33 parts at ticks 133/148),
  S303 (generic vehicle avoidance), S311 (route spot on another path's node).

## What the original does

- `Ghost` → `./kit fn 0x0031a510`: builds on the base race activity (`./kit fn 0x002f7da0`), sets its
  own vtables, zeroes five words at +0x3a4..+0x3b8. 0x3c0 bytes.
- Its init `./kit fn 0x0031a620`: kind words +0xe4 = 5, +0xb0 = 3, clears a flag bit, runs the base
  setup, builds the gate props ("finishgate", "dropgate", or a name from the loaded track) and turns
  the drop gate its whole way at once (`./kit fn 0x002ffc50`).
- The score cards' ghost-race rules (race kind 3) are already ported in race.rs (S13c/S13d).

## How it was proved

Nothing new to prove: no code changed. The audit is a read of plan/DONE.md and the code. The factory's
names were read from the ELF's strings.

## Not verified / not 1:1

- The Ghost race as a whole (its activity, recording the best run, the ghost rider's playback): not
  ported. Filed as S12g, parked `later`: the user asked for races with bot opponents, which landed.
- What the `FastLap` activity set (event type 4) builds: not read. Its resource on the disc lists the
  classes; S12g asks for it to be read and filed.
- Whether the retail front end can start a ghost race at all: not checked.
