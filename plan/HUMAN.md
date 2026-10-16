# Needs the human

Work no agent can do: it needs the disc, the hardware, an account, a controller in someone's hands, or a
decision about what this port should be.

**Agents add items here instead of guessing.** If a task cannot be finished without one of these, write the
item (what is needed, what it unblocks, and exactly how to produce it), block the task with
`./kit plan block <ID> "needs <item> — plan/HUMAN.md"`, and move on. A guess in `kit.toml` or in a ported
constant is far more expensive than a blocked task: it looks like work and it is wrong.

Write each item so it can be done without re-reading the whole project: the slot or file name to save to,
how long it takes, and what to do if it goes wrong.

## Right now: a freeride state and a freeride recording

**Priority 1 is freeroam**: riding a dirt bike around a freeride area and doing tricks, with the
original's physics. Racing, opponents and the front end are stretch goals (backlog S12 and S9), so the
asks below changed shape — slot 4 is now a freeride area rather than a supercross start gate, and the
acceptance recording is free riding rather than laps.

The game boots under the kit (C1 is done), so the items under *Save states and recordings* below are
waiting on you, with a recipe each. The two that matter most are slot 4, which every physics comparison
starts from, and the recording, which is the only thing a port can be proved against exactly.

One thing may shrink this list: backlog **R7** drives the original straight into a freeride area by
writing StartMode (`app+0x408`) and the event kind, with no menus and nobody playing. If that lands, the
agents can make their own freeride starting point and slot 4 becomes a convenience rather than a blocker.
Slot 3 (the main menu) is now only worth making if you want a stable copy — nothing on the critical path
goes through a menu.

## Before anything works

- [x] **Dump the disc you own.** Done: `MX Unleashed (USA).iso` in the repo root (git-ignored),
      2,219,933,696 bytes, sha256 `fc64c537824f616277b821fd24da58949d13fe83b2e3be89ddd58a89795e1aa2`,
      serial `SLUS-20767`, US, version 1.00. Recorded in
      `research/journal/2026-10-15-d1-the-disc/`. **Every address in this repo is only true of that dump**
      — a PAL or 1.01 disc is a different game.
- [x] **A PS2 BIOS.** Done: three dumps supplied in `PS2_bios/`, copied to `~/.config/PCSX2/bios/`.
      `SCPH-70012.bin` (slim, US) is the region match for this NTSC-U disc and is the one in use;
      `SCPH-77008` and `SCPH-90006` are PAL. A PAL BIOS with an NTSC disc gives the wrong refresh rate,
      which would quietly poison every frame-timing measurement in the port. BIOS dumps are copyrighted
      and `.gitignore`'s `*.bin` keeps them out of git — the repo-root `PS2_bios/` copies are redundant
      now (git ignores them, so they are harmless) and can be deleted whenever you like.
- [x] **PCSX2 installed.** `pcsx2-latest-bin` 2.9.127, `/usr/bin/pcsx2-qt`, in `kit.toml` `[emu] pcsx2`.
      The game boots and reaches the main menu under the kit — `context/shots/c1-title.png`, taken by
      `./kit shot` — so there is nothing left to confirm by hand. Three settings had to be put in
      `~/.config/PCSX2/inis/PCSX2.ini` first, and will have to go back if that file is ever reset:
      `SetupWizardIncomplete = false` (otherwise every launch opens the first-run wizard and silently
      ignores the ISO), `BIOS = SCPH-70012.bin`, and `ConfirmShutdown = false`.
- [x] **Ghidra with the EmotionEngine extension.** Ghidra 12.1.2 at `/opt/ghidra` with
      `ghidra-emotionengine-reloaded` v2.1.38; the boot ELF is analysed (2m57s) and
      `./kit fn`/`xref`/`dis` work. One wrinkle, already fixed, worth knowing if Ghidra is ever
      reinstalled: the extension ships `r5900.slaspec` but no compiled `r5900.sla`, and Ghidra tries to
      compile it next to the spec in root-owned `/opt`, which fails. Fix:
      `sudo /opt/ghidra/support/sleigh -a /opt/ghidra/Ghidra/Extensions/ghidra-emotionengine-reloaded/data/languages`
      then `sudo chmod -R a+rX /opt/ghidra/Ghidra/Extensions/ghidra-emotionengine-reloaded` — the sleigh
      run writes the `.sla` root-readable-only, so without the chmod it fails identically.

## Save states and recordings

Agents can load states and replay recordings, but only a person can play the game to the interesting
moment. **These are the things waiting on you right now.** The slot convention is in `plan/REFERENCE.md`:
3, 4 and 5 are yours and no tool ever writes them; 8 and 9 are scratch and agents overwrite them without
asking.

