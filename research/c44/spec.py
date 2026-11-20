"""C44: write the watch spec for biplane.p2m2's state (free06) with the plane parked in an AI rider's way.

usage: python3 -B research/c44/spec.py FRAMES > SPEC.toml

One row a tick. The plane (MXBiplane 0x00c44a40, S252's lanes): its crash word and clock, its body's
position, velocity, rotation and spin (+0x160 -> +0x60/+0x70/+0x80/+0x90), its collision record's
count. Each of the state's three AI riders (research/c33/riders.py on the state; the rider addresses
are fixed for the session), prefix a0_/a1_/a3_ by actor order:

- the chassis (rider +0x7a4): centre +0x60, quat +0x1a0, velocity +0x260, world spin +0x280, rates
  +0x290, shock +0x170, twist +0x180, the node's world matrix (+0x254 -> +0x40) and the raw 0x2c0;
- the rider's on-ground word +0x574, wreck reason +0x748, its bike clock +0x13fc;
- the levers the bike rode (+0x628 steer, +0x62c lean, +0x63c gas, +0x640 brake, +0x648 reverse;
  research/oracle.txt S12e);
- its collision record (rider +0x7d0): count +0x100, struck +0x40, and the 11 entries of 0x80 at
  *(+0x108) (b7-truck-strike.toml's lanes). The count is cleared before the frame ends; the entries
  stay, the last list built.
"""
import sys

FRAMES = int(sys.argv[1]) if len(sys.argv) > 1 else 600
PLANE = 0x00c44a40
# (prefix, rider) — research/c33/riders.py on biplane.p2m2's RAM image; the human's is 2.
RIDERS = [('a0', 0x018f7290), ('a1', 0x019a44b0), ('a3', 0x01881b10)]

out = []
w = out.append
w('''# c44-plane-bike.toml — written by research/c44/spec.py (C44): biplane.p2m2's plane parked in an AI
# rider's way on free06, the three AI bikes and the plane, one row a tick.

[watch]
name        = "c44-plane-bike"
description = "the parked biplane and free06's three AI bikes: chassis, levers, collision lists"
every       = 1
frames      = %d

[setup]
warmup    = 0
settle_ms = 240
''' % FRAMES)


def field(name, addr, ty):
    w('[[field]]\nname = "%s"\naddr = "%s"\ntype = "%s"\n' % (name, addr, ty))


field('frame_counter', '@frame_counter', 'u32')
field('dt', '@frame_dt', 'f32')
field('rng_state', '@rand_state', 'u64')
P = '0x%x' % PLANE
field('crash', P + ' + 0x3b0', 'u32')
field('crash_time', P + ' + 0x3bc', 'f32')
field('plane_contacts', P + ' + 0x170 -> 0x100', 'u32')
for name, off in (('pos', 0x60), ('vel', 0x70), ('rot', 0x80), ('spin', 0x90)):
    field('plane_' + name, '%s + 0x160 -> 0x%x' % (P, off), 'f32x4')
field('plane_body', P + ' + 0x160 -> 0x0', 'bytes:704')
for pre, r in RIDERS:
    R = '0x%x' % r
    for name, off, ty in (('centre', 0x60, 'f32x4'), ('quat', 0x1a0, 'f32x4'), ('vel', 0x260, 'f32x4'),
                          ('spin', 0x280, 'f32x4'), ('rates', 0x290, 'f32x4'), ('shock', 0x170, 'f32x4'),
                          ('twist', 0x180, 'f32x4')):
        field('%s_ch_%s' % (pre, name), '%s + 0x7a4 -> 0x%x' % (R, off), ty)
    field(pre + '_ch_xform', R + ' + 0x7a4 -> 0x254 -> 0x40', 'mat4')
    field(pre + '_ch_raw', R + ' + 0x7a4 -> 0x0', 'bytes:704')
    for name, off, ty in (('on_ground', 0x574, 'u32'), ('wreck_reason', 0x748, 'u32'), ('clock', 0x13fc, 'f32'),
                          ('steer', 0x628, 'f32'), ('lean', 0x62c, 'f32'), ('gas', 0x63c, 'f32'),
                          ('brake', 0x640, 'f32'), ('reverse', 0x648, 'f32')):
        field('%s_%s' % (pre, name), '%s + 0x%x' % (R, off), ty)
    field(pre + '_rec_count', R + ' + 0x7d0 -> 0x100', 'u32')
    field(pre + '_rec_struck', R + ' + 0x7d0 -> 0x40', 'u32')
    w('[[array]]\nname   = "%s_c"\naddr   = "%s + 0x7d0 -> 0x108 -> 0x0"\ncount  = 11\nstride = 0x80\n' % (pre, R))
    for name, off, ty in (('kind', 0, 'u32'), ('from', 0x10, 'f32x3'), ('end', 0x20, 'f32x3'),
                          ('normal', 0x30, 'f32x3'), ('point', 0x40, 'f32x3'), ('arm', 0x50, 'f32x3'),
                          ('other_arm', 0x60, 'f32x3'), ('surface', 0x70, 'u32'), ('other', 0x74, 'u32'),
                          ('mover', 0x78, 'u32')):
        w('[[array.field]]\nname = "%s"\noff  = 0x%x\ntype = "%s"\n' % (name, off, ty))
print('\n'.join(out))
