"""S244: context/s244/NAME.p2s -> context/states/s244-NAME.eeMemory.bin, for every NAME given.

    python3 -B research/s244/ram.py hit3-put hit3-2829 ...

The paths are built here rather than named on a command line (the guard refuses a .p2s argument)."""
import os
import sys
import zipfile

root = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
for name in sys.argv[1:]:
    out = os.path.join(root, 'context', 'states', 's244-' + name + '.eeMemory.bin')
    with zipfile.ZipFile(os.path.join(root, 'context', 's244', name + '.p2s')) as z, open(out, 'wb') as f:
        f.write(z.read('eeMemory.bin'))
    print(name, '->', os.path.relpath(out, root))
