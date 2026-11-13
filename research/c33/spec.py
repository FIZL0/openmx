"""C33: write the watch spec for the free09 4-racer capture from a state's riders (riders.py --json).

Per tick: frame_counter, dt, rand_state, in_game; then per rider, in actor-list order, its body
(rider +0x7a4) decoded -- centre +0x60, quat +0x1a0, world velocity +0x260, world spin +0x280, body-frame
spin +0x290, forward speed +0x278 -- and the body's raw first 0x2c0 bytes; the on-ground word (rider
+0x574). The bodies are read at the fixed addresses the state holds (heap objects do not move once the
world is loaded). Columns are `aiK_` for the K-th AI rider in actor order and `human_` for the player.

usage: spec.py RIDERS.json FRAMES > c33.toml
"""
import json, sys

rs = json.load(open(sys.argv[1]))
if sum(1 for r in rs if not r['brain']) != 1 or any(not r['body'] for r in rs):
    raise SystemExit('want one human and every body built: %r' % rs)
frames = int(sys.argv[2])
out = ['# c33.toml -- written by research/c33/spec.py; do not edit by hand.',
       '# C33: free09, 4 racers, the riders\' bodies and rand_state per tick (research/c33/README.md).',
       '# riders (actor order): ' + ', '.join('%s rider %#x body %#x brain %#x'
                                             % ('AI' if r['brain'] else 'human', r['rider'], r['body'], r['brain']) for r in rs),
       '', '[watch]', 'name        = "c33-free09-ai4"',
       'description = "free09, 4 racers from a state before the AI riders move: bodies + rand_state per tick"',
       'every       = 1', 'frames      = %d' % frames, '', '[setup]', 'warmup = 0', 'settle_ms = 100', '']


def field(name, addr, typ):
    out.extend(['[[field]]', 'name = "%s"' % name, 'addr = "%s"' % addr, 'type = "%s"' % typ, ''])


field('frame_counter', '@frame_counter', 'u32')
field('dt', '@frame_dt', 'f32')
field('rng_state', '@rand_state', 'u64')
field('in_game', '0x004e2650', 'u32')
k = 0
for r in rs:
    if r['brain']:
        pre = 'ai%d_' % k
        k += 1
    else:
        pre = 'human_'
    b = r['body']
    field(pre + 'pos', '0x%08x' % (b + 0x60), 'f32x4')
    field(pre + 'quat', '0x%08x' % (b + 0x1a0), 'f32x4')
    field(pre + 'vel', '0x%08x' % (b + 0x260), 'f32x4')
    field(pre + 'spin', '0x%08x' % (b + 0x280), 'f32x4')
    field(pre + 'rates', '0x%08x' % (b + 0x290), 'f32x4')
    field(pre + 'fwd_speed', '0x%08x' % (b + 0x278), 'f32')
    field(pre + 'on_ground', '0x%08x' % (r['rider'] + 0x574), 'u32')
    field(pre + 'body_raw', '0x%08x' % b, 'bytes:704')
print('\n'.join(out))
