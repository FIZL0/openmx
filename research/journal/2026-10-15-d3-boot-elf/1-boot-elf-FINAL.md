# 1 — the boot ELF, and the overlays that do not exist (FINAL)

Tasks D3, D4. 2026-10-15.

## What I did

### The EE image

`./kit elf --info context/elf/boot.elf`:

```
boot.elf: EXEC MIPS entry 0x100008, 2 segments, 8 sections, 0 symbols
  LOAD  0x00100000-0x004eac80  file +0x000080  0x35f800  bss 0x8b480  rwx
  LOAD  0x004eac80-0x004eac80  file +0x35f880       0x0  bss     0x0  rw-
```

So:

| | |
|---|---|
| entry | `0x100008` — eight bytes past the load address, so there is a two-word header or an aligned stub ahead of `main`'s path |
| code + data | `0x100000` – `0x45f800` (`0x35f800` = 3,537,408 bytes, the whole file bar its 0x80 header) |
| bss | `0x45f800` – `0x4eac80` (`0x8b480` = 570,496 bytes) |
| above that | nothing in the image: `0x4eac80` to the top of RAM is heap |
| symbols | **none.** The image is stripped |

The second `LOAD` is zero-length and carries nothing; it is the SDK's usual empty tail.

A single `rwx` segment covering everything is the normal shape for a PS2 game built with the Sony SDK:
there is no separate `.text` mapping, and self-modifying code is possible in principle (nothing says it
happens).

**0 symbols** is the fact that shapes R2: every name in this port comes from what a function *does* and
from the strings it references, never from a symbol table, because there is not one.

### No overlays

`./kit elf --scan` scores every disc file on opcode density, prologue density, and what fraction of the
file's own `jal` targets land inside a plausible `[load, load+size)`. Two files scored at all, and both
are rejected by the third column, which is the one that matters:

```
  MX.PSR                 2163644 KB  opcodes 100%  prologues  0.0/KB  jal    33 in-range   0%  load  ?
  MODULES/IOPRP270.IMG       243 KB  opcodes  96%  prologues  2.2/KB  jal  2513 in-range   0%  load  ?
```

- **`MX.PSR`** — 100% opcode density with *zero* prologues per KB and 0% of its `jal`s in range. That is
  the signature of compressed data, not code: deflate output is high-entropy, and high-entropy bytes
  decode as valid MIPS roughly all the time while containing no function prologues at all. It is the
  archive (see `d5-archives`).
- **`IOPRP270.IMG`** — real code, but IOP code: the SDK's standard module pack, R3000 rather than R5900,
  and not loaded into EE memory.

The disc has only four files, so an overlay could only have come from inside `MX.PSR`. I checked that
directly rather than inferring it. The archive's 633 members are `.res`, `.lsi`, `.pss`, `.ico`, `.img`
and one `.cmt` — no ELF, no IRX at the top level — and the two members that could plausibly have held
code hold code that is not an EE overlay:

- **`modules.res`** → seven IOP modules: `cdvdstm.irx`, `libsd.irx`, `mcman.irx`, `mcserv.irx`,
  `padman.irx`, `ps2sound.irx`, `sio2man.irx`. Six are stock SDK; `ps2sound.irx` is 50 KB with 112
  function symbols and is presumably the game's own sound driver. All IOP, all R3000.
- **`shaders.res`** → 108 `.vuo` VU1 microprograms, usefully named: `Vu1DynamicMesh`, `Vu1KickGS`,
  `Vu1Morph16StripDiffuseTexTransform`, `Vu1Lines`, `Vu1MemClear`. These are the renderer's vertex
  pipeline. They are code, and they will matter in the V tasks, but they are not EE functions and they do
  not get loaded over an EE address.

`kit.toml` therefore has no `[[overlay]]` blocks, and the example block that shipped in
`kit.toml.example` was removed rather than filled in.

## What the original does

At boot: the console loads `SLUS_207.67` to `0x100000` and jumps to `0x100008`. Everything the game ever
executes on the EE is in that one image, already resident. Not yet traced further — the main loop is R3.

## How it was proved

- `./kit elf --info` on the extracted ELF, whose length (3,537,456) equals its ISO9660 directory record.
- `./kit elf --scan` over the whole disc; the two candidates rejected on their own numbers, as above.
- `./kit elf --irx` on `ps2sound.irx`, which parses it as an IOP module — i.e. it is IOP code, confirmed
  by the loader that would load it, not by its extension.
- The archive's full member list, read with the reader from `d5-archives`, grepped for every member whose
  contents could be code.

## Not verified / not 1:1

- **The entry stub.** I have not looked at what the eight bytes before the entry point are, nor followed
  the entry through the SDK's startup to the game's own `main`. R3's job.
- **Nothing has executed.** All of the above is header parsing. The claim "no overlays" is a claim about
  what is *on the disc* and about the absence of a loaded image; I have not watched the game's memory to
  confirm nothing is ever copied into a fixed region at runtime. A game can stream code into a buffer
  without that looking like an overlay file. If a function's address ever turns out to be unstable
  between runs, this is the assumption to re-examine first.
- `IOPRP270.IMG`'s contents are not listed, and the IRX modules are not decompiled. Nothing in the
  simulation should need them; audio will eventually need to know what `ps2sound.irx` expects to be fed.
- The `.vuo` microprograms are not disassembled. Rendering only has to look right, so this is deferred to
  the V tasks rather than unknown-and-worrying.
- `0x4eac80` upward is "heap" by elimination, not by having found the allocator.

## Next

D5: the archive format, so the track and the bike can be read. Then R1/R2 on the stripped image.
