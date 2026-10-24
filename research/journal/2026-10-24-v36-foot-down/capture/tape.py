"""V36: a shell tape that rides off, brakes to a stand and stands (one line a 30 Hz tick).

    python3 -B research/journal/2026-10-24-v36-foot-down/capture/tape.py > target/v36-stop.tape
    target/release/openmx --play target/v36-stop.tape --shot context/shots/v36/port.png --shot-frame 260 --seconds 10
"""

IDLE = bytes.fromhex("0079ffff7f7f7f7f") + bytes(24)


def report(cross=False, square=False):
    r = bytearray(IDLE)
    if cross:
        r[3] &= 0xBF
        r[14] = 0xFF
    if square:
        r[3] &= 0x7F
        r[15] = 0xFF
    return bytes(r)


legs = [(45, report(cross=True)), (100, report(square=True)), (110, IDLE)]
n = sum(k for k, _ in legs)
print(f"# game-sim pad tape: {n} frames")
print("format 1")
print("tape v36-stop")
for k, r in legs:
    for _ in range(k):
        print(f"frame 0x3d088889 0x{r.hex()} -")
