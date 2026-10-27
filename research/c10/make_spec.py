"""Write watch/2p.toml, C10's spec for the user's 2p recording (both riders, every column C10 asks for).

    python3 -B research/c10/make_spec.py > watch/2p.toml

One generator rather than 1,500 hand-written lines: the two riders' columns are the same list on two
chains. The chains and the path cars were measured in the 2p start state's RAM (research/c10/peek.py,
research/c10/cars.py). Research, not product.
"""

HEAD = '@game_manager -> 0x34 -> 0x34'


def entry(i):
    return HEAD + ' -> 0x8' * i + ' -> 0x0'


# In the 2p state the list is: 0, 1 the biplanes; 2 P2's vehicle (player +0x30 = 1); 3 P1's vehicle
# (player 0); 4 P1's bike actor; 5 P2's. S93's tapes had the two vehicles the other way round.
RIDERS = {'p1': (entry(3), entry(4)), 'p2': (entry(2), entry(5))}

# The six path cars (vtable at +8), heap addresses fixed once the level is loaded: three trailer cabs
# and their trailers, in RAM order.
CARS = [0x00febf40, 0x01b7fda0, 0x01b8f300, 0x01b93ef0, 0x01ba0a00, 0x01ba5610]

out = []


def field(name, addr, typ, comment=''):
    out.append('[[field]]')
    out.append('name = "%s"%s' % (name, ('  # ' + comment) if comment else ''))
    out.append('addr = "%s"' % addr)
    out.append('type = "%s"' % typ)
    out.append('')


def array(name, addr, count, stride, fields, comment='', deref=False):
    out.append('[[array]]')
    out.append('name   = "%s"%s' % (name, ('  # ' + comment) if comment else ''))
    out.append('addr   = "%s"' % addr)
    out.append('count  = %d' % count)
    out.append('stride = 0x%x' % stride)
    if deref:
        out.append('deref  = true')
    out.append('')
    for n, off, t in fields:
        out.append('[[array.field]]')
        out.append('name = "%s"' % n)
        out.append(('off  = "%s"' if isinstance(off, str) else 'off  = 0x%x') % off)
        out.append('type = "%s"' % t)
        out.append('')


