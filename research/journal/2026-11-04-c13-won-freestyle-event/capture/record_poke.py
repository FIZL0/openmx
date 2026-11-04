"""C13: `./kit record` with one PINE write in the middle, on the recorder's own connection.

usage: record_poke.py SPEC OUT FRAMES AT VALUE

Runs ps2kit.record.record() on SPEC, but between two frames, once, as soon as the stunt
competition's clock (manager +0x9c) has reached AT seconds with one player on the activity's list,
writes VALUE (f32) over the first card's total (card +0xdc, the card at manager +0x34): the
"total past the target" mid-event. The frame counter and the clock at the write go to stdout and
to OUT.poke. One connection only: a second PINE client while the recorder polls hangs both.
"""
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
from ps2kit import record as R                # noqa: E402
from ps2kit.config import cfg                 # noqa: E402
from ps2kit.pine import Pine                  # noqa: E402

ACTIVITY = 0x018532e0                         # first_recording's heap (c9-event.toml)
PLAYERS = ACTIVITY + 0x180
CARD = ACTIVITY + 0x50 + 0x34
CLOCK = ACTIVITY + 0x50 + 0xf8

spec, out, frames, at, value = sys.argv[1], sys.argv[2], int(sys.argv[3]), float(sys.argv[4]), \
    float(sys.argv[5])
FRAME = cfg().symbols['frame_counter']


class Poking:
    """The recorder's Pine, plus the one write."""

    def __init__(self, p):
        self.p = p
        self.done = False

    def __getattr__(self, name):
        return getattr(self.p, name)

    def next_frame(self, last, *a, **k):
        v = self.p.next_frame(last, *a, **k)
        if not self.done and self.p.read32(PLAYERS) == 1:
            clock = struct.unpack('<f', struct.pack('<I', self.p.read32(CLOCK)))[0]
            if at <= clock < at + 2:
                card = self.p.read32(CARD)
                before = self.p.read32(card + 0xdc)
                self.p.write_f32(card + 0xdc, value)
                after = self.p.read32(FRAME)
                self.done = True
                line = (f'poke: card {card:#x} +0xdc {before:#010x} -> {value} between frame {v} '
                        f'and {after}, clock {clock!r}')
                print(line, flush=True)
                Path(out + '.poke').write_text(line + '\n')
        return v


res = R.record(R.Watch(spec), out, frames=frames, pine=Poking(Pine(check=True)))
print(res.summary())
