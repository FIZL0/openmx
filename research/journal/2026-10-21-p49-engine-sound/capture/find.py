"""Find each engine sample in the original's recording: a normalised cross-correlation of every clip
the port decodes (written by `the_originals_ride_replays` to context/p49/clips/) against the
recording's audio, the strongest few starts printed per clip.

usage: python3 -B find.py context/p49   (after ffmpeg made original22.wav: mono, 22050 Hz)

Then every sample start hands.py logged (the note's queue head changing) is looked up: within 0.15 s of
where it should be in the recording (the capture runs LAG seconds ahead of the log), how the clip the
original started ranks among all nineteen.

A clip that is the sample the original played scores near 1 where it started, under the music and the
world's sounds; a wrong file scores what any engine noise does.
"""
import os
import sys
import wave

import numpy as np


def read(path):
    with wave.open(path) as w:
        n, ch, rate = w.getnframes(), w.getnchannels(), w.getframerate()
        x = np.frombuffer(w.readframes(n), dtype='<i2').astype(np.float64)
    return x.reshape(-1, ch).mean(axis=1), rate


def ncc(sig, clip):
    n = len(sig) + len(clip)
    size = 1 << (n - 1).bit_length()
    num = np.fft.irfft(np.fft.rfft(sig, size) * np.conj(np.fft.rfft(clip, size)), size)[: len(sig) - len(clip) + 1]
    sq = np.concatenate([[0.0], np.cumsum(sig * sig)])
    energy = sq[len(clip):] - sq[: len(sig) - len(clip) + 1]
    return num / (np.sqrt(np.maximum(energy, 1e-9)) * np.linalg.norm(clip))


def peaks(score, rate, k=4, apart=0.5):
    out, s = [], score.copy()
    for _ in range(k):
        i = int(np.argmax(s))
        out.append((i / rate, s[i]))
        lo, hi = max(0, i - int(apart * rate)), i + int(apart * rate)
        s[lo:hi] = -1
    return out


# The first_recording state's sound objects (obj.py's dump of the note's lists), by the clip the
# port writes for them.
HEADS = {0xdcb9b0: 'Idle0', 0xdcaba0: 'LowAccel0', 0xdcac80: 'LowAccel1', 0xdcad60: 'HighAccel0',
         0xdcae40: 'HighAccel1', 0xdcaf20: 'HighAccel2', 0xdcb000: 'HighAccel3', 0xdcb0e0: 'HighAccel4',
         0xdcb1c0: 'HighAccel5', 0xdcb2a0: 'HighAccel6', 0xdcb460: 'Power0', 0xdcb630: 'Power1',
         0xdcb710: 'Power2', 0xdcb7f0: 'Sputter0', 0xdcb8d0: 'Sputter1', 0xdcb380: 'Decel0',
         0xdd0300: 'Clutch0', 0xdd06b0: 'Rev0', 0xdd0220: 'Rev1'}
LAG = 0.11
# The 125's files in the archive's order: P42's directory reading gave each name the file before it.
ARCHIVE = ['Clutch0', 'Decel0', 'HighAccel3', 'HighAccel4', 'HighAccel5', 'HighAccel6', 'Sputter1', 'Idle0',
           'LowAccel1', 'LowAccel0', 'HighAccel0', 'HighAccel1', 'HighAccel2', 'Rev1', 'Power0', 'Power2',
           'Power1', 'Sputter0', 'Rev0']
P42 = {b: a for a, b in zip(ARCHIVE, ARCHIVE[1:])}


def main(folder):
    sig, rate = read(os.path.join(folder, 'original22.wav'))
    scores = {}
    for name in sorted(os.listdir(os.path.join(folder, 'clips'))):
        clip, crate = read(os.path.join(folder, 'clips', name))
        assert crate == rate, (name, crate)
        # The first 0.2 s: long enough to be the file's own, short enough for the next sample cutting in.
        head = clip[: int(0.2 * rate)]
        scores[name[:-4]] = ncc(sig, head)
        found = peaks(scores[name[:-4]], rate)
        print(f'{name:16} ' + '  '.join(f'{t:6.2f}s {v:.2f}' for t, v in found))
    print()
    last, firsts = None, 0
    for line in open(os.path.join(folder, 'original.csv')).read().splitlines()[1:]:
        f = line.split(',')
        t, head = float(f[0]), int(f[8], 16)
        if head == last:
            continue
        last = head
        want = HEADS[head]
        at = int((t - LAG) * rate)
        lo, hi = max(0, at - int(0.15 * rate)), at + int(0.15 * rate)
        best = sorted(((s[lo:hi].max(), n) for n, s in scores.items()), reverse=True)
        names = [n for _, n in best]
        rank = names.index(want) + 1
        was = names.index(P42[want]) + 1 if want in P42 else 0
        firsts += rank == 1
        print(f'{t:6.2f}s {want:11} rank {rank:2} (P42 played {P42.get(want, "-"):11} rank {was:2})  best {best[0][1]} {best[0][0]:.2f}')
    print(f'the started sample is the best match at {firsts} starts')


main(sys.argv[1])