How to make any of them, the same four steps every time:

1. Boot the game in **your own** PCSX2, not a kit copy — a copy is muted, runs on its own config and is
   thrown away with the agent's worktree. From the repo root, `./kit pcsx2 start` with `KIT_PCSX2` unset
   launches your own PCSX2 on the right ISO; starting PCSX2 by hand and opening
   `MX Unleashed (USA).iso` does the same job.
2. Play to the moment described below.
3. Save into the slot: PCSX2's `System → Save State → Slot N`, or `F1` after picking the slot with `F2`
   if your hotkeys are still the defaults. The file lands in `~/.config/PCSX2/savestates/` as
   `SLUS-20767 (9866FE54).0N.p2s` — about 6–12 MB.
4. Fill in the state's row in `plan/REFERENCE.md` → *What each save state holds*, and tick the box here.
   What an agent cannot work out for itself, and must not guess: the bike, the difficulty, which
   controller ports were plugged in, and whether a profile was loaded.

Two things that bite:

- **Agents do not see a new state until someone re-runs `./kit pcsx2 copy N`.** Each copy got a snapshot
  of your savestates folder when it was made. Re-running the copy command pulls new states in; that is
  the agents' job, not yours, but it is why "I saved it an hour ago" and "the agent cannot find it" are
  both true at once.
- **A state belongs to the PCSX2 version that wrote it** (2.9.127 today) and to this ISO. After a PCSX2
  upgrade an old state may refuse to load, and the fix is to make it again — which is the other reason
  each one has a written recipe.

- [ ] **The main menu** (slot 3). Boot, press Start past the attract video, the warning screen and the
      controls diagram, and save on the menu with nothing selected. About two minutes. Agents can reach
      this screen by themselves (C3 did: six Start presses, three seconds apart, from a cold boot) and
      scratch slot 9 already holds one — so this slot is only worth making as the stable copy that no
      tool will ever overwrite.
- [ ] **In a freeride area, stopped on open ground** (slot 4) — whichever freeride area the game lets you
      into soonest. This is the state every physics comparison starts from, so make it boring on purpose:
      bike upright, stationary, engine running, pad down, nothing in the air and no jump in the next
      second. Somewhere with a lip or a table within riding distance is ideal, since the same state has to
      serve S3 (ground), S10 (air and landing) and S11 (tricks). Note here which bike you were on —
      `bike\*.phy` parameter sets differ, and the port has to know which one it is being compared
      against — and which freeride area it is. About five minutes.
- [ ] **Something that runs itself** (slot 5): attract mode, or a race left to the AI. The most valuable
      state you will make, because a capture that needs a person playing cannot be repeated exactly, and
      a port can only be proved against something repeatable. Save it a few seconds *before* the
      interesting part starts, put the pad down, and note here how long it runs for and what ends it.
- [x] **One longer recording of free riding** for the acceptance test (S8): start from the slot-4 state
      and just ride — no laps, no objectives. Include the awkward things, because they are what the port
      will get wrong: a jump taken too slow and one taken too fast, a trick landed clean, a trick landed
      sideways, a crash, a wall, a slow scrape along a slope, and a few seconds of sitting still. Save
      under `context/recordings/` with a line here saying which state it starts from and which controller
      port. A recording whose save state is lost is worthless.

      What this proves: the same button presses fed to the port produce the same bike position, frame by
      frame. It is the only test that can show the physics really match rather than merely look right.

      **Done — `context/recordings/first_recording.p2m2`** (+ `first_recording.p2m2_SaveState.p2s`, which
      is the state it starts from; the recording is anchored to it, so keep the pair together). Measured
      from the files, not reported: PCSX2 2.9.127, author `rycs`, 3621 frames — about 60.4 s at 59.94 Hz —
      and both controller ports recorded (570-byte header, then 18 bytes per frame per pad).

      What the human says is in it: some tricks, and **a collision with an NPC rider** at some point. The
      save state's RAM says the rest: the area is **Free09** (`Tracks\Free09\Free09.ps2.trn`) and the bike
      is a **125cc, Race tier** — `Bike\Race125A.phy` and `Bike\Race125P.phy` are the only bike physics
      pair resolved in RAM. `Bike\Wreckbike.phy` is loaded too, which is the crash bike.

      Two consequences, both now tasks. The port assumes a 250 (`FREERIDE_ENGINE` in `track.rs`) and is
      therefore comparing against the wrong parameter set — **D7**. And opponents are not ported, so the
      tick-exact window cannot run past the NPC contact — **S14** finds that frame. **I4** reads the file
      into the port's pad tape.

      125cc is the engine class, and it is one of four: the disc ships all sixteen `bike\*.phy` sets
      (Race/Pro x 50/125/250/500 x A/P), plus non-bike vehicles (baja, buggy, biplane, monstertruck,
      kiowa). `OPPONENT_ENABLED` and `OPPONENT_DIFFICULTY` turned out **not** to be settings keys (S14):
      they are front-end identifiers, and nothing on the simulation side reads either name. Freeride
      opponents are not switchable — a one-player freeride is always four racers, so any recording made
      on a freeride area has three AI riders in it whether you want them or not.

      **Still open from S14, and only you can do it:** which frame the NPC contact lands on. PCSX2 plays
      a `.p2m2` from its GUI only — no command-line flag, no ini key — so no unattended session can
      measure it. The recipe is in S14's journal entry: play the recording from the GUI with a recorder
      attached to the bike's wreck flag, and the first frame it reads 1 is the contact. Until then the
      number in use is an inference from the tape's idle window (video frame ~1019, simulation tick
      ~510), good to a few frames at best.

