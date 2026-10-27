"""S117: which lanes change between two rows of a context/capture/s58-slip case (a row the recorder
read late repeats the one before it).

    python3 -B same.py CASE ROW_A ROW_B"""
import csv
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(here))))
rows = list(csv.DictReader(open(os.path.join(root, 'context', 'capture', 's58-slip', sys.argv[1] + '.csv'))))
a, b = rows[int(sys.argv[2])], rows[int(sys.argv[3])]
diff = [k for k in a if k not in ('frame', 'frame_counter') and not k.startswith('stamp') and a[k] != b[k]]
print(len(diff), 'lanes differ:', ' '.join(diff[:30]))
