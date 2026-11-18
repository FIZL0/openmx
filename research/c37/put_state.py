"""C37: copy a .p2s into this copy's save-state slot. usage: python3 -B research/c37/put_state.py P2S SLOT"""
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent))
from ps2kit import emu  # noqa: E402

dst = emu._folder('Savestates', 'sstates') / f'SLUS-20767 (9866FE54).{int(sys.argv[2]):02d}.p2s'
shutil.copyfile(sys.argv[1], dst)
print('slot', sys.argv[2], '<-', sys.argv[1], dst.stat().st_size)
