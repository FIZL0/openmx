"""C16: the frame_counter (0x00479d04) a .p2s holds (a .p2s is a zip; eeMemory.bin is the EE RAM).
usage: state_frame.py STATE.p2s"""
import struct, sys, zipfile

with zipfile.ZipFile(sys.argv[1]) as z:
    name = next(n for n in z.namelist() if n.lower().startswith('eememory'))
    with z.open(name) as f:
        f.seek(0x479d04)
        print(struct.unpack('<I', f.read(4))[0])
