# Setup

Eight steps. Each one ends with a command that tells you whether it worked, so you never move on with a
broken foundation. Budget about an hour of attention, plus however long your machine takes to decompile
(step 6 is slow and unattended).

If you only want to read code and not run the game, you can stop after step 6. If you only want to
capture the running game, you can skip steps 5–6.

Throughout: `./kit doctor` is the answer to "what is wrong right now". Run it whenever something
surprises you.

---

## What you need to have

| Thing | Why | Where it comes from |
|---|---|---|
| The PS2 game, on disc | The kit reads code and data from the image at runtime and never ships any | **You own it**; you dump it yourself |
| A PS2 BIOS | PCSX2 cannot boot anything without one | **Your own console**, dumped by you |
| PCSX2 2.x | Runs the game so you can watch RAM and record what it does | <https://pcsx2.net> |
| Ghidra 11+ | Disassembles and decompiles the game's code | <https://ghidra-sre.org> |
| The EmotionEngine processor extension | Ghidra does not know the PS2's CPU out of the box | `ghidra-emotionengine-reloaded` |
| Python 3.11+ | The kit itself | your package manager |
| A build toolchain for your port | Whatever you write the re-implementation in | your choice |
| ~30 GB free disk | Extracted disc, Ghidra project, decompilation dumps, captures | — |

Neither the game nor the BIOS can be downloaded legally, and neither one is in this repository. Dump
your own. The kit refuses to run against a disc whose serial does not match the one in your `kit.toml`,
which also means it will tell you clearly if you point it at the wrong image.

---

## 1. Get the kit, check Python

```sh
git clone <your fork of this repo> myport && cd myport
python3 --version          # must be 3.11 or newer
./kit                      # should print the command list
```

