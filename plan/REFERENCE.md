# REFERENCE — background, not rules

The rules are in `PLAN.md` and `AGENT.md`; the backlog is `plan/TODO.md`. This file is the things you look
*up*: how to drive the emulator, what the save states hold, which proof suits which code, and what is known
to be missing. Where `docs/` already explains something, this points at it rather than repeating it — a
second copy of an explanation is a second copy to get out of date.

| Looking for | Read |
|---|---|
| How a task is done, start to finish | `docs/WORKFLOW.md` |
| Proving a port against the original | `docs/PROVING.md` |
| PCSX2, PINE, states, the pad | `docs/PCSX2.md` |
| Ghidra, dumps, finding functions | `docs/GHIDRA.md` |
| PS2 hardware: EE, VU, GS, the FPU | `docs/PS2.md` |
| What lives in `context/` | `docs/CONTEXT.md` |
| When a tool misbehaves | `docs/TROUBLESHOOTING.md` |
| How the kit's own modules fit together | `docs/INTERNALS.md` |

## Controlling the real game

Everything live goes through `./kit`, never through PCSX2's own UI, because the kit's copies have their own
config, their own PINE slot and their own lock:

    ./kit pcsx2 start            launch this copy and boot the ISO
    ./kit pcsx2 status           is it up, what is it running, which PINE slot
    ./kit pcsx2 stop             close this copy (waits for anything mid-use)
    ./kit pcsx2 with -- CMD…     hold the emulator for a whole multi-step job
    ./kit pine save 9 / load 9   scratch save states (8 and 9 are the kit's)
    ./kit pine read <addr> …     read RAM; `watch` follows a value per frame
    ./kit pad send "press cross 80"  press buttons on the running game
    ./kit shot out.png           screenshot of the emulator window
    ./kit record …               a per-frame capture to CSV (see `docs/PCSX2.md`)

Two rules that cost real time when broken:

- **One PCSX2 per agent.** `KIT_PCSX2=N` selects copy N and every kit tool follows it — config directory,
  PINE slot, pad, lock. `./kit run` sets it per slot. Two agents on one copy will load states into each
  other's game and both will conclude the game is haunted.
- **Never kill PCSX2 by name.** `pkill pcsx2` takes out the other agents' copies too (and, with `-f`, often
  the caller itself — see the comment in `ps2kit/hooks/guard.py`). `./kit pcsx2 stop` ends yours.

A multi-step job (load a state, press buttons, record, read RAM) must run as **one** command under
`./kit pcsx2 with -- …` so it holds the lock for the whole job; a sequence of separate calls can be
interleaved with another agent's and the capture will be nonsense. Each call also honours
`KIT_PCSX2_LOCKED=1` so nothing deadlocks against its own parent.

Launching is never a blocker: if nothing is running, start it. "I could not capture because the emulator was
not up" is a missing `./kit pcsx2 start`, not a reason to stop.

## What each save state holds

The slots, and who owns them. **Nothing in the kit enforces this** — `./kit pine save 3` would overwrite
the human's slot without a word — so the convention is the only guard there is:

| Slot | What it holds | Whose |
|---|---|---|
| 3 | the main menu, nothing chosen | the human's: load only, never overwrite |
| 4 | on the `sx01` start gate, the first frame of play | the human's: load only, never overwrite |
| 5 | something that runs itself: attract loop, or a race left to the AI | the human's: load only, never overwrite |
| 8, 9 | scratch | anyone's; assume they are overwritten between one command and the next |

Status on 2026-10-15: **3, 4 and 5 do not exist yet** — `plan/HUMAN.md` has the recipe for each, and a
task that needs one blocks on that item rather than inventing a substitute. Slot 9 holds an agent-made
main-menu state (C3, `Career` highlighted, no profile loaded, pad in port 1): a convenience, not a
reference, and the next agent that needs a scratch slot will overwrite it.

### Recordings

A recording is a PCSX2 input movie (`.p2m2`) plus the save state it starts from, both under
`context/recordings/` and both git-ignored. The pair is the unit: a recording whose state is lost
replays against nothing.

| File | What it is |
|---|---|
| `first_recording.p2m2` | 60.4 s (3621 frames at 59.94 Hz) of free riding on **Free09**, both pads recorded, PCSX2 2.9.127. Contains tricks and a collision with an NPC rider. The acceptance fixture for S8. |
| `first_recording.p2m2_SaveState.p2s` | the state it starts from — 32 MiB of EE RAM, unpacked with `./kit state ram <path>` |
| `recording2.p2m2` | 63.2 s (3788 frames at 59.94 Hz) on **Free09** with the same Race125, both pads, PCSX2 2.9.127, author `ryha`. Bailing, tricks, backflips and plain riding — C6 captures it and indexes them. Read-only: PCSX2 records over a movie it plays. |
| `recording2.p2m2_SaveState.p2s` | the state it starts from |
| `biplane.p2m2` | 57.3 s (3437 frames at 59.94 Hz) in the **biplane** on **Free06**, both pads, PCSX2 2.9.127, author `ryha`; the state also has a Race250 loaded. C7 captures it. Read-only. |
| `biplane.p2m2_SaveState.p2s` | the state it starts from |

