"""S129: copy S89's along state out of s3's context into this slot's (context/ does not travel)."""
import shutil
from pathlib import Path
src = Path('/home/rycs/repos/openmx-slots/s3/context/s89')
for n in ('along', 'free06-bike'):
    shutil.copyfile(src / (n + '.p2s'), Path('context/s129') / (n + '.p2s'))
    print(n, 'ok')
