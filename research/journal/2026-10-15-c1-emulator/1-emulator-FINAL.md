# 1 — emulator (FINAL)

Task C1. 2026-10-15.

## What I did

Got MX Unleashed booting under the kit, on a kit-managed copy, driven only by kit commands — and found
three defects in the kit on the way, each of which silently produces a plausible-looking wrong result
rather than an error. All three are fixed in `ps2kit/`; issues are filed upstream.

The working sequence, end to end:

```
pcsx2-qt -testconfig                     # once, so PCSX2 writes ~/.config/PCSX2/inis/PCSX2.ini
./kit pcsx2 copy 6                       # copy N from that config: own PINE slot, pad, states, snaps
KIT_PCSX2=6 ./kit pcsx2 start            # boots the configured ISO with PINE on
KIT_PCSX2=6 ./kit pcsx2 status
KIT_PCSX2=6 ./kit pad send "press start 300" "sleep 2500" "press start 300"
KIT_PCSX2=6 ./kit shot context/shots/c1-title.png
```

### Defect 1 — a fresh PCSX2 config boots the setup wizard, not the game

`./kit pcsx2 start` runs `pcsx2-qt -- <iso>`. On a config PCSX2 has never completed interactively,
`[UI] SetupWizardIncomplete = true` and `[Filenames] BIOS` is empty, so PCSX2 opens its five-page
first-run wizard and ignores the ISO. The kit reports "PCSX2 starting (pid …), booting MX Unleashed
(USA).iso" and `./kit pcsx2 status` reports it running, because both are true: the process is up. The
screenshot that follows is a clean, non-black PNG — **of the wizard**. Nothing in the kit's output says
the game never started.

`pcsx2-qt -testconfig` writes a default ini and exits 0, but does not clear the wizard flag. What the ini
needs, which `copy_config` should also set and now does for `ConfirmShutdown`:

```
[UI]        SetupWizardIncomplete = false
[Filenames] BIOS = SCPH-70012.bin      # a file in [Folders] Bios, not a path
[UI]        ConfirmShutdown = false    # else `./kit pcsx2 stop` waits on a modal nobody can click
```

### Defect 2 — a kit copy's controller port is bound to a keyboard that is switched off

This is the expensive one. `copy_config` copies the user's ini, then sets
`[InputSources] Keyboard = false` ("a copy stealing focus must not eat the user's keys") and binds two
*hotkeys* to the pad — `TogglePause = SDL-0/Guide`, `Screenshot = SDL-0/LeftStick & SDL-0/RightStick`.
It never touches `[Pad1]`. So a copy inherits whatever the user's controller port 1 was bound to — here
`Up = Keyboard/Up`, `Cross = Keyboard/K`, … — and that input source is then disabled.

The result is a copy where `./kit pad check` passes, `./kit pad send` reports success, `./kit shot` works
(it names `SDL-0` explicitly), and **the game receives nothing at all**. I spent several minutes watching
attract-mode footage change on its own and reading it as "Start doesn't skip this video".

Fix: `copy_config` now writes all 24 `[Pad1]` bindings explicitly, from a `PAD1_SDL` table, matching the
Xbox-style layout `ps2kit/pad.py` presents. Proof it was the cause: with the bindings in place and nothing
else changed, the same `press start` walked the game attract → warning screen → controls screen → main
menu, which is `context/shots/c1-title.png`.

### Defect 3 — the window grab can hang forever, and takes the fallback with it

`plat.screenshot_window` ran `grim`/`import`/`spectacle` with no timeout. On this machine (Hyprland
0.56.2, grim, wlroots screencopy) `grim` returns instantly while PCSX2 is stopped and **never returns**
once PCSX2 is rendering a game — the emulator's game surface wedges screencopy. `./kit shot` therefore
hung indefinitely instead of falling through to the emulator's own screenshot hotkey, which works fine.

Fix: `GRAB_TIMEOUT = 15` around every capture subprocess, so a wedged compositor costs 15 seconds and
then uses the fallback.

## What the original does