What the state's RAM says, so nobody has to guess it again: the bike is a **125cc, Race tier**
(`Bike\Race125A.phy` and `Bike\Race125P.phy` are the only bike physics pair resolved in RAM; the `%s`-
prefixed names at ~0x42E000 are the ELF's format strings, not loads). `Bike\Wreckbike.phy` — the crash
bike — is loaded alongside. The engine class is one of four, and all sixteen `bike\*.phy` sets ship on
the disc, so the class is a real variable and not a constant of the mode: see **D7**, which exists
because `FREERIDE_ENGINE` in `crates/game-sim/src/track.rs` says `Cc250`.

Header layout, for whoever writes the reader (**I4**): `fileVersion` u8, `emulator` char[50], `author`
char[255], `game` char[255], `totalFrames` u32, `undoCount` u32, `savestate` u8 — 570 bytes — then 18
bytes per frame per pad, two pads.

### Where the files are, and why yours may not have them

A state is `<SERIAL> (<crc>).NN.p2s` in PCSX2's savestates folder, and *which* folder depends on who is
running the emulator:

| Who | Folder |
|---|---|
| the human's own PCSX2 | `~/.config/PCSX2/savestates/` |
| kit copy N of a worktree | `<worktree>-slots/pcsx2/sN/PCSX2/savestates/` |

The copies are made from the human's config and never shared afterwards, so:

- A state the human makes **does not appear in a copy that already exists.** `./kit pcsx2 copy N` is what
  pulls it in — re-running it on an existing copy copies new state files in and overwrites same-named
  ones. Verified in C3 both ways, on a fresh copy and on an existing one.
- A state an agent saves lives **only in that worktree's copy**, which is deleted with the worktree. If
  a state is worth keeping, copy the `.p2s` into `~/.config/PCSX2/savestates/` — that is the only
  directory every future copy inherits from. (That is how slot 9 above survives this task.)

`./kit pcsx2 state` lists the slots the current copy can see with their `kit.toml` `[states]` note;
`kit.toml` is git-ignored, so this table is the copy that survives.

### What to write down about a state

A state nobody can describe gets reloaded by three different agents who each work out what it is. For
each one, record here: the screen or track it sits on, the bike and difficulty (`bike\race250a.phy` and
`bike\race250p.phy` are different parameter sets — a capture is worthless if you do not know which one it
is being compared against), which controller ports are in use, whether a profile is loaded, and the
vsync counter's value at the moment it was saved (once C2 has found the counter; `[emu] vsync_addr` is
still `auto` today, so no state here has a frame number yet).

A recording is only usable with the state it was recorded from: keep the pair named together under
`context/recordings/` and name the state in the line that announces the recording.

### Using one

```sh
./kit pine save 9      ·      ./kit pine load 9          # the running game (slots 8/9 only)
./kit pcsx2 state                                        # what this copy has, with its note
./kit state list 4                                       # the parts inside the file
./kit state ram 4 --out context/states/ram4.bin          # the 32 MiB EE RAM image, no emulator needed
./kit state extract 4 --addr @table --len 0x400 --as f32
```

`docs/PCSX2.md` has the detail. Two things C3 measured that are worth knowing before you plan around
them: a state file is 6–12 MB (it compresses the 32 MiB EE image plus IOP RAM, the VU memories, the
scratchpad, the hardware registers and SPU2), and `./kit pine load` restores the game's own state but
the kit has no way to tell you *whether* it did — the cheapest check is a screenshot of something you
changed just before loading.

## Which proof for which code

`docs/PROVING.md` is the long version. The short one:

| The code is… | Prove it with |
|---|---|
| a pure function of its arguments and RAM (math, a table lookup, a decision) | the **oracle** — ≥10k generated inputs, bit-exact |
| per-frame behaviour over time (a state machine, movement, timing) | a **capture** of the real game, compared per frame |
| one-off structure (a table layout, a loader) | a **save state** dump parsed against the documented layout |
| a whole subsystem under real input | the **replay harness** and the state hash |
| rendering | a **screenshot** beside `./kit shot`, by eye |

Oracle runs are cheap; captures are not. If a function can be proved with the oracle, do that rather than
spending an hour on the emulator — and if it cannot, say in the journal why not, because that sentence is
usually where the next person starts.

## Known gaps

Keep this list short and honest: things the port does not do, or does differently, with the reason. It is
the first thing to read when something behaves oddly, and the first thing to update when a gap closes.
Every journal entry's `## Not verified / not 1:1` section feeds it; promote anything that will outlive its
task.

- Nothing yet — the port has not started. (When you add one: what differs, why, and what would close it.)

## Running a long session unattended

`./kit run` keeps N agents going through `plan/TODO.md` (see `docs/WORKFLOW.md` → *Long and unattended
runs*, and agent-runner's README for the picking rules). What matters when reading the
results in the morning:

- Each task got **one** attempt. A task that comes back blocked has a reason on its line.
- A session that only hit the usage limit was discarded, not counted — its task is still open.
- A branch that could not be merged is parked as `wip/<ID>` (pushed; the task stays open), with the reason in `context/notes/ledger.md`.
- `context/notes/slotN.log` is slot N's session output; `./kit plan` shows what landed.

The hooks in `.claude/settings.json` are what keep those sessions cheap: `ps2kit/hooks/guard.py` refuses the
expensive mistakes (reading a dump whole, polling a background task, `pgrep -f` killing the caller) and
`ps2kit/hooks/inbox.py` hands over anything you wrote into `context/inbox.md` while they were working —
write `- [ ] <ticket>` lines there any time, and the next agent to finish a tool call picks them up.
