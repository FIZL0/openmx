#!/usr/bin/env python3
"""MD81c: a scheduling-blind fingerprint of two `./kit dis` listings of twin functions.

sdiff.py pairs lines in order, so a load hoisted past a branch or a branch-likely with its delay slot
filled reads as a difference. This compares what the order does not change instead: the multiset of
operations (nops dropped, branch-likely folded into its plain branch), the float constants built with
lui (high halves in float range, not addresses), the small integer immediates (addiu/ori/slti off
$zero or a register, memory offsets left out), and the number of calls. Equal fingerprints with
sdiff.py's moved fields explained is the verdict "scheduling only"; anything left is printed.

    python3 -I research/md81c/fp.py ATV.s UNT.s
"""
import collections
import re
import sys

LINE = re.compile(r'^([0-9a-f]{8})\s+[0-9a-f]{8}\s+(\S+)\s*(.*?)\s*(;.*)?$')
LIKELY = {'beql': 'beq', 'bnel': 'bne', 'beqzl': 'beqz', 'bnezl': 'bnez', 'bc1tl': 'bc1t', 'bc1fl': 'bc1f',
          'blezl': 'blez', 'bgtzl': 'bgtz', 'bltzl': 'bltz', 'bgezl': 'bgez'}


def floatish(h):
    e = (h >> 7) & 0xff
    return h != 0 and 100 <= e <= 150


def body(path):
    """The listing's lines up to the function's end: the next frame prologue after a `jr $ra`."""
    out, seen_jr = [], False
    for raw in open(path):
        m = LINE.match(raw.rstrip('\n'))
        if not m:
            continue
        op, args = m.group(2), m.group(3)
        if seen_jr and out and op == 'addiu' and args.startswith('$sp, $sp, -'):
            break
        if op == 'jr' and args == '$ra':
            seen_jr = True
        out.append((m.group(1), op, args))
    return out


def read(path):
    ops, consts, imms, calls = collections.Counter(), collections.Counter(), collections.Counter(), 0
    lines = body(path)
    print(f'  {path.rsplit("/", 1)[-1]}: {lines[0][0]}..{lines[-1][0]} ({4 * len(lines)} B)')
    for _, op, args in lines:
        if op == 'nop':
            continue
        op = LIKELY.get(op, op)
        ops[op] += 1
        if op in ('jal', 'jalr'):
            calls += 1
        if op == 'lui':
            h = int(args.split(',')[-1].strip(), 0) & 0xffff
            if floatish(h):
                consts[hex(h)] += 1
        if op in ('addiu', 'ori', 'slti', 'sltiu', 'andi', 'daddiu', 'xori') and '$sp' not in args:
            v = args.split(',')[-1].strip()
            try:
                n = int(v, 0)
            except ValueError:
                continue
            if -64 <= n <= 64 or op in ('slti', 'sltiu', 'andi', 'xori'):
                imms[(op, n)] += 1
    return ops, consts, imms, calls


def diff(name, a, u):
    out = []
    for k in sorted(set(a) | set(u), key=str):
        if a[k] != u[k]:
            out.append(f'{k}: {a[k]} -> {u[k]}')
    if out:
        print(f'  {name}: ' + '; '.join(out))
    return bool(out)


a, u = read(sys.argv[1]), read(sys.argv[2])
bad = diff('ops', a[0], u[0]) | diff('float consts', a[1], u[1]) | diff('small imms', a[2], u[2])
if a[3] != u[3]:
    print(f'  calls: {a[3]} -> {u[3]}')
    bad = True
print('  fingerprint SAME' if not bad else '  fingerprint DIFFERS')
