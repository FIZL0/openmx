"""Find a byte pattern (regex) in a RAM image and print address + the surrounding string (S93): name regex [max]."""
import os, re, sys
ram = open(os.path.join('context', 'states', sys.argv[1] + '.ram'), 'rb').read()
mx = int(sys.argv[3]) if len(sys.argv) > 3 else 40
for i, m in enumerate(re.finditer(sys.argv[2].encode(), ram)):
    if i >= mx:
        break
    s = m.start()
    while s > 0 and 0x20 <= ram[s - 1] < 0x7f:
        s -= 1
    e = m.end()
    while e < len(ram) and 0x20 <= ram[e] < 0x7f:
        e += 1
    print('%08x %s' % (s, ram[s:e][:100].decode()))
