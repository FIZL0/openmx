"""A rider's wheels on a path car's deck, row by row (C12): contact flag, path-car word, car velocity.

    python3 -B research/c12/deck.py truck 132086 132893 [p1]     # context/capture/2p/<name>.csv

Counts the rows in the range whose wheel names a car, whose wheel subtracts a moving car's velocity
(|x|+|z| > 1), and whose wheel is in contact, then prints a dozen rows. Research, not product.
"""
import csv
import os
import sys

name, a, b = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
p = sys.argv[4] if len(sys.argv) > 4 else 'p1'
rows = [r for r in csv.DictReader(open(os.path.join('context', 'capture', '2p', name + '.csv')))
        if r['frame_counter'].isdigit() and a <= int(r['frame_counter']) <= b]


def moving(r, w):
    return abs(float(r['%s_%s_car_vel.x' % (p, w)] or 0)) + abs(float(r['%s_%s_car_vel.z' % (p, w)] or 0)) > 1


print('rows', len(rows))
print('car word', sum(1 for r in rows if any(r['%s_%s_car' % (p, w)] not in ('0', '') for w in ('wf', 'wr'))))
print('car velocity', sum(1 for r in rows if any(moving(r, w) for w in ('wf', 'wr'))))
print('contact', sum(1 for r in rows if any(r['%s_%s_contact' % (p, w)] not in ('0', '') for w in ('wf', 'wr'))))
for r in rows[200:212]:
    print(r['frame_counter'], *(r['%s_%s_%s' % (p, w, k)] for w in ('wf', 'wr') for k in ('contact', 'car', 'car_vel.x')))
