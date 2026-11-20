"""R51: print quadwords of an Untamed RAM image as hex and floats.

    python3 -I research/r51/dump.py md147/spawn ADDR LENGTH

The image is context/untamed/<NAME>.ee.bin (named by its stem, as the context guard asks)."""
import glob, os, struct, sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
path = glob.glob(os.path.join(ROOT, 'context', 'untamed', sys.argv[1] + '.ee*'))[0]
data = open(path, 'rb').read()
base, n = int(sys.argv[2], 16), int(sys.argv[3], 16)
for o in range(0, n, 0x10):
    w = struct.unpack_from('<4I', data, base + o)
    f = struct.unpack_from('<4f', data, base + o)
    print(f'+{o:03x}', ' '.join(f'{x:08x}' for x in w), ' '.join(f'{x:.5g}' for x in f))
