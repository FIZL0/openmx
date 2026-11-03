#!/usr/bin/env python3
"""MD81a: diff two `./kit dis` listings of a function's twins, scheduling and moved fields aside.

nops are dropped, branch/jump targets masked, and a line that differs only in a memory offset is a
"moved field" (listed once per pair, with its base register) rather than a difference. What is left is
printed side by side: the clauses that really differ. Load-delay reordering shows up as a delete on
one side and an insert of the same line on the other; those pairs are cancelled too.

    python3 -I research/md81a/sdiff.py ATV.s UNT.s
"""
import difflib
import re
import sys

LINE = re.compile(r'^([0-9a-f]{8})\s+[0-9a-f]{8}\s+(\S+)\s*(.*?)\s*(;.*)?$')
MEM = re.compile(r'(-?\d+)\((\$\w+)\)')


def read(path):
    out = []
    for raw in open(path):
        m = LINE.match(raw.rstrip('\n'))
        if not m:
            continue
        addr, op, args = m.group(1), m.group(2), m.group(3)
        if op == 'nop':
            continue
        if op.startswith('b') or op in ('j', 'jal'):
            args = re.sub(r'-?\d+$', 'T', args)
        if op in ('lui',):
            args = re.sub(r'\d+$', 'HI', args)
        key = MEM.sub(lambda q: f'OFF({q.group(2)})', args)
        out.append((addr, op, args, f'{op} {key}'))
    return out


def main():
    a, u = read(sys.argv[1]), read(sys.argv[2])
    ak, uk = [x[3] for x in a], [x[3] for x in u]
    sm = difflib.SequenceMatcher(None, ak, uk, autojunk=False)
    moved = {}
    left = []
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == 'equal':
            for x, y in zip(a[i1:i2], u[j1:j2]):
                mx, my = MEM.search(x[2]), MEM.search(y[2])
                if mx and my and mx.group(1) != my.group(1):
                    moved[(mx.group(2), int(mx.group(1)), int(my.group(1)))] = x[0]
            continue
        left.append((tag, a[i1:i2], u[j1:j2]))
    # cancel a deleted line against an equal inserted one (a load moved across a branch)
    dels = [x for t, xs, ys in left for x in xs]
    ins = [y for t, xs, ys in left for y in ys]
    dk = {}
    for x in dels:
        dk.setdefault(x[3], []).append(x)
    gone_a, gone_u = set(), set()
    for y in ins:
        if dk.get(y[3]):
            x = dk[y[3]].pop()
            gone_a.add(x[0])
            gone_u.add(y[0])
    print('moved fields (base, atv, untamed):')
    for (reg, x, y), at in sorted(moved.items(), key=lambda kv: (kv[0][0], kv[0][1])):
        print(f'  {reg:5s} {x:#06x} -> {y:#06x}   (atv {at})')
    print('differences:')
    for tag, xs, ys in left:
        xs = [x for x in xs if x[0] not in gone_a]
        ys = [y for y in ys if y[0] not in gone_u]
        if not xs and not ys:
            continue
        print(f'  -- {tag}')
        for x in xs:
            print(f'   atv {x[0]} {x[1]} {x[2]}')
        for y in ys:
            print(f'   unt {y[0]} {y[1]} {y[2]}')


if __name__ == '__main__':
    main()
