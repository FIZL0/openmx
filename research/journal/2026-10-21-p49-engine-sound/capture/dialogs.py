"""List the dialogs PCSX2 has open, with their text fields and buttons. usage: dialogs.py PID"""
import os, sys
here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, '../../2026-10-20-c6-c6-recording2-capture/playback'))
import atspi_lib as A


def walk(n, depth=0):
    if depth > 14:
        return
    try:
        role, name = n.get_role_name(), n.get_name()
    except Exception:
        return
    if role in ('dialog', 'text', 'push button', 'button', 'file chooser', 'combo box', 'alert'):
        print('  ' * depth + f'{role}: {name!r}')
    for i in range(n.get_child_count()):
        walk(n.get_child_at_index(i), depth + 1)


walk(A.app_of(int(sys.argv[1])))
