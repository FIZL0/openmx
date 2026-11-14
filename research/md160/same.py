"""MD160: research/c15/same.py on two md160 captures by name (context/capture/md160/NAME.csv).

  python3 -I research/md160/same.py A B
"""
import os
import runpy
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.argv[1:] = [os.path.join(ROOT, 'context', 'capture', 'md160', n + '.csv') for n in sys.argv[1:3]]
runpy.run_path(os.path.join(ROOT, 'research', 'c15', 'same.py'), run_name='__main__')
