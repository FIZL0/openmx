#!/usr/bin/env python3
r"""Read one moment out of a save state's EE RAM, as a file the port can be posed from.

    ./kit state ram <state.p2s> --out context/states/ram.bin
    python3 research/shots/moment.py context/states/ram.bin -o context/shots/moment.txt

What it reads, and why these addresses (they are research, never product — the port is handed the
numbers, not the addresses):

  0x004e1e98  the GameManager singleton (R6). +0x30 the player manager, +0x34 the actor manager;
              each is {count, head, tail} at +0x30, and a list node is {item, prev, next}.
  actor+0x00  the actor's type: 4 is the rideable vehicle (R6). +0x20 is its player, which is how a
              vehicle is matched to the player that rides it — the first player in the manager is
              the human's.
  actor+0x180 the vehicle's 4x4 world transform, rows x/y/z/position, position at +0x1b0 with w=1.
  0x0046a540  the render camera: a 4x4 with the eye in row 3, immediately followed at 0x0046a580 by
              the frame's view-projection *relative to the eye* — its columns are the camera's right,
              up and forward scaled by 1/tan(fov/2), which is where the 85 degrees horizontal (and
              the 4:3 that makes it 69 vertical) are read from. Verified for the frame this was
              written against: the bike's origin projects to pixel (318, 305) of 640x448 and the seat
              to dead centre, which is where the original's own screenshot has them.

With `--biplane` (V21) the moment is of the plane instead:

  actor type 1 the biplane, spawned by the vehicle spawner 0x0020eb10 when its kind is 1 (the class
              is MXBiplane, ctor 0x00353d10). Matched to the first player by +0x20 like a bike.
  actor+0x18c the plane's model root node (the soultree). The plane's actor+0x180 is not a world
              transform the way a bike's is; the node's world matrix at node+0x80 is, rows x/y/z/
              position in the same convention, and is what the plane is drawn with. Checked on the
              biplane recording's states: parked at the spawn (actor+0xe0) with a 10-degree nose-up
              pitch on its tail wheel, and on screen where the original's own screenshot has it.
  actor+0x3b0 the crashed flag the node poser 0x00352ff0 switches the prop cones on: written out as
              `crashed`.

The track name comes from the one resolved terrain path in RAM (`Tracks\Free09\Free09.ps2.trn`),
which is V1's trick for naming the loaded track.
"""

import math
import os
import re
import struct
import sys

GAME_MANAGER = 0x004E1E98
PLAYER_MANAGER = 0x30
ACTOR_MANAGER = 0x34
LIST = 0x30  # {count, head, tail}
ACTOR_TRANSFORM = 0x180
ACTOR_PLAYER = 0x20
VEHICLE_TYPE = 4
BIPLANE_TYPE = 1
BIPLANE_ROOT = 0x18C
NODE_WORLD = 0x80
BIPLANE_CRASHED = 0x3B0
RENDER_CAMERA = 0x0046A540
VIEW_PROJECTION = 0x0046A580


class Ram:
    def __init__(self, path):
        self.b = open(path, "rb").read()

    def off(self, a):
        return a & 0x1FFFFFF

    def u32(self, a):
        return struct.unpack_from("<I", self.b, self.off(a))[0]

    def f32(self, a):
        return struct.unpack_from("<f", self.b, self.off(a))[0]

    def vec3(self, a):
        return [self.f32(a + 4 * i) for i in range(3)]

    def walk(self, manager):
        node = self.u32(manager + LIST + 4)
        while node:
            yield self.u32(node)
            node = self.u32(node + 8)


def unit(v):
    length = math.sqrt(sum(x * x for x in v))
    return [x / length for x in v], length


def track_name(ram):
    hit = re.search(rb"Tracks\\([A-Za-z0-9_]+)\\[A-Za-z0-9_]+\.ps2\.trn", ram.b)
    if not hit:
        return None
    return hit.group(1).decode("latin1").lower()


def main(argv):
    if not argv:
        print(__doc__)
        return 1
    path = argv[0]
    out = None
    if "-o" in argv:
        out = argv[argv.index("-o") + 1]
    plane = "--biplane" in argv
    ram = Ram(path)

    manager = ram.u32(GAME_MANAGER)
    if not manager:
        print("no GameManager: this state is not in a level")
        return 1
    players = list(ram.walk(ram.u32(manager + PLAYER_MANAGER)))
    kind = BIPLANE_TYPE if plane else VEHICLE_TYPE
    vehicles = [a for a in ram.walk(ram.u32(manager + ACTOR_MANAGER)) if ram.u32(a) == kind]
    if not players or not vehicles:
        print(f"{len(players)} players, {len(vehicles)} vehicles: nothing to pose from")
        return 1
    mine = [v for v in vehicles if ram.u32(v + ACTOR_PLAYER) == players[0]]
    if not mine:
        print("the first player rides none of the vehicles in the actor list")
        return 1
    actor = mine[0]

    transform = actor + ACTOR_TRANSFORM
    if plane:
        transform = ram.u32(actor + BIPLANE_ROOT) + NODE_WORLD
    rows = [ram.vec3(transform + 16 * r) for r in range(4)]
    eye = ram.vec3(RENDER_CAMERA + 0x30)
    vp = [[ram.f32(VIEW_PROJECTION + 16 * r + 4 * c) for c in range(4)] for r in range(4)]
    column = lambda c: [vp[r][c] for r in range(3)]  # noqa: E731
    right, _ = unit(column(0))
    up, vertical = unit(column(1))
    forward, _ = unit(column(3))
    fov_v = 2 * math.degrees(math.atan(1.0 / vertical))

    lines = [
        "# one moment of the original, read out of a save state by research/shots/moment.py",
        f"track {track_name(ram) or '?'}",
        "bike_pos %.4f %.4f %.4f" % tuple(rows[3]),
        "bike_look %.6f %.6f %.6f" % tuple(rows[2]),
        "bike_up %.6f %.6f %.6f" % tuple(rows[1]),
        "camera_eye %.4f %.4f %.4f" % tuple(eye),
        "camera_look %.6f %.6f %.6f" % tuple(forward),
        "camera_up %.6f %.6f %.6f" % tuple(up),
        "camera_fov_v %.4f" % fov_v,
    ]
    if plane:
        lines += ["vehicle biplane", "crashed %d" % (1 if ram.u32(actor + BIPLANE_CRASHED) else 0)]
    text = "\n".join(lines) + "\n"
    sys.stdout.write(text)
    sys.stderr.write(
        "# %d players, %d vehicles; the player's is actor %08x, camera right %s\n"
        % (len(players), len(vehicles), actor, [round(x, 4) for x in right])
    )
    if out:
        os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
        open(out, "w").write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
