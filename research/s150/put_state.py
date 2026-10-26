"""S150: put recording2's save state (context/recordings) into this copy's slot 3, keeping what was there
as .03.p2s.s150bak. usage: python3 -B research/s150/put_state.py SAVESTATES_DIR"""
import shutil, sys
from pathlib import Path
d = Path(sys.argv[1])
dst = d / 'SLUS-20767 (9866FE54).03.p2s'
bak = dst.with_name(dst.name + '.s150bak')
if dst.exists() and not bak.exists():
    shutil.copyfile(dst, bak)
shutil.copyfile(Path('context/recordings') / ('recording2.p2m2' + '_SaveState.p2s'), dst)
print('slot 3 <- recording2', dst.stat().st_size)
