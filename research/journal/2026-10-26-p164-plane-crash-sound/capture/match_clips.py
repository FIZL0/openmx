"""Where each [biplaneWreck] clip sounds in the capture's video: a normalised cross-correlation of each
clip over the video's audio, both mono s16 at 22050 Hz, the four best peaks a clip.

    ffmpeg -v error -i context/capture/p164/crash.mp4 -vn -ac 1 -ar 22050 -f s16le OUT/crash.pcm
    python3 -I match_clips.py OUT/crash.pcm OUT/wreck1.pcm ... OUT/wreck5.pcm

The wreck clips are the disc's Vehicle\\mchn_wreck1..5 decoded by the shell's audio::Library
(clip.pcm written out as little-endian s16)."""
import sys
import numpy as np

R = 22050
x = np.fromfile(sys.argv[1], dtype='<i2').astype(np.float64)


def ncc(x, c):
    n = len(x) + len(c)
    N = 1 << (n - 1).bit_length()
    cc = np.fft.irfft(np.fft.rfft(x, N) * np.conj(np.fft.rfft(c, N)), N)[:len(x) - len(c) + 1]
    e = np.concatenate([[0], np.cumsum(x * x)])
    win = e[len(c):] - e[:-len(c)]
    return cc / (np.sqrt(np.maximum(win, 1e-9)) * np.sqrt((c * c).sum()))


print('capture', len(x) / R, 's')
for path in sys.argv[2:]:
    c = np.fromfile(path, dtype='<i2').astype(np.float64)
    r = ncc(x, c)
    top = []
    for _ in range(4):
        k = int(np.argmax(r))
        top.append((round(k / R, 3), round(float(r[k]), 3)))
        r[max(0, k - R // 2):k + R // 2] = -1
    print(path, round(len(c) / R, 4), 's:', top)
