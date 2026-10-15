# Driving the emulator

PCSX2 is how you watch the real game. Everything here is about using it as an instrument rather than by
hand: reading its memory, pressing its buttons, taking its picture, and running several copies without
them fighting.

---

## PINE: reading the running game's memory

PINE is PCSX2's memory IPC. On Linux and macOS it is a Unix socket in the runtime directory; on Windows
it is a TCP port on localhost. Either way:

```sh
./kit pcsx2 start                      # launch with PINE enabled
./kit pine                             # version, serial, title, status, frame counter
./kit pine read 0x3f2a10 --as f32      # one value
./kit pine read 0x3f2a10 4             # four of them
./kit pine block 1c0000 0x200 --out context/probe/dump.bin
./kit pine watch @score @frame --frames 120
./kit pine write 0x3f2a10 5 --as u32   # poke (careful)
./kit pine save 9 && ./kit pine load 9 # scratch slots are 8 and 9
```

You do not need to enable PINE in the GUI. PCSX2 rewrites its ini on exit, so the kit sets the setting
immediately before every launch it manages.

**The serial check.** Every connection compares the running game's serial against `kit.toml` and refuses
if they differ. Keep it. A capture taken against the wrong region or revision looks entirely normal and
is entirely wrong, and that mistake is expensive precisely because nothing about it looks broken.

**One client at a time.** PINE handles one connection; a second client while a recorder is polling hangs
both, and you then have a wedged emulator and a half-written capture. Two consequences:

- Every tool that talks to PINE takes a lock first. Multi-step jobs hold it across the whole job:
  ```sh
  ./kit pcsx2 with -- ./my-script.sh         # holds the lock, sets KIT_PCSX2_LOCKED=1
  ```
- To read a static value *during* a capture, read it from a save state instead (`./kit state extract`).
  No lock, no connection, and it works while the recorder runs.

**Batched reads.** PINE accepts several operations in one message. `./kit record` uses this: it coalesces
all the fields in a watch spec into the fewest contiguous regions and fetches them in one request per
frame. The difference between that and one request per field is the difference between recording fifty
fields for 600 frames in ten seconds and in ten minutes.

**The frame counter is the clock.** `./kit probe vsync --save` finds it once per game and writes it into
`kit.toml`. Every wait-for-next-frame goes through it, and a counter that has stopped moving is how a tool
knows the game paused or the thing it was recording ended — which is why waits bail out with a message
instead of spinning forever holding the lock.

---

## Save states

```sh
./kit pine save 9                              # the running game, into slot 9
./kit state ram 4 --out context/ram4.bin       # the EE RAM out of a state file
./kit state extract 4 --addr @table --len 0x400 --as f32
./kit state list 4                             # what the file contains
```

A state is a 7z or zip archive; the EE's 32 MB RAM image is a member of it. Reading data out of one
needs no emulator at all, which makes it the cheapest source for tables, struct layouts and constants,
and the only safe one while a capture is in flight.

**Slot discipline.** Slots 3, 4 and 5 are yours — made by you, loaded by tools, never written by them.
Slots 8 and 9 are scratch and the kit's tools overwrite them freely. Record what each of your slots holds
in `kit.toml`'s `[states]`.

The state worth hunting for is one that **runs itself**: a demo, an attract-mode loop, a CPU-versus-CPU
match. A capture that needs a human playing cannot be repeated exactly, and a port can only be proved
against something repeatable. Put that one in slot 5.

---

## The virtual controller

Driving the game unattended needs a pad the emulator believes in.

```sh
./kit pad check                        # is the backend usable, and if not, the exact fix
./kit pad serve &                      # create the pad; leave it running
./kit pad send "press start 100"       # milliseconds, not frames
./kit pad send "stick l 1 0" "sleep 400" "press cross 80" "release"
```

On Linux this is `/dev/uinput` through `python-evdev`, and the pad identifies itself as an Xbox 360
controller so that SDL — and therefore PCSX2 — applies its standard mapping without any configuration.
`./kit pad check` prints the `setfacl` command and the udev rule if permissions are missing.

**Never send `select`.** PCSX2's hotkeys are Select plus a shoulder button, so a `select` press
triggers save-state and turbo hotkeys instead of reaching the game, and the resulting mess looks like a
game bug. `./kit pad` refuses it outright.

