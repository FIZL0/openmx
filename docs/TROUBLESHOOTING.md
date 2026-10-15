# Troubleshooting

The failures that actually happen, with what to do. `./kit doctor` is always a reasonable first move — it
checks every prerequisite and prints the command that fixes each problem it finds. `./kit doctor --live`
additionally talks to a running game.

---

## Setup and configuration

**`no kit.toml` / `kit.toml is incomplete`**
Run `./kit setup`. It writes the file and fills in what it can discover. `./kit doctor` lists what is
still missing, field by field.

**`./kit setup` cannot find my disc image**
It searches the repository, its parent and the usual download directories for raw images. Point it
directly: `./kit setup --iso /path/to/game.iso`, or set `[game] iso` in `kit.toml` by hand.

**The disc image will not open**
The kit reads raw images only. A `.chd` or `.cso` must be converted first:
```sh
chdman extractcd -i game.chd -o game.cue -ob game.bin
maxcso --decompress game.cso
```
A raw image is 500 MB–8 GB; a `.bin` track's size is divisible by 2352.

**`serial mismatch: disc says SXXX-NNNNN, kit.toml says SYYY-MMMMM`**
You are pointing at a different release of the game than the one you configured — a different region, or
a different revision. Decide which one the project targets and use only that. Do not "fix" it by editing
`kit.toml` to match whatever happens to be mounted: regions differ in physics constants and frame rate,
and captures from the wrong one look perfectly normal.

---

## Ghidra

**The PS2 processor is not in the list / everything decompiles as generic MIPS**
The EmotionEngine extension is not loaded. It must be built for your *exact* Ghidra version; a mismatch
fails quietly. Unzip into `<ghidra>/Ghidra/Extensions/`, start the GUI once so it registers, close it,
then `./kit doctor`.

**`analyse` has been running for forty minutes**
That is normal. It is tens of minutes per program and it only happens once. `./kit ghidra dump` afterwards
is seconds, because it runs post-scripts with analysis disabled against the saved project. If you find
yourself waiting for analysis repeatedly, something is re-importing — check `./kit ghidra status`.

**The decompilation is tiny, or the interesting code is missing**
The game's logic is in an overlay that was not in the boot ELF. `./kit elf --scan`, add the `[[overlay]]`
blocks, `./kit elf`, then analyse again. See `docs/PS2.md` on overlays.

**Every function looks like garbage; references point nowhere**
The program is loaded at the wrong address. Check with `./kit elf --info`: with the correct base, string
references land on strings and function pointers land on prologues. `--scan`'s answer is a guess and is
sometimes wrong.

**`./kit fn <addr>` says the address is not in any function**
Three usual causes: the address is in a different program (`-p <name>`; the default is the last overlay),
you have an uncached-mirror address (mask off the top bits — `0x2…`/`0xa…` are the same RAM), or Ghidra
did not identify a function there because nothing references it. For the last, `./kit dis <addr>` still
works.

---

## PINE and the emulator

**`could not connect to PINE`**
In order: is the emulator running (`./kit pcsx2 status`)? Is a game actually booted — PINE answers only
once something is running, not at the BIOS screen. Is this the right copy (`KIT_PCSX2`)? Launch through
`./kit pcsx2 start`, which sets the PINE setting immediately before launching, since PCSX2 rewrites its
ini on exit and a hand-enabled setting may have been lost.

**PINE connects, then everything hangs**
Two clients. PINE serves one connection; a second while a recorder is polling wedges both. Find the other
one (`./kit pcsx2 status` shows who holds the lock), let it finish, and read static values from a save
state (`./kit state extract`) instead of opening a second connection.

**`the frame counter is not moving`**
Either the game is paused or the emulator is not running at full speed; or the configured address is not
actually the frame counter. Re-find it: `./kit probe vsync --save`. Tools deliberately bail out here
rather than spin, because spinning holds the lock.

**`./kit probe vsync` finds nothing**
It looks for words that increase once per frame. The game must be *running* — not paused, not in a load
screen, not at a still title screen in some games. Boot into gameplay or a demo, then run it again.
`--range` narrows the search if the heuristic picks noise.

