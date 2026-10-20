"""Copy a movie's save state beside a new movie: cpstate.py SRC_MOVIE DST_MOVIE"""
import os, shutil, sys
src, dst = sys.argv[1], sys.argv[2]
shutil.copyfile(src + '_SaveState.p2s', dst + '_SaveState.p2s')
os.chmod(dst + '_SaveState.p2s', 0o644)
print('state', dst + '_SaveState.p2s')
