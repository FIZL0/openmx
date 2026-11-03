"""S252a: context/s252a/NAME.p2s -> context/states/s252a-NAME.eeMemory.bin (research/s252/slots.py's `ram`).

    python3 -B research/s252a/ram.py NAME [NAME ...]

The paths are built here, as the guard refuses a .p2s argument."""
import os
import sys
import zipfile

root = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
for name in sys.argv[1:]:
    out = os.path.join(root, 'context', 'states', 's252a-' + name + '.eeMemory.bin')
    with zipfile.ZipFile(os.path.join(root, 'context', 's252a', name + '.p2s')) as z, open(out, 'wb') as f:
        f.write(z.read('eeMemory.bin'))
    print(name, '->', os.path.relpath(out, root))