VEHICLE = [
    # name, offset, type, comment
    ('type', '0x0', 'u32', '2 = the ridden vehicle'),
    ('index', '0x20 -> 0x30', 'u32', 'player +0x30: 0 for P1, 1 for P2'),
    ('ctl_src', '0x20 -> 0x34', 'u32', '1 = a local human'),
    ('wreck_reason', '0x748', 'u32', '0 riding, else the reason'),
    ('wreck_state', '0xb7c', 'u32', 'the wrecked flag, 2 = put-back ghost'),
    ('on_ground', '0x574', 'u32', 'the air step: either wheel touches'),
    ('both_wheels', '0x578', 'u32', 'both wheels touch'),
    ('airborne', '0x57c', 'u32', 'neither wheel touches'),
    ('g_peak', '0x550', 'f32x4', 'the substeps\' max-by-magnitude body-frame acceleration'),
    ('joust_vel', '0x560', 'f32x3', 'the velocity a joust copies in'),
    ('flip_torqued', '0x4f4', 'u32', 'the backflip (C6)'),
    ('flip_ramp', '0x4f8', 'u32', ''),
    ('flip_on', '0x4fc', 'u32', 'fired'),
    ('flip_arm', '0x500', 'u32', 'armed'),
    ('flip_grace', '0x50c', 'f32', ''),
    ('flip_torque', '0x520', 'f32x4', ''),
    ('fresh', '0x750', 'u32', 'just put back; no trick may start'),
    ('trick_latched', '0x864', 'u32', 'the trick machine (C6)'),
    ('trick_group', '0x9fc', 'u32', ''),
    ('trick_dir', '0xa00', 'u32', ''),
    ('trick_count', '0xa04', 'u32', ''),
    ('trick_running', '0xa0c', 'u32', ''),
    ('trick_safe', '0xa10', 'u32', ''),
    ('trick_armed', '0xa18', 'u32', ''),
    ('trick_elapsed', '0xa1c', 'f32', ''),
    ('trick_id', '0xa78', 'u32', ''),
    ('trick_req', '0xa7c', 'u32', ''),
    ('was_airborne', '0x580', 'u32', 'the air step: airborne the substep before'),
    ('hang_time', '0x4f0', 'f32', 'seconds in the air'),
    ('crossup', '0x600', 'f32', ''),
    ('ch_centre', '0x7a4 -> 0x60', 'f32x3', 'chassis: centre'),
    ('ch_speed', '0x7a4 -> 0x34', 'f32', 'chassis_measure: speed'),
    ('ch_euler', '0x7a4 -> 0x90', 'f32x3', 'chassis_measure: pitch, yaw, roll (the wreck test reads +0x90, +0x98)'),
    ('ch_force', '0x7a4 -> 0x130', 'f32x3', ''),
    ('ch_moment', '0x7a4 -> 0x140', 'f32x3', ''),
    ('ch_shock', '0x7a4 -> 0x170', 'f32x3', 'impulse shock'),
    ('ch_twist', '0x7a4 -> 0x180', 'f32x3', 'impulse twist'),
    ('ch_vel', '0x7a4 -> 0x260', 'f32x3', ''),
    ('ch_rates', '0x7a4 -> 0x290', 'f32x3', ''),
    ('rec_count', '0x7d0 -> 0x100', 'u32', 'the collision record: contacts (cap 11)'),
    ('rec_struck', '0x7d0 -> 0x40', 'bytes:16', 'raised when another record mirrors a hit in'),
]
WHEEL = [
    ('contact', '0xd4', 'u32', 'wheel_set_contact\'s flag'),
    ('owner', '0x1f0', 'u32', 'the object the contact belongs to'),
    ('car', '0x1f4', 'u32', 'the vehicle under the wheel: a path car when riding one (S136)'),
    ('car_vel', '0x1b0', 'f32x3', 'the car velocity the wheel subtracts'),
]
BIKE = [
    ('bike_type', '0x0', 'u32', '4 = a bike actor'),
    ('bike_player', '0x20 -> 0x30', 'u32', ''),
    ('bike_vel', '0x130', 'f32x3', '0.8 of the body\'s'),
    ('bike_xform', '0x180', 'mat4', 'row 3 the position'),
]
# The rider body actor, 0x400 in front of the bike actor (rider-joints.toml, s56-ragdoll.toml).
BODY = [
    ('joint_count', '0x230', 'u32', '22'),
    ('mode', '0x47c', 's32', '0 placed, 1 riding, 2 wreck ragdoll, 3 table/hold, 4 start/look'),
    ('mode_prev', '0x480', 's32', ''),
    ('touching', '0x48c', 'u32', ''),
    ('roll', '0x490', 'f32', ''),
    ('rag_flags', '0x2a4', 'bytes:24', 'riding, repose, placed, active, stiff, torques'),
    ('off_bike', '0x6b0', 'u32', ''),
    ('settled', '0x6b4', 'u32', ''),
    ('flung', '0x6b8', 'u32', ''),
    ('m_finished', '0x3c', 'u32', 'the motion clock'),
    ('m_clock', '0x48', 'bytes:80', '+0x48 clip, +0x4c from, +0x50 last, +0x54 time, +0x80.. fade'),
    ('d_flags', '0x680', 'bytes:40', 'the machine: +0x68c ramp'),
    ('d_cross', '0x8dc', 'bytes:36', '+0x8f0 state, +0x8f4 want'),
    ('d_extra', '0x9f0', 's32', ''),
    ('d_reverse', '0xa3c', 'f32', ''),
    ('req_placing', '0x488 -> 0xc', 'u32', 'the request: placement flag'),
    ('req_wreck', '0x488 -> 0x104', 'u32', 'the request: wreck reason'),
    ('req_axes', '0x488 -> 0x170', 'f32x3', ''),
    ('req_progress', '0x488 -> 0x1a4', 'f32', ''),
]


