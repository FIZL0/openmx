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

- [x] **Give the kit's pad back its `/dev/uinput`.** Done by the user 2026-10-20 (`getfacl /dev/uinput` shows `user:rycs:rw-`): `./kit pad` can drive the original again, curves and all. Since the 2026-10-19 reboot `/dev/uinput` is
      `crw------- root`, so `./kit pad` cannot open it and no agent can drive the game's pad. P21 worked
      round it with a generated `.p2m2` played over the accessibility bus, which only does straight
      lines. Fix (2 minutes, `docs/TROUBLESHOOTING.md`): `sudo setfacl -m u:$USER:rw /dev/uinput` for
      now, and the udev rule there to keep it across reboots. `./kit pad check` says whether it took.

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
- [ ] **Two players, split screen** (slots 6 and 7, new; add their rows to `plan/REFERENCE.md`). Asked by
      R9 (research/journal/2026-10-20-r9-r9-two-players). R9 read from the decompilation how the original
      splits the screen, gives out pads and loads a two-player world, but nothing of it has been seen
      running. Two pads are needed, one in each controller port. Make two states, each saved on the
      first frame the riders can move, before anyone has touched the throttle:
      - **slot 6: two-player freeride, horizontal split** (one view above the other) — any freeride area.
      - **slot 7: two-player race, vertical split** (side by side) — a supercross track if the game
        offers one, with the AI riders left on.
      Note here which split you picked in the options, which area or track, both bikes, and whether
      the display was 4:3 or 16:9. Those two answer what the decompilation could not show: where the
      second rider starts (in freeride and on a start gate), which camera preset each view really uses,
      and the HUD as drawn. About ten minutes for both. If the front end has only one split option,
      make both states in it and say so.
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

      **S14's open frame is measured** (2026-10-16, no human needed in the end — see the next item). The
      recording was played back on the original with the rider's `wreck_reason` (+0x748) recorded beside
      the bike: it is 0 while riding and non-zero in two windows, ticks **3259–3349** and **4167–4190**,
      reason 5 in both. So the tick-exact window runs from the recording's first tick to **3258**, about
      sixteen seconds, rather than the ~510 inferred from the idle prefix. What reason 5 *is* — the NPC
      contact you remember, or a crash of your own — is still a number without a name.