Nothing decompiled in this task. One thing the game told us that the port needs, though —
`context/shots/c1-controls.png` is the game's own "Default Driving & Stunt Controls" diagram, which names
the inputs S3 has to accept:

| control | button |
| --- | --- |
| Gas | ✕ |
| Brake | □ |
| Stunts | △, ○ |
| Stunt Modifier | R1 |
| Thrill Camera | R2 |
| Clutch | L2 |
| Look Back | L1 |
| Steering + Leaning | left stick **and** d-pad |
| Pan / Change Camera | right stick / L3 |
| Reverse | □ + ✕ |
| Pause | Start |

Two of those matter for the physics port specifically: steering and leaning come from the *same* stick
(so the lean axis is not a separate input the port can ignore), and there is a clutch, which means the
drivetrain model has a disengage path.

## How it was proved

Looked right beside a screenshot, which is what C1 asks for:

- `context/shots/c1-boot.png` — the game's own loader, logo and progress bar. Mean luminance 0.605.
- `context/shots/c1-controls.png` — the controls diagram, mean 0.608.
- `context/shots/c1-title.png` — the main menu (Career / Supercross / Nationals / Freestyle / Profile
  Manager / Store / Options / Training), rider and bike rendered in 3D beside it. Mean 0.613, 936×702.

All three are `./kit shot` output, via the pad-hotkey path, on copy 6. Non-black is measured, not judged:
`magick <f> -colorspace Gray -format "%[fx:mean]" info:` on a black frame reads 0.00; these read 0.61.

Second independent copy (the other half of C1's test — copy 2 is in use by a running agent, so 6 and 7):

```
PCSX2 copy 6  running (pid 233001)   config …/openmx-slots/pcsx2/s6/PCSX2   PINE 28017
PCSX2 copy 7  running (pid 237997)   config …/openmx-slots/pcsx2/s7/PCSX2   PINE 28018
/run/user/1000/pcsx2.sock.28017
/run/user/1000/pcsx2.sock.28018
```

Independent in state, not just in configuration: with both running, a shot of copy 6 was the main menu
while a shot of copy 7, booted a minute later, was still the loader. Distinct pids, config dirs, PINE
sockets, pads (`0x1209/0x5006` and `0x1209/0x5007`), save-state and snapshot directories.

## Not verified / not 1:1

- **The emulator is not the console.** Everything measured through PCSX2 is PCSX2's behaviour. For the
  render path that is fine (rendering only has to look right); for timing it is not settled, and C2's
  vsync counter is the first thing that will depend on it.
- **BIOS choice is assumed, not tested.** `SCPH-70012` (slim, US) is in use because it is the region match
  for this NTSC-U disc. I did not boot the other two dumps to confirm the game behaves identically, and a
  PAL BIOS would give a different refresh rate.
- **`PAD1_SDL` is not verified binding by binding.** Start, and the L3+R3 screenshot chord, are proved by
  the screenshots above. The other 22 bindings are written from PCSX2's SDL naming and `ps2kit/pad.py`'s
  evdev layout; each one is only proved when something presses it. The analogue stick bindings in
  particular (`-LeftY`/`+LeftY` as `LUp`/`LDown`) are the ones most likely to be inverted, and S3 will
  find out.
- **`GRAB_TIMEOUT = 15` is a guess.** It is long enough that a slow-but-working compositor is not cut off
  and short enough that a wedged one is not fatal. Not measured against a loaded machine.
- **The wizard flag is set on the user's own config, not just on copies.** `pcsx2-qt -testconfig` plus the
  three ini keys above was done by hand here; the kit does not yet do it in `start`. Issue filed.
- **Controls diagram read by eye.** The table above is transcribed from a 936×702 screenshot of a PS2 UI,
  not from the game's code. The L1/L2 assignment for Look Back / Clutch is the pair I am least sure of —
  the diagram's leader lines for the two left shoulder buttons are close together.

## Next

C2: the vsync counter (`./kit probe vsync --save`), which is the first task that needs PINE rather than
pixels, and the first that will show whether the emulator's timing is stable enough to measure against.
Copy 6 is up and at the main menu if it is still running; otherwise `KIT_PCSX2=6 ./kit pcsx2 start`.
