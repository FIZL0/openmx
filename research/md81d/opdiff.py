#!/usr/bin/env python3
"""MD81d: diff two `./kit dis` listings of a function's twins by their operations alone.

Registers, memory offsets, stack frame offsets, branch targets and lui halves are all set aside: each
line is kept as its mnemonic, with branch-likely folded onto the plain branch and nops, unconditional
`b`, register moves (`move`, `mov.s`) and stack saves/restores dropped. Float compares keep their
condition (c.olt.s vs c.ole.s matters) and immediates of `addiu`/`ori`/`daddiu` against $zero are
kept (they are constants, a call's ids or a loop's bound). What is left is what really differs:
added or dropped calls, compares and arithmetic. A delete matched by an equal insert elsewhere (an
instruction moved across a branch) is cancelled.

    python3 -I research/md81d/opdiff.py ATV.s UNT.s
"""
import difflib
import re
import sys

LINE = re.compile(r'^([0-9a-f]{8})\s+[0-9a-f]{8}\s+(\S+)\s*(.*?)\s*(;.*)?$')
LIKELY = {'beql': 'beq', 'bnel': 'bne', 'beqzl': 'beqz', 'bnezl': 'bnez', 'bc1tl': 'bc1t', 'bc1fl': 'bc1f',
          'blezl': 'blez', 'bgtzl': 'bgtz', 'bltzl': 'bltz', 'bgezl': 'bgez'}
DROP = {'nop', 'b', 'move', 'mov.s', 'sq', 'lq', 'sd', 'ld'}


def read(path):
    out = []
    for raw in open(path):
        m = LINE.match(raw.rstrip('\n'))
        if not m:
            continue
        addr, op, args = m.group(1), m.group(2), m.group(3)
        op = LIKELY.get(op, op)
        if op in DROP:
            continue
        if '$sp' in args and op in ('addiu', 'sw', 'lw', 'swc1', 'lwc1'):
            continue
        key = op
        k = re.match(r'\$\w+, \$zero, (-?\d+)$', args)
        if op in ('addiu', 'daddiu', 'ori') and k:
            key = f'{op} {k.group(1)}'
        out.append((addr, op, args, key))
    return out


def main():
    a, u = read(sys.argv[1]), read(sys.argv[2])
    sm = difflib.SequenceMatcher(None, [x[3] for x in a], [x[3] for x in u], autojunk=False)
    left = [(t, a[i1:i2], u[j1:j2]) for t, i1, i2, j1, j2 in sm.get_opcodes() if t != 'equal']
    dk = {}
    for t, xs, ys in left:
        for x in xs:
            dk.setdefault(x[3], []).append(x)
    gone_a, gone_u = set(), set()
    for t, xs, ys in left:
        for y in ys:
            if dk.get(y[3]):
                gone_a.add(dk[y[3]].pop()[0])
                gone_u.add(y[0])
    print(f'ops: atv {len(a)}, untamed {len(u)}')
    for t, xs, ys in left:
        xs = [x for x in xs if x[0] not in gone_a]
        ys = [y for y in ys if y[0] not in gone_u]
        if not xs and not ys:
            continue
        print(f'  -- {t}')
        for x in xs:
            print(f'   atv {x[0]} {x[1]} {x[2]}')
        for y in ys:
            print(f'   unt {y[0]} {y[1]} {y[2]}')


if __name__ == '__main__':
    main()