**The emulator is unresponsive and I want to stop it**
`./kit pcsx2 stop`, which waits for anything mid-use and stops only your copy. Do not kill by name: that
takes out the user's own session, and in a parallel run the other agents' copies. (Also avoid
`pkill -f <pattern>` generally — a `-f` pattern matches the searching shell's own command line.)

**The emulator runs but the game is at the BIOS screen**
No BIOS, or the wrong path. `./kit doctor` checks the BIOS folder for your OS; the paths are in
`SETUP.md`. The BIOS has to be your own dump.

---

## Captures

**`CAPTURE FAILED: …`**
Whatever that run wrote is incomplete and must not be analysed. The message names the cause — a gap in
the frame counter, a dropped connection, a counter that stopped. Fix it and record again. This is loud on
purpose: a broken capture that looks plausible is the most expensive bug class in this kind of work.

**Skipped frames**
The emulator is not keeping up, or the watch spec is asking for more than fits in one frame. Reduce the
speed multiplier, trim the spec, or raise the allowed gap deliberately and knowingly. Note that the
recorder already coalesces fields into contiguous regions and uses one batched read per frame, so if it is
still too slow you are reading a lot.

**My capture does not match my port, and I cannot tell where it starts to differ**
Compare the first differing *frame*, not the end state. Record the neighbouring fields too — a divergence
in position usually shows up a frame earlier in a velocity or a state number. And check the obvious:
same region, same save state, same seed.

**Captures are not reproducible between runs**
The state needs a human playing. Find a self-running one (demo, attract mode, CPU-versus-CPU) and keep it
in slot 5. Wall-clock pad timing is not frame-exact and never will be; do not build a proof on it.

---

## The virtual pad

**`/dev/uinput: permission denied`**
```sh
sudo modprobe uinput
sudo setfacl -m u:$USER:rw /dev/uinput
```
Permanently, in `/etc/udev/rules.d/99-uinput.rules`:
```
KERNEL=="uinput", MODE="0660", GROUP="input", OPTIONS+="static_node=uinput"
```
`./kit pad check` prints whichever of these you need.

**The pad does not appear in PCSX2**
`./kit pad serve` must stay running — the device disappears when it exits. Start it before the emulator,
then map it as controller 1 in the emulator's controller settings. With several copies, each needs its own
pad pinned by the SDL device filter, which `KIT_PCSX2` arranges.

**Pressing buttons triggers save states and turbo**
You sent `select`. PCSX2's hotkeys are Select plus a shoulder button. `./kit pad` refuses `select`; if you
are driving the pad another way, do not send it.

---

## Tests, builds, disk

**`./kit check` says the build failed**
It is a build error, not a test failure — the distinction is in the first line. The full log is in the
private directory at the path it prints; read the end of it, not all of it.

**Tests pass individually and fail together**
Usually shared state: the same emulator, the same scratch save-state slot, the same output path. Give each
test its own scratch slot and its own output directory, or make them share one setup deliberately.

**A test takes minutes**
Keep tests under about ten seconds. Force the rare case directly instead of simulating until it happens
naturally. If a test genuinely needs a long run, mark it ignored with a comment saying how to run it and
when it matters, and run it when you touch that area.

**The disk is full**
Build directories. A Rust workspace reaches tens of gigabytes; a parallel run multiplies that by the
number of worktrees. `./kit check` cleans the oldest when the total exceeds the cap in `kit.toml`; lower
the cap, or `cargo clean` in the worktrees you are done with.

---

## Unattended runs

**An agent stopped early with tasks remaining**
Read its journal entry and `./kit plan` — a blocked task is a result, with its reason recorded. If it
stopped for no recorded reason, that is a bug in the run, not in the task.

**Two agents edited the same file**
Pick tasks for different files: `./kit run 3 --different-files`. Worktrees make the conflict a merge
rather than corruption, but it is still wasted work.

**An agent is waiting on an interactive prompt**
Something it ran needs confirmation — most often a delete with a wildcard, which prompts even in
permissive modes. Avoid globbed deletes entirely: write each run to a fresh directory, or let the tool
overwrite its own files.

---

## When none of this helps

Write what happened in the task's journal entry — what you tried and what you observed, not a conclusion
— block the task with a one-line reason, and move to another section. Something that genuinely needs a
human goes in `plan/HUMAN.md`. What does not help is the same fix a third time.
