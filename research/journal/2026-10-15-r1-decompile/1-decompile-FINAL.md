# 1 — the decompilation (FINAL)

Task R1. 2026-10-15.

## What I did

`./kit ghidra analyse` then `./kit ghidra dump`, on the one program this game has.

| | |
|---|---|
| Ghidra | 12.1.2 (Arch `extra/ghidra`), chosen to match the extension release's `ghidra_12.1.2` asset |
| processor | `ghidra-emotionengine-reloaded` v2.1.38, language `r5900:LE:32:default` |
| analysis | **2m57s** on this machine, for a 3,455 KB image |
| output | `boot.c` 13.4 MB · `boot.funcs.tsv` 469 KB · `boot.xrefs.tsv` 4.6 MB |
| functions | **8,558** |
| range | `0x100008` – `0x44cd00` |

Getting there needed one fix, now in `plan/HUMAN.md` in case Ghidra is ever reinstalled: the extension
ships `r5900.slaspec` but no compiled `r5900.sla`, and Ghidra compiles it next to the spec inside
root-owned `/opt`, which fails with a bare `Permission denied` after about five seconds. Pre-compiling it
with `sudo .../support/sleigh -a` and then `chmod -R a+rX` fixes it — both halves are needed, because the
sleigh run writes the `.sla` as `rw-r-----` root and it then fails identically. Filed upstream as kit
issue #2.

### The dump is at the right addresses

Two independent checks, which matter more here than usual because a wrong load address produces a dump
that decompiles *beautifully* and describes nothing:

1. **Every function is inside the ELF's code range.** Functions run `0x100008`–`0x44cd00`; D3 read
   `0x100000`–`0x45f800` as code and data out of the program headers. Nothing landed in bss, and nothing
   landed outside the image.
2. **The decompiled entry point agrees with the ELF headers about bss.** `./kit fn 0x100008` opens with a
   loop storing zero in four-word steps from `0x45f800` until it passes `0x4eac80` — which is exactly the
   bss range D3 derived from `filesz`/`memsz` (`0x45f800` + `0x8b480` = `0x4eac80`), read from a
   completely different source. Two derivations, same two numbers.

The usual overlay check — "a function in the overlay dump should sit inside the overlay's load range" —
does not apply: there are no overlays (D4), so there is one program and `./kit fn <addr>` prints a
function from it.

### What the dump looks like

- **89 of 8,558 functions have a name**, and most of those are thunks (`thunk_FUN_...`). The rest are SDK
  calls Ghidra recognised from their syscall numbers or their shape: `FlushCache`, `EI`, `SYNC`,
  `AddIntcHandler`, `syscall`. So ~99% of the image is `FUN_<addr>` and naming is R2's whole job, done
  from strings and the call graph rather than from a symbol table — the image is stripped (D3).
- **The entry point is 448 bytes and does the standard SDK startup**: zero bss, two syscalls (0x3c and
  0x3d — `SetupThread` and `SetupHeap` in the SDK's numbering), a 16-argument call that is the thread and
  heap setup, `FlushCache`, `EI`, then one call into the game.
- **That one call is `main`.** `0x0016c720`, 1,232 bytes, called by exactly one function (the entry point)
  and calling 36, among them `AddIntcHandler` — i.e. it installs the interrupt handlers. Its two arguments
  come from bss words the startup code filled, which is `argc`/`argv`. R3 starts here; noting it now
  because finding it cost one `xref`.

## What the original does

Boots the way every Sony-SDK PS2 game boots, then calls one function that never returns. Not traced
further — R3.

## How it was proved

- `./kit fn 0x100008` prints the entry point; `./kit xref 0x0016c720` prints its callers and callees. Both
  work, which is R1's test.
- The two address checks above.
- `./kit doctor` reports `decompilation dumps: boot`.

## Not verified / not 1:1

- **A decompilation is a hypothesis.** Ghidra's output is not the game; it is a C-shaped reading of the
  instructions, and for R5900 specifically it mis-models the 128-bit registers and the VU0 macro-mode
  instructions more often than it mis-models ordinary MIPS. Every function that matters gets proved by
  the oracle against the real instructions (O1), not by reading it. Nothing in this task proves any
  function's behaviour.
- **8,558 is Ghidra's count, not the truth.** Functions reached only through a jump table or a function
  pointer may be missing, and data misidentified as code may be counted. The number is a rough size, not
  an inventory.
- `syscall(0x3c)` and `syscall(0x3d)` are read as `SetupThread`/`SetupHeap` from the SDK's numbering, not
  confirmed against anything here.
- `0x0016c720` is `main` by position and shape — the sole callee of the entry point, installing interrupt
  handlers — not by a symbol or by having watched it run.
- The VU1 microprograms (`shaders.res`, 108 of them) are not disassembled, and the IOP modules
  (`modules.res`, 7 of them) are not analysed. Neither is EE code; both are listed in `d3-boot-elf`.
- Nothing has run in an emulator yet (no BIOS — see `d1-the-disc/2-the-bios-READY.md`), so the dump has
  never been cross-checked against live memory.

## Next

R2: names. The levers, in the order worth pulling — the `.res` and `.phy` loaders found from
`./kit data --strings boot`, since both formats are now understood and their field names are literal
strings in the image; then `main`'s 36 callees, which are the top of every subsystem.
