"""Print the printable strings inside [addr, addr+len) of a RAM image (S93): name addr len."""
import os, re, sys
ram = open(os.path.join('context', 'states', sys.argv[1] + '.ram'), 'rb').read()
a, n = int(sys.argv[2], 16) & 0x1ffffff, int(sys.argv[3], 16)
for m in re.finditer(rb'[\x20-\x7e]{4,}', ram[a:a + n]):
    print('+0x%04x %s' % (m.start(), m.group().decode()))
