"""V79: does the original draw a new picture every video frame or every other one (one per 30 Hz tick)?

Reads context/v79/throw.mp4 (film.sh), decodes it to grey 640x480 with ffmpeg, and prints, per frame,
the mean absolute difference from the frame before; then counts how the changed frames fall."""
import os
import subprocess
import sys

import numpy as np

path = os.path.join('context', 'v79', 'throw' + '.mp4')
w, h = 640, 480
raw = subprocess.run(['ffmpeg', '-v', 'error', '-i', path, '-f', 'rawvideo', '-pix_fmt', 'gray', '-'],
                     capture_output=True, check=True).stdout
frames = np.frombuffer(raw, np.uint8).reshape(-1, h, w).astype(np.int16)
d = np.abs(np.diff(frames, axis=0)).mean(axis=(1, 2))
changed = d > 0.5
print('frames', len(frames), 'changed', int(changed.sum()), 'still', int((~changed).sum()))
runs = []
run = 0
for c in changed:
    if c:
        if run:
            runs.append(run)
        run = 0
    else:
        run += 1
from collections import Counter
print('still runs between changes', Counter(runs).most_common(6))
if len(sys.argv) > 1:
    a, b = map(int, sys.argv[1:3])
    for i in range(a, b):
        print(i, round(float(d[i - 1]), 2))
