"""Fill PCSX2's open save dialog (by its title) with a path and press Save. usage: save_as.py PID TITLE PATH"""
import os, sys
here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, '../../2026-10-20-c6-c6-recording2-capture/playback'))
import atspi_lib as A
from gi.repository import Atspi

pid, title, path = int(sys.argv[1]), sys.argv[2], sys.argv[3]
app = A.app_of(pid)
dialog = A.find(app, lambda x: x.get_role_name() == 'dialog' and x.get_name() == title)
assert dialog is not None, f'no {title!r} dialog open'
field = A.find(dialog, lambda x: x.get_role_name() == 'text')
save = A.find(dialog, lambda x: x.get_name() == 'Save' and 'button' in x.get_role_name())
Atspi.EditableText.set_text_contents(field, path)
print('save:', A.press(save), flush=True)