- [ ] **Play that recording back in PCSX2 while the kit records** — the one step of S8 that needs a
      person at the desktop, and the only thing between the project and a tick-exact acceptance run.
      About ten minutes.

      Why an agent cannot: PCSX2 starts an input-recording *playback* from its GUI only (Tools → Input
      Recording → Play), there is no CLI flag and no `.ini` key for it, and this machine has no way for
      an agent to reach that GUI — Hyprland's dispatcher would not move focus to the emulator window,
      `grim` hung, and there is no `ydotool`. So the capture S8 made is the original's own minute from
      the recording's start state with **nobody touching the pad**, which proves the step and the actor
      but cannot prove the physics.

      The recipe, in your own PCSX2 (not a kit copy — the kit's copies are for agents):

      1. Copy `context/recordings/first_recording.p2m2_SaveState.p2s` into your PCSX2's `savestates/` as
         `SLUS-20767 (9866FE54).08.p2s` (slot 8; any free slot will do if you tell S8's successor which).
      2. Boot `MX Unleashed (USA).iso`, then Tools → Input Recording → **Play** →
         `context/recordings/first_recording.p2m2`. It starts from the recording's own save state, so do
         not load a state by hand first.
      3. With the playback running, in a second terminal:
         `./kit record watch/acceptance.toml -n 1811 -o context/capture/acceptance-ride.csv` — with no
         `--state`, because the playback loads the state itself. Starting a few ticks late costs
         alignment, not correctness: the capture carries the game's own `frame_counter` and the test
         aligns on it (`RECORDING_START_TICK` = 2773, the counter in that save state).
      4. Leave the pad alone for the whole minute. A single press of your own is a frame of input the
         `.p2m2` does not contain, and it will show up as a divergence nobody can explain later.

      What it unblocks: the two questions I4 and S8 left open — **which of a tick's two video fields the
      game reads** (`TickPhase`; the right phase agrees with the capture on all 1811 ticks, the wrong one
      disagrees on the 18 where the two fields differ in a button) and whether the recorded input really
      reproduces the recorded motion. With it, `the_acceptance_run` in `crates/game-sim/tests/replay.rs`
      becomes the port's verdict with no code to write.

      If the playback desyncs (the bike doing something the human does not remember), say so here rather
      than deleting it: a `.p2m2` that does not replay on the emulator it was recorded on cannot be
      expected to replay on the port either, and that is worth knowing before anybody ports physics to
      match it.

## Decisions only you can make

- [x] **What this port is for, first.** Decided: **freeroam** — riding the dirt bike around a freeride
      area and doing tricks, with the original's physics. That is backlog S3 (ground), S4 (collision),
      S10 (air and landing) and S11 (tricks), with D6 (the freeride area's data), V1 (first draw) and P1
      (the playable shell — a window and a real pad) behind them. It sets the order everything before
      them is done in, and it puts two things out of the way until it is done: opponent AI and the race
      activities (S12) and the front end (S9), both stretch.
- [ ] **What has to be exact, and what only has to look right.** The default in `AGENT.md` is: simulation
      exact, rendering by eye, front end free. That fits the goal above, so it stands unless you say
      otherwise — the thing worth confirming is whether you want the *bike's* arithmetic bit-exact (which
      is the plan, and is slow) or merely close (which is faster and will drift).
- [ ] **How far past the bike to go** — one track and one bike riding correctly is the near goal; whether
      this becomes the whole game, a mod platform, or a research record changes which tasks are worth
      doing after that.
