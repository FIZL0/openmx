"""MD164: copy the md164 states (context/untamed/md164), their RAM images, the md164 captures and tapes, and
the scratch SLUS-21701 kit.toml into kept-context/md164.

  python3 -I research/md164/keep.py [KIT.toml]
"""
import os
import shutil
import struct
import sys
import zipfile

HOME = os.path.expanduser('~')
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
KEEP = os.path.join(HOME, 'repos/openmx-slots/kept-context/md164')
STATES = os.path.join(ROOT, 'context/untamed/md164')
os.makedirs(KEEP, exist_ok=True)
for f in sorted(os.listdir(STATES)):
    if f.endswith('.p2s'):
        shutil.copy2(os.path.join(STATES, f), os.path.join(KEEP, f))
for name in ('spawn', 'air'):
    with zipfile.ZipFile(os.path.join(STATES, name + '.p2s')) as z:
        ram = z.read(next(n for n in z.namelist() if n.endswith('eeMemory.bin')))
    assert struct.unpack_from('<I', ram, 0x01be4740 + 4)[0] == 0x003f09d0, 'the player truck moved'
    open(os.path.join(KEEP, name + '.ee.bin'), 'wb').write(ram)
for sub in ('capture', 'recordings'):
    src = os.path.join(ROOT, 'context', sub, 'md164')
    dst = os.path.join(KEEP, sub)
    os.makedirs(dst, exist_ok=True)
    for f in sorted(os.listdir(src)):
        shutil.copy2(os.path.join(src, f), os.path.join(dst, f))
if len(sys.argv) > 1:
    shutil.copy2(sys.argv[1], os.path.join(KEEP, 'slus21701-kit.toml'))
for root, _, files in os.walk(KEEP):
    for f in sorted(files):
        print(os.path.relpath(os.path.join(root, f), KEEP), os.path.getsize(os.path.join(root, f)))