- [x] **Play that recording back in PCSX2 while the kit records** — **done 2026-10-16, by the agent, so
      you can skip it.** It was listed here because PCSX2 starts an input-recording playback from its GUI
      only (Tools → Input Recording → Play): no CLI flag, no `.ini` key, no hotkey, and nothing on this
      desktop can send the emulator a keystroke or move focus to its window. What it *can* do is press a
      Qt menu item over the desktop's accessibility bus, which needs neither focus nor a pointer. The
      emulator was started with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1`, the menu item pressed over the bus,
      the file dialog filled the same way, and the save-state load caught on the tick it happened
      (`frame_counter` jumping back to 2773, which is the counter in the recording's own state).

      What it produced: `context/capture/` now holds the human's own minute as the original played it —
      1,811 rows, one per tick, no gap; 1,449 ticks with a control held; the bike to 77 units/s over
      3,071 units of ground; two crashes. Both questions this item was for are answered. **The game reads
      the second of a tick's two video fields** (`TickPhase::Second` matches the recording byte for byte
      on all 1,810 overlapping ticks; `First` loses it at the first press), and the recorded input does
      reproduce the recorded motion. `the_acceptance_run` in `crates/game-sim/tests/replay.rs` is now the
      port's verdict on every `./kit check`; today it prints `NoSuchField bike.pos[0]`, which is the
      honest state of the port rather than a problem with the data. The details are in
      `research/journal/2026-10-15-s8-s8-acceptance/3-s8-acceptance-PART.md`.

      **One thing to know if you ever play a recording back yourself: PCSX2 writes to the `.p2m2`.** At
      the end of the movie it switches to record mode and keeps appending to the same file — the human's
      recording grew from 130,962 bytes to over 1.2 MB while the emulator sat open after the capture. It
      was restored byte for byte from the copies in the worktrees (md5
      `59900dc955641ee3ef9fcf50211b5d22`) and the save state was never touched, and
      `context/recordings/first_recording.p2m2` is now **read-only** so it cannot happen again. Point any
      playback at a copy.

      **What is still worth your time:** a *longer* recording. S8 asks for several minutes and this one is
      60.4 seconds, so the hundredth awkward landing is still not in any data. Recording is still a person
      at a pad; playing it back and capturing it is not, any more.

## Decisions only you can make

- [x] **What this port is for, first.** Decided: **freeroam** — riding the dirt bike around a freeride
      area and doing tricks, with the original's physics. That is backlog S3 (ground), S4 (collision),
      S10 (air and landing) and S11 (tricks), with D6 (the freeride area's data), V1 (first draw) and P1
      (the playable shell — a window and a real pad) behind them. It sets the order everything before
      them is done in, and it puts two things out of the way until it is done: opponent AI and the race
      activities (S12) and the front end (S9), both stretch.
- [x] **What has to be exact, and what only has to look right.** Answered 2026-10-20: the bike is exact, bit for bit, as planned. The default in `AGENT.md` is: simulation
      exact, rendering by eye, front end free. That fits the goal above, so it stands unless you say
      otherwise — the thing worth confirming is whether you want the *bike's* arithmetic bit-exact (which
      is the plan, and is slow) or merely close (which is faster and will drift).
- [x] **How far past the bike to go** Answered 2026-10-20: all of it — every bike, machine and track ("we should get them all"). — one track and one bike riding correctly is the near goal; whether
      this becomes the whole game, a mod platform, or a research record changes which tasks are worth
      doing after that.
- [x] **A bail the original does not have (S29).** Answered 2026-10-20: match the original — no bail; S29 closed. MX Unleashed has no bail: no wreck entry reads the pad,
      and recording2's three wrecks (reasons 8, 12, 6) follow no press (research/journal/2026-10-20-s29-bail).
      Either S29 closes as "matches the original by having none", or you name the bail you want — the
      button, the wreck reason (and so the throw), ground, air or both — and it goes in as an addition
      behind an off-by-default flag, with nothing in the original to prove it against.
- [ ] **A ghost on the machine switch (P57).** The bike's put-back ghost is in: after L1+R1 or a wreck
      the bike and the rider are drawn see-through and fade back to solid over 3 s, as the original's.
      P57 was asked whether the M key and P41's menu switch should use it too. The original's swap does
      not: it hides the machine left and hands the controller over with no put-back, so nothing starts
      the ghost there (research/journal/2026-10-21-p57-bike-reset-ghost-fade). P57 left it so. Say if
      you want the ghost on a switch back to the bike anyway — an addition with nothing in the original
      to prove it against.
- [ ] **The New layout's gas: switch or analogue (P77).** P77 decided, unattended, to match the original:
      the original's bike reads its gas, brake and clutch as switches (any press is 1.0, and its limiter
      walks the lever up at the bike's GasRateLimit), and since P40's New layout is a remap onto the
      original's Cross and Square before the simulation reads them, New's R2 and L2 are now switches too.
      Part pressure still reaches the biplane's throttle, which is analogue in the original. If you want
      New's R2 analogue on the bike after all, say so: it goes in as an addition (the bike's pad read
      keeping the strength under the New layout only, recorded on the tape), with nothing in the original
      to prove it against, and the Original layout stays the switch (research/journal/2026-10-26-p77-lever-switches).

## A tail landing the port rides out and the original does not (S145)

S144 put twelve tail-first landings to the original (slams, a hard drop, crossed up, leaned, on the gas)
and the port judged every one as the original does, so the generous landing you saw is not among them.
To find it: ride the shell with `--record context/tapes/tail.tape` (from the repo root, the track you
were on), land the way that rode out, quit, and add the tape's path, the track and roughly where on it
to S145 (`./kit plan S145`). Five minutes. If the shell will not record, say which track and jump; an
agent can then try to make the same landing in the original by teleport.
