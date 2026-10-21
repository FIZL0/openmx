"""S56: capture the original wrecking, with a save state from the tick the wreck starts.

usage: python3 -B research/s56_wreck.py NAME --pad SCRIPT [--state SLOT] [--save SLOT] [--frames N]

Runs watch/s56-ragdoll.toml through `ps2kit.record` (one row a tick: every ragdoll bone's body, the
wreck's flags and clocks, the holds' targets, the pad) into context/capture/s56/NAME.csv. `--state`
(default 3) is loaded first and the pad script is played from there, as `./kit record --pad` does.

From the first tick the human's rider reads a wreck reason, the emulator is told to save a state
into each of the `--save` slots in turn (default 4,5,8,9), one a tick, and the capture carries on:
the state is what a test starts the port from, and the rows after it are what the port is compared against. PCSX2 saves at its next safe point, which
may be later than the tick that asked; the test finds the state's own tick by its frame counter.
A state saved while the game was mid-tick is torn (research/s56_state_check.py tells), which is
why there are several. Each is copied to context/capture/s56/NAME-<frame asked>.p2s at the end.

Run it under `./kit pcsx2 with --`, with `./kit pad serve` up for the copy.
"""

import argparse
import shutil
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from ps2kit import emu                   # noqa: E402
from ps2kit import pine as pine_mod      # noqa: E402
from ps2kit import record                # noqa: E402
from ps2kit.config import cfg            # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
RIDER = (0x34, 0x34, 0x8, 0x8, 0x0)      # @game_manager's chain to the human's rider record
WRECK = 0x748                            # its wreck reason


class Saving(pine_mod.Pine):
    """The recorder's PINE client, with one more read a tick: the wreck reason. The first tick it is
    set, a save state is requested into `slot`."""

    def __init__(self, slots: list[int]):
        super().__init__(check=True)
        self.slots = list(slots)
        self.saved: list[tuple[int, int]] = []
        self.saved_at: int | None = None
        self.wreck_at: int | None = None
        self.reason = 0

    def _reason(self) -> int:
        p = self.read32(cfg().symbols['game_manager'])
        for off in RIDER:
            if not 0 < p < 0x0200_0000:
                return 0
            p = self.read32(p + off)
        if not 0 < p < 0x0200_0000:
            return 0
        return self.read32(p + WRECK)

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        if self.saved_at is None:
            r = self._reason()
            if r:
                self.wreck_at, self.reason = v, r
                self.saved_at = v
                print(f'wreck reason {r} at frame {v}', flush=True)
        if self.saved_at is not None and self.slots:
            slot = self.slots.pop(0)
            self.save_state(slot)
            self.saved.append((v, slot))
            print(f'  frame {v}: saving a state into slot {slot}', flush=True)
        return v


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('--pad', required=True)
    ap.add_argument('--state', type=int, default=3)
    ap.add_argument('--save', default='4,5,8,9', help='slots, one saved a tick from the wreck on')
    ap.add_argument('--frames', type=int, default=600)
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's56'
    out.mkdir(parents=True, exist_ok=True)
    p = Saving([int(x) for x in a.save.split(',')])
    watch = record.Watch(ROOT / 'watch' / 's56-ragdoll.toml')
    res = record.record(watch, out / f'{a.name}.csv', frames=a.frames, state=a.state, pad=a.pad, pine=p)
    print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
          f'ended: {res.ended}')
    if p.saved_at is None:
        print('no wreck: no state saved')
        return 1
    print(f'wreck reason {p.reason} first read at frame {p.wreck_at}')
    time.sleep(2.0)                                   # let the last save finish writing
    for v, slot in p.saved:
        dst = out / f'{a.name}-{v}.p2s'
        shutil.copyfile(emu.state_path(str(slot)), dst)
        print(f'  slot {slot} (asked at frame {v}) -> {dst.relative_to(ROOT)}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
