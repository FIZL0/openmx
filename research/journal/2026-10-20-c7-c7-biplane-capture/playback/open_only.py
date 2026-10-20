"""Fill PCSX2's open dialog with the recording copy and press Open. No PINE: the recorder holds it.

usage: open_only.py PID PATH
"""
import os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import atspi_lib as A
from gi.repository import Atspi

pid, path = int(sys.argv[1]), sys.argv[2]
dialog = None
for _ in range(40):
    app = A.app_of(pid)
    dialog = A.find(app, lambda x: x.get_role_name() == 'dialog' and x.get_name() == 'Select a File')
    if dialog is not None:
        break
    time.sleep(0.25)
assert dialog is not None, 'no file dialog open'
field = A.find(dialog, lambda x: x.get_role_name() == 'text')
opener = A.find(dialog, lambda x: x.get_name() == 'Open' and 'button' in x.get_role_name())
Atspi.EditableText.set_text_contents(field, path)
print('open:', A.press(opener), flush=True)
