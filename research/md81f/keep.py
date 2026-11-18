"""MD81f: copy the md81f states (context/untamed/md81f), the spawn RAM image, the pack's sandrail files, the
md81f captures and tapes, addresses.txt and the scratch SLUS-21701 kit.toml into kept-context/md81f
(research/md81e/keep.py's, for the sandrail).

  python3 -I research/md81f/keep.py [KIT.toml]
"""
import os
import shutil
import struct
import sys
import zipfile

HOME = os.path.expanduser('~')
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
KEEP = os.path.join(HOME, 'repos/openmx-slots/kept-context/md81f')
STATES = os.path.join(ROOT, 'context/untamed/md81f')
os.makedirs(KEEP, exist_ok=True)
for f in sorted(os.listdir(STATES)):
    if f.endswith(('.p2s', '.phy.bpb', '.psg')):
        shutil.copy2(os.path.join(STATES, f), os.path.join(KEEP, f))
with zipfile.ZipFile(os.path.join(STATES, 'spawn.p2s')) as z:
    ram = z.read(next(n for n in z.namelist() if n.endswith('eeMemory.bin')))
assert struct.unpack_from('<I', ram, 0x01be46c0 + 4)[0] == 0x003f09d0, 'the player sandrail moved'
open(os.path.join(KEEP, 'spawn.ee.bin'), 'wb').write(ram)
shutil.copy2(os.path.join(HERE, 'addresses.txt'), os.path.join(KEEP, 'addresses.txt'))
for sub in ('capture', 'recordings'):
    src = os.path.join(ROOT, 'context', sub, 'md81f')
    dst = os.path.join(KEEP, sub)
    os.makedirs(dst, exist_ok=True)
    for f in sorted(os.listdir(src)):
        shutil.copy2(os.path.join(src, f), os.path.join(dst, f))
if len(sys.argv) > 1:
    shutil.copy2(sys.argv[1], os.path.join(KEEP, 'slus21701-kit.toml'))
for root, _, files in os.walk(KEEP):
    for f in sorted(files):
        print(os.path.relpath(os.path.join(root, f), KEEP), os.path.getsize(os.path.join(root, f)))
