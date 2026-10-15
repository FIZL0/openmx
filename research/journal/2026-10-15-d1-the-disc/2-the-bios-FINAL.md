# 2 — the BIOS (READY → resolved)

Task D1. 2026-10-15.

## What I did

Got `./kit doctor` to one remaining `[FIX]`, the one I could not clear myself:

```
[ FIX  ] no PS2 BIOS
        Dump your own console's BIOS into /home/rycs/.config/PCSX2/bios.
```

**Resolved the same day.** The human supplied three BIOS dumps in `PS2_bios/` in the repo root, copied
into `~/.config/PCSX2/bios/`:

| file | console | region |
|---|---|---|
| `SCPH-70012.bin` | slim, US | NTSC-U — **the one this disc wants** |
| `SCPH-77008.bin` | slim, Europe | PAL |
| `SCPH-90006.bin` | slim, Europe | PAL |

4,194,304 bytes each. The disc is `SLUS-20767`, NTSC-U, so `SCPH-70012` is the region match and is what
PCSX2 is pointed at; a PAL BIOS with an NTSC disc gives the wrong refresh rate, which would quietly
poison every frame-timing measurement the port is built on.

`./kit doctor` now reports `[ ok ] BIOS present` and `Ready.` — no `[FIX]` left.

Note for anyone adding files to this repo: BIOS dumps are copyrighted and must never be committed.
`.gitignore`'s `*.bin` already covers `PS2_bios/`, and `git status` was clean before and after, so nothing
leaked. The working copies live in PCSX2's own directory now; the ones in the repo root are redundant.

## What the original does

n/a.

## How it was proved

`./kit doctor`: every check `[ ok ]`, the two remaining `[note]`s being "PCSX2 not running" and "frame
counter not found yet", both of which are the next task's job rather than a problem.

## Not verified / not 1:1

- **The game has still never been booted here.** A present BIOS is not a running game: that is C1.
- The BIOS dumps are taken on trust as being from the user's own consoles; I have not checked their
  hashes against a database, and the kit deliberately has no mechanism to.
- `SCPH-70012` is chosen on region alone. If the game turns out to depend on a particular BIOS version's
  behaviour — rare, but kernel-version checks exist — this is a variable to remember.

## Next

C1: `./kit pcsx2 start`, boot the ISO, confirm the title screen. Then C2, the vsync counter, which every
recorder needs as its clock.
