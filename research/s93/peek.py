"""Follow a pointer chain in a RAM image and print words + strings there (S93): name addr [off ...] [-n LEN]."""
import os, re, struct, sys
args = sys.argv[1:]
n = 0x100
if '-n' in args:
    i = args.index('-n'); n = int(args[i + 1], 16); del args[i:i + 2]
ram = open(os.path.join('context', 'states', args[0] + '.ram'), 'rb').read()
u32 = lambda a: struct.unpack_from('<I', ram, a & 0x1ffffff)[0]
a = int(args[1], 16)
for off in args[2:]:
    a = u32(a + int(off, 16))
print('at %08x' % a)
b = ram[a & 0x1ffffff:(a & 0x1ffffff) + n]
for o in range(0, len(b), 16):
    w = struct.unpack_from('<4I', b, o)
    print('+%03x %s  %s' % (o, ' '.join('%08x' % x for x in w), re.sub(rb'[^\x20-\x7e]', b'.', b[o:o + 16]).decode()))
