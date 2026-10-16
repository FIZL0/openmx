#!/usr/bin/env python3
"""Put two frames next to each other: the original on the left, the port on the right.

    python3 research/shots/side_by_side.py ORIGINAL.png PORT.png OUT.png [--scale N] [--gap N]

Both frames are cropped to the same height (PCSX2 writes 640x480 for a 640x448 picture, so the
original is trimmed to the port's height, top and bottom equally) and laid out left to right with a
grey divider. `--scale N` box-averages both down by N first, which is how you get a pair small enough
to look at in one go.

Game-derived images are git-ignored: write the output under `context/shots/`.
"""

import sys
import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import png  # noqa: E402

DIVIDER = (96, 96, 96)


def crop_to(height, rows):
    """Trim equally top and bottom — PCSX2's 480 lines hold the game's 448 centred."""
    extra = len(rows) - height
    top = extra // 2
    return rows[top : top + height]


def main(argv):
    if len(argv) < 3:
        print(__doc__)
        return 1
    left_path, right_path, out_path = argv[0], argv[1], argv[2]
    factor, gap = 1, 8
    rest = argv[3:]
    while rest:
        flag = rest.pop(0)
        if flag == "--scale":
            factor = int(rest.pop(0))
        elif flag == "--gap":
            gap = int(rest.pop(0))
        else:
            print(f"unknown argument {flag}")
            return 1

    lw, lh, lrows = png.read(left_path)
    rw, rh, rrows = png.read(right_path)
    lw, lh, lrows = png.scale(lw, lh, lrows, factor)
    rw, rh, rrows = png.scale(rw, rh, rrows, factor)

    height = min(lh, rh)
    lrows, rrows = crop_to(height, lrows), crop_to(height, rrows)
    width = lw + gap + rw
    out = []
    for y in range(height):
        row = bytearray()
        row += lrows[y][: lw * 3]
        row += bytes(DIVIDER) * gap
        row += rrows[y][: rw * 3]
        out.append(row)
    png.write(out_path, width, height, out)
    print(f"{out_path}: {width}x{height}  ({lw}x{lh} | {rw}x{rh})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