def rider(p):
    veh, bike = RIDERS[p]
    out.append('# ' + '-' * 100)
    out.append('# %s: vehicle %s' % (p.upper(), veh))
    out.append('')
    for n, off, t, c in VEHICLE:
        field('%s_%s' % (p, n), '%s -> %s' % (veh, off), t, c)
    array('%s_c' % p, veh + ' -> 0x7d0 -> 0x108 -> 0x0', 11, 0x80,
          [('kind', 0, 'u32'), ('end', 0x20, 'f32x3'), ('normal', 0x30, 'f32x3'), ('point', 0x40, 'f32x3'),
           ('other', 0x74, 'u32')], 'the record\'s contact list, 0x80-byte entries')
    for w, off in (('wf', '0x7b0'), ('wr', '0x7b4')):
        for n, o, t, c in WHEEL:
            field('%s_%s_%s' % (p, w, n), '%s -> %s -> %s' % (veh, off, o), t, c)
    for n, off, t, c in BIKE:
        field('%s_%s' % (p, n), '%s -> %s' % (bike, off), t, c)
    for n, off, t, c in BODY:
        field('%s_%s' % (p, n), '%s -> %s - 0x400' % (bike, off) if '->' not in off else
              '%s -> %s - 0x400 -> %s' % (bike, off.split(' -> ')[0], off.split(' -> ')[1]), t, c)
    array('%s_j' % p, bike + ' -> 0x234 - 0x400 -> 0x0', 22, 0x60,
          [('t', 0x40, 'f32x4'), ('q', 0x50, 'f32x4')], 'the 22 posed joints')
    array('%s_bone' % p, bike + ' -> 0x2f4 - 0x400 -> 0x0', 18, 0xb0,
          [('pos', '0xa4 -> 0x60', 'f32x3'), ('vel', '0xa4 -> 0x70', 'f32x3'), ('quat', '0xa4 -> 0x170', 'f32x4')],
          'the 18 ragdoll parts\' rigid bodies')


out.append(open('research/c10/spec_head.toml').read().rstrip() + '\n')
field('frame_counter', '@frame_counter', 'u32')
field('dt', '@frame_dt', 'f32')
field('rng_state', '@rand_state', 'u64')
array('ctl0', '@pad0_control_state', 16, 0x18, [('held', 0, 'u32')])
array('ctl1', '@pad1_control_state', 16, 0x18, [('held', 0, 'u32')])
for n, a in (('pad0_a', '@pad_buffers'), ('pad0_b', '@pad_buffers + 0x80'), ('pad1_a', '@pad_buffers + 0x400'),
             ('pad1_b', '@pad_buffers + 0x480')):
    field(n, a, 'bytes:20')
field('stamp0_a', '@pad0_stamp_a', 'u32')
field('stamp0_b', '@pad0_stamp_b', 'u32')
field('stamp1_a', '@pad_buffers + 0x458', 'u32')
field('stamp1_b', '@pad_buffers + 0x4d8', 'u32')
rider('p1')
rider('p2')
out.append('# ' + '-' * 100)
out.append('# the six path cars: car +0x2a0 the world matrix the step left (row 3 the position), +0x310 the')
out.append('# position the tick before. A car\'s velocity is a virtual (S136): (xform row 3 - prev) * 60 gives it.')
out.append('# car%d_vel (+0x260) reads 0 and car%d_clock (+0x43c) stays put in the 2p run: not what they were')
out.append('# taken for, kept so the captures keep their columns')
out.append('')
for i, c in enumerate(CARS):
    field('car%d_xform' % i, '0x%08x' % (c + 0x2a0), 'mat4')
    field('car%d_prev' % i, '0x%08x' % (c + 0x310), 'f32x3', 'the position row before the step')
    field('car%d_vel' % i, '0x%08x' % (c + 0x260), 'f32x3', 'reads 0: not the velocity')
    field('car%d_clock' % i, '0x%08x' % (c + 0x43c), 'f32', 'constant: not the path clock')
print('\n'.join(out).rstrip())
