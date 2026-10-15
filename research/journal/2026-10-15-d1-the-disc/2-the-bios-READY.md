# 2 — the BIOS (READY)

Task D1. 2026-10-15.

## What I did

Got `./kit doctor` to one remaining `[FIX]`, which is the one I cannot clear myself:

```
[ FIX  ] no PS2 BIOS
        Dump your own console's BIOS into /home/rycs/.config/PCSX2/bios.
```

Everything else is clean: python, `kit.toml`, the image, the extraction, PCSX2 at
`/usr/bin/pcsx2-qt`, Ghidra at `/opt/ghidra`, the decompilation dumps, the virtual controller, screen
capture, the test command, git, and `context/` being ignored.

## What the original does

n/a.

## How it was proved

`./kit doctor`, read above.

## Not verified / not 1:1

- Everything that needs the game *running*. Specifically blocked: `./kit pcsx2 start`, `./kit probe vsync`
  (which every recorder needs as its clock), `./kit record`, `./kit shot`, `./kit state`, and therefore the
  whole replay-acceptance idea in A2 and the capture comparisons in S8. Static work — disc formats, the
  ELF, Ghidra, `ps2-float`, and the oracle, which runs the game's code in the kit's own R5900
  interpreter against a program image rather than in an emulator — needs none of it and is proceeding.

## Next

**The human:** dump the BIOS from your own PS2 and drop the `.bin` (and its `.nvm`/`.mec` siblings if the
dumper produced them) into `~/.config/PCSX2/bios/`, then `./kit doctor` to confirm. Written up in
`plan/HUMAN.md`. After that, two more things will be wanted, in this order:

1. A short recorded session — a minute of riding one track — via `./kit record`, which becomes the first
   replay fixture.
2. Save states in slots 3/4/5 (menu, race start, mid-race), leaving 8 and 9 as scratch, so the oracle and
   the capture tasks have a known RAM image to start from.

Nothing else is waiting on you.
