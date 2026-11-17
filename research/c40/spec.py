"""C40: the watch spec for the drop gate's sink on a race (research/c40/README.md).

usage: spec.py [CARDS] [FRAMES] > NAME.toml
Per tick: the gate object (activity +0x3ac): the group's place (+0x40, y at +0x44), the sink's clock
(+0x50 t, +0x54 length, +0x58 on), +0xac, the trigger distance +0xb0, armed +0xb8, the counter +0xc0,
the list count +0x60; each gathered collision piece's collidable words +0x120..+0x220 (+0x1a0 is the
matrix vt +0xac stores, +0x1e0 the one it replaced); the group node's first 0x80 bytes; and per card,
in list order, its id and the two line distances the trigger compares (+0x10 kept, +0x14 now).
"""
import sys

cards = int(sys.argv[1]) if len(sys.argv) > 1 else 4
frames = int(sys.argv[2]) if len(sys.argv) > 2 else 12000
act = '@activity_manager -> 0x58 -> 0x0'
gate = act + ' -> 0x3ac'
out = []
w = out.append
w('# written by research/c40/spec.py; do not edit by hand.')
w('[watch]')
w('name        = "c40-drop-gate-sink"')
w('description = "drop gate sink on a race: the gate group y, its clock and trigger, the collision pieces, the cards\' line distances"')
w('every       = 1')
w('frames      = %d' % frames)
w('start       = "in_game == 1"')
w('')
w('[setup]')
w('warmup = 0')


def field(name, addr, ty):
    w('')
    w('[[field]]')
    w('name = "%s"' % name)
    w('addr = "%s"' % addr)
    w('type = "%s"' % ty)


field('frame_counter', '@frame_counter', 'u32')
field('dt', '@frame_dt', 'f32')
field('in_game', '0x004e2650', 'u32')
field('gate', act + ' -> 0x3ac', 'u32')
field('gate_x', gate + ' -> 0x40', 'f32')
field('gate_y', gate + ' -> 0x44', 'f32')
field('gate_y_bits', gate + ' -> 0x44', 'u32')
field('gate_z', gate + ' -> 0x48', 'f32')
field('sink_t', gate + ' -> 0x50', 'f32')
field('sink_len', gate + ' -> 0x54', 'f32')
field('sink_on', gate + ' -> 0x58', 'u32')
field('group', gate + ' -> 0x5c', 'u32')
field('pieces', gate + ' -> 0x60', 's32')
field('lowered', gate + ' -> 0xac', 'u32')
field('trigger', gate + ' -> 0xb0', 'f32')
field('armed', gate + ' -> 0xb8', 'u32')
field('count', gate + ' -> 0xc0', 's32')
field('gate_raw', gate + ' -> 0x0', 'bytes:208')
field('group_raw', gate + ' -> 0x5c -> 0x0', 'bytes:304')
for k in range(2):
    field('col%d_obj' % k, gate + ' -> 0x68 -> 0x%x' % (4 * k), 'u32')
    field('col%d_node' % k, gate + ' -> 0x68 -> 0x%x -> 0x34 -> 0x230' % (4 * k), 'u32')
    field('col%d_raw' % k, gate + ' -> 0x68 -> 0x%x -> 0x34 -> 0x120' % (4 * k), 'bytes:256')
chain = act + ' -> 0x84'
for k in range(cards):
    field('card%d_addr' % k, chain, 'u32')
    field('card%d_id' % k, chain + ' -> 0x0', 's32')
    field('card%d_kept' % k, chain + ' -> 0x10', 'f32')
    field('card%d_now' % k, chain + ' -> 0x14', 'f32')
    chain += ' -> 0x1c8'
print('\n'.join(out))
