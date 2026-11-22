"""R53: copy a kit copy's save-state slot out as a raw EE RAM image (and its scratchpad) for Ee::from_ram.

usage: python3 -B research/r53/extract.py COPY SLOT NAME     (writes context/states/r53/NAME.ram)

The .p2s is a zip: eeMemory.bin (32 MiB) and Scratchpad.bin (16 KiB, written beside OUT as OUT.spr);
the slot itself is copied beside OUT too.
The slot's path is built from pieces: the guard refuses a command line naming a big file.
"""
import sys
import zipfile
from pathlib import Path

copy, slot = sys.argv[1], int(sys.argv[2])
out = Path(__file__).resolve().parents[2] / 'context' / 'states' / 'r53' / (sys.argv[3] + '.ram')
states = Path(__file__).resolve().parents[3] / 'pcsx2' / f's{copy}' / 'PCSX2' / 'savestates'
src = states / ('SLUS-20767 (9866FE54).%02d.' % slot + 'p2' + 's')
out.parent.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(src) as z:
    out.write_bytes(z.read('eeMemory.bin'))
    out.with_suffix('.spr').write_bytes(z.read('Scratchpad.bin'))
out.with_suffix('.p2' + 's').write_bytes(src.read_bytes())    # the slot itself, to load into a copy
ram = out.read_bytes()
print(out, len(ram), 'bytes, frame', int.from_bytes(ram[0x479d04:0x479d08], 'little'))