Reading the disc, the ELFs and the decompilation runs on a stock Python with no packages. The task list
(`./kit plan`, `./kit done`) and the unattended run (`./kit run`) use
[agent-runner](https://github.com/FIZL0/agent-runner), a stdlib-only package; install it for the Python
that runs `./kit`:

```sh
pip install --user git+https://github.com/FIZL0/agent-runner@v0.5.0
```

On a distribution that refuses `pip install --user` (externally managed Python, e.g. Arch), add
`--break-system-packages`: agent-runner has no dependencies and installs only into `~/.local`, so it
cannot conflict with system packages.
`uv tool install` and `pipx` put it in an isolated environment the kit cannot import from, so they do not
work here.

Two optional packages, each needed by exactly one feature:

```sh
pip install evdev          # the virtual controller (Linux). Skip if you will drive the game by hand.
pip install numpy          # makes ./kit probe faster. It works without.
```

**Check:** `./kit doctor` runs and complains only that there is no `kit.toml` yet.

### Before you go any further: do the loop once, on a game that does not exist

```sh
./kit demo --check         # under a second, no disc, no emulator, no BIOS
```

That generates a synthetic PS2 program — a fake ISO with a fake boot ELF and a fake overlay, every byte
of it assembled by `ps2kit/demo.py` — and runs the entire workflow on it: extract, find the overlay's
load address, read the code, call a function with the oracle, and prove a Rust port of it bit-exact over
ten thousand inputs. **[docs/EXAMPLE.md](docs/EXAMPLE.md)** is that walkthrough with the reasoning.

Half an hour there will save you a day of the steps below, because you will already know what the output
of each command is supposed to look like. It writes nothing outside `context/demo/` and never touches the
`kit.toml` you are about to create.

---

## 2. Dump your disc

Use any PS2-capable optical drive and a tool that produces a raw image (`.iso`, or `.bin`+`.cue`).

Put it where you like — beside the repo is tidy, inside the repo is fine too because `*.iso` is
git-ignored. **Never commit it.**

If you already have a compressed image, convert it first; the kit reads raw images only:

```sh
chdman extractcd -i game.chd -o game.cue -ob game.bin    # CHD
maxcso --decompress game.cso                             # CSO
```

**Check:** the file is 500 MB–8 GB and `file game.iso` says ISO 9660, or it is a `.bin` of a size
divisible by 2352.

---

## 3. Install and configure PCSX2

Install PCSX2 2.x, run it once, and put your BIOS dump where it expects it:

| OS | BIOS folder |
|---|---|
| Linux | `~/.config/PCSX2/bios/` |
| Windows | `Documents\PCSX2\bios\` |
| macOS | `~/Library/Application Support/PCSX2/bios/` |

Then boot your game once by hand, so you know it runs at all before you automate anything. Note the
serial PCSX2 shows in its window title — it should match the one the kit reads from the disc in step 4.

You do **not** need to enable PINE by hand. PCSX2 rewrites its ini on exit, so the kit sets
`EnablePINE` in the ini before every launch it manages. (If you want it on for your own manual
sessions too: Settings → Advanced → Enable PINE.)

**Check:** the game boots to its title screen in PCSX2 and runs at full speed.

---

## 4. Point the kit at the game

```sh
./kit setup
```

It finds PCSX2, finds Ghidra, finds your image, reads `SYSTEM.CNF` off the disc to learn the serial and
the boot ELF, and writes `kit.toml`. Answer the one or two questions it asks.

`kit.toml` is the **only** file in this repository that knows anything about your particular game.
Everything else is generic. Open it now and read it — it is short, and it is the thing you will come
back to.

```sh
./kit extract          # unpack the disc into context/iso (a few minutes)
```

`context/` is the private, git-ignored working directory. Everything derived from your disc or from the
running game lives there: extracted files, decompilation dumps, RAM images, captures. Nothing in it is
ever committed, and the kit will not write game-derived data anywhere else.

**Check:**

```sh
./kit doctor           # disc, serial, PCSX2, BIOS all green
ls context/iso         # the disc's files, including SYSTEM.CNF and the boot ELF
```

---

## 5. Work out the program layout

Most PS2 games are a small boot ELF plus one or more **overlays**: blobs of code the game loads over a
fixed address at runtime, usually one per major mode (menu, gameplay, movie player). Decompiling the boot
ELF alone often gets you almost nothing, because the interesting code is in an overlay that was not
loaded when the ELF was dumped — and an overlay disassembled at the wrong address is unreadable, because
every absolute reference points at nonsense.

```sh
./kit elf --scan
```

This walks the extracted disc for files that look like raw MIPS code and, for each, reports the load
address implied by its own internal references, printing a ready-to-paste `[[overlay]]` block. Paste the
ones that look right into `kit.toml`.

If the scan finds nothing, the game may have no overlays (then just delete the `[[overlay]]` block) or
may pack them inside an archive format of its own — in which case that archive is your first real
reverse-engineering task, and `plan/TODO.md` has it as one.

```sh
./kit elf              # build one ELF per program into context/elf
```

**Check:** `./kit elf --info context/elf/boot.elf` prints plausible segments, and the overlay ELFs exist
with the load addresses you configured.

---

## 6. Decompile

Install Ghidra, then add the PS2 CPU:

1. Download `ghidra-emotionengine-reloaded` for **your exact Ghidra version**.
2. Unzip it into `<ghidra>/Ghidra/Extensions/`.
3. Start Ghidra's GUI once so it registers the extension, then close it.

Set `[ghidra] home` in `kit.toml` if `./kit setup` did not find it, then:

```sh
./kit ghidra analyse        # SLOW: tens of minutes per program. Run it once, go and do something else.
./kit ghidra dump           # seconds: writes the decompilation and xref dumps into context/decomp
```

Only `analyse` is expensive. Everything after this reads the dumps, and the dumps are refreshed by
`dump` in seconds, so you never pay the analysis cost twice unless you re-import.

**Check:**

```sh
./kit ghidra status        # what is in the project, and how old each dump is
./kit fn 100008            # prints one decompiled function
./kit xref 100008          # its callers and callees
```

> **Never open those dumps whole.** A decompilation dump of a PS2 game is 10–30 MB. `cat`ing one into an
> AI agent's context ends the session; opening one in an editor is merely unpleasant. Use `./kit fn`,
> `./kit xref` and ranged reads. The kit ships a hook that blocks the whole-file read and suggests the
> cheap alternative, which is there because this mistake is easy to make and expensive every time.

---

## 7. Reach the running game

Live memory access is how you prove things. PCSX2 exposes it over PINE, a local socket.

```sh
./kit pcsx2 start          # boots your game with PINE enabled
./kit pine                 # version, serial, title, status, frame counter
```

If `./kit pine` reports a different serial than your `kit.toml`, it refuses to go further. That check has
saved more time than any other line in the kit: captures taken against the wrong region or revision look
completely normal and are completely wrong.

Now find the game's frame counter — the clock every capture is timed against:

```sh
./kit probe vsync --save
```

It samples RAM while the game runs, finds the words that increase at the frame rate, verifies the best
candidates, and writes the winner into `kit.toml`. One command, once per game.

**Check:** `./kit pine` prints a frame counter that climbs when the game is running and holds still when
it is paused.

### The virtual controller (optional, Linux)

To drive the game unattended you need a virtual pad. On Linux that is `/dev/uinput`:

```sh
sudo modprobe uinput
sudo setfacl -m u:$USER:rw /dev/uinput            # until reboot
```

Permanent, instead of the `setfacl`, in `/etc/udev/rules.d/99-uinput.rules`:

```
KERNEL=="uinput", MODE="0660", GROUP="input", OPTIONS+="static_node=uinput"
```

Then:

```sh
./kit pad check
./kit pad serve &                                  # keep running; PCSX2 binds it as a controller
./kit pad send "press start 100"                   # times are milliseconds
```

In PCSX2, map the new pad as controller 1 (Settings → Controllers). Timing is wall-clock, not
frame-exact; for frame-exact input use PCSX2's own input recording.

**One thing never to do:** do not send `select`. PCSX2's hotkeys are Select plus a shoulder button, so
sending it triggers save-state and turbo hotkeys instead of reaching the game. `./kit pad` refuses it.

---

## 8. Make your save states

The kit's recorders load a save state to reach an exact moment reproducibly. Only you can make them:
play to the moment, then save to a slot. Write down what each slot holds in `kit.toml`'s `[states]` so
tools and agents know which to load.

Three that earn their keep in almost any game:

| Slot | What to save there | Why |
|---|---|---|
| 3 | the title screen, just booted | a clean start for menu and loading work |
| 4 | mid-action, paused at an interesting moment | the usual starting point for a capture |
| 5 | something that **runs itself** — a demo, attract mode, a CPU-vs-CPU match | the most valuable state you own: captures that need no input, so they are perfectly reproducible |
| 8, 9 | leave empty | scratch; the kit's tools overwrite these |

Slot 5's kind of state is worth hunting for. A capture that needs a human playing is a capture you
cannot repeat exactly, and a port can only be proved against something repeatable.

**Check:** `./kit pine load 5` reaches the moment you expect, and `./kit record watch/frame.toml
--frames 120` writes 120 rows with no skipped frames.

---

## You are set up

```sh
./kit doctor --live        # everything, including a live PINE check
```

Read `docs/WORKFLOW.md` next: it is the loop you will be in from now on — find the code, port it, prove
it, write down what you could not prove. Then `./kit plan next` gives you the first task from a backlog
already written for a new port.

---

## When something is wrong

`docs/TROUBLESHOOTING.md` covers the failures that actually happen: PINE not connecting, the serial
mismatch, Ghidra's extension not loading, the frame counter not being found, captures full of skipped
frames, the pad not appearing in PCSX2, a build directory eating the disk.
