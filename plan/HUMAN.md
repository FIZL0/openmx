# Needs the human

Work no agent can do: it needs the disc, the hardware, an account, a controller in someone's hands, or a
decision about what this port should be.

**Agents add items here instead of guessing.** If a task cannot be finished without one of these, write the
item (what is needed, what it unblocks, and exactly how to produce it), block the task with
`./kit plan block <ID> "needs <item> — plan/HUMAN.md"`, and move on. A guess in `kit.toml` or in a ported
constant is far more expensive than a blocked task: it looks like work and it is wrong.

Write each item so it can be done without re-reading the whole project: the slot or file name to save to,
how long it takes, and what to do if it goes wrong.

## Right now, nothing is waiting on you

The save states and the recording below are the next things that will be, and they need the game booting
first (C1). Nothing is blocked today.

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
      and `.gitignore`'s `*.bin` keeps them out of git — the repo-root copies are redundant now and can
      be deleted.
- [x] **PCSX2 installed.** `pcsx2-latest-bin` 2.9.34, `/usr/bin/pcsx2-qt`, in `kit.toml` `[emu] pcsx2`.
      Still to do once the BIOS is in: **boot the ISO by hand once** and confirm it reaches the title
      screen, before any agent drives it.
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
moment. Save states go in PCSX2's slots; by convention slots 3, 4 and 5 are the human's and are never
overwritten, 8 and 9 are scratch for agents.

The BIOS is in, so this is doable as soon as the game boots under the kit (C1).

- [ ] **The title screen** (slot 3).
- [ ] **On a supercross track, on the start gate** (slot 4), on `sx01` if the track select lets you pick
      it by name — that is the track the port is being built against (`Tracks\sx01.res`), and its archive
      is already extracted and understood. Note here which bike and which difficulty, because
      `bike\race250a.phy` and `bike\race250p.phy` are different parameter sets and the port has to know
      which one it is being compared against.
- [ ] **Something that runs itself** (slot 5): attract mode, or a race left to the AI. The most valuable
      state you will make, because a capture that needs a person playing cannot be repeated exactly, and
      a port can only be proved against something repeatable.
- [ ] **One longer recording of real play** for the acceptance test: start from the slot-4 state, ride a
      couple of laps normally, and include the awkward things — a crash, a whip, a landing from height, a
      wall. Save under `context/recordings/` with a line here saying which state it starts from and which
      controller port. A recording whose save state is lost is worthless.

      What this proves: the same button presses fed to the port produce the same bike position, frame by
      frame. It is the only test that can show the physics really match rather than merely look right.

## Decisions only you can make

- [x] **What this port is for, first.** Decided: the map and the player controller, riding the dirt bike
      with the original's physics. That is backlog S3 (movement) and S4 (collision) with V1 (first draw)
      behind it, and it sets the order everything before them is done in.
- [ ] **What has to be exact, and what only has to look right.** The default in `AGENT.md` is: simulation
      exact, rendering by eye, front end free. That fits the goal above, so it stands unless you say
      otherwise — the thing worth confirming is whether you want the *bike's* arithmetic bit-exact (which
      is the plan, and is slow) or merely close (which is faster and will drift).
- [ ] **How far past the bike to go** — one track and one bike riding correctly is the near goal; whether
      this becomes the whole game, a mod platform, or a research record changes which tasks are worth
      doing after that.
