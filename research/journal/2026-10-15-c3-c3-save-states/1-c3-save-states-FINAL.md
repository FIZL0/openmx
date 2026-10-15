# 1 — c3 save states (FINAL)

Task C3. 2026-10-15.

## What I did

Wrote down the save-state convention and what each slot is for, and proved the mechanics around it rather
than describing them from the source:

- `plan/REFERENCE.md` → *What each save state holds*: the slot table with an owner per slot, where the
  `.p2s` files actually live (the human's config vs. a per-worktree kit copy), how a state the human
  makes reaches an agent, what has to be written down about each state, and the commands for using one.
- `plan/HUMAN.md` → *Save states and recordings*: the three states and the recording, each with a recipe
  — where to make it, what moment to catch, how long it takes, and what the agents cannot work out for
  themselves (bike, difficulty, controller ports, profile). The page's lead paragraph said nothing was
  waiting on the human; C1 landed, so these four items now are, and it says so.
- Left a main-menu state in **scratch slot 9** and copied it into `~/.config/PCSX2/savestates/`, which is
  the only directory future kit copies inherit from. It saves the next agent a two-minute boot-and-press
  dance. It is scratch: anyone may overwrite it.

The states that need judgement and a controller — on the `sx01` gate (slot 4) and something that runs
itself (slot 5) — are the human's and are not made. The documentation describes what they must hold, not
what they do hold.

## What the original does

Nothing decompiled in this task. One observation about the game itself, used in the HUMAN.md recipe: from
a cold boot the attract video, the warning screen and the controls diagram each take a Start press, and
the menu that follows is Career / Supercross / Nationals / Freestyle / Profile Manager / Store / Options /
Training with Career highlighted — the same screen C1 photographed.

## How it was proved

All on copy 3, by measurement rather than by reading `ps2kit/emu.py`:

- **A state the human makes reaches an agent only through `./kit pcsx2 copy N`.** Put a marker file in
  `~/.config/PCSX2/savestates/`, made copy 3 — marker present in the copy's savestates folder. Added a
  second marker, re-ran `./kit pcsx2 copy 3` on the now-existing copy — second marker present too. So a
  refresh works on an existing copy, and a state saved after the copy was made is invisible until then.
  Both markers removed afterwards.
- **Copies are per-worktree.** `platform_.copies_root()` is `<repo>.parent/<repo>-slots/pcsx2`, and the
  repo here is the worktree: copy 3 of this task is `…/openmx-slots/s3-slots/pcsx2/s3/PCSX2`, not the
  main checkout's `…/openmx-slots/pcsx2`. Anything saved in it dies with the worktree, which is why slot
  9 was copied back into the user's own config.
- **Save/load round trip.** At the main menu (`Career` highlighted) `./kit pine save 9` wrote
  `SLUS-20767 (9866FE54).09.p2s`, 11.1 MiB. Pressed Down — screenshot shows `Supercross` highlighted.
  `./kit pine load 9`, screenshot again — back on `Career`. The menu's 3D rider animates, so whole-frame
  RMSE is never zero (0.099 menu vs. reloaded); the cursor is the observable that settles it.
- **The state tooling, on that file.** `./kit pcsx2 state` lists slots 8 and 9 with their `kit.toml`
  notes; `./kit state list 9` shows the members (`eeMemory.bin` 33554432, `iopMemory.bin` 2097152,
  `vu0/vu1Memory`, `Scratchpad.bin`, `eeHwRegs.bin`, `SPU2.bin`, …); `./kit state ram 9` produced a
  33554432-byte EE image; `./kit state extract 9 --addr 0x100000 --len 16 --as u32` read it without an
  emulator running.
- **Reaching the menu unattended**, for the HUMAN.md note that an agent can do this part itself: from a
  cold `./kit pcsx2 start`, six `press start 300` at three-second intervals ended on the main menu
  (screenshot). An earlier attempt with three presses at 2.5 s was still in the attract video — the
  first save, slot 8, caught that video rather than the menu.

## Not verified / not 1:1

- **The human's three states do not exist.** Slots 3, 4 and 5 are described, not observed: what they must
  hold is specified by this task, and the first agent to load one will find out whether the description
  matches. Nothing in the port depends on them yet.
- **The human-facing UI steps are not tested.** `System → Save State → Slot N` and the `F1`/`F2` hotkeys
  in the HUMAN.md recipe are PCSX2's standard bindings, written from its documented defaults. Agents
  drive copies through PINE and the pad, never the menus, so nothing here exercised them — and a kit copy
  deliberately disables its keyboard input source, so it could not have.
- **"A state belongs to the PCSX2 version that wrote it"** is general PCSX2 behaviour, not measured here:
  only 2.9.127 is installed, so there was no second version to fail against.
- **No frame number on any state.** `[emu] vsync_addr` is still `auto` (C2 has not run), so slot 9's row
  has no vsync value, which is one of the fields REFERENCE.md asks for. Whoever finishes C2 can fill it
  in for any state that still exists.
- **The load was proved by pixels, not by RAM.** I did not dump `eeMemory.bin` before and after a load
  and compare them, which would have proved restoration of state the screen does not show.
- **State sizes are two samples.** The "6–12 MB" in REFERENCE.md is slot 8 (6.1 MiB, during an FMV) and
  slot 9 (11.1 MiB, at the menu) on this build. A state taken in a race may well be larger.
- **The Start-press sequence is wall-clock, not frame-exact**, and worked once. It is a convenience for
  reaching the menu, not a reproducible input sequence — `docs/PCSX2.md` already says why pad timing
  cannot be that.
- **The user's own PCSX2 now has a slot 9 it did not make** (the main-menu state copied there on
  purpose). Scratch by convention, but it is a file in their config directory, and deleting it is the
  whole of undoing it.
- **Not a save-state matter, but it cost this task twenty minutes:** `./kit pcsx2 start` reported
  "starting (pid …)" and PCSX2 exited immediately because this session's environment has no
  `WAYLAND_DISPLAY` (the compositor's socket is `/run/user/1000/wayland-1`); the only trace was
  "Could not load the Qt platform plugin" in `context/logs/pcsx23.log`, and `./kit pcsx2 status` then
  said "not running". `WAYLAND_DISPLAY=wayland-1 ./kit pcsx2 start` works. Same shape as C1's defects —
  the kit reports a plausible success for a launch that never happened. Not fixed here: C3 is not a kit
  task, and the fix (have `start` check the process is alive a second later, and pass a display through)
  belongs with whoever owns `ps2kit/emu.py` next.

## Next

Nothing in C3 is waiting. The next session that needs a state beyond the main menu must stop at
`plan/HUMAN.md` rather than improvise one: slot 4 (on the `sx01` gate) is what S3's physics comparison
starts from. If you are about to run the emulator and it reports "starting" but nothing happens, read the
last bullet above first.
