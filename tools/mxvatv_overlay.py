#!/usr/bin/env python3
"""Fold one of MX vs. ATV Unleashed PS2's overlays into its boot ELF, for Ghidra.

SLUS_211.04 holds the engine; the game (vehicles, riders, tracks) is GAME_REL.BIN and the front end
UI_REL.BIN, loaded at run time into one of the ELF's empty LOAD segments at the same fixed address.
An overlay is a CodeWarrior "MWo3" image: a 64-byte header {magic, version, load address, text size,
data size, bss size, two table pointers, name[32]}, then text and data. The file is loaded whole, header
and all, at that address, with no relocations: the main ELF's vtables point at the load address plus a
function's offset in the file, and header + text + data + bss fills the slot exactly. So the overlay's
bytes become that segment's file contents and nothing else changes.

    # SLUS_211.04 and GAME_REL.BIN off the disc into context/mxvatv-ps2/ (ps2kit.disc.Disc(iso).read)
    tools/mxvatv_overlay.py context/mxvatv-ps2/SLUS_211.04 context/mxvatv-ps2/GAME_REL.BIN \
        context/mxvatv-ps2/atv_game.elf

Write into context/ only: it is game code and never committed.
"""
import struct, sys

HEADER = 64


def fold(elf: bytes, ovl: bytes) -> bytes:
    magic, _ver, addr, text, data, bss = struct.unpack_from('<6I', ovl, 0)
    if magic != 0x336f574d:
        raise SystemExit('not an MWo3 overlay')
    body = ovl[:HEADER + text + data]
    phoff, = struct.unpack_from('<I', elf, 0x1c)
    phentsize, phnum = struct.unpack_from('<HH', elf, 0x2a)
    out = bytearray(elf)
    # The empty slots at the overlay's address (one per overlay): fill the smallest that holds it with
    # the overlay, and drop the others, which would overlap it.
    slots = []
    for i in range(phnum):
        at = phoff + i * phentsize
        kind, _off, vaddr, _pa, filesz, memsz = struct.unpack_from('<6I', out, at)
        if kind == 1 and vaddr == addr and filesz == 0:
            slots.append((memsz, at))
    fits = sorted(s for s in slots if s[0] >= HEADER + text + data + bss)
    if not fits:
        raise SystemExit(f'no empty LOAD segment at {addr:#x} holds {HEADER + text + data + bss:#x} bytes')
    for memsz, at in slots:
        if at == fits[0][1]:
            struct.pack_into('<I', out, at + 4, len(out))
            struct.pack_into('<I', out, at + 16, len(body))
        else:
            struct.pack_into('<I', out, at, 0)           # PT_NULL
    # The section headers would still call the slot empty, so drop them (Ghidra then loads by segment),
    # and keep .reginfo, which carries $gp, as a PT_MIPS_REGINFO segment in the gap after the table.
    shoff, = struct.unpack_from('<I', elf, 0x20)
    shentsize, shnum, shstrndx = struct.unpack_from('<HHH', elf, 0x2e)
    for i in range(shnum):
        name, kind, _f, _a, off, size = struct.unpack_from('<6I', elf, shoff + i * shentsize)
        if kind == 0x70000006:                           # SHT_MIPS_REGINFO
            at = phoff + phnum * phentsize
            if any(out[at:at + phentsize]) or at + phentsize > 0x100:
                raise SystemExit('no room for another program header')
            struct.pack_into('<8I', out, at, 0x70000000, off, 0, 0, size, size, 4, 4)
            struct.pack_into('<H', out, 0x2c, phnum + 1)
    struct.pack_into('<I', out, 0x20, 0)                 # e_shoff
    struct.pack_into('<HHH', out, 0x2e, 0, 0, 0)         # e_shentsize, e_shnum, e_shstrndx
    return bytes(out + body)


if __name__ == '__main__':
    if len(sys.argv) != 4:
        raise SystemExit(__doc__)
    elf, ovl, dst = sys.argv[1:]
    open(dst, 'wb').write(fold(open(elf, 'rb').read(), open(ovl, 'rb').read()))
