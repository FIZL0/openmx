# Needs the human

Work no agent can do: it needs the disc, the hardware, an account, a controller in someone's hands, or a
decision about what this port should be.

**Agents add items here instead of guessing.** If a task cannot be finished without one of these, write the
item (what is needed, what it unblocks, and exactly how to produce it), block the task with
`./kit plan block <ID> "needs <item> — plan/HUMAN.md"`, and move on. A guess in `kit.toml` or in a ported
constant is far more expensive than a blocked task: it looks like work and it is wrong.

Write each item so it can be done without re-reading the whole project: the slot or file name to save to,
how long it takes, and what to do if it goes wrong.

## Before anything works

- [ ] **Dump the disc you own** to `game.iso` in the repo root (never committed; `.gitignore` has it). Note
      the serial, the build date and the file's SHA-256 in the journal — every address in this repo is only
      true of that one dump, and a different region's disc is a different game.
- [ ] **A PS2 BIOS from your own console**, in PCSX2's `bios/`. The kit will not download one and cannot run
      anything live without it.
- [ ] **PCSX2 2.x installed**, launchable as the command in `kit.toml` `[emu] pcsx2`, and able to boot the
      ISO by hand once before any agent drives it.
- [ ] **Ghidra with the EmotionEngine processor extension**, path in `kit.toml` `[ghidra] home`. Without it
      there is no decompilation and most of the backlog is impossible.

## Save states and recordings

Agents can load states and replay recordings, but only a person can play the game to the interesting
moment. Save states go in PCSX2's slots; by convention slots 3, 4 and 5 are the human's and are never
overwritten, 8 and 9 are scratch for agents.

- [ ] **The title screen** (slot 3) and **a mid-action moment** (slot 4), with a line here saying what each
      holds: characters, difficulty, any options that change the simulation.
- [ ] **Something that runs itself** (slot 5): a demo, attract mode, a CPU-versus-CPU match. This is the
      most valuable state you will make, because a capture that needs a person playing cannot be repeated
      exactly, and a port can only be proved against something repeatable.
- [ ] **One longer recording of real play** (a few minutes, PCSX2 input recording) for the acceptance test:
      start from the slot-4 state, play normally, include whatever is rare (a failure, a special move, a
      mode change). Save it under `context/recordings/` and note which state it starts from and which
      controller ports were used — a recording whose save state is lost is worthless.

## Decisions only you can make

- [ ] **What has to be exact, and what only has to look right.** The default in `AGENT.md` is: simulation
      exact, rendering by eye, front end free. Confirm or change it per area, once, and write it there so
      nobody relitigates it per task.
- [ ] **What this port is for** (a faithful remaster, a mod platform, a research record). It changes which
      tasks are worth doing, and in which order.
