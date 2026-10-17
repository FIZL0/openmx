#!/usr/bin/env python3
"""Read a save state's EE RAM without an emulator: chase a pointer chain, print a field.

    ./research/poke.py <slot> <addr> [-><off> ...] [--as f32x4|u32|...] [--count N] [--stride S]

<slot> is a PCSX2 save-state slot number; <addr> is hex or @name from kit.toml's [symbols].
`-><off>` reads the pointer and adds the offset; `+<off>` is plain arithmetic. Written for V12's hunt
for the animated objects' joint arrays; kept because every later pointer hunt wants one.
"""
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from ps2kit.config import cfg          # noqa: E402
from ps2kit.emu import state_ee        # noqa: E402

KINDS = {'u8': 'B', 'u16': 'H', 'u32': 'I', 's8': 'b', 's16': 'h', 's32': 'i', 'f32': 'f', 'u64': 'Q'}


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    state, argv = argv[0], argv[1:]
    kind, count, stride, words = 'u32', 1, 0, []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == '--as':
            kind = argv[i + 1]; i += 2
        elif a == '--count':
            count = int(argv[i + 1], 0); i += 2
        elif a == '--stride':
            stride = int(argv[i + 1], 0); i += 2
        else:
            words.append(a); i += 1
    ram = open(state_ee(state), 'rb')
    size_of_ram = ram.seek(0, 2)
    syms = cfg().symbols

    def rd(off, n):
        ram.seek(off & 0x1fff_ffff)
        return ram.read(n)

    def u32(off):
        return struct.unpack_from('<I', rd(off, 4))[0]

    base = words[0]
    addr = syms[base[1:]] if base.startswith('@') else int(base, 16)
    for op in words[1:]:
        if op.startswith('->'):
            addr = u32(addr) + int(op[2:], 16)
        elif op.startswith('+'):
            addr = addr + int(op[1:], 16)
        else:
            addr = u32(addr) + int(op, 16)
        if not 0 < (addr & 0x1fff_ffff) < size_of_ram:
            print(f'null/garbage after {op}: {addr:#010x}')
            return 1
    lanes = 4 if kind.endswith('x4') else 3 if kind.endswith('x3') else 1
    base_kind = kind.split('x')[0]
    fmt = KINDS[base_kind]
    size = struct.calcsize(fmt)
    stride = stride or size * lanes
    for n in range(count):
        at = addr + n * stride
        vals = struct.unpack_from('<' + fmt * lanes, rd(at, size * lanes))
        shown = ' '.join(f'{v:g}' if fmt == 'f' else f'{v:#x}' for v in vals)
        print(f'+{n * stride:#06x} {at:#010x}  {shown}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main(sys.argv[1:]))
