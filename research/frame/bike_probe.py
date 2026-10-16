#!/usr/bin/env python3
r"""Walk the live actor list to the human's rider and print the fields A5's watch spec samples.

    ./kit pcsx2 with -- python3 research/frame/bike_probe.py            # the state already loaded
    ./kit pcsx2 with -- python3 research/frame/bike_probe.py --state 8  # load it first

Research, not product: the addresses here are the ones the watch spec turns into `@symbol -> …`
chains, and the point of the script is to say which list entry the human is riding *today* and
which function sits in each dispatch slot the bike's per-frame update calls — the two things a
hand-written hop count gets wrong silently.

  0x004e1e98  the GameManager singleton (R6): +0x34 the actor manager, whose list is {count, head,
              tail} at +0x30; a node is {item, prev, next}.
  actor+0x00  type; 2 is the ridden actor that carries the ride state machine, the tricks and the
              animation (S5's journal calls it "the bike"), 4 is the chassis the physics moves.
              +0x20 its player record, whose +0x34 is the control source: 1 a local human, 2 an AI
              (S8). Free09 holds four of each, so the control source is the only way to tell the
              human's from the three AI riders'.
  actor+0x48  the dispatch table the per-frame update calls through. The slots it uses, in the
              order the update calls them, are DISPATCH below.
"""

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

from ps2kit.pine import Pine  # noqa: E402

GAME_MANAGER = 0x004E1E98
ACTOR_MANAGER = 0x34
LIST = 0x30          # {count, head, tail}
NODE_NEXT = 0x8
NODE_ITEM = 0x0
VEHICLE = 4

# The slots the ridden actor's per-frame update (0x002520d0) calls, in call order; resolve each
# address against research/oracle.txt to see which function the slot holds today.
DISPATCH = [0x13C, 0xEC, 0x124, 0x128, 0x130, 0xF8, 0xF0, 0x10C, 0x110, 0x118, 0x120, 0xFC]

# Fields the frame writes, by the stage that writes them — the columns of watch/frame-bike.toml, which
# is where each one's evidence is written down. The three in the physics group that are *inputs* to the
# animation (the crossup and the two leans) were not found by reading a listing: A5 captured the whole
# object as one `bytes:2688` field and searched it for the field each animation output is an exact
# function of, at this frame and at the one before (the recipe is in the spec's comments).
FIELDS = {
    'physics': [(0x600, 'f32', 'crossup rad'), (0x670, 'f32', 'roll lean -1..1'),
                (0x674, 'f32', 'fore-aft lean'), (0x6D8, 'f32', 'MaxCrossupAngle'),
                (0x748, 'u32', 'ride reason'), (0x80C, 'u32', 'rider state'),
                (0x820, 'f32x4', 'seat position')],
    'air': [(0x6C0, 'f32', 'air clock?'), (0x788, 'f32', 'shown crossup')],
    'anim': [(0x9E4, 'f32', 'MaxCrossupAngle'), (0x9E8, 'f32', 'shown pose'),
             (0x9EC, 'f32', 'lean 0..1'), (0x9F0, 'u32', 'cross-fade ran'),
             (0x9F4, 'f32', 'cross-fade ramp'), (0x9F8, 'f32', 'blend rate'),
             (0xA04, 'u32', 'request count'), (0xA08, 'u32', 'count, last frame'),
             (0xA78, 'u32', 'latched action'), (0x864, 'u32', 'requested action'),
             # +0x8a0 reads as a packed word (0xd4002100 and friends), not the small count S6's
             # note for stunt_state_pack suggests. The restart test's count is +0xa04.
             (0x8A0, 'u32', 'packed state')],
}


def chain(p, addr, *offs):
    for off in offs:
        addr = p.read32(addr)
        if not addr or addr < 0x100000 or addr >= 0x2000000:
            return 0
        addr += off
    return addr


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('--state', type=int)
    ap.add_argument('--hops', type=int, default=None, help='print one entry instead of searching')
    a = ap.parse_args()
    p = Pine()
    if a.state is not None:
        p.load_state(a.state)
        import time
        time.sleep(2.0)

    head = p.read32(chain(p, GAME_MANAGER, ACTOR_MANAGER, LIST + 4))
    count = p.read32(chain(p, GAME_MANAGER, ACTOR_MANAGER, LIST))
    print(f'actor manager: {count} actors')
    node = head
    hops = 0
    found = {}
    while node and hops < 64:
        item = p.read32(node + NODE_ITEM)
        if not item:
            break
        kind = p.read32(item)
        player = p.read32(item + 0x20)
        src = p.read32(player + 0x34) if player else 0
        mark = ''
        if src == 1:
            mark = '  <- the human'
            found.setdefault(kind, (hops, item))
        print(f'  entry {hops:2d}  item 0x{item:08x}  type {kind}  player 0x{player:08x}'
              f'  ctl_src {src}{mark}')
        nxt = p.read32(node + NODE_NEXT)
        if nxt == node:
            break
        node, hops = nxt, hops + 1
    if not found:
        print('no actor with control source 1 in the list')
        return 1
    for kind, (hops, obj) in sorted(found.items()):
        print(f"\n=== the human's type-{kind} actor: list entry {hops}, at 0x{obj:08x}")
        vt = p.read32(obj + 0x48)
        print(f'dispatch table 0x{vt:08x}')
        for slot in DISPATCH:
            print(f'  +0x{slot:03x}  0x{p.read32(vt + slot):08x}')
        for stage, fields in FIELDS.items():
            print(f'{stage}:')
            for off, ty, what in fields:
                if ty == 'f32':
                    v = p.read_f32(obj + off)
                elif ty == 'f32x4':
                    v = [p.read_f32(obj + off + i * 4) for i in range(4)]
                elif ty == 'u8':
                    v = p.read8(obj + off)
                else:
                    v = p.read32(obj + off)
                print(f'  +0x{off:03x}  {what:16s} {v}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