Durations are milliseconds, with a `f` suffix for frames (`press cross 4f`) when you are thinking in
frames — but see the next paragraph before relying on that.

**Timing is wall-clock, not frame-exact.** Good enough to navigate menus, start a match, or hold a
direction for two seconds. Not good enough to reproduce a frame-perfect input sequence — for that use
PCSX2's own input recording, or better, find a state that needs no input at all.

**With several emulator copies**, pin each pad to its own: `SDL_GAMECONTROLLER_IGNORE_DEVICES_EXCEPT`
keeps copy *N* from grabbing copy *M*'s controller. `./kit pcsx2 start` with `KIT_PCSX2` set does this
for you.

---

## Screenshots

```sh
./kit shot context/shots/title.png
```

It grabs the emulator's window — through the compositor on Wayland, through X on X11 — and falls back to
PCSX2's own screenshot hotkey if neither works. Screenshots go in the private directory, never in the
journal or the repo: they are game data.

**On this machine the fallback is the normal path, not the exception.** Hyprland's screencopy wedges as
soon as PCSX2 is rendering a game: `grim` returns instantly with the emulator stopped and never returns
with a game running. The grabs are capped at `platform_.GRAB_TIMEOUT` seconds for that reason, so a shot
costs fifteen seconds and then goes through the pad. A copy always has a pad; the user's own emulator
does not, so `./kit shot` without `KIT_PCSX2` can have no way to take a picture at all — **take
screenshots on a copy.**

A shot is only worth what you check: `magick <f> -colorspace Gray -format "%[fx:mean]" info:` reads 0.00
on a black frame. And `ps2kit/hooks/guard.py` currently refuses any command naming a `context/` file over
512 KB, which includes screenshots, so write big ones into the scratchpad, downscale there, and keep the
small one in `context/shots/`.

For anything visual, take the original and your port at the same moment and look at them side by side.
It costs a minute and settles arguments that otherwise go several messages.

---

## Several copies at once

Unattended runs want more than one emulator. `KIT_PCSX2=N` gives a process its own copy of everything:

| | |
|---|---|
| config directory | its own, copied from yours on first use |
| PINE slot | base slot + N, so sockets never collide |
| lock | its own, so copy 1 and copy 2 never block each other |
| pad | its own, pinned by the SDL device filter |

| controller port 1 | bound to that pad, so `./kit pad send` reaches the game |

So `KIT_PCSX2=2 ./kit pcsx2 start` is a second, independent emulator, and every tool run with the same
variable set talks to that one. This is what makes `./kit run 3` possible.

A copy is made from **your** config, so your config has to be able to boot a game before a copy can. On a
PCSX2 that has never been through its first-run wizard, that means three keys — otherwise every launch
opens the wizard and ignores the ISO, while the kit cheerfully reports it as booting the game:

```
[UI]        SetupWizardIncomplete = false
[Filenames] BIOS = SCPH-70012.bin      # a file in [Folders] Bios, not a path
[UI]        ConfirmShutdown = false
```

`pcsx2-qt -testconfig` writes a default ini and exits without a GUI, which is a good way to create the
file in the first place, but it does not clear the wizard flag.

---

## Housekeeping

**Stop the copy you started**, and only that one:

```sh
./kit pcsx2 stop         # waits for anything mid-use, then stops your copy
```

**Never kill the emulator by name.** `pkill pcsx2` kills the user's own session, and in a parallel run it
kills the other agents' copies. (There is a second, subtler trap: a bare `pgrep -f <pattern>` matches the
searching shell's own command line, so `pkill -f <pattern>` can kill the process doing the killing. The
kit matches process names only, which is why `platform_.pids_named()` exists.)

**Don't touch the emulator while a capture is running.** Check before you act, and if a recorder is
polling, wait or read from a save state.

**`CAPTURE FAILED` means the output is rubbish.** Whatever the run wrote is incomplete — a missing frame,
a dropped connection, a stopped counter. Fix the cause and record again. Never analyse a file that
announced that.

**Write each run to a fresh directory** (`context/<task>/run1/`, `run2/`, …) or let a tool overwrite its
own files. Deleting with a glob is worth avoiding: the safety prompts around `rm` with wildcards are
exactly the kind of thing that stalls an unattended run for an hour.
